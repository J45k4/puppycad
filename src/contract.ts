import type { Body, Sketch as RuntimeSketch, Vec2, Vec3 } from "./puppycad"
import type { Pin } from "./puppycad"

export type UUID = string

export type NamedReference = { id: UUID; name: string }

export type SchematicReference = NamedReference & {
	nets?: NamedReference[]
	components?: NamedReference[]
}

export type Pad = {
	type?: "smd" | "through"
	pin: Pin
	x: number
	y: number
	width: number
	height: number
	shape: "rectangular" | "circular" | "oval" | "polygon"
	rotation?: number
	net?: string
}

export interface FootprintOutline {
	points: { x: number; y: number }[]
	lineWidth?: number
}

export interface FootprintSpec {
	pads: Pad[]
	outline?: FootprintOutline
	referenceOrigin?: {
		x: number
		y: number
	}
	description?: string
}

export type PortKind = "mechanical" | "electrical"

export interface FeatureContext {
	target: Body | RuntimeSketch
}

export type LayerMaterial =
	| "copper"
	| "FR4"
	| "CEM1"
	| "Rogers_RT/duroid"
	| "polyimide"
	| "ceramic_filler"
	| "aluminum_core"
	| "epoxy_soldermask"
	| "coverlay_polyimide"
	| "silkscreen_ink"
	| "photoresist"
	| "adhesive_prepreg"

export interface LayerDefinition {
	name: string
	type: "copper" | "dielectric" | "soldermask" | "silkscreen" | "fabrication" | "drill" | "keepout"
	material?: LayerMaterial
	thickness?: number
}

export interface TraceSegment {
	start: Vec3
	end: Vec3
	width: number
	layer: string
	curvature?: number
}

export type BoardShape = { type: "polygon"; points: Vec2[] }

export type ScalarVariableValue = number | string | boolean

export type Variables = Record<string, ScalarVariableValue>

export type Point2D = { x: number; y: number }
export type Vector3D = { x: number; y: number; z: number }
export type Quaternion = { x: number; y: number; z: number; w: number }

export type LineEntity = {
	type: "line"
	p0: Point2D
	p1: Point2D
}

export type MidpointLine = {
	type: "midpointLine"
	midpoint: Point2D
	length: number
	angle?: number
}

export type CenteredRectangle = {
	type: "centeredRectangle"
	center: Point2D
	width: number
	height: number
	rotation?: number
}

export type CornerRectangle = {
	type: "cornerRectangle"
	p0: Point2D
	p1: Point2D
}

export type AlignedRectangle = {
	type: "alignedRectangle"
	p0: Point2D
	p1: Point2D
	height: number
}

export type CenterPointCircle = {
	type: "centerPointCircle"
	center: Point2D
	radius: number
}

export type SketchEntity = LineEntity | MidpointLine | CenteredRectangle | CornerRectangle | AlignedRectangle | CenterPointCircle

export type Profile = {
	id: string
	vertices: Point2D[]
	loops: number[][]
}

export type FeatureId = string

export type BaseFeature = {
	id: FeatureId
	type: string
	name?: string
	suppressed?: boolean
	dependsOn?: FeatureId[]
}

export type CompositeAliasReference = {
	type: "compositeAlias"
	compositeFeatureId: FeatureId
	aliasId: string
}

export type ProfileSelector = {
	type: "containsPoint"
	point: Point2D
}

export type SketchProfileReference = {
	type: "sketchProfile"
	sketchFeatureId: FeatureId
	selector: ProfileSelector
}

export type ProfileReference = SketchProfileReference | CompositeAliasReference

export type ExtrudeFaceSelector =
	| {
			type: "cap"
			side: "top" | "bottom"
	  }
	| {
			type: "side"
			index: number
	  }

export type ExtrudeFaceReference = {
	type: "extrudeFace"
	extrudeFeatureId: FeatureId
	selector: ExtrudeFaceSelector
}

export type FaceReference = ExtrudeFaceReference | CompositeAliasReference

export type ExtrudeEdgeSelector =
	| {
			type: "capLoop"
			side: "top" | "bottom"
			index: number
	  }
	| {
			type: "side"
			index: number
	  }

