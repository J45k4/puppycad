import { materializeSketch } from "../cad/sketch"
import {
	REFERENCE_PLANE_TO_SKETCH_PLANE,
	SKETCH_PLANE_TO_REFERENCE_PLANE,
	type EdgeNode,
	type EdgeReference,
	type ExtrudeNode,
	type FaceNode,
	type FaceReference,
	type PartDocument,
	type PartFeature,
	type PartTreeState,
	type PCadGraphNode,
	type PCadState,
	type ReferencePlaneName,
	type ReferencePlaneNode,
	type SerializedPCadState,
	type Sketch,
	type SketchNode
} from "../schema"

export const PART_REFERENCE_PLANE_NODE_IDS: Record<ReferencePlaneName, string> = {
	Top: "plane-top",
	Front: "plane-front",
	Right: "plane-right"
}

const PART_REFERENCE_PLANES: ReferencePlaneNode[] = [
	{ id: PART_REFERENCE_PLANE_NODE_IDS.Top, type: "referencePlane", name: "Top", plane: REFERENCE_PLANE_TO_SKETCH_PLANE.Top },
	{ id: PART_REFERENCE_PLANE_NODE_IDS.Front, type: "referencePlane", name: "Front", plane: REFERENCE_PLANE_TO_SKETCH_PLANE.Front },
	{ id: PART_REFERENCE_PLANE_NODE_IDS.Right, type: "referencePlane", name: "Right", plane: REFERENCE_PLANE_TO_SKETCH_PLANE.Right }
]

export type PartRuntimeState = {
	cad: PCadState
	tree: Required<PartTreeState>
}

export function createDefaultPartRuntimeState(): PartRuntimeState {
	return {
		cad: createPartReferencePlaneState(),
		tree: {
			orderedNodeIds: [],
			dirtySketchIds: []
		}
	}
}

export function createPartRuntimeState(document?: PartDocument): PartRuntimeState {
	if (!document) {
		return createDefaultPartRuntimeState()
	}

	if (document.cad) {
		const cad = deserializePCadState(document.cad)
		const orderedNodeIds = normalizeOrderedNodeIds(document.tree?.orderedNodeIds, cad)
		const dirtySketchIds = normalizeDirtySketchIds(document.tree?.dirtySketchIds, cad)
		return {
			cad,
			tree: {
				orderedNodeIds,
				dirtySketchIds
			}
		}
	}

	return createPartRuntimeStateFromFeatures(document.features)
}

export function createPartRuntimeStateFromFeatures(features: readonly PartFeature[]): PartRuntimeState {
	const cad = createPartReferencePlaneState()
	const nodes = new Map(cad.nodes)
	const rootNodeIds = new Set(cad.rootNodeIds)
	const orderedNodeIds: string[] = []
	const dirtySketchIds: string[] = []

	for (const feature of features) {
		if (feature.type === "sketch") {
			const targetId = resolveSketchTargetNodeId(feature, nodes)
			if (!targetId) {
				continue
			}
			const sketchNode: SketchNode = {
				id: feature.id,
				type: "sketch",
				name: feature.name,
				targetId,
				entities: feature.entities.map((entity) => ({ ...entity })),
				dimensions: feature.dimensions.map((dimension) => ({ ...dimension }))
			}
			nodes.set(sketchNode.id, sketchNode)
			orderedNodeIds.push(sketchNode.id)
			if (feature.dirty) {
				dirtySketchIds.push(sketchNode.id)
			}
			continue
		}

		if (feature.type === "extrude") {
			const extrudeNode: ExtrudeNode = {
				id: feature.id,
				type: "extrude",
				name: feature.name,
				sketchId: feature.target.sketchId,
				profileId: feature.target.profileId,
				operation: "newBody",
				depth: feature.depth
			}
			nodes.set(extrudeNode.id, extrudeNode)
			rootNodeIds.add(extrudeNode.id)
			orderedNodeIds.push(extrudeNode.id)
			continue
		}

		if (feature.type === "chamfer") {
			const edgeId = ensureEdgeReferenceNode(feature.target.edge, nodes)
			const chamferNode = {
				id: feature.id,
				type: "chamfer" as const,
				name: feature.name,
				edgeId,
				d1: feature.d1,
				...(feature.d2 === undefined ? {} : { d2: feature.d2 })
			}
			nodes.set(chamferNode.id, chamferNode)
			rootNodeIds.add(chamferNode.id)
			orderedNodeIds.push(chamferNode.id)
		}
	}

	return {
		cad: {
			nodes,
			rootNodeIds: [...rootNodeIds]
		},
		tree: {
			orderedNodeIds,
			dirtySketchIds
		}
	}
}

export function serializePCadState(state: PCadState): SerializedPCadState {
	return {
		nodes: [...state.nodes.values()].map(clonePCadNode),
		rootNodeIds: [...state.rootNodeIds]
	}
}

