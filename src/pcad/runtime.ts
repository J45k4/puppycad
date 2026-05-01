import type { Point2D, Vector3D } from "../types"

export interface PCadState {
	readonly nodes: ReadonlyMap<string, PCadGraphNode>
	readonly rootNodeIds: readonly string[]
}

export type SketchPlane = "XY" | "YZ" | "XZ"
export type ReferencePlaneName = "Front" | "Top" | "Right"

export interface PCadNode {
	readonly id: string
	readonly type: string
	readonly name?: string
}

export interface ReferencePlaneNode extends PCadNode {
	readonly type: "referencePlane"
	readonly plane: SketchPlane
}

export interface Line {
	readonly id: string
	readonly type: "line"
	readonly p0: Point2D
	readonly p1: Point2D
}

export interface CornerRectangle {
	readonly id: string
	readonly type: "cornerRectangle"
	readonly p0: Point2D
	readonly p1: Point2D
}

export type SketchEntity = Line | CornerRectangle

export type SketchDimension =
	| {
			readonly id: string
			readonly type: "lineLength"
			readonly entityId: string
			readonly value: number
	  }
	| {
			readonly id: string
			readonly type: "rectangleWidth"
			readonly entityId: string
			readonly value: number
	  }
	| {
			readonly id: string
			readonly type: "rectangleHeight"
			readonly entityId: string
			readonly value: number
	  }

export interface SketchNode extends PCadNode {
	readonly type: "sketch"
	readonly targetId: string
	readonly entities: readonly SketchEntity[]
	readonly dimensions: readonly SketchDimension[]
}

export const EXTRUDE_OPERATIONS = ["newBody", "join", "cut"] as const
export type ExtrudeOperation = (typeof EXTRUDE_OPERATIONS)[number]

export interface ExtrudeNode extends PCadNode {
	readonly type: "extrude"
	readonly sketchId: string
	readonly profileId: string
	readonly operation: ExtrudeOperation
	readonly depth: number
}

export interface FaceNode extends PCadNode {
	readonly type: "face"
	readonly sourceId: string
	readonly faceId: string
}

export interface EdgeNode extends PCadNode {
	readonly type: "edge"
	readonly sourceId: string
	readonly edgeId: string
}

export interface ChamferNode extends PCadNode {
	readonly type: "chamfer"
	readonly edgeId: string
	readonly d1: number
	readonly d2?: number
}

export type PCadGraphNode = ReferencePlaneNode | SketchNode | ExtrudeNode | FaceNode | EdgeNode | ChamferNode

export interface SolidVertex {
	readonly id: string
	readonly position: Vector3D
}

export interface SolidEdge {
	readonly id: string
	readonly vertexIds: readonly string[]
}

export interface SolidFace {
	readonly id: string
	readonly edgeIds: readonly string[]
}

export interface Solid {
	readonly id: string
	readonly sourceId: string
	readonly vertices: readonly SolidVertex[]
	readonly edges: readonly SolidEdge[]
	readonly faces: readonly SolidFace[]
}

export type PCadGraphRewrite =
	| {
			readonly type: "addNode"
			readonly node: PCadGraphNode
			readonly root?: boolean
	  }
	| {
			readonly type: "replaceNode"
			readonly node: PCadGraphNode
	  }
	| {
			readonly type: "removeNodes"
			readonly nodeIds: readonly string[]
	  }
	| {
			readonly type: "setRootNodes"
			readonly rootNodeIds: readonly string[]
	  }

export function createEmptyPCadState(): PCadState {
	return {
		nodes: new Map(),
		rootNodeIds: []
	}
}

export function isExtrudeOperation(value: unknown): value is ExtrudeOperation {
	return typeof value === "string" && EXTRUDE_OPERATIONS.includes(value as ExtrudeOperation)
}

