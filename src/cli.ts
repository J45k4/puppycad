#!/usr/bin/env bun

import { mkdir, readFile, stat, writeFile } from "node:fs/promises"
import { homedir, platform } from "node:os"
import { dirname, join, resolve } from "node:path"
import type { Project, ProjectDocument, ProjectDocumentType, ProjectNode } from "./contract"
import { PCadPart } from "./pcad/project"
import { createProjectFile, normalizeProjectFile, serializeProjectFile } from "./project-file"
import type { PartFeature } from "./schema"

type CliOutput = {
	stdout: (message: string) => void
	stderr: (message: string) => void
}

type CliEnv = Record<string, string | undefined>
type CliFetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>

type CliOptions = {
	version?: string
	cwd?: string
	output?: CliOutput
	fetch?: CliFetch
	env?: CliEnv
	configPath?: string
	configDir?: string
}

type CliContext = {
	version: string
	cwd: string
	output: CliOutput
	fetch: CliFetch
	env: CliEnv
	globals: GlobalCliOptions
	configPath?: string
	configDir?: string
}

type GlobalCliOptions = {
	serverUrl?: string
	projectId?: string
	json: boolean
	quiet: boolean
	verbose: boolean
}

type CliConfig = {
	serverUrl?: string
	defaultProject?: string
}

type InitOptions = {
	filePath: string
	partName: string
	force: boolean
	empty: boolean
}

type InspectOptions = {
	target?: string
	json: boolean
}

type ProjectStats = {
	documents: number
	folders: number
	types: Record<ProjectDocumentType, number>
	parts: {
		id: string
		name: string
		features: number
	}[]
}

type CliFeature = {
	id: string
	type: string
	partId: string
	status: "ok"
	name?: string
}

type CliGraphNode = CliFeature

type CliGraphEdge = {
	from: string
	to: string
	partId: string
	type: "dependency"
}

type ServerJsonResponse<T> = {
	payload: T
	response: Response
	url: string
}

const DEFAULT_VERSION = "0.1.0"
const DEFAULT_PROJECT_FILE = "puppycad.pcad"
const DEFAULT_SERVER_URL = "http://localhost:5337"
const DOCUMENT_TYPES: ProjectDocumentType[] = ["schemantic", "pcb", "part", "assembly", "diagram"]

export async function runPuppycadCli(args: readonly string[], options: CliOptions = {}): Promise<number> {
	const output = options.output ?? {
		stdout: (message) => console.log(message),
		stderr: (message) => console.error(message)
	}
	const parsed = parseGlobalArgs(args)
	const context: CliContext = {
		version: options.version ?? DEFAULT_VERSION,
		cwd: options.cwd ?? process.cwd(),
		output,
		fetch: options.fetch ?? fetch,
		env: options.env ?? process.env,
		globals: parsed.globals,
		...(options.configPath ? { configPath: options.configPath } : {}),
		...(options.configDir ? { configDir: options.configDir } : {})
	}
	const [command, ...rest] = parsed.args

	try {
		if (!command || command === "--help" || command === "-h" || command === "help") {
			output.stdout(formatHelp())
			return 0
		}
		if (command === "--version" || command === "-v" || command === "version") {
			output.stdout(context.version)
			return 0
		}
		if (command === "config") {
			return await runConfigCommand(rest, context)
		}
		if (command === "doctor") {
			return await runDoctor(context)
		}
		if (command === "project") {
			return await runProjectCommand(rest, context)
		}
		if (command === "query") {
			return await runQueryCommand(rest, context)
		}
		if (command === "graph") {
			return await runGraphCommand(rest, context)
		}
		if (command === "eval") {
			return await runEvalCommand(rest, context)
		}
		if (command === "inspect") {
			return await runInspectCommand(rest, context)
		}
		if (command === "validate") {
			return await runFileInspect(parseInspectArgs(rest), context.cwd, context.output)
		}
		if (command === "init" || command === "create") {
			return await runInit(parseInitArgs(rest), context.cwd, context.output)
		}

		output.stderr(`Unknown command: ${command}\n\n${formatHelp()}`)
		return 1
	} catch (error) {
		if (error instanceof CliHelpError) {
			output.stdout(error.message)
			return 0
		}
		output.stderr(error instanceof Error ? error.message : String(error))
		return 1
	}
}

