#!/usr/bin/env bun

import { mkdir, readFile, stat, writeFile } from "node:fs/promises"
import { homedir, platform } from "node:os"
import { dirname, join, resolve } from "node:path"
import { extrudeSolidFeature } from "./cad/extrude"
import type { Project, ProjectDocument, ProjectDocumentType, ProjectNode } from "./contract"
import { PCadPart, PuppyCadClient } from "./pcad/project"
import { applySyncedProjectCommands, type CadCommand, type SyncedProjectCommand } from "./project-commands"
import { createProjectFile, normalizeProjectFile, serializeProjectFile } from "./project-file"
import { renderProjectPreviewPng, type RenderLabel } from "./render"
import type { PartDocument, PartFeature, SketchDimension, SketchEntity, SketchPlane, Solid, SolidEdge, SolidFace, SolidVertex } from "./schema"
import type { Vector3D } from "./types"

type CliOutput = {
	stdout: (message: string) => void
	stderr: (message: string) => void
}

type CliEnv = Record<string, string | undefined>
type CliFetch = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
type CliRenderPng = (bodies: readonly CliGeometryBody[], options: RenderOptions & { labels?: readonly RenderLabel[] }) => Promise<Uint8Array>

type CliOptions = {
	version?: string
	cwd?: string
	output?: CliOutput
	fetch?: CliFetch
	env?: CliEnv
	configPath?: string
	configDir?: string
	renderPng?: CliRenderPng
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
	renderPng: CliRenderPng
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

type CliGeometryBody = {
	id: string
	partId: string
	sourceId: string
	vertices: readonly SolidVertex[]
	edges: readonly SolidEdge[]
	faces: readonly SolidFace[]
	bbox: CliBoundingBox | null
}

type CliBoundingBox = {
	min: Vector3D
	max: Vector3D
	size: Vector3D
}

type CliGeometry = {
	bodies: CliGeometryBody[]
	errors: { partId: string; featureId: string; message: string }[]
}

type QueryArgs = {
	target?: string
	bodyId?: string
}

type RenderOptions = {
	target?: string
	outPath: string
	width?: number
	height?: number
	showDimensions?: boolean
}

type DimensionSetOptions = {
	target?: string
	partId: string
	sketchId: string
	entityId: string
	type: SketchDimension["type"]
	value: number
	dimensionId?: string
}

type SketchCreateOptions = {
	target?: string
	partId: string
	sketchId: string
	name: string
	plane: SketchPlane
}

type SketchRectangleOptions = {
	target?: string
	partId: string
	sketchId: string
	entityId: string
	x0: number
	y0: number
	x1: number
	y1: number
}

type SketchFinishOptions = {
	target?: string
	partId: string
	sketchId: string
}

type ExtrudeCreateOptions = {
	target?: string
	partId: string
	extrudeId: string
	name: string
	sketchId: string
	profileId?: string
	depth: number
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
		renderPng: options.renderPng ?? renderProjectPreviewPng,
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
		if (command === "cad") {
			return await runCadCommand(rest, context)
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
		if (command === "render") {
			return await runRenderCommand(rest, context)
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
	try {
		const client = await createPuppyCadClient(context)
		const response = await client.getHealth()
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
		const payload = await parseServerJson<{ projects?: unknown }>(context, (await createPuppyCadClient(context)).listProjects())
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
		const payload = await parseServerJson<{ projectId?: unknown; project?: unknown; revision?: unknown }>(context, (await createPuppyCadClient(context)).createProject(project))
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

async function runCadCommand(args: readonly string[], context: CliContext): Promise<number> {
	const [area, action, ...rest] = args
	if (!area || area === "--help" || area === "-h") {
		context.output.stdout(formatCadHelp())
		return 0
	}
	if (area === "sketch" && action === "create") {
		const options = parseSketchCreateArgs(rest)
		return executeCadCommand(
			context,
			options.target,
			options.partId,
			{ type: "createSketch", sketchId: options.sketchId, name: options.name, target: { type: "plane", plane: options.plane } },
			`Created sketch ${options.sketchId}`
		)
	}
	if (area === "sketch" && action === "rectangle") {
		const options = parseSketchRectangleArgs(rest)
		return executeCadCommand(
			context,
			options.target,
			options.partId,
			{
				type: "addSketchEntity",
				sketchId: options.sketchId,
				entity: {
					id: options.entityId,
					type: "cornerRectangle",
					p0: { x: options.x0, y: options.y0 },
					p1: { x: options.x1, y: options.y1 }
				}
			},
			`Added rectangle ${options.entityId}`
		)
	}
	if (area === "sketch" && action === "finish") {
		const options = parseSketchFinishArgs(rest)
		return executeCadCommand(context, options.target, options.partId, { type: "finishSketch", sketchId: options.sketchId }, `Finished sketch ${options.sketchId}`)
	}
	if (area === "dimension" && action === "set") {
		const options = parseDimensionSetArgs(rest)
		const dimension: SketchDimension = {
			id: options.dimensionId ?? getDefaultDimensionId(options.sketchId, options.type, options.entityId),
			type: options.type,
			entityId: options.entityId,
			value: options.value
		}
		return executeCadCommand(
			context,
			options.target,
			options.partId,
			{ type: "setSketchDimension", sketchId: options.sketchId, dimension },
			`Set ${dimension.type} dimension ${dimension.id} on ${dimension.entityId} to ${dimension.value}`,
			{ sketchId: options.sketchId, dimension }
		)
	}
	if (area === "extrude" && action === "create") {
		const options = parseExtrudeCreateArgs(rest)
		return executeCadCommand(
			context,
			options.target,
			options.partId,
			{
				type: "createExtrude",
				extrudeId: options.extrudeId,
				name: options.name,
				sketchId: options.sketchId,
				profileId: options.profileId ?? `${options.sketchId}-profile-1`,
				depth: options.depth
			},
			`Created extrude ${options.extrudeId}`
		)
	}
	throw new Error(`Unknown cad command: ${[area, action].filter(Boolean).join(" ")}`)
}

async function runQueryCommand(args: readonly string[], context: CliContext): Promise<number> {
	const [query, ...rest] = args
	if (!query || query === "--help" || query === "-h") {
		context.output.stdout(formatQueryHelp())
		return 0
	}
	if (!["features", "geometry", "bodies", "faces", "edges", "bbox"].includes(query)) {
		throw new Error(`Unknown query: ${query}`)
	}
	const { target, bodyId } = parseQueryArgs(rest, `query ${query}`)
	const { projectId, project } = await loadServerProject(context, target)
	if (query !== "features") {
		const geometry = collectProjectGeometry(project)
		return writeGeometryQuery(query, projectId, geometry, bodyId, context)
	}
	const features = collectProjectFeatures(project)
	if (context.globals.json) {
		writeStdout(context, JSON.stringify({ projectId, features }, null, 2))
		return 0
	}
	writeStdout(context, features.length > 0 ? features.map((feature) => `${feature.id} ${feature.type} part=${feature.partId}`).join("\n") : "No features.")
	return 0
}

function writeGeometryQuery(query: string, projectId: string, geometry: CliGeometry, bodyId: string | undefined, context: CliContext): number {
	const bodies = bodyId ? geometry.bodies.filter((body) => body.id === bodyId) : geometry.bodies
	if (bodyId && bodies.length === 0) {
		throw new Error(`Body not found: ${bodyId}`)
	}
	if (query === "geometry") {
		if (context.globals.json) {
			writeStdout(context, JSON.stringify({ projectId, bodies, errors: geometry.errors }, null, 2))
			return geometry.errors.length > 0 ? 1 : 0
		}
		writeStdout(context, bodies.length > 0 ? bodies.map(formatBodySummary).join("\n") : "No generated geometry.")
		return geometry.errors.length > 0 ? 1 : 0
	}
	if (query === "bodies") {
		const summaries = bodies.map((body) => ({
			id: body.id,
			partId: body.partId,
			sourceId: body.sourceId,
			vertices: body.vertices.length,
			edges: body.edges.length,
			faces: body.faces.length,
			bbox: body.bbox
		}))
		if (context.globals.json) {
			writeStdout(context, JSON.stringify({ projectId, bodies: summaries, errors: geometry.errors }, null, 2))
			return geometry.errors.length > 0 ? 1 : 0
		}
		writeStdout(
			context,
			summaries.length > 0
				? summaries.map((body) => `${body.id} part=${body.partId} source=${body.sourceId} vertices=${body.vertices} edges=${body.edges} faces=${body.faces}`).join("\n")
				: "No bodies."
		)
		return geometry.errors.length > 0 ? 1 : 0
	}
	if (query === "faces") {
		const faces = bodies.flatMap((body) => body.faces.map((face) => ({ ...face, bodyId: body.id, partId: body.partId, sourceId: body.sourceId })))
		if (context.globals.json) {
			writeStdout(context, JSON.stringify({ projectId, faces, errors: geometry.errors }, null, 2))
			return geometry.errors.length > 0 ? 1 : 0
		}
		writeStdout(context, faces.length > 0 ? faces.map((face) => `${face.id} body=${face.bodyId} edges=${face.edgeIds.length}`).join("\n") : "No faces.")
		return geometry.errors.length > 0 ? 1 : 0
	}
	if (query === "edges") {
		const edges = bodies.flatMap((body) => body.edges.map((edge) => ({ ...edge, bodyId: body.id, partId: body.partId, sourceId: body.sourceId })))
		if (context.globals.json) {
			writeStdout(context, JSON.stringify({ projectId, edges, errors: geometry.errors }, null, 2))
			return geometry.errors.length > 0 ? 1 : 0
		}
		writeStdout(context, edges.length > 0 ? edges.map((edge) => `${edge.id} body=${edge.bodyId} vertices=${edge.vertexIds.join(",")}`).join("\n") : "No edges.")
		return geometry.errors.length > 0 ? 1 : 0
	}
	const bboxes = bodies.map((body) => ({ bodyId: body.id, partId: body.partId, sourceId: body.sourceId, bbox: body.bbox }))
	if (context.globals.json) {
		writeStdout(context, JSON.stringify({ projectId, bboxes, errors: geometry.errors }, null, 2))
		return geometry.errors.length > 0 ? 1 : 0
	}
	writeStdout(context, bboxes.length > 0 ? bboxes.map((body) => `${body.bodyId} ${formatBoundingBox(body.bbox)}`).join("\n") : "No bounding boxes.")
	return geometry.errors.length > 0 ? 1 : 0
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

async function runRenderCommand(args: readonly string[], context: CliContext): Promise<number> {
	const options = parseRenderArgs(args)
	const { projectId, project } = await loadProjectForRead(context, options.target)
	const geometry = collectProjectGeometry(project)
	if (geometry.errors.length > 0) {
		throw new Error(`Cannot render project with geometry errors: ${geometry.errors.map((error) => `${error.partId}/${error.featureId}: ${error.message}`).join("; ")}`)
	}
	if (geometry.bodies.length === 0) {
		throw new Error("Project has no generated solid geometry to render.")
	}
	const labels = options.showDimensions ? collectProjectDimensionLabels(project) : []
	const png = await context.renderPng(geometry.bodies, { ...options, labels })
	const outPath = resolve(context.cwd, options.outPath)
	await mkdir(dirname(outPath), { recursive: true })
	await writeFile(outPath, png)
	if (context.globals.json) {
		writeStdout(
			context,
			JSON.stringify({ projectId, out: outPath, width: options.width ?? 1024, height: options.height ?? 768, bodies: geometry.bodies.length, labels: labels.length }, null, 2)
		)
		return 0
	}
	writeStdout(context, `Rendered ${projectId} to ${outPath}`)
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

function collectProjectGeometry(project: Project): CliGeometry {
	const bodies: CliGeometryBody[] = []
	const errors: CliGeometry["errors"] = []
	visitProjectNodes(project.items, (node) => {
		if (isProjectFolder(node) || node.type !== "part") {
			return
		}
		const part = new PCadPart(node.data).getDocument() as PartDocument
		for (const feature of part.features) {
			if (feature.type !== "extrude") {
				continue
			}
			try {
				const generated = extrudeSolidFeature(part, feature)
				bodies.push(toCliGeometryBody(node.id, generated.solid))
			} catch (error) {
				errors.push({ partId: node.id, featureId: feature.id, message: formatFileError(error) })
			}
		}
	})
	return { bodies, errors }
}

function collectProjectDimensionLabels(project: Project): RenderLabel[] {
	const labels: RenderLabel[] = []
	visitProjectNodes(project.items, (node) => {
		if (isProjectFolder(node) || node.type !== "part") {
			return
		}
		const part = new PCadPart(node.data).getDocument() as PartDocument
		for (const feature of part.features) {
			if (feature.type !== "sketch" || feature.target.type !== "plane") {
				continue
			}
			const entitiesById = new Map(feature.entities.map((entity) => [entity.id, entity] as const))
			for (const dimension of feature.dimensions) {
				const entity = entitiesById.get(dimension.entityId)
				if (!entity) {
					continue
				}
				const position = getDimensionLabelPosition(feature.target.plane, entity, dimension)
				if (position) {
					labels.push({ text: `${formatNumber(dimension.value)}mm`, position })
				}
			}
		}
	})
	return labels
}

function getDimensionLabelPosition(plane: SketchPlane, entity: SketchEntity, dimension: SketchDimension): RenderLabel["position"] | null {
	if (entity.type === "line" && dimension.type === "lineLength") {
		return sketchPointToWorld(plane, { x: (entity.p0.x + entity.p1.x) / 2, y: (entity.p0.y + entity.p1.y) / 2 })
	}
	if (entity.type !== "cornerRectangle") {
		return null
	}
	const minX = Math.min(entity.p0.x, entity.p1.x)
	const maxX = Math.max(entity.p0.x, entity.p1.x)
	const minY = Math.min(entity.p0.y, entity.p1.y)
	const maxY = Math.max(entity.p0.y, entity.p1.y)
	const offset = Math.max(maxX - minX, maxY - minY, 1) * 0.18
	if (dimension.type === "rectangleWidth") {
		return sketchPointToWorld(plane, { x: (minX + maxX) / 2, y: minY - offset })
	}
	if (dimension.type === "rectangleHeight") {
		return sketchPointToWorld(plane, { x: minX - offset, y: (minY + maxY) / 2 })
	}
	return null
}

function sketchPointToWorld(plane: SketchPlane, point: { x: number; y: number }): RenderLabel["position"] {
	if (plane === "XZ") {
		return { x: point.x, y: 0, z: point.y }
	}
	if (plane === "YZ") {
		return { x: 0, y: point.x, z: point.y }
	}
	return { x: point.x, y: point.y, z: 0 }
}

function toCliGeometryBody(partId: string, solid: Solid): CliGeometryBody {
	return {
		id: solid.id,
		partId,
		sourceId: solid.featureId,
		vertices: solid.vertices,
		edges: solid.edges,
		faces: solid.faces,
		bbox: computeBoundingBox(solid.vertices)
	}
}

function computeBoundingBox(vertices: readonly SolidVertex[]): CliBoundingBox | null {
	if (vertices.length === 0) {
		return null
	}
	const first = vertices[0]
	if (!first) {
		return null
	}
	const min = { ...first.position }
	const max = { ...first.position }
	for (const vertex of vertices.slice(1)) {
		min.x = Math.min(min.x, vertex.position.x)
		min.y = Math.min(min.y, vertex.position.y)
		min.z = Math.min(min.z, vertex.position.z)
		max.x = Math.max(max.x, vertex.position.x)
		max.y = Math.max(max.y, vertex.position.y)
		max.z = Math.max(max.z, vertex.position.z)
	}
	return {
		min,
		max,
		size: {
			x: max.x - min.x,
			y: max.y - min.y,
			z: max.z - min.z
		}
	}
}

function formatBodySummary(body: CliGeometryBody): string {
	return `${body.id} part=${body.partId} source=${body.sourceId} vertices=${body.vertices.length} edges=${body.edges.length} faces=${body.faces.length} bbox=${formatBoundingBox(body.bbox)}`
}

function formatBoundingBox(bbox: CliBoundingBox | null): string {
	if (!bbox) {
		return "none"
	}
	return `min=(${formatNumber(bbox.min.x)},${formatNumber(bbox.min.y)},${formatNumber(bbox.min.z)}) max=(${formatNumber(bbox.max.x)},${formatNumber(bbox.max.y)},${formatNumber(bbox.max.z)}) size=(${formatNumber(bbox.size.x)},${formatNumber(bbox.size.y)},${formatNumber(bbox.size.z)})`
}

function formatNumber(value: number): string {
	return Number.isInteger(value) ? String(value) : value.toFixed(6).replace(/0+$/, "").replace(/\.$/, "")
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

function findProjectPart(project: Project, partId: string): Extract<ProjectDocument, { type: "part" }> | null {
	let part: Extract<ProjectDocument, { type: "part" }> | null = null
	visitProjectNodes(project.items, (node) => {
		if (!isProjectFolder(node) && node.type === "part" && node.id === partId) {
			part = node
		}
	})
	return part
}

function isProjectFolder(node: ProjectNode): node is Exclude<ProjectNode, ProjectDocument> {
	return "kind" in node && node.kind === "folder"
}

async function loadProjectForRead(context: CliContext, target: string | undefined): Promise<{ projectId: string; project: Project }> {
	if (target && (await shouldInspectLocalFile(target, context.cwd))) {
		const filePath = resolve(context.cwd, target)
		const raw = await readFile(filePath, "utf8").catch((error: unknown) => {
			throw new Error(`Unable to read project file: ${formatFileError(error)}`)
		})
		const project = normalizeProjectFile(JSON.parse(raw))
		if (!project) {
			throw new Error(`Invalid PuppyCAD project file: ${filePath}`)
		}
		return { projectId: filePath, project }
	}
	return loadServerProject(context, target)
}

async function executeCadCommand(context: CliContext, target: string | undefined, partId: string, command: CadCommand, message: string, jsonFields: Record<string, unknown> = {}): Promise<number> {
	const syncedCommand: SyncedProjectCommand = { type: "cad", partId, command }
	if (target && (await shouldInspectLocalFile(target, context.cwd))) {
		const filePath = resolve(context.cwd, target)
		const raw = await readFile(filePath, "utf8").catch((error: unknown) => {
			throw new Error(`Unable to read project file: ${formatFileError(error)}`)
		})
		const project = normalizeProjectFile(JSON.parse(raw))
		if (!project) {
			throw new Error(`Invalid PuppyCAD project file: ${filePath}`)
		}
		const nextProject = applySyncedProjectCommands(project, [syncedCommand])
		nextProject.revision = project.revision + 1
		await writeFile(filePath, `${serializeProjectFile(nextProject)}\n`, "utf8")
		if (context.globals.json) {
			writeStdout(context, JSON.stringify({ file: filePath, revision: nextProject.revision, partId, ...jsonFields, command }, null, 2))
			return 0
		}
		writeStdout(context, `${message} (file=${filePath} revision=${nextProject.revision})`)
		return 0
	}

	const { projectId, project } = await loadServerProject(context, target)
	const client = await createPuppyCadClient(context)
	const payload = await parseServerJson<{ projectId?: unknown; revision?: unknown; project?: unknown }>(
		context,
		client.postProjectCommands(projectId, {
			clientId: "puppycad-cli",
			baseRevision: project.revision,
			commands: [syncedCommand]
		})
	)
	const revision = typeof payload.revision === "number" ? payload.revision : project.revision + 1
	if (context.globals.json) {
		writeStdout(context, JSON.stringify({ projectId, revision, partId, ...jsonFields, command }, null, 2))
		return 0
	}
	writeStdout(context, `${message} (project=${projectId} revision=${revision})`)
	return 0
}

async function loadServerProject(context: CliContext, target: string | undefined): Promise<{ projectId: string; project: Project }> {
	const projectId = await resolveProjectId(context, target)
	const payload = await parseServerJson<{ projectId?: unknown; project?: unknown }>(context, (await createPuppyCadClient(context)).loadProject(projectId))
	const project = normalizeProjectFile(payload.project)
	if (!project) {
		throw new Error(`Server returned an invalid project for ${projectId}.`)
	}
	return {
		projectId: typeof payload.projectId === "string" ? payload.projectId : projectId,
		project
	}
}

async function createPuppyCadClient(context: CliContext): Promise<PuppyCadClient> {
	return new PuppyCadClient({
		fetch: context.fetch,
		apiBasePath: await resolveServerUrl(context)
	})
}

async function parseServerJson<T>(context: CliContext, responsePromise: Promise<Response>): Promise<T> {
	const serverUrl = await resolveServerUrl(context)
	let response: Response
	try {
		response = await responsePromise
	} catch (error) {
		throw new Error(`Unable to reach Puppycad server at ${serverUrl}: ${formatFileError(error)}`)
	}
	const payload = (await readJsonPayload(response)) as T & { ok?: unknown; message?: unknown }
	if (!response.ok || payload?.ok === false) {
		const message = typeof payload?.message === "string" ? payload.message : `Server responded with ${response.status}`
		throw new Error(message)
	}
	return payload
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

function parseQueryArgs(args: readonly string[], commandName: string): QueryArgs {
	let target: string | undefined
	let bodyId: string | undefined
	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) {
			continue
		}
		if (arg === "--help" || arg === "-h") {
			throw new CliHelpError(formatQueryHelp())
		}
		if (arg === "--body") {
			bodyId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--body=")) {
			bodyId = arg.slice("--body=".length)
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
	return {
		...(target ? { target } : {}),
		...(bodyId ? { bodyId } : {})
	}
}

function parseSketchCreateArgs(args: readonly string[]): SketchCreateOptions {
	let target: string | undefined
	let partId: string | undefined
	let sketchId: string | undefined
	let name = "Sketch"
	let plane: SketchPlane | undefined
	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) continue
		if (arg === "--help" || arg === "-h") throw new CliHelpError(formatCadHelp())
		if (arg === "--part") {
			partId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--part=")) {
			partId = arg.slice("--part=".length)
			continue
		}
		if (arg === "--sketch") {
			sketchId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--sketch=")) {
			sketchId = arg.slice("--sketch=".length)
			continue
		}
		if (arg === "--name") {
			name = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--name=")) {
			name = arg.slice("--name=".length)
			continue
		}
		if (arg === "--plane") {
			plane = parseSketchPlane(readOptionValue(args, index, arg))
			index += 1
			continue
		}
		if (arg.startsWith("--plane=")) {
			plane = parseSketchPlane(arg.slice("--plane=".length))
			continue
		}
		if (arg.startsWith("-")) throw new Error(`Unknown cad sketch create option: ${arg}`)
		if (target) throw new Error(`Unexpected cad sketch create argument: ${arg}`)
		target = arg
	}
	if (!partId || !sketchId || !plane) {
		throw new Error("Usage: puppycad cad sketch create [project-id|file] --part <part-id> --sketch <sketch-id> --plane <XY|YZ|XZ> [--name <name>]")
	}
	return { partId, sketchId, name, plane, ...(target ? { target } : {}) }
}

function parseSketchRectangleArgs(args: readonly string[]): SketchRectangleOptions {
	let target: string | undefined
	let partId: string | undefined
	let sketchId: string | undefined
	let entityId: string | undefined
	let x0: number | undefined
	let y0: number | undefined
	let x1: number | undefined
	let y1: number | undefined
	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) continue
		if (arg === "--help" || arg === "-h") throw new CliHelpError(formatCadHelp())
		if (arg === "--part") {
			partId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--part=")) {
			partId = arg.slice("--part=".length)
			continue
		}
		if (arg === "--sketch") {
			sketchId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--sketch=")) {
			sketchId = arg.slice("--sketch=".length)
			continue
		}
		if (arg === "--entity") {
			entityId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--entity=")) {
			entityId = arg.slice("--entity=".length)
			continue
		}
		if (arg === "--x0" || arg === "--y0" || arg === "--x1" || arg === "--y1") {
			const value = parseFiniteNumber(readOptionValue(args, index, arg), arg)
			if (arg === "--x0") x0 = value
			if (arg === "--y0") y0 = value
			if (arg === "--x1") x1 = value
			if (arg === "--y1") y1 = value
			index += 1
			continue
		}
		if (arg.startsWith("--x0=")) {
			x0 = parseFiniteNumber(arg.slice("--x0=".length), "--x0")
			continue
		}
		if (arg.startsWith("--y0=")) {
			y0 = parseFiniteNumber(arg.slice("--y0=".length), "--y0")
			continue
		}
		if (arg.startsWith("--x1=")) {
			x1 = parseFiniteNumber(arg.slice("--x1=".length), "--x1")
			continue
		}
		if (arg.startsWith("--y1=")) {
			y1 = parseFiniteNumber(arg.slice("--y1=".length), "--y1")
			continue
		}
		if (arg.startsWith("-")) throw new Error(`Unknown cad sketch rectangle option: ${arg}`)
		if (target) throw new Error(`Unexpected cad sketch rectangle argument: ${arg}`)
		target = arg
	}
	if (!partId || !sketchId || !entityId || x0 === undefined || y0 === undefined || x1 === undefined || y1 === undefined) {
		throw new Error("Usage: puppycad cad sketch rectangle [project-id|file] --part <part-id> --sketch <sketch-id> --entity <entity-id> --x0 <n> --y0 <n> --x1 <n> --y1 <n>")
	}
	return { partId, sketchId, entityId, x0, y0, x1, y1, ...(target ? { target } : {}) }
}

function parseSketchFinishArgs(args: readonly string[]): SketchFinishOptions {
	let target: string | undefined
	let partId: string | undefined
	let sketchId: string | undefined
	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) continue
		if (arg === "--help" || arg === "-h") throw new CliHelpError(formatCadHelp())
		if (arg === "--part") {
			partId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--part=")) {
			partId = arg.slice("--part=".length)
			continue
		}
		if (arg === "--sketch") {
			sketchId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--sketch=")) {
			sketchId = arg.slice("--sketch=".length)
			continue
		}
		if (arg.startsWith("-")) throw new Error(`Unknown cad sketch finish option: ${arg}`)
		if (target) throw new Error(`Unexpected cad sketch finish argument: ${arg}`)
		target = arg
	}
	if (!partId || !sketchId) {
		throw new Error("Usage: puppycad cad sketch finish [project-id|file] --part <part-id> --sketch <sketch-id>")
	}
	return { partId, sketchId, ...(target ? { target } : {}) }
}

function parseExtrudeCreateArgs(args: readonly string[]): ExtrudeCreateOptions {
	let target: string | undefined
	let partId: string | undefined
	let extrudeId: string | undefined
	let name = "Extrude"
	let sketchId: string | undefined
	let profileId: string | undefined
	let depth: number | undefined
	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) continue
		if (arg === "--help" || arg === "-h") throw new CliHelpError(formatCadHelp())
		if (arg === "--part") {
			partId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--part=")) {
			partId = arg.slice("--part=".length)
			continue
		}
		if (arg === "--extrude") {
			extrudeId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--extrude=")) {
			extrudeId = arg.slice("--extrude=".length)
			continue
		}
		if (arg === "--name") {
			name = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--name=")) {
			name = arg.slice("--name=".length)
			continue
		}
		if (arg === "--sketch") {
			sketchId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--sketch=")) {
			sketchId = arg.slice("--sketch=".length)
			continue
		}
		if (arg === "--profile") {
			profileId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--profile=")) {
			profileId = arg.slice("--profile=".length)
			continue
		}
		if (arg === "--depth") {
			depth = parsePositiveNumber(readOptionValue(args, index, arg), arg)
			index += 1
			continue
		}
		if (arg.startsWith("--depth=")) {
			depth = parsePositiveNumber(arg.slice("--depth=".length), "--depth")
			continue
		}
		if (arg.startsWith("-")) throw new Error(`Unknown cad extrude create option: ${arg}`)
		if (target) throw new Error(`Unexpected cad extrude create argument: ${arg}`)
		target = arg
	}
	if (!partId || !extrudeId || !sketchId || depth === undefined) {
		throw new Error(
			"Usage: puppycad cad extrude create [project-id|file] --part <part-id> --extrude <extrude-id> --sketch <sketch-id> --depth <number> [--profile <profile-id>] [--name <name>]"
		)
	}
	return { partId, extrudeId, name, sketchId, depth, ...(profileId ? { profileId } : {}), ...(target ? { target } : {}) }
}

function parseDimensionSetArgs(args: readonly string[]): DimensionSetOptions {
	let target: string | undefined
	let partId: string | undefined
	let sketchId: string | undefined
	let entityId: string | undefined
	let type: SketchDimension["type"] | undefined
	let value: number | undefined
	let dimensionId: string | undefined
	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) {
			continue
		}
		if (arg === "--help" || arg === "-h") {
			throw new CliHelpError(formatCadHelp())
		}
		if (arg === "--part") {
			partId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--part=")) {
			partId = arg.slice("--part=".length)
			continue
		}
		if (arg === "--sketch") {
			sketchId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--sketch=")) {
			sketchId = arg.slice("--sketch=".length)
			continue
		}
		if (arg === "--entity") {
			entityId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--entity=")) {
			entityId = arg.slice("--entity=".length)
			continue
		}
		if (arg === "--type") {
			type = parseSketchDimensionType(readOptionValue(args, index, arg))
			index += 1
			continue
		}
		if (arg.startsWith("--type=")) {
			type = parseSketchDimensionType(arg.slice("--type=".length))
			continue
		}
		if (arg === "--value") {
			value = parsePositiveNumber(readOptionValue(args, index, arg), arg)
			index += 1
			continue
		}
		if (arg.startsWith("--value=")) {
			value = parsePositiveNumber(arg.slice("--value=".length), "--value")
			continue
		}
		if (arg === "--id" || arg === "--dimension-id") {
			dimensionId = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--id=")) {
			dimensionId = arg.slice("--id=".length)
			continue
		}
		if (arg.startsWith("--dimension-id=")) {
			dimensionId = arg.slice("--dimension-id=".length)
			continue
		}
		if (arg.startsWith("-")) {
			throw new Error(`Unknown cad dimension set option: ${arg}`)
		}
		if (target) {
			throw new Error(`Unexpected cad dimension set argument: ${arg}`)
		}
		target = arg
	}
	if (!partId || !sketchId || !entityId || !type || value === undefined) {
		throw new Error(
			"Usage: puppycad cad dimension set [project-id] --part <part-id> --sketch <sketch-id> --entity <entity-id> --type <lineLength|rectangleWidth|rectangleHeight> --value <number>"
		)
	}
	return {
		partId,
		sketchId,
		entityId,
		type,
		value,
		...(target ? { target } : {}),
		...(dimensionId ? { dimensionId } : {})
	}
}

