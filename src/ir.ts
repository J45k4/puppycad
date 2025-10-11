/* =========================
 * Core & Top-level
 * ========================= */

export type Units = "mm" | "cm" | "m" | "in" | "mil"

export interface FeatureGraph {
	schema: "puppycad.featuregraph@0.1"
	units: Units
	doc: DocumentIR
	provenance?: Provenance
	meta?: {
		title?: string
		createdWith?: string
		[k: string]: unknown
	}
}

export interface DocumentIR {
	parts: Part[]
	assemblies: Assembly[]
}

/* =========================
 * Parts, Assemblies, Params
 * ========================= */

export interface Part {
	id: PartId // e.g. "part.bracket"
	name?: string
	material?: { name: string; density?: number; [k: string]: unknown }
	parameters?: Parameter[]
	features: Feature[] // ordered, deterministic
}

export type PartId = `part.${string}`

export interface Parameter {
	name: string
	type: "length" | "angle" | "ratio" | "integer" | "boolean"
	value: number | boolean
	min?: number
	max?: number
	step?: number
	unitHint?: Units | "deg"
}

export interface Assembly {
	id: `asm.${string}`
	name?: string
	instances: AssemblyInstance[]
	mates?: Mate[]
	meta?: Record<string, unknown>
}

export interface AssemblyInstance {
	id: `i_${string}`
	part: PartId
	transform?: {
		t?: Vec3 // translation
		rpy?: Vec3 // roll/pitch/yaw in degrees
	}
	meta?: Record<string, unknown>
}

export interface Mate {
	type: "fasten" | "revolute" | "prismatic" | "planar" | "ball"
	a: SelectorRef
	b: SelectorRef
	params?: Record<string, number | string | boolean>
}

/* =========================
 * Geometry Primitives
 * ========================= */

export type Vec2 = [number, number]
export type Vec3 = [number, number, number]

/* =========================
 * Feature Union
 * ========================= */

export type Feature = Sketch | Extrude | Revolve | Sweep | Fillet | Chamfer | Shell | Hole | HolePattern | Pattern | BooleanOp

export interface BaseFeature {
	op: FeatureKind
	id: FeatureId
	dependsOn?: FeatureId[] // data-flow/provenance
	tags?: string[]
	attributes?: Record<string, unknown>
}

export type FeatureKind = "Sketch" | "Extrude" | "Revolve" | "Sweep" | "Fillet" | "Chamfer" | "Shell" | "Hole" | "HolePattern" | "Pattern" | "Boolean"

export type FeatureId =
	| `sk${number}`
	| `ex${number}`
	| `rv${number}`
	| `sw${number}`
	| `f${number}`
	| `c${number}`
	| `sh${number}`
	| `h${number}`
	| `hp${number}`
	| `pt${number}`
	| `b${number}`
	| `${string}` // allow future/custom

/* =========================
 * Sketch
 * ========================= */

export interface Sketch extends BaseFeature {
	op: "Sketch"
	plane: PlaneRef
	entities: SketchEntity[] // ordered
	constraints?: SketchConstraint[]
	dimensions?: SketchDimension[]
	profiles?: SketchProfile[] // closed loops for solids
}

export type PlaneRef = { type: "datum"; name: "XY" | "YZ" | "ZX" | string } | { type: "offsetFace"; of: SelectorRef; offset: number } | { type: "planeBy3Pts"; p0: Vec3; p1: Vec3; p2: Vec3 }

export type SketchEntity = SketchLine | SketchArc | SketchCircle | SketchRect | SketchPolyline | SketchSpline

export interface SketchLine {
	id: SketchEntityId
	type: "line"
	p0: Vec2
	p1: Vec2
}

export interface SketchArc {
	id: SketchEntityId
	type: "arc"
	center: Vec2
	radius: number
	startDeg: number
	endDeg: number
	ccw?: boolean
}

export interface SketchCircle {
	id: SketchEntityId
	type: "circle"
	center: Vec2
	r: number
}

export interface SketchRect {
	id: SketchEntityId
	type: "rect"
	// Use either center+wh OR p0+p1 (diagonal)
	center?: Vec2
	w?: number
	h?: number
	p0?: Vec2
	p1?: Vec2
	fillet?: number // corner radius
}

export interface SketchPolyline {
	id: SketchEntityId
	type: "polyline"
	points: Vec2[]
	closed?: boolean
}

export interface SketchSpline {
	id: SketchEntityId
	type: "spline"
	points: Vec2[]
	degree?: number
	knots?: number[]
}

export type SketchEntityId = `${string}`

export type SketchConstraint =
	| { type: "coincident"; a: AnchorRef; b: AnchorRef }
	| { type: "parallel"; e: SketchEntityId }
	| { type: "perpendicular"; eA: SketchEntityId; eB: SketchEntityId }
	| { type: "equal"; eA: SketchEntityId; eB: SketchEntityId }
	| { type: "horizontal"; e: SketchEntityId }
	| { type: "vertical"; e: SketchEntityId }
	| { type: "tangent"; a: SketchEntityId; b: SketchEntityId }
	| { type: "concentric"; a: SketchEntityId; b: SketchEntityId }
	| { type: "midpoint"; point: AnchorRef; on: SketchEntityId }
	| { type: "symmetry"; a: SketchEntityId; b: SketchEntityId; about: SketchEntityId }

export type SketchDimension =
	| {
			id: string
			type: "distance"
			between: [AnchorRef, AnchorRef] | [ParamRef]
			value: number
	  }
	| {
			id: string
			type: "diameter" | "radius"
			of: SketchEntityId
			value: number
	  }
	| {
			id: string
			type: "angle"
			between: [SketchEntityId, SketchEntityId]
			value: number // degrees
	  }