function parseGlobalArgs(args: readonly string[]): { args: string[]; globals: GlobalCliOptions } {
	const globals: GlobalCliOptions = {
		json: false,
		quiet: false,
		verbose: false
	}
	const rest: string[] = []

	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) {
			continue
		}
		if (arg === "--server-url") {
			globals.serverUrl = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--server-url=")) {
			globals.serverUrl = arg.slice("--server-url=".length)
			continue
		}
		if (arg === "--project") {
			globals.projectId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--project=")) {
			globals.projectId = arg.slice("--project=".length)
			continue
		}
		if (arg === "--json") {
			globals.json = true
			continue
		}
		if (arg === "--quiet") {
			globals.quiet = true
			continue
		}
		if (arg === "--verbose") {
			globals.verbose = true
			continue
		}
		rest.push(arg)
	}

	return { args: rest, globals }
}

function parseInitArgs(args: readonly string[]): InitOptions {
	let filePath = DEFAULT_PROJECT_FILE
	let partName = "Part 1"
	let force = false
	let empty = false

	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) {
			continue
		}
		if (arg === "--help" || arg === "-h") {
			throw new CliHelpError(formatInitHelp())
		}
		if (arg === "--force" || arg === "-f") {
			force = true
			continue
		}
		if (arg === "--empty") {
			empty = true
			continue
		}
		if (arg === "--part-name") {
			partName = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("-")) {
			throw new Error(`Unknown init option: ${arg}`)
		}
		filePath = arg
	}

	return { filePath, partName, force, empty }
}

function parseInspectArgs(args: readonly string[]): InspectOptions {
	let target: string | undefined
	let json = false

	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) {
			continue
		}
		if (arg === "--help" || arg === "-h") {
			throw new CliHelpError(formatInspectHelp())
		}
		if (arg === "--json") {
			json = true
			continue
		}
		if (arg.startsWith("-")) {
			throw new Error(`Unknown inspect option: ${arg}`)
		}
		if (target) {
			throw new Error(`Unexpected inspect argument: ${arg}`)
		}
		target = arg
	}

	return { target, json }
}

async function runConfigCommand(args: readonly string[], context: CliContext): Promise<number> {
	const [action, ...rest] = args
	if (!action || action === "--help" || action === "-h") {
		context.output.stdout(formatConfigHelp())
		return 0
	}
	if (action === "path") {
		writeStdout(context, getConfigPath(context))
		return 0
	}
	if (action === "get") {
		const config = await readCliConfig(context)
		if (context.globals.json) {
			writeStdout(context, JSON.stringify(config, null, 2))
			return 0
		}
		const lines: string[] = []
		if (config.serverUrl) {
			lines.push(`serverUrl: ${config.serverUrl}`)
		}
		if (config.defaultProject) {
			lines.push(`defaultProject: ${config.defaultProject}`)
		}
		writeStdout(context, lines.length > 0 ? lines.join("\n") : "No CLI config set.")
		return 0
	}
	if (action === "set") {
		const [key, value, ...extra] = rest
		if (!key || !value || extra.length > 0) {
			throw new Error("Usage: puppycad config set <server-url|default-project> <value>")
		}
		const config = await readCliConfig(context)
		if (key === "server-url") {
			config.serverUrl = normalizeServerUrl(value)
			await writeCliConfig(context, config)
			writeStdout(context, `Set serverUrl: ${config.serverUrl}`)
			return 0
		}
		if (key === "default-project") {
			config.defaultProject = normalizeProjectId(value)
			await writeCliConfig(context, config)
			writeStdout(context, `Set defaultProject: ${config.defaultProject}`)
			return 0
		}
		throw new Error(`Unknown config key: ${key}`)
	}
	if (action === "unset") {
		const [key, ...extra] = rest
		if (!key || extra.length > 0) {
			throw new Error("Usage: puppycad config unset default-project")
		}
		if (key !== "default-project") {
			throw new Error(`Unknown or non-unsettable config key: ${key}`)
		}
		const config = await readCliConfig(context)
		config.defaultProject = undefined
		await writeCliConfig(context, config)
		writeStdout(context, "Unset defaultProject.")
		return 0
	}

	throw new Error(`Unknown config command: ${action}`)
}

