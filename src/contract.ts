import type { Body, Sketch as RuntimeSketch } from "./puppycad"
import type { Pin } from "./puppycad"
import type { PartDocument } from "./schema"
import type { Point2D, Transform3D, Vector3D } from "./types"

export type { Point2D, Point3D, Quaternion, Transform2D, Transform3D, Vector3D } from "./types"

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
	points: Point2D[]
	lineWidth?: number
}

export interface FootprintSpec {
	pads: Pad[]
	outline?: FootprintOutline
	referenceOrigin?: Point2D
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
	start: Vector3D
	end: Vector3D
	width: number
	layer: string
	curvature?: number
}

export type BoardShape = { type: "polygon"; points: Point2D[] }

export type ScalarVariableValue = number | string | boolean

export type Variables = Record<string, ScalarVariableValue>

export type SketchEntityId = string
export type AnchorRef = `${SketchEntityId}:${string}`

type BaseSketchEntity = {
	id?: SketchEntityId
}

export type LineEntity = BaseSketchEntity & {
	type: "line"
	p0: Point2D
	p1: Point2D
}

export type MidpointLine = BaseSketchEntity & {
	type: "midpointLine"
	midpoint: Point2D
	length: number
	angle?: number
}

export type CenteredRectangle = BaseSketchEntity & {
	type: "centeredRectangle"
	center: Point2D
	width: number
	height: number
	rotation?: number
}

export type CornerRectangle = BaseSketchEntity & {
	type: "cornerRectangle"
	p0: Point2D
	p1: Point2D
}

export type AlignedRectangle = BaseSketchEntity & {
	type: "alignedRectangle"
	p0: Point2D
	p1: Point2D
	height: number
}

export type CenterPointCircle = BaseSketchEntity & {
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

export type SketchConstraint =
	| {
			id?: string
			type: "coincident"
			a: AnchorRef
			b: AnchorRef
	  }
	| {
			id?: string
			type: "parallel"
			eA: SketchEntityId
			eB: SketchEntityId
	  }
	| {
			id?: string
			type: "perpendicular"
			eA: SketchEntityId
			eB: SketchEntityId
	  }
	| {
			id?: string
			type: "equal"
			eA: SketchEntityId
			eB: SketchEntityId
	  }
	| {
			id?: string
			type: "horizontal"
			e: SketchEntityId
	  }
	| {
			id?: string
			type: "vertical"
			e: SketchEntityId
	  }
	| {
			id?: string
			type: "tangent"
			a: SketchEntityId
			b: SketchEntityId
	  }
	| {
			id?: string
			type: "concentric"
			a: SketchEntityId
			b: SketchEntityId
	  }
	| {
			id?: string
			type: "midpoint"
			point: AnchorRef
			on: SketchEntityId
	  }
	| {
			id?: string
			type: "symmetry"
			a: SketchEntityId
			b: SketchEntityId
			about: SketchEntityId
	  }

export type SketchDimension =
	| {
			id: string
			type: "distance"
			between: [AnchorRef, AnchorRef]
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
			value: number
	  }

export type SketchFeature = BaseFeature & {
	type: "sketch"
	target: SketchTarget
	entities: SketchEntity[]
	profiles: Profile[]
	constraints?: SketchConstraint[]
	dimensions?: SketchDimension[]
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

export type ProjectDocumentType = "schemantic" | "pcb" | "part" | "assembly" | "diagram"

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

export type PartProjectPreviewRotation = {
	yaw: number
	pitch: number
}

export type PartProjectReferencePlaneVisibility = {
	Front: boolean
	Top: boolean
	Right: boolean
}

export type PartProjectItemData = PartDocument

export type ProjectSchemanticDocument = {
	id: string
	type: "schemantic"
	name: string
	data?: SchemanticProjectItemData
	visible?: boolean
}

export type ProjectPartDocument = {
	id: string
	type: "part"
	name: string
	data?: PartProjectItemData
	visible?: boolean
}

export type ProjectAssemblyDocument = {
	id: string
	type: "assembly"
	name: string
	data?: Assembly
	visible?: boolean
}

export type ProjectOtherDocument = {
	id: string
	type: Exclude<ProjectDocumentType, "schemantic" | "part" | "assembly">
	name: string
	visible?: boolean
}

export type ProjectDocument = ProjectSchemanticDocument | ProjectPartDocument | ProjectAssemblyDocument | ProjectOtherDocument

export type ProjectFolder = {
	id: string
	kind: "folder"
	name: string
	items: ProjectNode[]
	visible?: boolean
}

export type ProjectNode = ProjectDocument | ProjectFolder

export type ProjectVersion = 4

export type Project = {
	version: ProjectVersion
	revision: number
	items: ProjectNode[]
	selectedPath: number[] | null
}
