// PuppyCad – Core type & class skeleton
// MIT License – © PuppyCorp

export type UUID = string;

export class Vec3 {
	x: number;
	y: number;
	z: number;

	constructor(x: number = 0, y: number = 0, z: number = 0) {
		this.x = x;
		this.y = y;
		this.z = z;
	}
}

export class Vec2 {
	x: number;
	y: number;

	constructor(x: number = 0, y: number = 0) {
		this.x = x;
		this.y = y;
	}
}

export class Transform {
	position: Vec3;
	rotation: [number, number, number, number]; // quaternion (x, y, z, w)
	scale: Vec3;

	constructor(
		position: Vec3 = { x: 0, y: 0, z: 0 },
		rotation: [number, number, number, number] = [0, 0, 0, 1],
		scale: Vec3 = { x: 1, y: 1, z: 1 }
	) {
		this.position = position;
		this.rotation = rotation;
		this.scale = scale;
	}

	clone(): Transform {
		return new Transform(
			{ ...this.position },
			[...this.rotation] as [number, number, number, number],
			{ ...this.scale }
		);
	}

	// TODO: add combine, invert, apply methods
}

// ---- Core Scene Graph -------------------------------------------------------

export abstract class Entity {
	readonly id: UUID;
	name: string;
	transform: Transform;
	metadata: Record<string, unknown> = {}


	protected constructor(name: string = "Entity") {
		this.id = crypto.randomUUID();
		this.name = name;
		this.transform = new Transform();
	}
}

export class Group extends Entity {
	children: Entity[] = [];

	addChild(child: Entity): void {
		this.children.push(child);
	}
}

/** Pad shape types for electronic footprints */
export enum PadShape {
	Rectangular = "rectangular",
	Circular = "circular",
	Oval = "oval",
	Polygon = "polygon"
}

/** Represents a single pad in a footprint */
export interface Pad {
	name: string; // e.g., "1", "A", "GND"
	x: number;    // mm, relative to footprint origin
	y: number;    // mm, relative to footprint origin
	width: number; // mm
	height: number; // mm
	shape: PadShape;
	rotation?: number; // degrees
	net?: string; // optional net name
}

/** Represents the outline (body) of a footprint */
export interface FootprintOutline {
	points: { x: number; y: number }[]; // closed polygon, mm units
	lineWidth?: number; // mm
}

/** Represents an electronic component footprint */
export class Footprint {
	name: string // e.g., "SOIC-8"
	pads: Pad[]
	outline?: FootprintOutline
	referenceOrigin?: { 
		x: number 
		y: number 
	} // Where the refdes is placed
	description?: string
}

/** Entity representing a footprint in the scene */
export class FootprintEntity extends Entity {
	footprint: Footprint;
	constructor(name: string, footprint: Footprint) {
		super(name);
		this.footprint = footprint;
	}
}

// ---- Simulation / Kinematics -----------------------------------------------
export interface Motion { step(dt: number): void; }
export class RotationalMotion implements Motion {
	constructor(public entity: Entity, public axis: Vec3, public angularVelocity: number) { }
	step(dt: number) {
		const angle = this.angularVelocity * dt;
		const half = angle / 2;
		const [ax, ay, az] = [this.axis.x, this.axis.y, this.axis.z];
		const sinH = Math.sin(half), cosH = Math.cos(half);
		const dq: [number, number, number, number] = [ax * sinH, ay * sinH, az * sinH, cosH];
		const [qx, qy, qz, qw] = this.entity.transform.rotation;
		const [rx, ry, rz, rw] = dq;
		this.entity.transform.rotation = [
			rw * qx + rx * qw + ry * qz - rz * qy,
			rw * qy - rx * qz + ry * qw + rz * qx,
			rw * qz + rx * qy - ry * qx + rz * qw,
			rw * qw - rx * qx - ry * qy - rz * qz
		];
	}
}
export class SimEngine { motions: Motion[] = []; addMotion(m: Motion) { this.motions.push(m); } step(dt: number) { this.motions.forEach(m => m.step(dt)); } }

// ---- Constraint System ------------------------------------------------------

/** Enumerates available constraint types for mechanical and electrical domains */
export enum ConstraintType {
	// Mechanical constraints
	Coincident = "coincident",
	Distance = "distance",
	Angle = "angle",
	Parallel = "parallel",
	Perpendicular = "perpendicular",
	Concentric = "concentric",
	Tangent = "tangent",
	Fixed = "fixed",
	// Electrical constraints
	Clearance = "clearance",
	NetTie = "netTie",
	Alignment = "alignment"
}

