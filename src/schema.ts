import type { Point2D, Vector3D } from "./types"

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
	features: PartFeature[]
	solids?: Solid[]
	migrationWarnings?: string[]
}
