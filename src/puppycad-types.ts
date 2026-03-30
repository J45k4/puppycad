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

export interface Motion {
	step(dt: number): void
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

export type ProjectFileType = "schemantic" | "pcb" | "part" | "assembly" | "diagram"

export type ScalarVariableValue = number | string | boolean

export type Variables = Record<string, ScalarVariableValue>

export type SchemanticProjectComponentData = {
	type?: string
}

export type SchemanticProjectComponent = {
	id: number
	x: number
	y: number
	width: number
	height: number
	data?: SchemanticProjectComponentData
}

export type SchemanticProjectConnectionEndpoint = {
	componentId: number
	edge: "left" | "right" | "top" | "bottom"
	ratio: number
}

export type SchemanticProjectConnection = {
	from: SchemanticProjectConnectionEndpoint
	to: SchemanticProjectConnectionEndpoint
	style?: "solid" | "dashed"
}

export type SchemanticProjectItemData = {
	components: SchemanticProjectComponent[]
	connections: SchemanticProjectConnection[]
}

export type PartProjectPoint = { x: number; y: number }
export type PartProjectVector3 = { x: number; y: number; z: number }
export type PartProjectQuaternion = { x: number; y: number; z: number; w: number }

export type PartProjectExtrudedModel = {
	base: PartProjectPoint[]
	height: number
	scale: number
	rawHeight: number
	origin?: PartProjectVector3
	rotation?: PartProjectQuaternion
	startOffset?: number
}

export type PartProjectPreviewRotation = {
	yaw: number
	pitch: number
}

export type PartProjectReferencePlaneVisibility = {
	Front: boolean
	Top: boolean
	Right: boolean
}

export type LineEntity = {
	type: "line"
	p0: PartProjectPoint
	p1: PartProjectPoint
}

export type MidpointLine = {
	type: "midpointLine"
	midpoint: PartProjectPoint
	length: number
	angle?: number
}

export type CenteredRectangle = {
	type: "centeredRectangle"
	center: PartProjectPoint
	width: number
	height: number
	rotation?: number
}

export type CornerRectangle = {
	type: "cornerRectangle"
	p0: PartProjectPoint
	p1: PartProjectPoint
}

export type AlignedRectangle = {
	type: "alignedRectangle"
	p0: PartProjectPoint
	p1: PartProjectPoint
	height: number
}

export type CenterPointCircle = {
	type: "centerPointCircle"
	center: PartProjectPoint
	radius: number
}

export type SketchEntity = LineEntity | MidpointLine | CenteredRectangle | CornerRectangle | AlignedRectangle | CenterPointCircle

export type Profile = {
	id: string
	vertices: PartProjectPoint[]
	loops: number[][]
}

export type Sketch = {
	entities: SketchEntity[]
	profiles: Profile[]
}

export type ExtrudeTarget =
	| {
			type: "sketch"
			sketchId: string
			profileId?: string
	  }
	| {
			type: "profile"
			sketchId: string
			profileId: string
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
	faceId: string
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

export type Extrude = {
	type: "extrude"
	target: ExtrudeTarget
	extent: ExtrudeExtent
	operation: "newBody" | "add" | "remove" | "intersect"
	direction?: "positive" | "negative"
}

export type FilletEdgeTarget = {
	type: "extrudeEdge"
	extrudeId: string
	edgeId: string
}

export type Fillet = {
	type: "fillet"
	target: FilletEdgeTarget
	radius: number
}

export type ChamferEdgeTarget = FilletEdgeTarget

export type Chamfer = {
	type: "chamfer"
	target: ChamferEdgeTarget
	d1: number
	d2?: number
}

export type Part = {
	id: string
	name: string
	sketches: Sketch[]
	extrudes: Extrude[]
	fillets: Fillet[]
	chamfers: Chamfer[]
	variables?: Variables
}

export type PartProjectItemData = {
	sketchPoints: PartProjectPoint[]
	sketchName?: string
	isSketchClosed: boolean
	extrudedModels: PartProjectExtrudedModel[]
	height: number
	variables?: Variables
}

export type AssemblyMateType = "fasten" | "revolute" | "prismatic" | "planar" | "ball"

export type AssemblyMateReference = {
	instanceId: string
	connectorId: string
}

export type Transform3D = {
	translation?: PartProjectVector3
	rotation?: PartProjectVector3
	scale?: PartProjectVector3
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

export type ProjectFileSchemanticItem = {
	type: "schemantic"
	name: string
	data?: SchemanticProjectItemData
	visible?: boolean
}

export type ProjectFilePartItem = {
	type: "part"
	name: string
	data?: PartProjectItemData
	visible?: boolean
}

export type ProjectFileAssemblyItem = {
	type: "assembly"
	name: string
	data?: Assembly
	visible?: boolean
}

export type ProjectFileOtherItem = {
	type: Exclude<ProjectFileType, "schemantic" | "part" | "assembly">
	name: string
	visible?: boolean
}

export type ProjectFileItem = ProjectFileSchemanticItem | ProjectFilePartItem | ProjectFileAssemblyItem | ProjectFileOtherItem

export type ProjectFileFolder = {
	kind: "folder"
	name: string
	items: ProjectFileEntry[]
	visible?: boolean
}

export type ProjectFileEntry = ProjectFileItem | ProjectFileFolder

export type ProjectItem = ProjectFileEntry

export type ProjectFileVersion = 2

export type Project = {
	version: ProjectFileVersion
	items: ProjectFileEntry[]
	selectedPath: number[] | null
}

export type ProjectFile = Project

export type PuppyCadProject = Project