/** Base class for any constraint between two entities */
export abstract class Constraint {
	readonly id: UUID;
	type: ConstraintType;
	lhs: Entity;
	rhs: Entity;

	protected constructor(type: ConstraintType, lhs: Entity, rhs: Entity) {
		this.id = crypto.randomUUID();
		this.type = type;
		this.lhs = lhs;
		this.rhs = rhs;
	}
}

export class CoincidentConstraint extends Constraint {
	constructor(lhs: Entity, rhs: Entity) {
		super(ConstraintType.Coincident, lhs, rhs);
	}
}

export class DistanceConstraint extends Constraint {
	distance: number;
	constructor(lhs: Entity, rhs: Entity, distance: number) {
		super(ConstraintType.Distance, lhs, rhs);
		this.distance = distance;
	}
}

export class AngleConstraint extends Constraint {
	angleDegrees: number;
	constructor(lhs: Entity, rhs: Entity, angleDegrees: number) {
		super(ConstraintType.Angle, lhs, rhs);
		this.angleDegrees = angleDegrees;
	}
}

export class ParallelConstraint extends Constraint {
	constructor(lhs: Entity, rhs: Entity) {
		super(ConstraintType.Parallel, lhs, rhs);
	}
}

export class PerpendicularConstraint extends Constraint {
	constructor(lhs: Entity, rhs: Entity) {
		super(ConstraintType.Perpendicular, lhs, rhs);
	}
}

export class ConcentricConstraint extends Constraint {
	constructor(lhs: Entity, rhs: Entity) {
		super(ConstraintType.Concentric, lhs, rhs);
	}
}

export class TangentConstraint extends Constraint {
	constructor(lhs: Entity, rhs: Entity) {
		super(ConstraintType.Tangent, lhs, rhs);
	}
}

export class FixedConstraint extends Constraint {
	constructor(entity: Entity) {
		super(ConstraintType.Fixed, entity, entity);
	}
}


/** Ensures two nets maintain a minimum clearance distance */
export class ClearanceConstraint extends Constraint {
	clearance: number;
	constructor(lhs: Entity, rhs: Entity, clearance: number) {
		super(ConstraintType.Clearance, lhs, rhs);
		this.clearance = clearance;
	}
}

export class NetTieConstraint extends Constraint {
	constructor(lhs: Entity, rhs: Entity) {
		super(ConstraintType.NetTie, lhs, rhs);
	}
}

export class AlignmentConstraint extends Constraint {
	axis: "x" | "y";
	offset: number;
	constructor(lhs: Entity, rhs: Entity, axis: "x" | "y", offset: number) {
		super(ConstraintType.Alignment, lhs, rhs);
		this.axis = axis;
		this.offset = offset;
	}
}

export type PortKind = "mechanical" | "electrical";

export class Port extends Entity {
	kind: PortKind;
	constructor(name: string, kind: PortKind = "mechanical") {
		super(name);
		this.kind = kind;
	}
}

export class BlockTemplate extends Group {
	ports: Port[] = [];
}

export class BlockInstance extends Group {
	template: BlockTemplate;
	portMap: Map<UUID, Port> = new Map();

	constructor(template: BlockTemplate, name: string = `${template.name}_inst`) {
		super(name);
		this.template = template;
		template.children.forEach(c => this.addChild(structuredClone(c)));
		template.ports.forEach(p => {
			const cp = structuredClone(p) as Port;
			this.portMap.set(p.id, cp);
			this.addChild(cp);
		});
	}
}

export class Circle extends Entity {
	diameter: number

	public constructor(name: string, diameter: number) {
		super(name)
		this.diameter = diameter
	}
}

export class Rectangle {

}

export class Line {

}

export class Edge {
	start: Vec3
	end: Vec3
	type: "line" | "arc" | "circle" | "spline"
}

export class Face {
	edges: Edge[]

}

export class Feature {

}

export class Sketch extends Entity {
	// TODO: 2‑D curve definitions (lines, arcs, splines)
	extrusionDepth: number = 0
	direction: { x: number, y: number, z: number }
    operation: "add" | "cut" | "intersect" // boolean operation type
	features: Feature[] = []