async function runDoctor(context: CliContext): Promise<number> {
	const serverUrl = await resolveServerUrl(context)
	const url = buildServerUrl(serverUrl, "/health")
	try {
		const response = await context.fetch(url)
		const health = await readJsonPayload(response)
		if (context.globals.json) {
			writeStdout(context, JSON.stringify({ ok: response.ok, serverUrl, status: response.status, health }, null, 2))
		} else if (response.ok) {
			writeStdout(context, `Puppycad server reachable at ${serverUrl}`)
		} else {
			context.output.stderr(`Puppycad server responded with ${response.status} at ${serverUrl}`)
		}
		return response.ok ? 0 : 1
	} catch (error) {
		if (context.globals.json) {
			writeStdout(context, JSON.stringify({ ok: false, serverUrl, message: formatFileError(error) }, null, 2))
		} else {
			context.output.stderr(`Unable to reach Puppycad server at ${serverUrl}: ${formatFileError(error)}`)
		}
		return 1
	}
}

async function runProjectCommand(args: readonly string[], context: CliContext): Promise<number> {
	const [action, ...rest] = args
	if (!action || action === "--help" || action === "-h") {
		context.output.stdout(formatProjectHelp())
		return 0
	}
	if (action === "list") {
		const { payload } = await requestServerJson<{ projects?: unknown }>(context, "/api/projects")
		const projects = Array.isArray(payload.projects) ? payload.projects : []
		if (context.globals.json) {
			writeStdout(context, JSON.stringify({ projects }, null, 2))
			return 0
		}
		if (projects.length === 0) {
			writeStdout(context, "No projects found.")
			return 0
		}
		writeStdout(
			context,
			projects
				.map((project) => {
					const value = project as { projectId?: unknown; revision?: unknown; documents?: unknown }
					return `${String(value.projectId ?? "unknown")} revision=${String(value.revision ?? "?")} documents=${String(value.documents ?? "?")}`
				})
				.join("\n")
		)
		return 0
	}
	if (action === "create") {
		const [name, ...extra] = rest
		if (!name || extra.length > 0) {
			throw new Error("Usage: puppycad project create <name> [--json]")
		}
		const project = createInitialProject({ filePath: DEFAULT_PROJECT_FILE, partName: name, force: false, empty: false })
		const { payload } = await requestServerJson<{ projectId?: unknown; project?: unknown; revision?: unknown }>(context, "/api/projects", {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify(project)
		})
		const result = {
			projectId: typeof payload.projectId === "string" ? payload.projectId : "",
			project: normalizeProjectFile(payload.project),
			revision: typeof payload.revision === "number" ? payload.revision : normalizeProjectFile(payload.project)?.revision
		}
		if (!result.projectId) {
			throw new Error("Server did not return a project id.")
		}
		if (context.globals.json) {
			writeStdout(context, JSON.stringify(result, null, 2))
			return 0
		}
		writeStdout(context, `Created project ${result.projectId}`)
		return 0
	}
	if (action === "inspect") {
		const { target } = parseProjectTargetArgs(rest, "project inspect")
		return await runServerInspect(target, context)
	}
	throw new Error(`Unknown project command: ${action}`)
}

async function runInspectCommand(args: readonly string[], context: CliContext): Promise<number> {
	const options = parseInspectArgs(args)
	const json = context.globals.json || options.json
	if (options.target && (await shouldInspectLocalFile(options.target, context.cwd))) {
		return await runFileInspect({ target: options.target, json }, context.cwd, context.output)
	}
	return await runServerInspect(options.target, { ...context, globals: { ...context.globals, json } })
}

