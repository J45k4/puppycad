import type { Point2D } from "./types"

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

export type Loop = {
	id: string
	vertexIndices: number[]
}

export type Profile = {
	id: string
	outerLoopId: string
	holeLoopIds: string[]
}

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

export type SketchTarget = {
	type: "plane"
	plane: SketchPlane
}

export type Sketch = {
	type: "sketch"
	id: string
	name?: string
	dirty: boolean
	target: SketchTarget
	entities: SketchEntity[]
	vertices: Point2D[]
	loops: Loop[]
	profiles: Profile[]
}

export type PartFeature = Sketch | SolidExtrude

export type PartDocument = {
	features: PartFeature[]
	migrationWarnings?: string[]
}