	constructor(name: string) {
		super(name)
	}
}

export class Body extends Entity {
	// Placeholder for B‑Rep or mesh representation
}

export interface FeatureContext {
	target: Body | Sketch;
}

export class Assembly extends Group {
	constraints: Constraint[] = [];
}

export type NetNode = Pad | Port;

export class Pad extends Entity {
	number: string;
	position: Vec3;
	// ... other pad properties as before
	constructor(number: string, position: Vec3) {
		super("Pad");
		this.number = number;
		this.position = position;
	}
}

export class Pin {
	name: string = ""
}

export class Component extends Entity {
	footprint: Footprint
	position: Vec2
	pads: Pad[] = [];
	pins: Pin[] = [];

	public constructor(name: string) {
		super(name)
	}

	public addPin(pin: Pin) {
		this.pins.push(pin)
	}
}

export class Net {
	name: string
	pins: Pin[] = []
	constructor(name: string) {
		this.name = name
	}

	public connect(pin: Pin) {
		this.pins.push(pin)
	}
}

export class Schematic {
	public nets: Net[] = []
	public constructor(args: {
		nets: Net[]
	}) {
		this.nets = args.nets
	}
}

// Union of common PCB layer materials
export type LayerMaterial =
	// Conductors
	| "copper"                     // rolled annealed copper foil
	// Dielectrics & cores
	| "FR4"                        // epoxy-glass laminate
	| "CEM1"                       // paper-epoxy laminate
	| "Rogers_RT/duroid"           // high-frequency PTFE laminate
	| "polyimide"                  // flexible-board substrate
	| "ceramic_filler"             // low-loss, high-temp substrate
	| "aluminum_core"              // metal-core PCB for thermal dissipation
	// Masks & inks
	| "epoxy_soldermask"           // liquid photoimageable solder-resist
	| "coverlay_polyimide"         // flex-board protective overlay
	| "silkscreen_ink"             // epoxy or acrylic legend ink
	// Fabrication & tooling
	| "photoresist"                // for drill/route patterning
	| "adhesive_prepreg"           // bonding layer between cores
	;

// Extend LayerDefinition to include material
export interface LayerDefinition {
	name: string;
	type: "copper"
	| "dielectric"
	| "soldermask"
	| "silkscreen"
	| "fabrication"
	| "drill"
	| "keepout";
	material?: LayerMaterial;
	thickness?: number;
}

export interface TraceSegment { start: Vec3; end: Vec3; width: number; layer: string; curvature?: number; }
export class Trace extends Entity {
	constructor(name: string, public segments: TraceSegment[], public net?: Net) {
		super(name)
	}
}


export class PCB extends Entity {
	private material: LayerMaterial
	private thickness: number
	components: Component[] = []
	nets: Net[] = []
	outline?: Sketch
	layers: LayerDefinition[] = []

	constructor(args: {
		name: string
		material: LayerMaterial
		thickness: number
	}) {
		super(args.name)
		this.material = args.material
		this.thickness = args.thickness
	}

	addLayer(layer: LayerDefinition): void {
		this.layers.push(layer);
	}
}

// ---- Parametric & Expression System ----------------------------------------

export class Parameter {
	name: string;
	expression: string | number;
	constructor(name: string, expression: string | number) {
		this.name = name;
		this.expression = expression;
	}
}

// ---- Design Root ------------------------------------------------------------

export class Design {
	assemblies: Assembly[] = [];
	pcbs: PCB[] = [];
	templates: BlockTemplate[] = [];
	parameters: Record<string, Parameter> = {};

	addAssembly(asm: Assembly): void {
		this.assemblies.push(asm);
	}
	addPCB(board: PCB): void {
		this.pcbs.push(board);
	}
	addTemplate(tpl: BlockTemplate): void {
		this.templates.push(tpl);
	}
}

export class Netlist {
	nets: Net[] = []

	addNet(net: Net): void {
		this.nets.push(net);
	}
}

export class Plane {

}

export class Path {

}

export class Profile {
	points: Vec3[] = []

	public constructor(points: number[][]) {

	}
}

export class Solid {
	private profile: Profile

	public constructor(profile: Profile) {
		this.profile = profile
	}

	public sweep(path: Path) {

	}
}

export class LinePath {
	public constructor(start: Vec3, end: Vec3) {

	}
}