export function applyGraphRewrite(state: PCadState, rewrite: PCadGraphRewrite): PCadState {
	switch (rewrite.type) {
		case "addNode": {
			const nodes = new Map(state.nodes).set(rewrite.node.id, rewrite.node)
			const rootNodeIds = rewrite.root && !state.rootNodeIds.includes(rewrite.node.id) ? [...state.rootNodeIds, rewrite.node.id] : state.rootNodeIds
			return { nodes, rootNodeIds }
		}
		case "replaceNode":
			if (!state.nodes.has(rewrite.node.id)) {
				return state
			}
			return {
				...state,
				nodes: new Map(state.nodes).set(rewrite.node.id, rewrite.node)
			}
		case "removeNodes": {
			const removedIds = new Set(rewrite.nodeIds)
			const nodes = new Map(state.nodes)
			for (const id of removedIds) {
				nodes.delete(id)
			}
			return {
				nodes,
				rootNodeIds: state.rootNodeIds.filter((id) => !removedIds.has(id))
			}
		}
		case "setRootNodes":
			return {
				...state,
				rootNodeIds: [...rewrite.rootNodeIds]
			}
	}
}

export function applyGraphRewrites(state: PCadState, rewrites: readonly PCadGraphRewrite[]): PCadState {
	return rewrites.reduce((nextState, rewrite) => applyGraphRewrite(nextState, rewrite), state)
}

export function getNodeDependencies(node: PCadGraphNode): readonly string[] {
	switch (node.type) {
		case "referencePlane":
			return []
		case "sketch":
			return [node.targetId]
		case "extrude":
			return [node.sketchId]
		case "face":
		case "edge":
			return [node.sourceId]
		case "chamfer":
			return [node.edgeId]
	}
}

export function collectDependentNodeIds(state: PCadState, nodeIds: readonly string[]): Set<string> {
	const reverseDependencies = new Map<string, string[]>()
	for (const [id, node] of state.nodes) {
		for (const dependencyId of getNodeDependencies(node)) {
			reverseDependencies.set(dependencyId, [...(reverseDependencies.get(dependencyId) ?? []), id])
		}
	}

	const collectedIds = new Set(nodeIds)
	const queue = [...nodeIds]
	for (let index = 0; index < queue.length; index += 1) {
		const id = queue[index]
		if (!id) {
			continue
		}
		for (const dependentId of reverseDependencies.get(id) ?? []) {
			if (!collectedIds.has(dependentId)) {
				collectedIds.add(dependentId)
				queue.push(dependentId)
			}
		}
	}

	return collectedIds
}

export function validatePCadState(state: PCadState): void {
	for (const rootNodeId of state.rootNodeIds) {
		if (!state.nodes.has(rootNodeId)) {
			throw new Error(`Root node "${rootNodeId}" does not exist.`)
		}
	}

	for (const [id, node] of state.nodes) {
		if (!id || node.id !== id) {
			throw new Error(`Node "${node.id}" is stored under invalid id "${id}".`)
		}
		validateNode(state, node)
	}
}

export class CadEditor {
	private past: PCadState[] = []
	private present: PCadState
	private future: PCadState[] = []

	public constructor(initialState = createEmptyPCadState()) {
		validatePCadState(initialState)
		this.present = initialState
	}

	public getState(): PCadState {
		return this.present
	}

	public undo(): void {
		const previous = this.past.at(-1)
		if (!previous) {
			return
		}
		this.future = [this.present, ...this.future]
		this.present = previous
		this.past = this.past.slice(0, -1)
	}

	public redo(): void {
		const [next, ...rest] = this.future
		if (!next) {
			return
		}
		this.past = [...this.past, this.present]
		this.present = next
		this.future = rest
	}

	public addSketchEntity(sketchId: string, entity: SketchEntity): SketchNode {
		const sketch = this.getSketchOrThrow(sketchId)
		const nextSketch: SketchNode = {
			...sketch,
			entities: [...sketch.entities, entity]
		}
		this.commit({ type: "replaceNode", node: nextSketch })
		return nextSketch
	}

	public clearSketch(sketchId: string): SketchNode {
		const sketch = this.getSketchOrThrow(sketchId)
		const nextSketch: SketchNode = {
			...sketch,
			entities: [],
			dimensions: []
		}
		this.commit({ type: "replaceNode", node: nextSketch })
		return nextSketch
	}

	public setSketchDimension(sketchId: string, dimension: SketchDimension): SketchNode {
		const sketch = this.getSketchOrThrow(sketchId)
		if (!sketch.entities.some((entity) => entity.id === dimension.entityId)) {
			throw new Error(`Sketch entity "${dimension.entityId}" does not exist.`)
		}
		if (!Number.isFinite(dimension.value) || dimension.value <= 0) {
			throw new Error("Sketch dimension value must be greater than zero.")
		}

		const dimensions = sketch.dimensions.filter((item) => !(item.entityId === dimension.entityId && item.type === dimension.type))
		const nextSketch: SketchNode = {
			...sketch,
			dimensions: [...dimensions, dimension]
		}
		this.commit({ type: "replaceNode", node: nextSketch })
		return nextSketch
	}