export function materializePartFeatures(state: PCadState, tree: Required<PartTreeState>): PartFeature[] {
	const features: PartFeature[] = []
	const dirtySketchIds = new Set(tree.dirtySketchIds)
	const orderedNodeIds = normalizeOrderedNodeIds(tree.orderedNodeIds, state)

	for (const nodeId of orderedNodeIds) {
		const node = state.nodes.get(nodeId)
		if (!node) {
			continue
		}
		if (node.type === "sketch") {
			const sketch = materializeSketchNode(state, node, dirtySketchIds.has(node.id))
			if (sketch) {
				features.push(sketch)
			}
			continue
		}
		if (node.type === "extrude") {
			features.push({
				type: "extrude",
				id: node.id,
				name: node.name,
				target: {
					type: "profileRef",
					sketchId: node.sketchId,
					profileId: node.profileId
				},
				depth: node.depth
			})
			continue
		}
		if (node.type === "chamfer") {
			const edge = state.nodes.get(node.edgeId)
			if (!edge || edge.type !== "edge") {
				continue
			}
			features.push({
				type: "chamfer",
				id: node.id,
				name: node.name,
				target: {
					edge: {
						type: "extrudeEdge",
						extrudeId: edge.sourceId,
						edgeId: edge.edgeId
					}
				},
				d1: node.d1,
				...(node.d2 === undefined ? {} : { d2: node.d2 })
			})
		}
	}

	return features
}

export function createFaceNodeFromReference(reference: FaceReference): FaceNode {
	return {
		id: getFaceNodeId(reference),
		type: "face",
		sourceId: reference.extrudeId,
		faceId: reference.faceId
	}
}

export function createEdgeNodeFromReference(reference: EdgeReference): EdgeNode {
	return {
		id: getEdgeNodeId(reference),
		type: "edge",
		sourceId: reference.extrudeId,
		edgeId: reference.edgeId
	}
}

export function getReferencePlaneNodeId(plane: ReferencePlaneName): string {
	return PART_REFERENCE_PLANE_NODE_IDS[plane]
}

function createPartReferencePlaneState(): PCadState {
	return {
		nodes: new Map(PART_REFERENCE_PLANES.map((plane) => [plane.id, plane])),
		rootNodeIds: PART_REFERENCE_PLANES.map((plane) => plane.id)
	}
}

function deserializePCadState(serialized: SerializedPCadState): PCadState {
	const base = createPartReferencePlaneState()
	const nodes = new Map(base.nodes)
	for (const node of serialized.nodes) {
		nodes.set(node.id, clonePCadNode(node))
	}
	const rootNodeIds = serialized.rootNodeIds.filter((id) => nodes.has(id))
	const referencePlaneIds = PART_REFERENCE_PLANES.map((plane) => plane.id)
	return {
		nodes,
		rootNodeIds: [...referencePlaneIds, ...rootNodeIds.filter((id) => !referencePlaneIds.includes(id))]
	}
}

function normalizeOrderedNodeIds(input: readonly string[] | undefined, state: PCadState): string[] {
	const authoredNodes = [...state.nodes.values()].filter((node) => node.type === "sketch" || node.type === "extrude" || node.type === "chamfer")
	const fallback = authoredNodes.map((node) => node.id)
	const candidates = input && input.length > 0 ? input : fallback
	const seen = new Set<string>()
	const ordered = candidates.filter((id) => {
		const node = state.nodes.get(id)
		if (!node || seen.has(id) || (node.type !== "sketch" && node.type !== "extrude" && node.type !== "chamfer")) {
			return false
		}
		seen.add(id)
		return true
	})
	for (const id of fallback) {
		if (!seen.has(id)) {
			ordered.push(id)
		}
	}
	return ordered
}

function normalizeDirtySketchIds(input: readonly string[] | undefined, state: PCadState): string[] {
	return (input ?? []).filter((id, index, values) => values.indexOf(id) === index && state.nodes.get(id)?.type === "sketch")
}

function materializeSketchNode(state: PCadState, node: SketchNode, dirty: boolean): Sketch | null {
	const target = state.nodes.get(node.targetId)
	if (!target) {
		return null
	}
	const sketchTarget =
		target.type === "referencePlane"
			? {
					type: "plane" as const,
					plane: target.plane
				}
			: target.type === "face"
				? {
						type: "face" as const,
						face: {
							type: "extrudeFace" as const,
							extrudeId: target.sourceId,
							faceId: target.faceId
						}
					}
				: null
	if (!sketchTarget) {
		return null
	}
	return materializeSketch({
		type: "sketch",
		id: node.id,
		name: node.name,
		dirty,
		target: sketchTarget,
		entities: node.entities.map((entity) => ({ ...entity })),
		dimensions: node.dimensions.map((dimension) => ({ ...dimension })),
		vertices: [],
		loops: [],
		profiles: []
	})
}

function resolveSketchTargetNodeId(sketch: Sketch, nodes: Map<string, PCadGraphNode>): string | null {
	if (sketch.target.type === "plane") {
		return PART_REFERENCE_PLANE_NODE_IDS[SKETCH_PLANE_TO_REFERENCE_PLANE[sketch.target.plane]]
	}
	const faceNode = createFaceNodeFromReference(sketch.target.face)
	nodes.set(faceNode.id, faceNode)
	return faceNode.id
}

function ensureEdgeReferenceNode(reference: EdgeReference, nodes: Map<string, PCadGraphNode>): string {
	const edgeNode = createEdgeNodeFromReference(reference)
	if (!nodes.has(edgeNode.id)) {
		nodes.set(edgeNode.id, edgeNode)
	}
	return edgeNode.id
}

function getFaceNodeId(reference: FaceReference): string {
	return `face-${reference.extrudeId}-${reference.faceId}`
}

function getEdgeNodeId(reference: EdgeReference): string {
	return `edge-${reference.extrudeId}-${reference.edgeId}`
}

function clonePCadNode<T extends PCadGraphNode>(node: T): T {
	return structuredClone(node) as T
}
