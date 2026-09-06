import { evaluateSolid, type SolidCombine, type SolidStep, type ProfileFinish } from "./solid-model"
import { requireValue } from "./required"
import { solveFixedAssembly, transformMatrix } from "./assembly-solver"
export { solveFixedAssembly, transformMatrix } from "./assembly-solver"
import type { Assembly, AssemblyConnector, AssemblyMateReference, ProjectAssemblyDocument, ProjectPartDocument, Transform3D, Vector3D } from "./contract"
import { extrudeSolidFeature, getExtrudedFaceDescriptors } from "./cad/extrude"
import { materializeSketch } from "./cad/sketch"
import { compileModel, type BodySpec, type ModelDefinition } from "./model-dsl"
import { PCadProject, PuppyCadClient, type PCadProjectSyncResult } from "./pcad/project"
import { createProjectFile } from "./project-file"
import type { PartDocument, SketchTarget } from "./schema"
import { exportPartStl } from "./part-mesh"
import type { SyncedProjectCommand } from "./project-commands"
export { capsule, circle, component, defineModel, rectangle, roundedRectangle, v2 } from "./model-dsl"
export type { BodySpec, ModelDefinition } from "./model-dsl"
export type ExtrusionSpec = Pick<BodySpec, "name" | "outline" | "depth"> & {
	holes?: readonly (readonly {
		x: number
		y: number
	}[])[]
	operation?: SolidCombine
	topScale?: number
	translation?: Vector3D
	on?: SketchTarget
}
export type PartRef = {
	readonly id: string
	readonly kind: "part"
}
export type InstanceRef = {
	readonly id: string
	readonly kind: "instance"
}
/** A part stays in its own coordinates. Assembly instances supply placement. */
export class PartBuilder {
	readonly document: PartDocument = { features: [], solidSteps: [] }
	revolve(id: string, spec: { outline: readonly { x: number; y: number }[]; angle?: number; segments?: number; operation?: SolidCombine; translation?: Vector3D }): void {
		this.addStep({ ...structuredClone(spec), outline: spec.outline.map((p) => ({ ...p })), id, type: "revolve", operation: spec.operation ?? "join" })
	}
	/** Round selected profile corners: vertical extrusion edges or circular revolve edges. */
	fillet(featureId: string, radius: number, vertices?: number[], segments = 8): void {
		this.finish(featureId, { kind: "fillet", radius, vertices, segments })
	}
	chamfer(featureId: string, distance: number, vertices?: number[]): void {
		this.finish(featureId, { kind: "chamfer", radius: distance, vertices })
	}
	/** Fillet a convex straight edge of an extrusion, identified by its source topology id. */
	filletEdge(featureId: string, edgeId: string, radius: number, segments = 8): void {
		const step = this.document.solidSteps?.find((step) => step.id === featureId && step.type === "extrusion")
		if (!step) throw new Error(`Unknown extrusion: ${featureId}`)
		const previous = step.edgeFinishes
		step.edgeFinishes = [...(previous ?? []), { kind: "fillet", edgeId, radius, segments }]
		try {
			evaluateSolid(this.document)
		} catch (error) {
			step.edgeFinishes = previous
			throw error
		}
	}
	private finish(featureId: string, finish: ProfileFinish): void {
		const step = this.document.solidSteps?.find((step) => step.id === featureId)
		if (!step) throw new Error(`Unknown solid feature: ${featureId}`)
		const previous = step.finishes
		step.finishes = [...(previous ?? []), finish]
		try {
			evaluateSolid(this.document)
		} catch (error) {
			step.finishes = previous
			throw error
		}
	}
	private addStep(step: SolidStep): void {
		const steps = requireValue(this.document.solidSteps)
		if (steps.some((s) => s.id === step.id)) throw new Error(`Duplicate solid feature: ${step.id}`)
		steps.push(step)
		try {
			evaluateSolid(this.document)
		} catch (error) {
			steps.pop()
			throw error
		}
	}
	extrude(id: string, spec: ExtrusionSpec): SketchTarget {
		if (!id.trim() || this.document.features.some((feature) => feature.id === id || feature.id === `${id}/sketch`)) throw new Error(`Duplicate or empty feature id: ${id}`)
		if (!Number.isFinite(spec.depth) || spec.depth <= 0) throw new Error("Extrusion depth must be positive.")
		const loops = [spec.outline, ...(spec.holes ?? [])]
		for (const loop of loops) {
			if (loop.length < 3 || loop.some((point) => !Number.isFinite(point.x) || !Number.isFinite(point.y))) throw new Error("An outline must contain at least three finite points.")
		}
		const sketchId = `${id}/sketch`
		const sketch = materializeSketch({
			type: "sketch",
			id: sketchId,
			name: spec.name ?? id,
			dirty: false,
			target: spec.on ?? { type: "plane", plane: "XY" },
			entities: loops.flatMap((loop, loopIndex) =>
				loop.map((point, index) => ({
					id: `${sketchId}/${loopIndex}/${index}`,
					type: "line" as const,
					p0: { ...point },
					p1: { ...requireValue(loop[(index + 1) % loop.length]) }
				}))
			),
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		})
		const profile = sketch.profiles.find((candidate) => candidate.holeLoopIds.length === (spec.holes?.length ?? 0))
		if (!profile) throw new Error("Outline and holes must form one closed profile.")
		const feature = { type: "extrude" as const, id, name: spec.name ?? id, target: { type: "profileRef" as const, sketchId, profileId: profile.id }, depth: spec.depth }
		const candidate = { features: [...this.document.features, sketch, feature] }
		const solid = extrudeSolidFeature(candidate, feature)
		const top = getExtrudedFaceDescriptors(solid).find((face) => face.label === "Top Face")
		if (!top) throw new Error("Extrusion did not produce a top face.")
		this.document.features.push(sketch, feature)
		try {
			this.addStep({ id, type: "extrusion", featureId: id, operation: spec.operation ?? "join", topScale: spec.topScale, translation: spec.translation })
		} catch (error) {
			this.document.features.splice(-2)
			throw error
		}
		return { type: "face", face: { type: "extrudeFace", extrudeId: id, faceId: top.faceId } }
	}
}
/** Assembly rotations are Euler XYZ angles in degrees; dimensions are millimetres. */
export class AssemblyBuilder {
	readonly data: Assembly
	constructor(id: string, name = id) {
		this.data = { id, name, instances: [], connectors: [], mates: [] }
	}
	instance(id: string, part: PartRef, transform?: Transform3D): InstanceRef {
		if (!id.trim() || this.data.instances.some((entry) => entry.id === id)) throw new Error(`Duplicate or empty instance id: ${id}`)
		transformMatrix(transform)
		this.data.instances.push({ id, partId: part.id, ...(transform ? { transform: structuredClone(transform) } : {}) })
		return { kind: "instance", id }
	}
	connector(id: string, instance: InstanceRef | null, position: Vector3D, rotation?: Vector3D): AssemblyMateReference {
		if (!id.trim() || requireValue(this.data.connectors).some((entry) => entry.id === id)) throw new Error(`Duplicate or empty connector id: ${id}`)
		if (instance && !this.data.instances.some((entry) => entry.id === instance.id)) throw new Error(`Unknown instance: ${instance.id}`)
		transformMatrix({ translation: position, rotation })
		const ref = { instanceId: instance?.id ?? null, connectorId: id }
		requireValue(this.data.connectors).push({ id, instanceId: ref.instanceId, position: { ...position }, ...(rotation ? { rotation: { ...rotation } } : {}) })
		return ref
	}
	/** Add a fixed constraint. The graph is solved again when saved or displayed. */
	fasten(id: string, parent: AssemblyMateReference, child: AssemblyMateReference): void {
		if (!id.trim() || requireValue(this.data.mates).some((m) => m.id === id)) throw new Error(`Duplicate or empty mate id: ${id}`)
		this.getConnector(parent)
		this.getConnector(child)
		if (parent.instanceId === child.instanceId) throw new Error("A fixed mate needs distinct instances.")
		const mate = { id, type: "fasten" as const, a: { ...parent }, b: { ...child } }
		requireValue(this.data.mates).push(mate)
		try {
			this.data.instances = solveFixedAssembly(this.data).instances
		} catch (error) {
			requireValue(this.data.mates).pop()
			throw error
		}
	}
	solve(): Assembly {
		return solveFixedAssembly(this.data)
	}
	private getConnector(ref: AssemblyMateReference): AssemblyConnector {
		const connector = requireValue(this.data.connectors).find((entry) => entry.id === ref.connectorId && entry.instanceId === ref.instanceId)
		if (!connector) throw new Error(`Unknown connector: ${ref.connectorId}`)
		return connector
	}
}
export class PuppyCad {
	private readonly client: PuppyCadClient
	readonly serverUrl: string
	constructor(
		options: {
			serverUrl?: string
			fetch?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>
		} = {}
	) {
		this.serverUrl = (options.serverUrl ?? "http://localhost:5337").replace(/\/$/, "")
		this.client = new PuppyCadClient({ apiBasePath: this.serverUrl, fetch: options.fetch })
	}
	async openProject(projectId: string): Promise<LiveProject> {
		const project = new LiveProject(projectId, this.client, this.serverUrl)
		await project.refresh()
		return project
	}
	async createProject(): Promise<LiveProject> {
		const response = await this.client.createProject(createProjectFile({ items: [], selectedPath: null }))
		const payload = (await response.json()) as {
			projectId?: string
			message?: string
		}
		if (!response.ok || !payload.projectId) throw new Error(payload.message ?? "Unable to create project.")
		return this.openProject(payload.projectId)
	}
}
export class LiveProject {
	private readonly project: PCadProject
	private queue: Promise<unknown> = Promise.resolve()
	readonly viewerUrl: string
	constructor(
		readonly id: string,
		client: PuppyCadClient,
		serverUrl: string
	) {
		this.project = new PCadProject({ projectId: id, clientId: `sdk-${crypto.randomUUID()}`, client })
		this.viewerUrl = `${serverUrl}/project/${encodeURIComponent(id)}`
	}
	refresh(): Promise<PCadProjectSyncResult> {
		return this.project.load()
	}
	get snapshot() {
		return this.project.getProject()
	}
	async part(id: string, spec: ExtrusionSpec | ((part: PartBuilder) => void), name = id): Promise<PartRef> {
		const builder = new PartBuilder()
		if (typeof spec === "function") spec(builder)
		else {
			builder.extrude("body", spec)
		}
		const document: ProjectPartDocument = { id, type: "part", name: typeof spec === "function" ? name : (spec.name ?? name), data: builder.document }
		await this.commands([{ type: "upsertDocument", document }])
		return { id, kind: "part" }
	}
	async assembly(id: string, build: (assembly: AssemblyBuilder) => void, name = id): Promise<void> {
		const builder = new AssemblyBuilder(id, name)
		build(builder)
		const document: ProjectAssemblyDocument = { id, type: "assembly", name, data: builder.solve() }
		await this.commands([{ type: "upsertDocument", document }])
	}
	async exportStl(part: PartRef): Promise<string> {
		await this.queue
		const { project } = await this.refresh()
		const find = (nodes: typeof project.items): ProjectPartDocument | undefined => {
			for (const node of nodes) {
				if ("kind" in node) {
					const found = find(node.items)
					if (found) return found
				} else if (node.type === "part" && node.id === part.id) return node
			}
			return undefined
		}
		const document = find(project.items)
		if (!document?.data) throw new Error(`Unknown part: ${part.id}`)
		return exportPartStl(document.data)
	}
	/** Apply a legacy defineModel/component graph as one undoable server operation. */
	async model(model: ModelDefinition): Promise<void> {
		const commands: SyncedProjectCommand[] = compileModel(model).project.items.map((document) => {
			if ("kind" in document || (document.type !== "part" && document.type !== "assembly")) throw new Error("Unsupported model document.")
			return { type: "upsertDocument", document }
		})
		await this.commands(commands)
	}
	/** One batch is one revision, disk save, undo step and viewer event. */
	commands(commands: readonly SyncedProjectCommand[]): Promise<PCadProjectSyncResult> {
		const captured = structuredClone(commands)
		const result = this.queue.then(() => this.project.postCommands(captured))
		this.queue = result.catch(() => undefined)
		return result
	}
}
