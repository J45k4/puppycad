import type { Point2D, Vector3D } from "./types"

export interface PCadState {
	readonly nodes: ReadonlyMap<string, PCadGraphNode>
	readonly rootNodeIds: readonly string[]
}

export type SerializedPCadState = {
	readonly nodes: readonly PCadGraphNode[]
	readonly rootNodeIds: readonly string[]
}

export type PartTreeState = {
	readonly orderedNodeIds: readonly string[]
	readonly dirtySketchIds?: readonly string[]
}

export type SketchPlane = "XY" | "YZ" | "XZ"
export type ReferencePlaneName = "Front" | "Top" | "Right"

export const REFERENCE_PLANE_TO_SKETCH_PLANE: Record<ReferencePlaneName, SketchPlane> = {
	Front: "XY",
	Top: "XZ",
	Right: "YZ"
}

export const SKETCH_PLANE_TO_REFERENCE_PLANE: Record<SketchPlane, ReferencePlaneName> = {
	XY: "Front",
	XZ: "Top",
	YZ: "Right"
}

export interface PCadNode {
	readonly id: string
	readonly type: string
	readonly name?: string
}

export interface ReferencePlaneNode extends PCadNode {
	readonly type: "referencePlane"
	readonly plane: SketchPlane
}

export type Line = {
	id: string
	type: "line"
	p0: Point2D
	p1: Point2D
}

export type CornerRectangle = {
	id: string
	type: "cornerRectangle"
	p0: Point2D
	p1: Point2D
}

export type SketchEntity = Line | CornerRectangle

export type SketchDimension =
	| {
			id: string
			type: "lineLength"
			entityId: string
			value: number
	  }
	| {
			id: string
			type: "rectangleWidth"
			entityId: string
			value: number
	  }
	| {
			id: string
			type: "rectangleHeight"
			entityId: string
			value: number
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

export type Loop = {
	id: string
	vertexIndices: number[]
}

export type Profile = {
	id: string
	outerLoopId: string
	holeLoopIds: string[]
}

export type SolidVertex = {
	id: string
	position: Vector3D
}

export type SolidEdge = {
	id: string
	vertexIds: string[]
}

export type SolidFace = {
	id: string
	edgeIds: string[]
}

export type Solid = {
	id: string
	featureId: string
	vertices: SolidVertex[]
	edges: SolidEdge[]
	faces: SolidFace[]
}

export type PCadSolid = {
	readonly id: string
	readonly sourceId: string
	readonly vertices: readonly SolidVertex[]
	readonly edges: readonly SolidEdge[]
	readonly faces: readonly SolidFace[]
}

export type ExtrudeFaceReference = {
	type: "extrudeFace"
	extrudeId: string
	faceId: string
}

export type FaceReference = ExtrudeFaceReference

export type ExtrudeEdgeReference = {
	type: "extrudeEdge"
	extrudeId: string
	edgeId: string
}

export type EdgeReference = ExtrudeEdgeReference

export type ProfileReference = {
	type: "profileRef"
	sketchId: string
	profileId: string
}

export type SolidExtrude = {
	type: "extrude"
	id: string
	name?: string
	target: ProfileReference
	depth: number
}

export type ChamferEdgeTarget = {
	edge: EdgeReference
}

export type SolidChamfer = {
	type: "chamfer"
	id: string
	name?: string
	target: ChamferEdgeTarget
	d1: number
	d2?: number
}

export type SketchTarget =
	| {
			type: "plane"
			plane: SketchPlane
	  }
	| {
			type: "face"
			face: FaceReference
	  }

export type Sketch = {
	type: "sketch"
	id: string
	name?: string
	dirty: boolean
	target: SketchTarget
	entities: SketchEntity[]
	dimensions: SketchDimension[]
	vertices: Point2D[]
	loops: Loop[]
	profiles: Profile[]
}

export type PartFeature = Sketch | SolidExtrude | SolidChamfer

export type PartDocument = {
	cad?: SerializedPCadState
	tree?: PartTreeState
	features: PartFeature[]
	solids?: Solid[]
	migrationWarnings?: string[]
}