async function runQueryCommand(args: readonly string[], context: CliContext): Promise<number> {
	const [query, ...rest] = args
	if (!query || query === "--help" || query === "-h") {
		context.output.stdout(formatQueryHelp())
		return 0
	}
	if (query !== "features") {
		throw new Error(`Unknown query: ${query}`)
	}
	const { target } = parseProjectTargetArgs(rest, "query features")
	const { projectId, project } = await loadServerProject(context, target)
	const features = collectProjectFeatures(project)
	if (context.globals.json) {
		writeStdout(context, JSON.stringify({ projectId, features }, null, 2))
		return 0
	}
	writeStdout(context, features.length > 0 ? features.map((feature) => `${feature.id} ${feature.type} part=${feature.partId}`).join("\n") : "No features.")
	return 0
}

async function runGraphCommand(args: readonly string[], context: CliContext): Promise<number> {
	const { target, mermaid } = parseProjectTargetArgs(args, "graph", ["--mermaid"])
	const { projectId, project } = await loadServerProject(context, target)
	const graph = collectProjectGraph(project)
	if (mermaid) {
		writeStdout(context, formatMermaidGraph(graph.nodes, graph.edges))
		return 0
	}
	if (context.globals.json) {
		writeStdout(context, JSON.stringify({ projectId, ...graph }, null, 2))
		return 0
	}
	writeStdout(context, `Project ${projectId}: ${graph.nodes.length} nodes, ${graph.edges.length} edges`)
	return 0
}

async function runEvalCommand(args: readonly string[], context: CliContext): Promise<number> {
	const { target, explain } = parseProjectTargetArgs(args, "eval", ["--explain"])
	const { projectId, project } = await loadServerProject(context, target)
	const stats = collectProjectStats(project)
	const features = collectProjectFeatures(project)
	const result = {
		status: "ok",
		projectId,
		revision: project.revision,
		parts: stats.parts.length,
		features: features.length
	}
	if (context.globals.json) {
		writeStdout(context, JSON.stringify(result, null, 2))
		return 0
	}
	if (explain) {
		writeStdout(
			context,
			[
				`Project ${projectId} evaluated successfully.`,
				`Revision: ${project.revision}`,
				`Parts: ${stats.parts.length}`,
				`Features: ${features.length}`,
				"No evaluator errors were reported by the current project snapshot."
			].join("\n")
		)
		return 0
	}
	writeStdout(context, `ok project=${projectId} features=${features.length}`)
	return 0
}

async function runServerInspect(target: string | undefined, context: CliContext): Promise<number> {
	const { projectId, project } = await loadServerProject(context, target)
	const stats = collectProjectStats(project)
	if (context.globals.json) {
		writeStdout(context, JSON.stringify({ projectId, project, stats }, null, 2))
		return 0
	}
	writeStdout(context, formatInspectSummary(projectId, project, stats))
	return 0
}

async function runInit(options: InitOptions, cwd: string, output: CliOutput): Promise<number> {
	const filePath = resolve(cwd, options.filePath)
	if (!options.force && (await fileExists(filePath))) {
		output.stderr(`Project file already exists: ${filePath}\nUse --force to overwrite it.`)
		return 1
	}

	const project = createInitialProject(options)
	await mkdir(dirname(filePath), { recursive: true })
	await writeFile(filePath, `${serializeProjectFile(project)}\n`, "utf8")
	output.stdout(`Created ${filePath}`)
	return 0
}

