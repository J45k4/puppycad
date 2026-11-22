// PuppyCad – Core type & class skeleton
// MIT License – © PuppyCorp

import type { BoardShape, LayerDefinition, LayerMaterial, NamedReference, Pad, SchematicReference, TraceSegment, UUID } from "./contract"

export { PCadPart, PCadProject, PCadProjectSyncError, PuppyCadClient } from "./pcad/project"
export type { PCadProjectSyncResult } from "./pcad/project"
export * from "./model-dsl"

export type {
	AssemblyActuator,
	AssemblyConnector,
	AssemblyInstance,
	AssemblyMate,
	AssemblyMateReference,
	AssemblyMateType,
	AssemblyServoActuator,
	BaseFeature,
	BoardShape,
	ChamferEdgeTarget,
	ChamferFeature,
	CompositeAliasReference,
	CompositeFeature,
	CompositeEdgeAlias,
	CompositeFaceAlias,
	CompositeFeatureAlias,
	CompositeProfileAlias,
	EdgeReference,
	ExtrudeEdgeReference,
	ExtrudeEdgeSelector,
	ExtrudeFaceReference,
	ExtrudeFaceSelector,
	ExtrudeBlindExtent,
	ExtrudeExtent,
	ExtrudeFeature,
	ExtrudeThroughAllExtent,
	ExtrudeUpToFaceExtent,
	ExtrudeUpToNextExtent,
	ExtrudeUpToPartExtent,
	ExtrudeUpToVertexExtent,
	FaceReference,
	Feature,
	FeatureId,
	FilletEdgeTarget,
	FilletFeature,
	FootprintOutline,
	FootprintSpec,
	LayerDefinition,
	LayerMaterial,
	NamedReference,
	Pad,
	Part,
	PortKind,
	ProfileReference,
	ProfileSelector,
	ScalarVariableValue,
	ResolvedAliasReference,
	ResolvedAliases,
	ResolvedEdgeReference,
	ResolvedFaceReference,
	ResolvedProfileReference,
	PartProjectItemData,
	Project,
	ProjectAssemblyDocument,
	SchematicReference,
	SketchProfileReference,
	SketchEntity,
	SketchFeature,
	SketchTarget,
	Transform3D,
	TraceSegment,
	UUID,
	Variables
} from "./contract"

export abstract class Entity {
	readonly id: UUID
	name: string
	metadata: Record<string, unknown> = {}
	transform?: Transform

	protected constructor(name = "Entity", id: UUID = crypto.randomUUID()) {
		this.id = id
		this.name = name
	}

	public visit(cb: (obj: Entity) => void, visited: Set<Entity> = new Set()): void {
		if (visited.has(this)) {
			return
		}
		visited.add(this)
		cb(this)
	}

	public serialize(): { type: string; [key: string]: unknown } {
		return {
			type: "entity",
			id: this.id,
			name: this.name
		}
	}

	public equal(other: Entity): boolean {
		return this.id === other.id
	}
}

export class Vec3 {
	x: number
	y: number
	z: number

	constructor(x = 0, y = 0, z = 0) {
		this.x = x
		this.y = y
		this.z = z
	}
}

export class Vec2 {
	x: number
	y: number

	constructor(x = 0, y = 0) {
		this.x = x
		this.y = y
	}
}

export class Transform {
	position: Vec3
	rotation: [number, number, number, number] // quaternion (x, y, z, w)
	scale: Vec3

	constructor(position: Vec3 = { x: 0, y: 0, z: 0 }, rotation: [number, number, number, number] = [0, 0, 0, 1], scale: Vec3 = { x: 1, y: 1, z: 1 }) {
		this.position = position
		this.rotation = rotation
		this.scale = scale
	}

	clone(): Transform {
		return new Transform({ ...this.position }, [...this.rotation] as [number, number, number, number], { ...this.scale })
	}

	// TODO: add combine, invert, apply methods
}

/** Pad shape types for electronic footprints */
export enum PadShape {
	Rectangular = "rectangular",
	Circular = "circular",
	Oval = "oval",
	Polygon = "polygon"
}

/** Represents an electronic component footprint */
export class Footprint extends Entity {
	public points: Vec2[] = []
	public lineWidth = 0
	public referenceOrigin: Vec2
	public pads: Pad[] = []

	constructor(args: {
		name: string
		points: Vec2[]
		lineWidth: number
		referenceOrigin: Vec2
		pads: Pad[]
	}) {
		super(args.name)
		this.lineWidth = args.lineWidth
		this.points = args.points
		this.referenceOrigin = args.referenceOrigin
		this.pads = args.pads
	}

	public visit(cb: (obj: Entity) => void, visited: Set<Entity>): void {
		// stub or implement as needed
	}

	public serialize(): { type: string; [key: string]: unknown } {
		return {
			...super.serialize(),
			lineWidth: this.lineWidth,
			referenceOrigin: this.referenceOrigin,
			pads: this.pads.map((pad) => pad.pin.id)
		}
	}
}

export class Pin extends Entity {
	public component?: Component

	public constructor(name = "Pin", id?: UUID) {
		super(name, id)
	}

	public serialize() {
		return {
			type: "pin",
			id: this.id,
			name: this.name,
			componentId: this.component?.id ?? null
		}
	}

	public visit(cb: (obj: Entity) => void, visited: Set<Entity> = new Set()) {
		super.visit(cb, visited)
	}

	public static parse(obj: NamedReference): Pin {
		return new Pin(obj.name, obj.id)
	}
}

export class Component extends Entity {
	public pins: Pin[]

