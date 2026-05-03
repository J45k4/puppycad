import { existsSync } from "node:fs"
import { mkdir, readFile } from "node:fs/promises"
import type { Project } from "../contract"
import { applySyncedProjectCommands, ProjectCommandError, type SyncedProjectCommand } from "../project-commands"
import { normalizeProjectFile, serializeProjectFile } from "../project-file"

const SAVE_DIRECTORY_URL = new URL("../../workdir/saved-projects/", import.meta.url)

type ProjectEventSubscriber = {
	clientId: string
	controller: ReadableStreamDefaultController<Uint8Array>
}

type ProjectChangedEvent = {
	type: "projectChanged"
	projectId: string
	revision: number
	originClientId: string
	commands: SyncedProjectCommand[]
	project: Project
	canUndo?: boolean
	canRedo?: boolean
}

const textEncoder = new TextEncoder()
const subscribersByProjectId = new Map<string, Set<ProjectEventSubscriber>>()
const projectQueues = new Map<string, Promise<void>>()
const undoStacksByProjectId = new Map<string, Project[]>()
const redoStacksByProjectId = new Map<string, Project[]>()
const MAX_PROJECT_HISTORY = 100

async function ensureDirectory() {
	if (existsSync(SAVE_DIRECTORY_URL)) {
		return
	}
	await mkdir(SAVE_DIRECTORY_URL, { recursive: true })
}

export function getProjectFileUrl(projectId: string): URL {
	return new URL(`${sanitizeProjectId(projectId)}.json`, SAVE_DIRECTORY_URL)
}

export async function loadProject(projectId: string): Promise<Project | null> {
	try {
		const contents = await readFile(getProjectFileUrl(projectId), "utf8")
		const parsed = JSON.parse(contents) as unknown
		return normalizeProjectFile(parsed)
	} catch (error) {
		if (isNotFoundError(error)) {
			return null
		}
		throw error
	}
}

export async function persistProject(projectId: string, project: Project): Promise<Project> {
	const normalized = normalizeProjectFile(project)
	if (!normalized) {
		throw new Error("Invalid project file")
	}
	await ensureDirectory()
	await Bun.write(getProjectFileUrl(projectId), serializeProjectFile(normalized))
	return normalized
}

export async function getProject(request: Request, projectId: string): Promise<Response> {
	void request
	try {
		const project = await loadProject(projectId)
		if (!project) {
			return Response.json({ ok: false, code: "not_found", message: "Project not found." }, { status: 404 })
		}
		return Response.json({ ok: true, projectId, revision: project.revision, project, ...getProjectHistoryState(projectId) })
	} catch (error) {
		console.error("Failed to load project", error)
		return Response.json({ ok: false, code: "load_failed", message: "Unable to load project." }, { status: 500 })
	}
}

export async function putProject(request: Request, projectId: string): Promise<Response> {
	let payload: unknown
	try {
		payload = await request.json()
	} catch (error) {
		console.error("Failed to parse project payload", error)
		return Response.json({ ok: false, code: "invalid_json", message: "Invalid JSON body." }, { status: 400 })
	}

	const projectFile = normalizeProjectFile(payload)
	if (!projectFile) {
		return Response.json({ ok: false, code: "invalid_project", message: "Invalid project file." }, { status: 400 })
	}

	try {
		const currentProject = await loadProject(projectId)
		if (currentProject) {
			pushProjectHistory(undoStacksByProjectId, projectId, currentProject)
			clearProjectRedoHistory(projectId)
		}
		if (currentProject) {
			projectFile.revision = currentProject.revision + 1
		}
		const project = await persistProject(projectId, projectFile)
		const originClientId = request.headers.get("X-PuppyCAD-Client-Id")?.trim() || "unknown"
		broadcastProjectChanged({
			type: "projectChanged",
			projectId,
			revision: project.revision,
			originClientId,
			commands: [],
			project,
			...getProjectHistoryState(projectId)
		})
		return Response.json({ ok: true, projectId, revision: project.revision, project, ...getProjectHistoryState(projectId) })
	} catch (error) {
		console.error("Failed to persist project", error)
		return Response.json({ ok: false, code: "persist_failed", message: "Unable to persist project." }, { status: 500 })
	}
}

export async function postProject(request: Request): Promise<Response> {
	const projectId = crypto.randomUUID()
	const response = await putProject(request, projectId)
	if (!response.ok) {
		return response
	}
	const payload = (await response.json()) as { project?: Project }
	return Response.json({ status: "ok", projectId, fileName: `${projectId}.json`, project: payload.project }, { status: 201 })
}