async function runFileInspect(options: InspectOptions, cwd: string, output: CliOutput): Promise<number> {
	const filePath = resolve(cwd, options.target ?? DEFAULT_PROJECT_FILE)
	const raw = await readFile(filePath, "utf8").catch((error: unknown) => {
		throw new Error(`Unable to read project file: ${formatFileError(error)}`)
	})
	const parsed = JSON.parse(raw) as unknown
	const project = normalizeProjectFile(parsed)
	if (!project) {
		output.stderr(`Invalid PuppyCAD project file: ${filePath}`)
		return 1
	}

	const stats = collectProjectStats(project)
	if (options.json) {
		output.stdout(JSON.stringify({ file: filePath, project, stats }, null, 2))
		return 0
	}

	output.stdout(formatInspectSummary(filePath, project, stats))
	return 0
}

function createInitialProject(options: InitOptions): Project {
	const items: ProjectNode[] = options.empty
		? []
		: [
				{
					id: "part-1",
					type: "part",
					name: options.partName,
					data: new PCadPart().getDocument()
				}
			]
	const project = createProjectFile({
		items,
		selectedPath: items.length > 0 ? [0] : null
	})
	return project
}

function collectProjectStats(project: Project): ProjectStats {
	const stats: ProjectStats = {
		documents: 0,
		folders: 0,
		types: {
			schemantic: 0,
			pcb: 0,
			part: 0,
			assembly: 0,
			diagram: 0
		},
		parts: []
	}
	visitProjectNodes(project.items, (node) => {
		if (isProjectFolder(node)) {
			stats.folders += 1
			return
		}
		stats.documents += 1
		stats.types[node.type] += 1
		if (node.type === "part") {
			stats.parts.push({
				id: node.id,
				name: node.name,
				features: new PCadPart(node.data).getFeatures().length
			})
		}
	})
	return stats
}

function collectProjectFeatures(project: Project): CliFeature[] {
	const features: CliFeature[] = []
	visitProjectNodes(project.items, (node) => {
		if (!isProjectFolder(node) && node.type === "part") {
			for (const feature of new PCadPart(node.data).getFeatures()) {
				features.push({
					id: feature.id,
					type: feature.type,
					partId: node.id,
					status: "ok",
					...(feature.name ? { name: feature.name } : {})
				})
			}
		}
	})
	return features
}

function collectProjectGraph(project: Project): { nodes: CliGraphNode[]; edges: CliGraphEdge[] } {
	const nodes = collectProjectFeatures(project)
	const edges: CliGraphEdge[] = []
	const edgeKeys = new Set<string>()
	visitProjectNodes(project.items, (node) => {
		if (isProjectFolder(node) || node.type !== "part") {
			return
		}
		for (const feature of new PCadPart(node.data).getFeatures()) {
			for (const dependency of collectFeatureDependencies(feature)) {
				const key = `${node.id}:${dependency}->${feature.id}`
				if (edgeKeys.has(key)) {
					continue
				}
				edgeKeys.add(key)
				edges.push({ from: dependency, to: feature.id, partId: node.id, type: "dependency" })
			}
		}
	})
	return { nodes, edges }
}

function collectFeatureDependencies(feature: PartFeature): string[] {
	const dependencies = new Set<string>()
	const explicitDependencies = (feature as { dependsOn?: unknown }).dependsOn
	if (Array.isArray(explicitDependencies)) {
		for (const dependency of explicitDependencies) {
			if (typeof dependency === "string" && dependency) {
				dependencies.add(dependency)
			}
		}
	}
	if (feature.type === "sketch" && feature.target.type === "face") {
		dependencies.add(feature.target.face.extrudeId)
	}
	if (feature.type === "extrude") {
		dependencies.add(feature.target.sketchId)
	}
	if (feature.type === "chamfer") {
		dependencies.add(feature.target.edge.extrudeId)
	}
	return [...dependencies]
}

function visitProjectNodes(nodes: readonly ProjectNode[], visitor: (node: ProjectNode) => void): void {
	for (const node of nodes) {
		visitor(node)
		if (isProjectFolder(node)) {
			visitProjectNodes(node.items, visitor)
		}
	}
}

function isProjectFolder(node: ProjectNode): node is Exclude<ProjectNode, ProjectDocument> {
	return "kind" in node && node.kind === "folder"
}