	public constructor(name: string, id?: UUID) {
		super(name, id)
		this.pins = []
	}

	public addPin(pin: Pin) {
		if (pin.component && pin.component !== this) {
			pin.component.removePin(pin)
		}
		if (!this.pins.includes(pin)) {
			this.pins.push(pin)
		}
		pin.component = this
	}

	public removePin(pin: Pin) {
		const index = this.pins.indexOf(pin)
		if (index !== -1) {
			this.pins.splice(index, 1)
		}
		if (pin.component === this) {
			pin.component = undefined
		}
	}

	public visit(cb: (obj: Entity) => void, visited: Set<Entity> = new Set()) {
		if (visited.has(this)) {
			return
		}
		super.visit(cb, visited)
		for (const pin of this.pins) {
			pin.visit(cb, visited)
		}
	}

	public serialize() {
		return {
			type: "component",
			id: this.id,
			name: this.name,
			pins: this.pins.map((pin) => pin.id)
		}
	}

	public static parse(obj: NamedReference): Component {
		return new Component(obj.name, obj.id)
	}
}

export class Net extends Entity {
	public pins: Pin[]

	public constructor(name: string, id?: UUID) {
		super(name, id)
		this.pins = []
	}

	public connect(pin: Pin) {
		if (!this.pins.includes(pin)) {
			this.pins.push(pin)
		}
	}

	public static parse(obj: NamedReference): Net {
		return new Net(obj.name, obj.id)
	}

	public serialize() {
		return {
			type: "net",
			id: this.id,
			name: this.name,
			pins: this.pins.map((pin) => pin.id)
		}
	}

	public visit(cb: (obj: Entity) => void, visited: Set<Entity> = new Set()) {
		if (visited.has(this)) {
			return
		}
		super.visit(cb, visited)
		for (const pin of this.pins) {
			pin.visit(cb, visited)
		}
	}
}

export class Schematic extends Entity {
	public nets: Net[] = []
	public components: Component[] = []

	public constructor(
		args: {
			name?: string
			nets?: Net[]
			components?: Component[]
		},
		id?: UUID
	) {
		super(args.name ?? "Schematic", id)
		this.nets = args.nets ?? []
		this.components = args.components ?? []
	}

	public addNet(net: Net) {
		if (!this.nets.includes(net)) {
			this.nets.push(net)
		}
	}

	public addComponent(component: Component) {
		if (!this.components.includes(component)) {
			this.components.push(component)
		}
	}

	public serialize() {
		return {
			type: "schematic",
			id: this.id,
			name: this.name,
			nets: this.nets.map((net) => net.id),
			components: this.components.map((component) => component.id)
		}
	}

	public static parse(obj: SchematicReference): Schematic {
		return new Schematic(
			{
				name: obj.name,
				nets: [],
				components: []
			},
			obj.id
		)
	}

	public visit(cb: (obj: Entity) => void, visited: Set<Entity> = new Set()) {
		if (visited.has(this)) {
			return
		}
		super.visit(cb, visited)
		for (const net of this.nets) {
			net.visit(cb, visited)
		}
		for (const component of this.components) {
			component.visit(cb, visited)
		}
	}
}

export class Trace extends Entity {
	constructor(
		name: string,
		public segments: TraceSegment[],
		public net?: Net
	) {
		super(name)
	}
}

export class Layer extends Entity {
	type: "copper" | "dielectric" | "soldermask" | "silkscreen" | "fabrication" | "drill" | "keepout"
	material?: LayerMaterial
	thickness?: number
	traces: Trace[] = []

	constructor(args: {
		name: string
		type: "copper" | "dielectric" | "soldermask" | "silkscreen" | "fabrication" | "drill" | "keepout"
		material?: LayerMaterial
		thickness?: number
		traces?: Trace[]
	}) {
		super(args.name)
		this.type = args.type
		this.material = args.material
		this.thickness = args.thickness
		this.traces = args.traces ?? []
	}
}

export class Via extends Entity {
	public pos: Vec2
	public diameter: number
	public drillDiameter: number
	public layers: Layer[] = []
	public shape: BoardShape
	public constructor() {
		super("Via")
		this.pos = new Vec2(0, 0)
		this.diameter = 0
		this.drillDiameter = 0
		this.shape = { type: "polygon", points: [] }
	}
}

export class PCB extends Entity {
	private thickness: number
	public material?: LayerMaterial
	public layers: Layer[] = []
	public topFootprints: Footprint[] = []
	public bottomFootprints: Footprint[] = []
	public vias: Via[] = []
	public components: Entity[] = []
	public nets: Net[] = []

	constructor(
		args: {
			name: string
			thickness: number
			material?: LayerMaterial
		},
		id?: UUID
	) {
		super(args.name, id)
		this.thickness = args.thickness
		this.material = args.material
	}

	public addLayer(layer: Layer | LayerDefinition) {
		const nextLayer = layer instanceof Layer ? layer : new Layer({ ...layer })
		this.layers.push(nextLayer)
		return nextLayer
	}

	public addComponent(component: Entity) {
		if (!this.components.includes(component)) {
			this.components.push(component)
		}
	}

	public addNet(net: Net) {
		if (!this.nets.includes(net)) {
			this.nets.push(net)
		}
	}

	public serialize() {
		return {
			type: "pcb",
			id: this.id,
			name: this.name,
			thickness: this.thickness,
			material: this.material,
			layers: this.layers.map((layer) => layer.id),
			nets: this.nets.map((net) => net.id),
			components: this.components.map((component) => component.id)
		}
	}
}