function parseSketchDimensionType(value: string): SketchDimension["type"] {
	if (value === "lineLength" || value === "rectangleWidth" || value === "rectangleHeight") {
		return value
	}
	throw new Error(`Invalid dimension type: ${value}`)
}

function parseSketchPlane(value: string): SketchPlane {
	if (value === "XY" || value === "YZ" || value === "XZ") {
		return value
	}
	throw new Error(`Invalid sketch plane: ${value}`)
}

function parseRenderArgs(args: readonly string[]): RenderOptions {
	let target: string | undefined
	let outPath: string | undefined
	let width: number | undefined
	let height: number | undefined
	let showDimensions = false
	for (let index = 0; index < args.length; index += 1) {
		const arg = args[index]
		if (!arg) {
			continue
		}
		if (arg === "--help" || arg === "-h") {
			throw new CliHelpError(formatRenderHelp())
		}
		if (arg === "--out" || arg === "-o") {
			outPath = readOptionValue(args, index, arg)
			index += 1
			continue
		}
		if (arg.startsWith("--out=")) {
			outPath = arg.slice("--out=".length)
			continue
		}
		if (arg === "--width") {
			width = parsePositiveInteger(readOptionValue(args, index, arg), arg)
			index += 1
			continue
		}
		if (arg.startsWith("--width=")) {
			width = parsePositiveInteger(arg.slice("--width=".length), "--width")
			continue
		}
		if (arg === "--height") {
			height = parsePositiveInteger(readOptionValue(args, index, arg), arg)
			index += 1
			continue
		}
		if (arg.startsWith("--height=")) {
			height = parsePositiveInteger(arg.slice("--height=".length), "--height")
			continue
		}
		if (arg === "--show-dimensions") {
			showDimensions = true
			continue
		}
		if (arg.startsWith("-")) {
			throw new Error(`Unknown render option: ${arg}`)
		}
		if (target) {
			throw new Error(`Unexpected render argument: ${arg}`)
		}
		target = arg
	}
	if (!outPath) {
		throw new Error("Usage: puppycad render [project-id] --out <preview.png>")
	}
	return {
		outPath,
		...(target ? { target } : {}),
		...(width ? { width } : {}),
		...(height ? { height } : {}),
		...(showDimensions ? { showDimensions } : {})
	}
}