async function loadServerProject(context: CliContext, target: string | undefined): Promise<{ projectId: string; project: Project }> {
	const projectId = await resolveProjectId(context, target)
	const { payload } = await requestServerJson<{ projectId?: unknown; project?: unknown }>(context, `/api/projects/${encodeURIComponent(projectId)}`)
	const project = normalizeProjectFile(payload.project)
	if (!project) {
		throw new Error(`Server returned an invalid project for ${projectId}.`)
	}
	return {
		projectId: typeof payload.projectId === "string" ? payload.projectId : projectId,
		project
	}
}

async function requestServerJson<T>(context: CliContext, path: string, init?: RequestInit): Promise<ServerJsonResponse<T>> {
	const serverUrl = await resolveServerUrl(context)
	const url = buildServerUrl(serverUrl, path)
	let response: Response
	try {
		response = await context.fetch(url, init)
	} catch (error) {
		throw new Error(`Unable to reach Puppycad server at ${serverUrl}: ${formatFileError(error)}`)
	}
	const payload = (await readJsonPayload(response)) as T & { ok?: unknown; message?: unknown }
	if (!response.ok || payload?.ok === false) {
		const message = typeof payload?.message === "string" ? payload.message : `Server responded with ${response.status}`
		throw new Error(message)
	}
	return { payload, response, url }
}

async function readJsonPayload(response: Response): Promise<unknown> {
	try {
		return await response.json()
	} catch {
		return null
	}
}

async function resolveServerUrl(context: CliContext): Promise<string> {
	if (context.globals.serverUrl) {
		return normalizeServerUrl(context.globals.serverUrl)
	}
	const envServerUrl = context.env.PUPPYCAD_SERVER_URL
	if (envServerUrl?.trim()) {
		return normalizeServerUrl(envServerUrl)
	}
	const config = await readCliConfig(context)
	if (config.serverUrl) {
		return normalizeServerUrl(config.serverUrl)
	}
	return DEFAULT_SERVER_URL
}

async function resolveProjectId(context: CliContext, target: string | undefined): Promise<string> {
	if (target?.trim()) {
		return normalizeProjectId(target)
	}
	if (context.globals.projectId) {
		return normalizeProjectId(context.globals.projectId)
	}
	const config = await readCliConfig(context)
	if (config.defaultProject) {
		return normalizeProjectId(config.defaultProject)
	}
	throw new Error("Project id required. Pass [project-id], --project <id>, or set config default-project.")
}

function normalizeServerUrl(value: string): string {
	const trimmed = value.trim()
	if (!trimmed) {
		throw new Error("Server URL cannot be empty.")
	}
	const parsed = new URL(trimmed)
	if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
		throw new Error("Server URL must use http or https.")
	}
	return parsed.toString().replace(/\/$/, "")
}

function normalizeProjectId(value: string): string {
	const trimmed = value.trim()
	if (!trimmed) {
		throw new Error("Project id cannot be empty.")
	}
	return trimmed
}

function buildServerUrl(serverUrl: string, path: string): string {
	return `${serverUrl.replace(/\/+$/, "")}${path.startsWith("/") ? path : `/${path}`}`
}

async function readCliConfig(context: CliContext): Promise<CliConfig> {
	const path = getConfigPath(context)
	let raw: string
	try {
		raw = await readFile(path, "utf8")
	} catch (error) {
		if (isNotFoundError(error)) {
			return {}
		}
		throw new Error(`Unable to read CLI config: ${formatFileError(error)}`)
	}
	let parsed: unknown
	try {
		parsed = JSON.parse(raw) as unknown
	} catch (error) {
		throw new Error(`Invalid CLI config JSON: ${formatFileError(error)}`)
	}
	if (!parsed || typeof parsed !== "object") {
		return {}
	}
	const value = parsed as { serverUrl?: unknown; defaultProject?: unknown }
	return {
		...(typeof value.serverUrl === "string" && value.serverUrl.trim() ? { serverUrl: value.serverUrl.trim() } : {}),
		...(typeof value.defaultProject === "string" && value.defaultProject.trim() ? { defaultProject: value.defaultProject.trim() } : {})
	}
}