export async function postProjectCommands(request: Request, projectId: string): Promise<Response> {
	let payload: unknown
	try {
		payload = await request.json()
	} catch {
		return Response.json({ ok: false, code: "invalid_json", message: "Invalid JSON body." }, { status: 400 })
	}
	const commandRequest = normalizeCommandRequest(payload)
	if (!commandRequest) {
		return Response.json({ ok: false, code: "invalid_request", message: "Command request must include clientId, baseRevision, and commands." }, { status: 400 })
	}

	return withProjectQueue(projectId, async () => {
		const currentProject = await loadProject(projectId)
		if (!currentProject) {
			return Response.json({ ok: false, code: "not_found", message: "Project not found." }, { status: 404 })
		}
		try {
			pushProjectHistory(undoStacksByProjectId, projectId, currentProject)
			const nextProject = applySyncedProjectCommands(currentProject, commandRequest.commands)
			clearProjectRedoHistory(projectId)
			nextProject.revision = currentProject.revision + 1
			const project = await persistProject(projectId, nextProject)
			broadcastProjectChanged({
				type: "projectChanged",
				projectId,
				revision: project.revision,
				originClientId: commandRequest.clientId,
				commands: commandRequest.commands,
				project,
				...getProjectHistoryState(projectId)
			})
			return Response.json({ ok: true, projectId, revision: project.revision, project, ...getProjectHistoryState(projectId) })
		} catch (error) {
			popProjectHistory(undoStacksByProjectId, projectId)
			if (error instanceof ProjectCommandError) {
				return Response.json({ ok: false, code: error.code, message: error.message, revision: currentProject.revision }, { status: 400 })
			}
			console.error("Failed to apply project commands", error)
			return Response.json({ ok: false, code: "command_failed", message: "Unable to apply commands.", revision: currentProject.revision }, { status: 500 })
		}
	})
}

export async function postProjectUndo(request: Request, projectId: string): Promise<Response> {
	return postProjectHistoryAction(request, projectId, "undo")
}

export async function postProjectRedo(request: Request, projectId: string): Promise<Response> {
	return postProjectHistoryAction(request, projectId, "redo")
}

async function postProjectHistoryAction(request: Request, projectId: string, action: "undo" | "redo"): Promise<Response> {
	let payload: unknown
	try {
		payload = await request.json()
	} catch {
		return Response.json({ ok: false, code: "invalid_json", message: "Invalid JSON body." }, { status: 400 })
	}
	const commandRequest = normalizeHistoryRequest(payload)
	if (!commandRequest) {
		return Response.json({ ok: false, code: "invalid_request", message: "History request must include clientId and baseRevision." }, { status: 400 })
	}

	return withProjectQueue(projectId, async () => {
		const currentProject = await loadProject(projectId)
		if (!currentProject) {
			return Response.json({ ok: false, code: "not_found", message: "Project not found." }, { status: 404 })
		}
		const sourceStack = action === "undo" ? undoStacksByProjectId : redoStacksByProjectId
		const destinationStack = action === "undo" ? redoStacksByProjectId : undoStacksByProjectId
		const historyProject = popProjectHistory(sourceStack, projectId)
		if (!historyProject) {
			return Response.json(
				{ ok: false, code: `nothing_to_${action}`, message: `Nothing to ${action}.`, revision: currentProject.revision, ...getProjectHistoryState(projectId) },
				{ status: 409 }
			)
		}
		pushProjectHistory(destinationStack, projectId, currentProject)
		historyProject.revision = currentProject.revision + 1
		const project = await persistProject(projectId, historyProject)
		const historyState = getProjectHistoryState(projectId)
		broadcastProjectChanged({
			type: "projectChanged",
			projectId,
			revision: project.revision,
			originClientId: commandRequest.clientId,
			commands: [],
			project,
			...historyState
		})
		return Response.json({ ok: true, projectId, revision: project.revision, project, ...historyState })
	})
}

export async function getProjectEvents(request: Request, projectId: string): Promise<Response> {
	const url = new URL(request.url)
	const clientId = url.searchParams.get("clientId")?.trim() || "anonymous"
	const project = await loadProject(projectId)
	if (!project) {
		return Response.json({ ok: false, code: "not_found", message: "Project not found." }, { status: 404 })
	}

	let subscriber: ProjectEventSubscriber | null = null
	const stream = new ReadableStream<Uint8Array>({
		start(controller) {
			subscriber = { clientId, controller }
			let subscribers = subscribersByProjectId.get(projectId)
			if (!subscribers) {
				subscribers = new Set<ProjectEventSubscriber>()
				subscribersByProjectId.set(projectId, subscribers)
			}
			subscribers.add(subscriber)
			controller.enqueue(encodeSseEvent("connected", { type: "connected", projectId, revision: project.revision, clientId, ...getProjectHistoryState(projectId) }))
		},
		cancel() {
			if (subscriber) {
				removeSubscriber(projectId, subscriber)
			}
		}
	})

	return new Response(stream, {
		headers: {
			"Content-Type": "text/event-stream",
			"Cache-Control": "no-cache",
			Connection: "keep-alive"
		}
	})
}

