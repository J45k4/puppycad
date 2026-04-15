import type { Point2D } from "./types"

export type Line = {
	type: "line"
	p0: Point2D
	p1: Point2D
}

export type CornerRectangle = {
	p0: Point2D
	p1: Point2D
}

export type Loop = {
	id: string
	points: []
}

export type Profile = {
	type: "profile"
	vertices: Point2D[]
	loops: number[][]
}

export type DepthExtrude = {
	depth: number
}

export type LoopExtrudeTarget = {
	type: "loopExtrudeTarget"
	
}

export type SolidExtrude = {
	type: "extrude"
	target: 
}

export type Sketch = {
	type: "sketch"
	id: string
	name?: string
	dirty: boolean
	entities: (Line | CornerRectangle)[]
	profiles: Profile
}