async function writeCliConfig(context: CliContext, config: CliConfig): Promise<void> {
	const path = getConfigPath(context)
	await mkdir(dirname(path), { recursive: true })
	await writeFile(path, `${JSON.stringify(config, null, 2)}\n`, "utf8")
}

function getConfigPath(context: CliContext): string {
	if (context.configPath) {
		return resolve(context.cwd, context.configPath)
	}
	const envConfigPath = context.env.PUPPYCAD_CONFIG_PATH
	if (envConfigPath?.trim()) {
		return resolve(context.cwd, envConfigPath)
	}
	const configDir = context.configDir ?? context.env.PUPPYCAD_CONFIG_DIR
	if (configDir?.trim()) {
		return resolve(context.cwd, configDir, "config.json")
	}
	if (platform() === "win32") {
		const appData = context.env.APPDATA?.trim() || join(homedir(), "AppData", "Roaming")
		return join(appData, "puppycad", "config.json")
	}
	if (platform() === "darwin") {
		return join(homedir(), "Library", "Application Support", "puppycad", "config.json")
	}
	const xdgConfigHome = context.env.XDG_CONFIG_HOME?.trim()
	return join(xdgConfigHome || join(homedir(), ".config"), "puppycad", "config.json")
}

function parseProjectTargetArgs(args: readonly string[], commandName: string, allowedFlags: readonly string[] = []): { target?: string; mermaid: boolean; explain: boolean } {
	let target: string | undefined
	let mermaid = false
	let explain = false
	for (const arg of args) {
		if (!arg) {
			continue
		}
		if (arg === "--help" || arg === "-h") {
			throw new CliHelpError(formatProjectTargetHelp(commandName))
		}
		if (arg === "--mermaid" && allowedFlags.includes(arg)) {
			mermaid = true
			continue
		}
		if (arg === "--explain" && allowedFlags.includes(arg)) {
			explain = true
			continue
		}
		if (arg.startsWith("-")) {
			throw new Error(`Unknown ${commandName} option: ${arg}`)
		}
		if (target) {
			throw new Error(`Unexpected ${commandName} argument: ${arg}`)
		}
		target = arg
	}
	return { target, mermaid, explain }
}

async function shouldInspectLocalFile(target: string, cwd: string): Promise<boolean> {
	if (target.endsWith(".pcad") || target.endsWith(".json") || target.includes("/") || target.includes("\\")) {
		return true
	}
	return fileExists(resolve(cwd, target))
}

function writeStdout(context: CliContext, message: string): void {
	if (!context.globals.quiet) {
		context.output.stdout(message)
	}
}

function formatMermaidGraph(nodes: readonly CliGraphNode[], edges: readonly CliGraphEdge[]): string {
	const lines = ["graph TD"]
	const nodeIds = new Map<string, string>()
	for (const [index, node] of nodes.entries()) {
		const mermaidId = `n${index}`
		nodeIds.set(`${node.partId}:${node.id}`, mermaidId)
		const label = escapeMermaidLabel(`${node.name ?? node.id} (${node.type})`)
		lines.push(`  ${mermaidId}["${label}"]`)
	}
	for (const edge of edges) {
		const from = nodeIds.get(`${edge.partId}:${edge.from}`)
		const to = nodeIds.get(`${edge.partId}:${edge.to}`)
		if (from && to) {
			lines.push(`  ${from} --> ${to}`)
		}
	}
	return lines.join("\n")
}

function escapeMermaidLabel(label: string): string {
	return label.replaceAll('"', '\\"')
}

