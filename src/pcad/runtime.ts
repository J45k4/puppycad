import { EXTRUDE_OPERATIONS } from "../schema"
import { getSketchEntityNodes, sketchEntityNodeToEntity, sketchEntityToNode } from "./sketch-entities"
import type { ChamferNode, EdgeNode, FaceNode, ExtrudeNode, ExtrudeOperation, PCadGraphNode, PCadGraphRewrite, PCadState, SketchDimension, SketchEntity, SketchEntityNode, SketchNode } from "../schema"

export { EXTRUDE_OPERATIONS } from "../schema"
export type {
	ChamferNode,
	CornerRectangle,
	EdgeNode,
	ExtrudeNode,
	ExtrudeOperation,
	FaceNode,
	Line,
	PCadGraphNode,
	PCadGraphRewrite,
	PCadNode,
	PCadSolid as Solid,
	PCadState,
	ReferencePlaneName,
	ReferencePlaneNode,
	SketchDimension,
	SketchEntity,
	SketchEntityNode,
	SketchNode,
	SketchPlane,
	SolidEdge,
	SolidFace,
	SolidVertex
} from "../schema"

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
		case "sketchLine":
		case "sketchCornerRectangle":
			return [node.sketchId]
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

	public addSketch(args: { id: string; name?: string; targetId: string }): SketchNode {
		this.assertUnusedId(args.id)
		requireNodeType(this.present, args.targetId, ["referencePlane", "face"], `Sketch "${args.id}" target`)
		const sketch: SketchNode = {
			id: args.id,
			type: "sketch",
			name: args.name,
			targetId: args.targetId,
			dimensions: []
		}
		this.commit({ type: "addNode", node: sketch })
		return sketch
	}

	public addFace(node: FaceNode): FaceNode {
		const existing = this.present.nodes.get(node.id)
		if (existing) {
			if (existing.type !== "face") {
				throw new Error(`Node "${node.id}" already exists.`)
			}
			return existing
		}
		requireNodeType(this.present, node.sourceId, ["extrude", "chamfer"], `Face "${node.id}" source`)
		this.commit({ type: "addNode", node })
		return node
	}

	public addEdge(node: EdgeNode): EdgeNode {
		const existing = this.present.nodes.get(node.id)
		if (existing) {
			if (existing.type !== "edge") {
				throw new Error(`Node "${node.id}" already exists.`)
			}
			return existing
		}
		requireNodeType(this.present, node.sourceId, ["extrude", "chamfer"], `Edge "${node.id}" source`)
		this.commit({ type: "addNode", node })
		return node
	}

	public renameNode(nodeId: string, name: string): PCadGraphNode {
		const node = this.getNodeOrThrow(nodeId)
		const trimmedName = name.trim()
		if (!trimmedName) {
			throw new Error("Node name must be provided.")
		}
		const nextNode = {
			...node,
			name: trimmedName
		} as PCadGraphNode
		this.commit({ type: "replaceNode", node: nextNode })
		return nextNode
	}

	public addSketchEntity(sketchId: string, entity: SketchEntity): SketchEntityNode {
		this.getSketchOrThrow(sketchId)
		this.assertUnusedId(entity.id)
		const node = sketchEntityToNode(sketchId, entity)
		this.commit({ type: "addNode", node })
		return node
	}

	public removeLastSketchEntity(sketchId: string): SketchNode {
		const sketch = this.getSketchOrThrow(sketchId)
		const entityNodes = getSketchEntityNodes(this.present, sketch.id)
		const removedEntity = entityNodes.at(-1)
		if (!removedEntity) {
			return sketch
		}
		const nextSketch: SketchNode = {
			...sketch,
			dimensions: removedEntity ? sketch.dimensions.filter((dimension) => dimension.entityId !== removedEntity.id) : sketch.dimensions
		}
		this.commitMany([
			{ type: "removeNodes", nodeIds: [removedEntity.id] },
			{ type: "replaceNode", node: nextSketch }
		])
		return nextSketch
	}

	public clearSketch(sketchId: string): SketchNode {
		const sketch = this.getSketchOrThrow(sketchId)
		const entityNodeIds = getSketchEntityNodes(this.present, sketch.id).map((node) => node.id)
		const nextSketch: SketchNode = {
			...sketch,
			dimensions: []
		}
		this.commitMany([
			{ type: "removeNodes", nodeIds: entityNodeIds },
			{ type: "replaceNode", node: nextSketch }
		])
		return nextSketch
	}

	public setSketchDimension(sketchId: string, dimension: SketchDimension): SketchNode {
		const sketch = this.getSketchOrThrow(sketchId)
		const entityNode = this.getSketchEntityOrThrow(sketch.id, dimension.entityId)
		const entity = sketchEntityNodeToEntity(entityNode)
		if (!entity) {
			throw new Error(`Sketch entity "${dimension.entityId}" does not exist.`)
		}
		if (!Number.isFinite(dimension.value) || dimension.value <= 0) {
			throw new Error("Sketch dimension value must be greater than zero.")
		}

		const dimensions = sketch.dimensions.filter((item) => !(item.entityId === dimension.entityId && item.type === dimension.type))
		if (!entity || !canApplyDimensionToEntity(entity, dimension)) {
			throw new Error(`Sketch dimension "${dimension.type}" cannot be applied to entity "${dimension.entityId}".`)
		}
		const nextEntityNode = sketchEntityToNode(sketch.id, applyDimensionToEntity(entity, dimension))
		const nextSketch: SketchNode = {
			...sketch,
			dimensions: [...dimensions, dimension]
		}
		this.commitMany([
			{ type: "replaceNode", node: nextEntityNode },
			{ type: "replaceNode", node: nextSketch }
		])
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

	private getSketchEntityOrThrow(sketchId: string, entityId: string): SketchEntityNode {
		const node = this.present.nodes.get(entityId)
		if (!node || (node.type !== "sketchLine" && node.type !== "sketchCornerRectangle") || node.sketchId !== sketchId) {
			throw new Error(`Sketch entity "${entityId}" does not exist.`)
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

function canApplyDimensionToEntity(entity: SketchEntity, dimension: SketchDimension): boolean {
	if (entity.type === "line") {
		return dimension.type === "lineLength"
	}
	return dimension.type === "rectangleWidth" || dimension.type === "rectangleHeight"
}

function applyDimensionToEntity(entity: SketchEntity, dimension: SketchDimension): SketchEntity {
	if (entity.type === "line" && dimension.type === "lineLength") {
		const dx = entity.p1.x - entity.p0.x
		const dy = entity.p1.y - entity.p0.y
		const length = Math.hypot(dx, dy)
		if (length <= 1e-9) {
			return {
				...entity,
				p1: {
					x: entity.p0.x + dimension.value,
					y: entity.p0.y
				}
			}
		}
		const scale = dimension.value / length
		return {
			...entity,
			p1: {
				x: entity.p0.x + dx * scale,
				y: entity.p0.y + dy * scale
			}
		}
	}

	if (entity.type === "cornerRectangle" && dimension.type === "rectangleWidth") {
		const sign = entity.p1.x === entity.p0.x ? 1 : Math.sign(entity.p1.x - entity.p0.x)
		return {
			...entity,
			p1: {
				x: entity.p0.x + sign * dimension.value,
				y: entity.p1.y
			}
		}
	}

	if (entity.type === "cornerRectangle" && dimension.type === "rectangleHeight") {
		const sign = entity.p1.y === entity.p0.y ? 1 : Math.sign(entity.p1.y - entity.p0.y)
		return {
			...entity,
			p1: {
				x: entity.p1.x,
				y: entity.p0.y + sign * dimension.value
			}
		}
	}

	return entity
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
			validateSketchDimensions(state, node)
			return
		case "sketchLine":
			requireNodeType(state, node.sketchId, ["sketch"], `Sketch line "${node.id}" sketch`)
			validatePoint2D(node.p0, `Sketch line "${node.id}" p0`)
			validatePoint2D(node.p1, `Sketch line "${node.id}" p1`)
			return
		case "sketchCornerRectangle":
			requireNodeType(state, node.sketchId, ["sketch"], `Sketch rectangle "${node.id}" sketch`)
			validatePoint2D(node.p0, `Sketch rectangle "${node.id}" p0`)
			validatePoint2D(node.p1, `Sketch rectangle "${node.id}" p1`)
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

function validateSketchDimensions(state: PCadState, sketch: SketchNode): void {
	for (const dimension of sketch.dimensions) {
		const entityNode = getSketchEntityNodes(state, sketch.id).find((entity) => entity.id === dimension.entityId)
		if (!entityNode) {
			throw new Error(`Sketch "${sketch.id}" dimension "${dimension.id}" references missing entity "${dimension.entityId}".`)
		}
		if (!Number.isFinite(dimension.value) || dimension.value <= 0) {
			throw new Error(`Sketch "${sketch.id}" dimension "${dimension.id}" value must be greater than zero.`)
		}
		if (!canApplyDimensionToEntity(sketchEntityNodeToEntity(entityNode), dimension)) {
			throw new Error(`Sketch "${sketch.id}" dimension "${dimension.id}" cannot be applied to entity "${dimension.entityId}".`)
		}
	}
}

function validatePoint2D(point: { x: number; y: number }, label: string): void {
	if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) {
		throw new Error(`${label} must contain finite coordinates.`)
	}
}

function formatExpectedTypes(types: readonly PCadGraphNode["type"][]): string {
	return types.length === 1 ? `a ${types[0]} node` : `one of: ${types.join(", ")}`
}