export type ExtrudeEdgeReference = {
	type: "extrudeEdge"
	extrudeFeatureId: FeatureId
	selector: ExtrudeEdgeSelector
}

export type EdgeReference = ExtrudeEdgeReference | CompositeAliasReference

export type SketchTarget =
	| {
			type: "plane"
			plane: "XY" | "YZ" | "XZ"
	  }
	| {
			type: "face"
			face: FaceReference
	  }

export type SketchFeature = BaseFeature & {
	type: "sketch"
	target: SketchTarget
	entities: SketchEntity[]
	profiles: Profile[]
}

export type ExtrudeBlindExtent = {
	type: "blind"
	distance: number
}

export type ExtrudeUpToNextExtent = {
	type: "upToNext"
}

export type ExtrudeUpToFaceExtent = {
	type: "upToFace"
	face: FaceReference
}

export type ExtrudeUpToPartExtent = {
	type: "upToPart"
	partId: string
}

export type ExtrudeUpToVertexExtent = {
	type: "upToVertex"
	vertexId: string
}

export type ExtrudeThroughAllExtent = {
	type: "throughAll"
}

export type ExtrudeExtent = ExtrudeBlindExtent | ExtrudeUpToNextExtent | ExtrudeUpToFaceExtent | ExtrudeUpToPartExtent | ExtrudeUpToVertexExtent | ExtrudeThroughAllExtent

export type ExtrudeFeature = BaseFeature & {
	type: "extrude"
	target: ProfileReference
	extent: ExtrudeExtent
	operation: "newBody" | "add" | "remove" | "intersect"
	direction?: "positive" | "negative"
}

export type FilletEdgeTarget = {
	edge: EdgeReference
}

export type FilletFeature = BaseFeature & {
	type: "fillet"
	target: FilletEdgeTarget
	radius: number
}

export type ChamferEdgeTarget = FilletEdgeTarget

export type ChamferFeature = BaseFeature & {
	type: "chamfer"
	target: ChamferEdgeTarget
	d1: number
	d2?: number
}

export type CompositeFeature = BaseFeature & {
	type: "composite"
	features: Feature[]
	aliases?: CompositeFeatureAlias[]
	variables?: Variables
	transform?: Transform3D
}

export type Feature = SketchFeature | ExtrudeFeature | FilletFeature | ChamferFeature | CompositeFeature

export type CompositeProfileAlias = {
	id: string
	type: "profile"
	source: SketchProfileReference
}

export type CompositeFaceAlias = {
	id: string
	type: "face"
	source: ExtrudeFaceReference
}

export type CompositeEdgeAlias = {
	id: string
	type: "edge"
	source: ExtrudeEdgeReference
}

export type CompositeFeatureAlias = CompositeProfileAlias | CompositeFaceAlias | CompositeEdgeAlias

export type ResolvedProfileReference = {
	type: "profile"
	sketchFeatureId: FeatureId
	profileId: string
}

export type ResolvedFaceReference = {
	type: "face"
	solidId: string
	faceId: string
}

export type ResolvedEdgeReference = {
	type: "edge"
	solidId: string
	edgeId: string
}

export type ResolvedAliasReference = ResolvedProfileReference | ResolvedFaceReference | ResolvedEdgeReference

export type ResolvedAliases = Record<FeatureId, Record<string, ResolvedAliasReference>>

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
	featureId: FeatureId
	vertices: SolidVertex[]
	edges: SolidEdge[]
	faces: SolidFace[]
}

export type Part = {
	id: string
	name: string
	features: Feature[]
	solids?: Solid[]
	resolvedAliases?: ResolvedAliases
	variables?: Variables
}

export type AssemblyMateType = "fasten" | "revolute" | "prismatic" | "planar" | "ball"

export type AssemblyMateReference = {
	instanceId: string
	connectorId: string
}

export type Transform3D = {
	translation?: Vector3D
	rotation?: Vector3D
	scale?: Vector3D
}

export type AssemblyInstance = {
	id: string
	partId: string
	transform?: Transform3D
}

export type AssemblyMate = {
	type: AssemblyMateType
	a: AssemblyMateReference
	b: AssemblyMateReference
	params?: Variables
}

export type Assembly = {
	id: string
	name: string
	instances: AssemblyInstance[]
	mates?: AssemblyMate[]
	variables?: Variables
}