export interface SketchProfile {
	id: string
	loops: SketchLoop[] // each loop is ordered list of entity ids
}

export type SketchLoop = SketchEntityId[]

// Anchor references within a sketch (e.g., "l1:p0")
export type AnchorRef = `${SketchEntityId}:p0` | `${SketchEntityId}:p1` | `${SketchEntityId}:center` | `${SketchEntityId}:${string}`

// Parameter path reference (e.g., "r1:w")
export type ParamRef = `${SketchEntityId}:${"w" | "h" | string}`

/* =========================
 * Solid Features
 * ========================= */

export interface Extrude extends BaseFeature {
	op: "Extrude"
	profile: ProfileRef
	depth: number
	taperDeg?: number
	direction?: DirectionRef // default: normal of sketch plane
	mode: "new" | "add" | "cut" | "intersect"
}

export interface Revolve extends BaseFeature {
	op: "Revolve"
	profile: ProfileRef
	axis: AxisRef
	angleDeg: number
	mode: "new" | "add" | "cut" | "intersect"
}

export interface Sweep extends BaseFeature {
	op: "Sweep"
	profile: ProfileRef
	path: PathRef
	mode: "new" | "add" | "cut" | "intersect"
}

export interface Fillet extends BaseFeature {
	op: "Fillet"
	edges: SelectorRef[]
	radius: number
}

export interface Chamfer extends BaseFeature {
	op: "Chamfer"
	edges: SelectorRef[]
	d1: number
	d2?: number // if omitted, symmetric chamfer (d1 = d2)
}

export interface Shell extends BaseFeature {
	op: "Shell"
	thickness: number
	removeFaces?: SelectorRef[]
}

export interface Hole extends BaseFeature {
	op: "Hole"
	on: SelectorRef // target face
	origin: Vec2 | Vec3 // 2D in sketch plane or 3D uv?
	diameter: number
	throughAll?: boolean
	depth?: number
	countersink?: { dia: number; angleDeg: number } | null
	counterbore?: { dia: number; depth: number } | null
}

export interface HolePattern extends BaseFeature {
	op: "HolePattern"
	on: SelectorRef // target face
	grid: { rows: number; cols: number; pitchX: number; pitchY: number; origin?: Vec2 } | { radial: true; count: number; radius: number; startDeg?: number }
	hole: Pick<Hole, "diameter" | "throughAll" | "depth" | "countersink" | "counterbore">
}

export interface Pattern extends BaseFeature {
	op: "Pattern"
	ofFeature: FeatureId
	type: "linear" | "circular"
	dir?: Vec3 // linear
	spacing?: number
	count?: number
	axisFeature?: FeatureId // circular
	angleDeg?: number
}

export interface BooleanOp extends BaseFeature {
	op: "Boolean"
	mode: "union" | "cut" | "intersect"
	targets: FeatureId[] // usually 1
	tools: FeatureId[] // one or more
}

/* =========================
 * Refs & Selectors
 * ========================= */

export type ProfileRef = `${FeatureId}` | `${FeatureId}:${string}` // e.g., "sk1:pf1"

export type DirectionRef =
	| { type: "normal"; of: FeatureId } // sketch/face normal
	| { type: "vector"; v: Vec3 }

export type AxisRef = { type: "sketchLine"; of: `${FeatureId}:${SketchEntityId}` } | { type: "edge"; of: SelectorRef } | { type: "datum"; name: string }

export type PathRef = `${FeatureId}:${SketchEntityId}` | SelectorRef

export type SelectorRef =
	| { select: "faceByNormal"; of: FeatureId; normal: "x+" | "x-" | "y+" | "y-" | "z+" | "z-" }
	| { select: "edgesByFace"; of: FeatureId; where?: "perimeterTop" | "perimeterBottom" | string }
	| { select: "byTag"; tag: string }
	| { select: "byId"; id: string }
	| { select: "edgesByQuery"; q: string }

/* =========================
 * Provenance & Validation
 * ========================= */

export interface Provenance {
	code?: Array<{
		node: string // FeatureId or path like "sk1.entities.l1"
		file: string
		range: [number, number] // byte offsets or UTF-16 positions
	}>
	[k: string]: unknown
}

export interface ValidationSummary {
	status: "ok" | "warn" | "error" | "unknown"
	checks: ValidationCheck[]
}

export interface ValidationCheck {
	driver: string // e.g., "3dp.prusa-mk4"
	result: "ok" | "warn" | "error"
	messages?: Array<{
		code: string // e.g., "thin_wall"
		at?: string // FeatureId or selector
		detail?: string
	}>
}

/* =========================
 * IR Patch (for live edits)
 * ========================= */

export type PatchOp = "add" | "remove" | "update" | "move" | "tag"

export interface IRPatch {
	op: PatchOp
	path: string // JSON Pointer path, e.g., "/doc/parts/0/features/2/depth"
	value?: unknown
	prev?: unknown
	ts?: number // epoch ms
	by?: "ui" | "code" | "ai" | "import"
}

/* =========================
 * Type Guards (optional helpers)
 * ========================= */

export const isFeature = (x: any): x is Feature => !!x && typeof x.op === "string" && typeof x.id === "string"
export const isSketch = (x: any): x is Sketch => isFeature(x) && x.op === "Sketch"
export const isExtrude = (x: any): x is Extrude => isFeature(x) && x.op === "Extrude"
export const isRevolve = (x: any): x is Revolve => isFeature(x) && x.op === "Revolve"
export const isSweep = (x: any): x is Sweep => isFeature(x) && x.op === "Sweep"
