import type { PartProjectItemData, Project } from "../contract"
import { applyCadCommand, type CadCommand, type SyncedProjectCommand } from "../project-commands"
import { PROJECT_FILE_MIME_TYPE, normalizeProjectFile, serializeProjectFile } from "../project-file"
import type { PartFeature, PCadState } from "../schema"
import { createPartRuntimeState, materializePartFeatures, serializePCadState, type PartRuntimeState } from "./part-state"

type PCadFetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
type PCadEventSource = {
	addEventListener(type: string, listener: (event: MessageEvent<string>) => void): void
	close(): void
	onerror: ((event: Event) => unknown) | null
}
type PCadEventSourceFactory = (url: string) => PCadEventSource

export type PCadProjectSyncResult = {
	project: Project
	revision: number
	canUndo?: boolean
	canRedo?: boolean
	originClientId?: string
}

export class PCadProjectSyncError extends Error {
	public readonly status: number

	public constructor(message: string, status: number) {
		super(message)
		this.name = "PCadProjectSyncError"
		this.status = status
	}
}

type PCadProjectResponsePayload = {
	ok?: unknown
	revision?: unknown
	project?: unknown
	canUndo?: unknown
	canRedo?: unknown
	message?: unknown
}

export class PuppyCadClient {
	private readonly fetch: PCadFetch
	private readonly apiBasePath: string
	private readonly createEventSource?: PCadEventSourceFactory

	public constructor(args?: { fetch?: PCadFetch; apiBasePath?: string; createEventSource?: PCadEventSourceFactory }) {
		this.fetch = args?.fetch ?? fetch
		this.apiBasePath = args?.apiBasePath?.replace(/\/$/, "") ?? ""
		this.createEventSource = args?.createEventSource
	}

	public loadProject(projectId: string): Promise<Response> {
		return this.fetch(this.projectUrl(projectId))
	}

	public postProjectCommands(projectId: string, request: { clientId: string; baseRevision: number; commands: readonly SyncedProjectCommand[] }): Promise<Response> {
		return this.fetch(this.projectUrl(projectId, "/commands"), {
			method: "POST",
			headers: {
				"Content-Type": "application/json"
			},
			body: JSON.stringify(request)
		})
	}

	public postProjectHistoryAction(projectId: string, action: "undo" | "redo", request: { clientId: string; baseRevision: number }): Promise<Response> {
		return this.fetch(this.projectUrl(projectId, `/${action}`), {
			method: "POST",
			headers: {
				"Content-Type": "application/json"
			},
			body: JSON.stringify(request)
		})
	}

	public putProjectSnapshot(projectId: string, clientId: string, project: Project): Promise<Response> {
		return this.fetch(this.projectUrl(projectId), {
			method: "PUT",
			headers: {
				"Content-Type": PROJECT_FILE_MIME_TYPE,
				"X-PuppyCAD-Client-Id": clientId
			},
			body: serializeProjectFile(project)
		})
	}

	public createProjectEventSource(projectId: string, clientId: string): PCadEventSource {
		const factory = this.createEventSource ?? ((url: string) => new EventSource(url))
		return factory(`${this.projectUrl(projectId, "/events")}?clientId=${encodeURIComponent(clientId)}`)
	}

	private projectUrl(projectId: string, suffix = ""): string {
		return `${this.apiBasePath}/api/projects/${encodeURIComponent(projectId)}${suffix}`
	}
}

export class PCadPart {
	private document: PartProjectItemData
	private runtime: PartRuntimeState

	public constructor(document?: PartProjectItemData) {
		this.document = clonePartDocument(document ?? { features: [] })
		this.runtime = createPartRuntimeState(this.document)
	}

	public getDocument(): PartProjectItemData {
		return {
			...clonePartDocument(this.document),
			cad: serializePCadState(this.runtime.cad),
			tree: {
				orderedNodeIds: [...this.runtime.tree.orderedNodeIds],
				dirtySketchIds: [...this.runtime.tree.dirtySketchIds]
			},
			features: this.getFeatures()
		}
	}

	public getPCadState(): PCadState {
		return createPartRuntimeState(this.getDocument()).cad
	}

	public getFeatures(): PartFeature[] {
		return structuredClone(materializePartFeatures(this.runtime.cad, this.runtime.tree)) as PartFeature[]
	}

	public applyCommand(command: CadCommand): PartProjectItemData {
		this.document = applyCadCommand(this.getDocument(), command)
		this.runtime = createPartRuntimeState(this.document)
		return this.getDocument()
	}
}

export class PCadProject {
	private project: Project | null
	private revision: number
	private readonly projectId: string
	private readonly clientId: string
	private readonly client: PuppyCadClient
	private eventSource: PCadEventSource | null = null

	public constructor(args: { projectId: string; clientId: string; project?: Project; client?: PuppyCadClient }) {
		this.projectId = args.projectId
		this.clientId = args.clientId
		this.client = args.client ?? new PuppyCadClient()
		this.project = args.project ? cloneProject(args.project) : null
		this.revision = args.project?.revision ?? 0
	}

