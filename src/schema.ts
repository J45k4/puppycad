import type { Point2D } from "./types"

export type Line = {
	id: string
	type: "line"
	p0: Point2D
	p1: Point2D
}

export type CornerRectangle = {
	id: string
	p0: Point2D
	p1: Point2D
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

export type DepthExtrude = {
	depth: number
}

export type LoopExtrudeTarget = {
	type: "loopExtrudeTarget"
	
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

export type SketchTarget =
	| {
			type: "plane"
			plane: "XY" | "YZ" | "XZ"
	  }
	| {
			type: "face"
			faceId: string
	  }

export type Sketch = {
	type: "sketch"
	id: string
	name?: string
	dirty: boolean
	target: SketchTarget
	entities: (Line | CornerRectangle)[]
	vertices: Point2D[]
	loops: Loop[]
	profiles: Profile[]
}