	public extrudeSketchProfile(args: {
		id: string
		name?: string
		sketchId: string
		profileId: string
		operation: ExtrudeOperation
		depth: number
	}): ExtrudeNode {
		const sketch = this.getSketchOrThrow(args.sketchId)
		this.assertUnusedId(args.id)
		if (!args.profileId) {
			throw new Error("Extrude profile id must be provided.")
		}
		if (!Number.isFinite(args.depth) || args.depth <= 0) {
			throw new Error("Extrude depth must be greater than zero.")
		}
		if (!isExtrudeOperation(args.operation)) {
			throw new Error("Extrude operation must be newBody, join, or cut.")
		}

		const extrude: ExtrudeNode = {
			id: args.id,
			type: "extrude",
			name: args.name,
			sketchId: sketch.id,
			profileId: args.profileId,
			operation: args.operation,
			depth: args.depth
		}

		this.commit({ type: "addNode", node: extrude, root: true })
		return extrude
	}

	public setExtrudeDepth(extrudeId: string, depth: number): ExtrudeNode {
		const extrude = this.getExtrudeOrThrow(extrudeId)
		if (!Number.isFinite(depth) || depth <= 0) {
			throw new Error("Extrude depth must be greater than zero.")
		}

		const nextExtrude: ExtrudeNode = {
			...extrude,
			depth
		}
		this.commit({ type: "replaceNode", node: nextExtrude })
		return nextExtrude
	}

	public chamferEdge(args: { id: string; name?: string; edgeId: string; d1: number; d2?: number }): ChamferNode {
		const edge = this.getEdgeOrThrow(args.edgeId)
		this.assertUnusedId(args.id)
		if (!Number.isFinite(args.d1) || args.d1 <= 0) {
			throw new Error("Chamfer distance must be greater than zero.")
		}
		if (args.d2 !== undefined && (!Number.isFinite(args.d2) || args.d2 <= 0)) {
			throw new Error("Chamfer second distance must be greater than zero.")
		}

		const chamfer: ChamferNode = {
			id: args.id,
			type: "chamfer",
			name: args.name,
			edgeId: edge.id,
			d1: args.d1,
			...(args.d2 === undefined ? {} : { d2: args.d2 })
		}

		this.commit({ type: "addNode", node: chamfer, root: true })
		return chamfer
	}

	public setChamferDistance(chamferId: string, d1: number, d2?: number): ChamferNode {
		const chamfer = this.getChamferOrThrow(chamferId)
		if (!Number.isFinite(d1) || d1 <= 0) {
			throw new Error("Chamfer distance must be greater than zero.")
		}
		if (d2 !== undefined && (!Number.isFinite(d2) || d2 <= 0)) {
			throw new Error("Chamfer second distance must be greater than zero.")
		}

		const nextChamfer: ChamferNode = {
			...chamfer,
			d1,
			...(d2 === undefined ? {} : { d2 })
		}
		this.commit({ type: "replaceNode", node: nextChamfer })
		return nextChamfer
	}

	public deleteNodeCascade(nodeId: string): void {
		this.getNodeOrThrow(nodeId)
		const deletedIds = collectDependentNodeIds(this.present, [nodeId])
		this.commit({ type: "removeNodes", nodeIds: [...deletedIds] })
	}

	private commit(rewrite: PCadGraphRewrite): void {
		this.commitMany([rewrite])
	}

	private commitMany(rewrites: readonly PCadGraphRewrite[]): void {
		const next = applyGraphRewrites(this.present, rewrites)
		if (next === this.present) {
			return
		}
		validatePCadState(next)
		this.past = [...this.past, this.present]
		this.present = next
		this.future = []
	}

	private getNodeOrThrow(id: string): PCadGraphNode {
		const node = this.present.nodes.get(id)
		if (!node) {
			throw new Error(`Node "${id}" does not exist.`)
		}
		return node
	}