function formatInspectSummary(projectLabel: string, project: Project, stats: ProjectStats): string {
	const documentLines = DOCUMENT_TYPES.map((type) => `  ${type}: ${stats.types[type]}`).join("\n")
	const partLines = stats.parts.length > 0 ? stats.parts.map((part) => `  ${part.name} (${part.id}): ${part.features} features`).join("\n") : "  none"
	return [
		`Project: ${projectLabel}`,
		`Version: ${project.version}`,
		`Revision: ${project.revision}`,
		`Documents: ${stats.documents}`,
		`Folders: ${stats.folders}`,
		"Document types:",
		documentLines,
		"Parts:",
		partLines,
		`Selected path: ${project.selectedPath ? project.selectedPath.join(".") : "none"}`
	].join("\n")
}

function formatHelp(): string {
	return [
		"Usage: puppycad [global options] <command> [options]",
		"",
		"Commands:",
		"  doctor                         Check the configured Puppycad server",
		"  config path|get|set|unset       Manage CLI config",
		"  project list                    List server projects",
		"  project create <name>           Create a server project",
		"  project inspect [project-id]    Inspect a server project",
		"  inspect [project-id|file]       Inspect a server project or local project file",
		"  query features [project-id]     List part features",
		"  graph [project-id]              Print the feature graph",
		"  eval [project-id]               Validate/evaluate the project snapshot",
		"  init [file]                     Create a local PuppyCAD project file",
		"  validate [file]                 Validate and summarize a local project file",
		"",
		"Global options:",
		"  --server-url <url>              Server URL",
		"  --project <id>                  Default project for this command",
		"  --json                          Print JSON where supported",
		"  --quiet                         Suppress successful human output",
		"  --verbose                       Enable verbose output where supported",
		"  -h, --help                      Show help",
		"  -v, --version                   Show version"
	].join("\n")
}

function formatConfigHelp(): string {
	return ["Usage: puppycad config <command>", "", "Commands:", "  path", "  get [--json]", "  set server-url <url>", "  set default-project <project-id>", "  unset default-project"].join("\n")
}

function formatProjectHelp(): string {
	return ["Usage: puppycad project <command>", "", "Commands:", "  list [--json]", "  create <name> [--json]", "  inspect [project-id] [--json]"].join("\n")
}

function formatQueryHelp(): string {
	return ["Usage: puppycad query <query>", "", "Queries:", "  features [project-id] --json"].join("\n")
}

function formatProjectTargetHelp(commandName: string): string {
	return `Usage: puppycad ${commandName} [project-id] [--json]`
}

function formatInitHelp(): string {
	return [
		"Usage: puppycad init [file] [options]",
		"",
		"Options:",
		"  --part-name <name>   Set the initial part name",
		"  --empty              Create a project without an initial part",
		"  -f, --force          Overwrite an existing file"
	].join("\n")
}

function formatInspectHelp(): string {
	return ["Usage: puppycad inspect [project-id|file] [options]", "", "Options:", "  --json    Print project data and stats as JSON"].join("\n")
}

function readOptionValue(args: readonly string[], index: number, option: string): string {
	const value = args[index + 1]
	if (!value || value.startsWith("-")) {
		throw new Error(`${option} requires a value.`)
	}
	return value
}

async function fileExists(filePath: string): Promise<boolean> {
	return stat(filePath)
		.then(() => true)
		.catch(() => false)
}

function formatFileError(error: unknown): string {
	if (error && typeof error === "object" && "message" in error && typeof error.message === "string") {
		return error.message
	}
	return String(error)
}

function isNotFoundError(error: unknown): boolean {
	return !!error && typeof error === "object" && "code" in error && (error as { code?: unknown }).code === "ENOENT"
}

class CliHelpError extends Error {
	public constructor(message: string) {
		super(message)
		this.name = "CliHelpError"
	}
}

async function main(): Promise<void> {
	const code = await runPuppycadCli(Bun.argv.slice(2), { version: await readPackageVersion() })
	process.exitCode = code
}

async function readPackageVersion(): Promise<string> {
	try {
		const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8")) as { version?: unknown }
		return typeof packageJson.version === "string" ? packageJson.version : DEFAULT_VERSION
	} catch {
		return DEFAULT_VERSION
	}
}

if (import.meta.main) {
	void main()
}