function parsePositiveInteger(value: string, option: string): number {
	const parsed = Number(value)
	if (!Number.isInteger(parsed) || parsed <= 0) {
		throw new Error(`${option} requires a positive integer.`)
	}
	return parsed
}

function parsePositiveNumber(value: string, option: string): number {
	const parsed = Number(value)
	if (!Number.isFinite(parsed) || parsed <= 0) {
		throw new Error(`${option} requires a positive number.`)
	}
	return parsed
}

function parseFiniteNumber(value: string, option: string): number {
	const parsed = Number(value)
	if (!Number.isFinite(parsed)) {
		throw new Error(`${option} requires a finite number.`)
	}
	return parsed
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

function getDefaultDimensionId(sketchId: string, type: SketchDimension["type"], entityId: string): string {
	return `${sketchId}-${type}-${entityId}`
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
		"  cad dimension set [project-id]  Set a sketch dimension constraint",
		"  inspect [project-id|file]       Inspect a server project or local project file",
		"  query features [project-id]     List part features",
		"  query geometry [project-id]     List generated bodies/faces/edges",
		"  query bodies|faces|edges|bbox   Inspect generated geometry",
		"  graph [project-id]              Print the feature graph",
		"  eval [project-id]               Validate/evaluate the project snapshot",
		"  render [project-id] --out <png> Render a PNG preview",
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

function formatCadHelp(): string {
	return [
		"Usage: puppycad cad dimension set [project-id] --part <part-id> --sketch <sketch-id> --entity <entity-id> --type <type> --value <number>",
		"",
		"Dimension types:",
		"  lineLength",
		"  rectangleWidth",
		"  rectangleHeight",
		"",
		"Options:",
		"  --part <id>          Part id",
		"  --sketch <id>        Sketch id",
		"  --entity <id>        Sketch entity id",
		"  --type <type>        Dimension type",
		"  --value <number>     Dimension value",
		"  --id <id>            Dimension id, defaults to sketch/type/entity"
	].join("\n")
}

function formatQueryHelp(): string {
	return [
		"Usage: puppycad query <query> [project-id] [options]",
		"",
		"Queries:",
		"  features [project-id] --json",
		"  geometry [project-id] [--body <body-id>] --json",
		"  bodies [project-id] --json",
		"  faces [project-id] [--body <body-id>] --json",
		"  edges [project-id] [--body <body-id>] --json",
		"  bbox [project-id] [--body <body-id>] --json"
	].join("\n")
}

function formatRenderHelp(): string {
	return [
		"Usage: puppycad render [project-id] --out <preview.png> [options]",
		"",
		"Options:",
		"  -o, --out <file>   Write PNG preview to file",
		"  --width <px>       Image width, default 1024",
		"  --height <px>      Image height, default 768",
		"  --show-dimensions  Draw sketch dimension labels"
	].join("\n")
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