function broadcastProjectChanged(event: ProjectChangedEvent): void {
	const subscribers = subscribersByProjectId.get(event.projectId)
	if (!subscribers) {
		return
	}
	const chunk = encodeSseEvent("projectChanged", event)
	for (const subscriber of [...subscribers]) {
		try {
			subscriber.controller.enqueue(chunk)
		} catch {
			removeSubscriber(event.projectId, subscriber)
		}
	}
}

function removeSubscriber(projectId: string, subscriber: ProjectEventSubscriber): void {
	const subscribers = subscribersByProjectId.get(projectId)
	if (!subscribers) {
		return
	}
	subscribers.delete(subscriber)
	if (subscribers.size === 0) {
		subscribersByProjectId.delete(projectId)
	}
}

async function withProjectQueue<T>(projectId: string, operation: () => Promise<T>): Promise<T> {
	const previous = projectQueues.get(projectId) ?? Promise.resolve()
	let release: () => void = () => undefined
	const current = new Promise<void>((resolve) => {
		release = resolve
	})
	const chained = previous.then(() => current)
	projectQueues.set(projectId, chained)
	await previous.catch(() => undefined)
	try {
		return await operation()
	} finally {
		release()
		if (projectQueues.get(projectId) === chained) {
			projectQueues.delete(projectId)
		}
	}
}

function normalizeCommandRequest(input: unknown): { clientId: string; baseRevision: number; commands: SyncedProjectCommand[] } | null {
	if (!input || typeof input !== "object") {
		return null
	}
	const value = input as { clientId?: unknown; baseRevision?: unknown; commands?: unknown }
	if (typeof value.clientId !== "string" || !value.clientId.trim() || typeof value.baseRevision !== "number" || !Number.isInteger(value.baseRevision) || !Array.isArray(value.commands)) {
		return null
	}
	return {
		clientId: value.clientId.trim(),
		baseRevision: value.baseRevision,
		commands: value.commands as SyncedProjectCommand[]
	}
}

function encodeSseEvent(eventName: string, data: unknown): Uint8Array {
	return textEncoder.encode(`event: ${eventName}\ndata: ${JSON.stringify(data)}\n\n`)
}

function sanitizeProjectId(projectId: string): string {
	const trimmed = projectId.trim()
	if (!trimmed || trimmed.includes("/") || trimmed.includes("\\") || trimmed.includes("..")) {
		throw new Error("Invalid project id")
	}
	return trimmed
}

function pushProjectHistory(stacks: Map<string, Project[]>, projectId: string, project: Project): void {
	const stack = stacks.get(projectId) ?? []
	stack.push(cloneProject(project))
	if (stack.length > MAX_PROJECT_HISTORY) {
		stack.shift()
	}
	stacks.set(projectId, stack)
}

function popProjectHistory(stacks: Map<string, Project[]>, projectId: string): Project | null {
	const stack = stacks.get(projectId)
	const project = stack?.pop() ?? null
	if (stack && stack.length === 0) {
		stacks.delete(projectId)
	}
	return project
}

function clearProjectRedoHistory(projectId: string): void {
	redoStacksByProjectId.delete(projectId)
}

function getProjectHistoryState(projectId: string): { canUndo: boolean; canRedo: boolean } {
	return {
		canUndo: (undoStacksByProjectId.get(projectId)?.length ?? 0) > 0,
		canRedo: (redoStacksByProjectId.get(projectId)?.length ?? 0) > 0
	}
}

function cloneProject(project: Project): Project {
	const normalized = normalizeProjectFile(JSON.parse(JSON.stringify(project)))
	if (!normalized) {
		throw new Error("Project cannot be normalized.")
	}
	return normalized
}

function normalizeHistoryRequest(input: unknown): { clientId: string; baseRevision: number } | null {
	if (!input || typeof input !== "object") {
		return null
	}
	const value = input as { clientId?: unknown; baseRevision?: unknown }
	if (typeof value.clientId !== "string" || !value.clientId.trim() || typeof value.baseRevision !== "number" || !Number.isInteger(value.baseRevision)) {
		return null
	}
	return {
		clientId: value.clientId.trim(),
		baseRevision: value.baseRevision
	}
}

function isNotFoundError(error: unknown): boolean {
	return !!error && typeof error === "object" && "code" in error && (error as { code?: unknown }).code === "ENOENT"
}