	private getSketchOrThrow(id: string): SketchNode {
		const node = this.present.nodes.get(id)
		if (!node || node.type !== "sketch") {
			throw new Error(`Sketch "${id}" does not exist.`)
		}
		return node
	}

	private getExtrudeOrThrow(id: string): ExtrudeNode {
		const node = this.present.nodes.get(id)
		if (!node || node.type !== "extrude") {
			throw new Error(`Extrude "${id}" does not exist.`)
		}
		return node
	}

	private getEdgeOrThrow(id: string): EdgeNode {
		const node = this.present.nodes.get(id)
		if (!node || node.type !== "edge") {
			throw new Error(`Edge "${id}" does not exist.`)
		}
		return node
	}

	private getChamferOrThrow(id: string): ChamferNode {
		const node = this.present.nodes.get(id)
		if (!node || node.type !== "chamfer") {
			throw new Error(`Chamfer "${id}" does not exist.`)
		}
		return node
	}

	private assertUnusedId(id: string): void {
		if (!id) {
			throw new Error("Node id must be provided.")
		}
		if (this.present.nodes.has(id)) {
			throw new Error(`Node "${id}" already exists.`)
		}
	}
}

function validateNode(state: PCadState, node: PCadGraphNode): void {
	switch (node.type) {
		case "referencePlane":
			if (!["XY", "YZ", "XZ"].includes(node.plane)) {
				throw new Error(`Reference plane "${node.id}" uses invalid sketch plane.`)
			}
			return
		case "sketch":
			requireNodeType(state, node.targetId, ["referencePlane", "face"], `Sketch "${node.id}" target`)
			validateSketchDimensions(node)
			return
		case "extrude":
			requireNodeType(state, node.sketchId, ["sketch"], `Extrude "${node.id}" sketch`)
			if (!node.profileId) {
				throw new Error(`Extrude "${node.id}" profile id must be provided.`)
			}
			if (!isExtrudeOperation(node.operation)) {
				throw new Error(`Extrude "${node.id}" operation must be newBody, join, or cut.`)
			}
			if (!Number.isFinite(node.depth) || node.depth <= 0) {
				throw new Error(`Extrude "${node.id}" depth must be greater than zero.`)
			}
			return
		case "face":
			requireNodeType(state, node.sourceId, ["extrude", "chamfer"], `Face "${node.id}" source`)
			if (!node.faceId) {
				throw new Error(`Face "${node.id}" face id must be provided.`)
			}
			return
		case "edge":
			requireNodeType(state, node.sourceId, ["extrude", "chamfer"], `Edge "${node.id}" source`)
			if (!node.edgeId) {
				throw new Error(`Edge "${node.id}" edge id must be provided.`)
			}
			return
		case "chamfer":
			requireNodeType(state, node.edgeId, ["edge"], `Chamfer "${node.id}" edge`)
			if (!Number.isFinite(node.d1) || node.d1 <= 0) {
				throw new Error(`Chamfer "${node.id}" distance must be greater than zero.`)
			}
			if (node.d2 !== undefined && (!Number.isFinite(node.d2) || node.d2 <= 0)) {
				throw new Error(`Chamfer "${node.id}" second distance must be greater than zero.`)
			}
			return
	}
}

function requireNodeType(state: PCadState, id: string, expectedTypes: readonly PCadGraphNode["type"][], label: string): PCadGraphNode {
	const node = state.nodes.get(id)
	if (!node) {
		throw new Error(`${label} "${id}" does not exist.`)
	}
	if (!expectedTypes.includes(node.type)) {
		throw new Error(`${label} "${id}" must reference ${formatExpectedTypes(expectedTypes)}.`)
	}
	return node
}

function validateSketchDimensions(sketch: SketchNode): void {
	for (const dimension of sketch.dimensions) {
		if (!sketch.entities.some((entity) => entity.id === dimension.entityId)) {
			throw new Error(`Sketch "${sketch.id}" dimension "${dimension.id}" references missing entity "${dimension.entityId}".`)
		}
		if (!Number.isFinite(dimension.value) || dimension.value <= 0) {
			throw new Error(`Sketch "${sketch.id}" dimension "${dimension.id}" value must be greater than zero.`)
		}
	}
}

function formatExpectedTypes(types: readonly PCadGraphNode["type"][]): string {
	return types.length === 1 ? `a ${types[0]} node` : `one of: ${types.join(", ")}`
}