	public getRevision(): number {
		return this.revision
	}

	public getProject(): Project | null {
		return this.project ? cloneProject(this.project) : null
	}

	public replaceProject(project: Project): void {
		const normalized = cloneProject(project)
		this.project = normalized
		this.revision = normalized.revision
	}

	public setRevision(revision: number): void {
		this.revision = revision
	}

	public async load(): Promise<PCadProjectSyncResult> {
		const response = await this.client.loadProject(this.projectId)
		return this.parseProjectResponse(response)
	}

	public async postCommand(command: SyncedProjectCommand): Promise<PCadProjectSyncResult> {
		return this.postCommands([command])
	}

	public async postCommands(commands: readonly SyncedProjectCommand[]): Promise<PCadProjectSyncResult> {
		const response = await this.client.postProjectCommands(this.projectId, {
			clientId: this.clientId,
			baseRevision: this.revision,
			commands
		})
		return this.parseProjectResponse(response)
	}

	public async postHistoryAction(action: "undo" | "redo"): Promise<PCadProjectSyncResult> {
		const response = await this.client.postProjectHistoryAction(this.projectId, action, {
			clientId: this.clientId,
			baseRevision: this.revision
		})
		return this.parseProjectResponse(response)
	}

	public async putSnapshot(project: Project): Promise<PCadProjectSyncResult> {
		const response = await this.client.putProjectSnapshot(this.projectId, this.clientId, project)
		return this.parseProjectResponse(response)
	}

	public connectEvents(args: {
		onProjectChanged: (result: PCadProjectSyncResult) => void
		onConnected?: (state: { revision?: number; canUndo?: boolean; canRedo?: boolean }) => void
		onError?: () => void
	}): void {
		if (this.eventSource) {
			return
		}
		const eventSource = this.client.createProjectEventSource(this.projectId, this.clientId)
		eventSource.addEventListener("connected", (event) => {
			const data = parseEventData<{ revision?: unknown; canUndo?: unknown; canRedo?: unknown }>(event)
			if (typeof data?.revision === "number") {
				this.revision = data.revision
			}
			args.onConnected?.({
				...(typeof data?.revision === "number" ? { revision: data.revision } : {}),
				...(typeof data?.canUndo === "boolean" ? { canUndo: data.canUndo } : {}),
				...(typeof data?.canRedo === "boolean" ? { canRedo: data.canRedo } : {})
			})
		})
		eventSource.addEventListener("projectChanged", (event) => {
			const data = parseEventData<{ originClientId?: unknown; revision?: unknown; project?: unknown; canUndo?: unknown; canRedo?: unknown }>(event)
			if (!data) {
				return
			}
			const project = normalizeProjectFile(data.project)
			if (!project) {
				return
			}
			this.project = project
			this.revision = typeof data.revision === "number" ? data.revision : project.revision
			args.onProjectChanged({
				project: cloneProject(project),
				revision: this.revision,
				...(typeof data.canUndo === "boolean" ? { canUndo: data.canUndo } : {}),
				...(typeof data.canRedo === "boolean" ? { canRedo: data.canRedo } : {}),
				...(typeof data.originClientId === "string" ? { originClientId: data.originClientId } : {})
			})
		})
		eventSource.onerror = () => args.onError?.()
		this.eventSource = eventSource
	}

	public disconnectEvents(): void {
		this.eventSource?.close()
		this.eventSource = null
	}

	private async parseProjectResponse(response: Response): Promise<PCadProjectSyncResult> {
		let payload: PCadProjectResponsePayload | null = null
		try {
			payload = (await response.json()) as PCadProjectResponsePayload
		} catch {
			payload = null
		}
		if (!response.ok || payload?.ok === false) {
			const message = typeof payload?.message === "string" ? payload.message : `Server responded with ${response.status}`
			throw new PCadProjectSyncError(message, response.status)
		}

		const project = normalizeProjectFile(payload?.project)
		if (!project) {
			throw new PCadProjectSyncError("Server response did not include a valid project.", response.status)
		}
		this.project = project
		this.revision = typeof payload?.revision === "number" ? payload.revision : project.revision
		return {
			project: cloneProject(project),
			revision: this.revision,
			...(typeof payload?.canUndo === "boolean" ? { canUndo: payload.canUndo } : {}),
			...(typeof payload?.canRedo === "boolean" ? { canRedo: payload.canRedo } : {})
		}
	}
}

function clonePartDocument(document: PartProjectItemData): PartProjectItemData {
	return structuredClone(document) as PartProjectItemData
}

function cloneProject(project: Project): Project {
	const normalized = normalizeProjectFile(structuredClone(project))
	if (!normalized) {
		throw new Error("Invalid PCAD project.")
	}
	return normalized
}

function parseEventData<T>(event: Event): T | null {
	const data = (event as MessageEvent).data
	if (typeof data !== "string") {
		return null
	}
	try {
		return JSON.parse(data) as T
	} catch {
		return null
	}
}
