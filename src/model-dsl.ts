import { materializeSketch } from "./cad/sketch"
import type { Assembly, AssemblyMate, Project, ProjectNode, Variables } from "./contract"
import { createProjectFile } from "./project-file"
import type { Sketch, SolidExtrude } from "./schema"
import type { Point2D, Vector3D } from "./types"

export type { Point2D, Vector3D } from "./types"

export const MODEL_DEFINITION_KIND = "puppycad.model/v1" as const
export const COMPONENT_DEFINITION_KIND = "puppycad.component/v1" as const

export type ModelUnits = "mm"

export type PlanarTransform = {
	translate?: Point2D
	rotateDeg?: number
}

export type ResolvedPlanarTransform = {
	translate: Point2D
	rotateDeg: number
}

export type ModelMetadata = Record<string, string | number | boolean | null>

export type BodyAppearance = {
	category?: string
	color?: string
}

export type BodyNode = {
	id: string
	name: string
	componentId: string
	outline: Point2D[]
	depth: number
	appearance?: BodyAppearance
	metadata?: ModelMetadata
}

export type FrameNode = {
	id: string
	name: string
	componentId: string
	bodyId: string | null
	position: Vector3D
	rotationDeg: number
}

export type FixedMateNode = {
	id: string
	name: string
	componentId: string
	type: "fixed"
	parentFrameId: string
	childFrameId: string
}

export type RevoluteMateNode = {
	id: string
	name: string
	componentId: string
	type: "revolute"
	parentFrameId: string
	childFrameId: string
	axis: Vector3D
	limits?: {
		minDeg: number
		maxDeg: number
	}
}

export type MateNode = FixedMateNode | RevoluteMateNode

export type ServoNode = {
	id: string
	name: string
	componentId: string
	jointId: string
	homeDeg: number
	commandRange: {
		minDeg: number
		maxDeg: number
	}
	maxTorqueNcm?: number
	maxSpeedDegPerSec?: number
}

export type ComponentInstanceNode = {
	id: string
	name: string
	parentId: string | null
	localTransform: ResolvedPlanarTransform
	worldTransform: ResolvedPlanarTransform
}

export type ModelDefinition = {
	kind: typeof MODEL_DEFINITION_KIND
	version: 1
	id: string
	name: string
	units: ModelUnits
	metadata?: ModelMetadata
	components: ComponentInstanceNode[]
	bodies: BodyNode[]
	frames: FrameNode[]
	mates: MateNode[]
	servos: ServoNode[]
}

export type BodyRef = {
	readonly kind: "body"
	readonly id: string
}

export type FrameRef = {
	readonly kind: "frame"
	readonly id: string
}

export type FixedMateRef = {
	readonly kind: "mate"
	readonly type: "fixed"
	readonly id: string
}

export type RevoluteMateRef = {
	readonly kind: "mate"
	readonly type: "revolute"
	readonly id: string
}

export type ServoRef = {
	readonly kind: "servo"
	readonly id: string
}

export type BodySpec = {
	name?: string
	outline: readonly Point2D[]
	depth: number
	appearance?: BodyAppearance
	metadata?: ModelMetadata
}

export type FrameSpec = {
	name?: string
	body?: BodyRef
	at: Point2D | Vector3D
	rotateDeg?: number
}

export type FixedMateSpec = {
	name?: string
	parent: FrameRef
	child: FrameRef
}

export type RevoluteMateSpec = {
	name?: string
	parent: FrameRef
	child: FrameRef
	axis?: Vector3D
	limits?: {
		minDeg: number
		maxDeg: number
	}
}

export type ServoSpec = {
	name?: string
	joint: RevoluteMateRef
	homeDeg?: number
	commandRange?: {
		minDeg: number
		maxDeg: number
	}
	maxTorqueNcm?: number
	maxSpeedDegPerSec?: number
}

export type ComponentDefinition<Props, Result> = {
	readonly kind: typeof COMPONENT_DEFINITION_KIND
	readonly name: string
	readonly build: (scope: ModelScope, props: Props) => Result
}

export type InstanceOptions = PlanarTransform & {
	name?: string
}

export type DefineModelOptions = {
	id: string
	name: string
	units?: ModelUnits
	metadata?: ModelMetadata
}

export type CompiledModel = {
	graph: ModelDefinition
	project: Project
}

type BuilderState = {
	components: ComponentInstanceNode[]
	bodies: BodyNode[]
	frames: FrameNode[]
	mates: MateNode[]
	servos: ServoNode[]
	componentIds: Set<string>
	bodyIds: Set<string>
	frameIds: Set<string>
	mateIds: Set<string>
	servoIds: Set<string>
}

const ID_SEGMENT_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]*$/
const ID_PATH_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]*(\/[A-Za-z0-9][A-Za-z0-9._-]*)*$/
const IDENTITY_TRANSFORM: ResolvedPlanarTransform = {
	translate: { x: 0, y: 0 },
	rotateDeg: 0
}

export class ModelScope {
	readonly path: string

	private readonly state: BuilderState
	private readonly worldTransform: ResolvedPlanarTransform

	constructor(state: BuilderState, path: string, worldTransform: ResolvedPlanarTransform) {
		this.state = state
		this.path = path
		this.worldTransform = worldTransform
	}

	body(id: string, spec: BodySpec): BodyRef {
		const nodeId = this.resolveId(id)
		reserveId(this.state.bodyIds, nodeId, "body")
		if (!Number.isFinite(spec.depth) || spec.depth <= 0) {
			throw new Error(`Body "${nodeId}" depth must be a positive finite number.`)
		}
		if (spec.outline.length < 3) {
			throw new Error(`Body "${nodeId}" outline must contain at least three points.`)
		}

		const outline = spec.outline.map((point, index) => {
			assertFinitePoint(point, `Body "${nodeId}" outline point ${index + 1}`)
			return transformPoint(point, this.worldTransform)
		})
		this.state.bodies.push({
			id: nodeId,
			name: spec.name?.trim() || humanizeId(id),
			componentId: this.path,
			outline,
			depth: spec.depth,
			...(spec.appearance ? { appearance: { ...spec.appearance } } : {}),
			...(spec.metadata ? { metadata: { ...spec.metadata } } : {})
		})
		return { kind: "body", id: nodeId }
	}

	frame(id: string, spec: FrameSpec): FrameRef {
		const nodeId = this.resolveId(id)
		reserveId(this.state.frameIds, nodeId, "frame")
		const position = point3(spec.at)
		assertFiniteVector(position, `Frame "${nodeId}" position`)
		const transformed = transformPoint(position, this.worldTransform)
		const rotationDeg = this.worldTransform.rotateDeg + (spec.rotateDeg ?? 0)
		assertFiniteNumber(rotationDeg, `Frame "${nodeId}" rotation`)
		this.state.frames.push({
			id: nodeId,
			name: spec.name?.trim() || humanizeId(id),
			componentId: this.path,
			bodyId: spec.body?.id ?? null,
			position: { x: transformed.x, y: transformed.y, z: position.z },
			rotationDeg
		})
		return { kind: "frame", id: nodeId }
	}

	fixed(id: string, spec: FixedMateSpec): FixedMateRef {
		const nodeId = this.resolveId(id)
		reserveId(this.state.mateIds, nodeId, "mate")
		this.state.mates.push({
			id: nodeId,
			name: spec.name?.trim() || humanizeId(id),
			componentId: this.path,
			type: "fixed",
			parentFrameId: spec.parent.id,
			childFrameId: spec.child.id
		})
		return { kind: "mate", type: "fixed", id: nodeId }
	}

	revolute(id: string, spec: RevoluteMateSpec): RevoluteMateRef {
		const nodeId = this.resolveId(id)
		reserveId(this.state.mateIds, nodeId, "mate")
		const axis = rotateVector3(spec.axis ?? { x: 0, y: 0, z: 1 }, this.worldTransform.rotateDeg)
		assertFiniteVector(axis, `Revolute mate "${nodeId}" axis`)
		if (Math.hypot(axis.x, axis.y, axis.z) <= Number.EPSILON) {
			throw new Error(`Revolute mate "${nodeId}" axis must be non-zero.`)
		}
		if (spec.limits) {
			assertFiniteNumber(spec.limits.minDeg, `Revolute mate "${nodeId}" minimum limit`)
			assertFiniteNumber(spec.limits.maxDeg, `Revolute mate "${nodeId}" maximum limit`)
			if (spec.limits.minDeg > spec.limits.maxDeg) {
				throw new Error(`Revolute mate "${nodeId}" minimum limit cannot exceed its maximum limit.`)
			}
		}

		this.state.mates.push({
			id: nodeId,
			name: spec.name?.trim() || humanizeId(id),
			componentId: this.path,
			type: "revolute",
			parentFrameId: spec.parent.id,
			childFrameId: spec.child.id,
			axis,
			...(spec.limits ? { limits: { ...spec.limits } } : {})
		})
		return { kind: "mate", type: "revolute", id: nodeId }
	}

	servo(id: string, spec: ServoSpec): ServoRef {
		const nodeId = this.resolveId(id)
		reserveId(this.state.servoIds, nodeId, "servo")
		const joint = this.state.mates.find((mate) => mate.id === spec.joint.id)
		const jointRange = joint?.type === "revolute" ? joint.limits : undefined
		const commandRange = spec.commandRange ?? jointRange ?? { minDeg: -90, maxDeg: 90 }
		const homeDeg = spec.homeDeg ?? 0
		assertFiniteNumber(commandRange.minDeg, `Servo "${nodeId}" minimum command`)
		assertFiniteNumber(commandRange.maxDeg, `Servo "${nodeId}" maximum command`)
		assertFiniteNumber(homeDeg, `Servo "${nodeId}" home command`)
		if (commandRange.minDeg > commandRange.maxDeg) {
			throw new Error(`Servo "${nodeId}" minimum command cannot exceed its maximum command.`)
		}
		if (homeDeg < commandRange.minDeg || homeDeg > commandRange.maxDeg) {
			throw new Error(`Servo "${nodeId}" home command must be within its command range.`)
		}
		if (spec.maxTorqueNcm !== undefined) {
			assertPositiveNumber(spec.maxTorqueNcm, `Servo "${nodeId}" maximum torque`)
		}
		if (spec.maxSpeedDegPerSec !== undefined) {
			assertPositiveNumber(spec.maxSpeedDegPerSec, `Servo "${nodeId}" maximum speed`)
		}

		this.state.servos.push({
			id: nodeId,
			name: spec.name?.trim() || humanizeId(id),
			componentId: this.path,
			jointId: spec.joint.id,
			homeDeg,
			commandRange: { ...commandRange },
			...(spec.maxTorqueNcm === undefined ? {} : { maxTorqueNcm: spec.maxTorqueNcm }),
			...(spec.maxSpeedDegPerSec === undefined ? {} : { maxSpeedDegPerSec: spec.maxSpeedDegPerSec })
		})
		return { kind: "servo", id: nodeId }
	}

	instance<Props, Result>(id: string, definition: ComponentDefinition<Props, Result>, props: Props, options: InstanceOptions = {}): Result {
		assertIdSegment(id)
		if (definition.kind !== COMPONENT_DEFINITION_KIND) {
			throw new Error(`Component instance "${id}" does not reference a PuppyCAD component definition.`)
		}
		const nodeId = `${this.path}/${id}`
		reserveId(this.state.componentIds, nodeId, "component")
		const localTransform = resolveTransform(options)
		const worldTransform = composeTransforms(this.worldTransform, localTransform)
		this.state.components.push({
			id: nodeId,
			name: options.name?.trim() || definition.name,
			parentId: this.path,
			localTransform,
			worldTransform
		})
		return definition.build(new ModelScope(this.state, nodeId, worldTransform), props)
	}

	private resolveId(id: string): string {
		assertIdSegment(id)
		return `${this.path}/${id}`
	}
}

export function component<Props, Result>(name: string, build: (scope: ModelScope, props: Props) => Result): ComponentDefinition<Props, Result> {
	if (!name.trim()) {
		throw new Error("Component name cannot be empty.")
	}
	return {
		kind: COMPONENT_DEFINITION_KIND,
		name: name.trim(),
		build
	}
}

export function defineModel(options: DefineModelOptions, build: (model: ModelScope) => void): ModelDefinition {
	assertIdPath(options.id)
	if (!options.name.trim()) {
		throw new Error("Model name cannot be empty.")
	}
	const rootTransform = cloneTransform(IDENTITY_TRANSFORM)
	const state: BuilderState = {
		components: [
			{
				id: options.id,
				name: options.name.trim(),
				parentId: null,
				localTransform: cloneTransform(rootTransform),
				worldTransform: cloneTransform(rootTransform)
			}
		],
		bodies: [],
		frames: [],
		mates: [],
		servos: [],
		componentIds: new Set([options.id]),
		bodyIds: new Set(),
		frameIds: new Set(),
		mateIds: new Set(),
		servoIds: new Set()
	}

	build(new ModelScope(state, options.id, rootTransform))
	const definition: ModelDefinition = {
		kind: MODEL_DEFINITION_KIND,
		version: 1,
		id: options.id,
		name: options.name.trim(),
		units: options.units ?? "mm",
		...(options.metadata ? { metadata: { ...options.metadata } } : {}),
		components: state.components,
		bodies: state.bodies,
		frames: state.frames,
		mates: state.mates,
		servos: state.servos
	}
	validateModel(definition)
	return definition
}

export function isModelDefinition(value: unknown): value is ModelDefinition {
	if (!value || typeof value !== "object") {
		return false
	}
	const candidate = value as Partial<ModelDefinition>
	return (
		candidate.kind === MODEL_DEFINITION_KIND &&
		candidate.version === 1 &&
		typeof candidate.id === "string" &&
		typeof candidate.name === "string" &&
		candidate.units === "mm" &&
		Array.isArray(candidate.components) &&
		Array.isArray(candidate.bodies) &&
		Array.isArray(candidate.frames) &&
		Array.isArray(candidate.mates) &&
		Array.isArray(candidate.servos)
	)
}

export function validateModel(model: ModelDefinition): void {
	if (!isModelDefinition(model)) {
		throw new Error("Value is not a PuppyCAD TypeScript model definition.")
	}
	assertIdPath(model.id)
	if (!model.name.trim()) {
		throw new Error("Model name cannot be empty.")
	}
	const componentIds = uniqueIds(model.components, "component")
	const bodyIds = uniqueIds(model.bodies, "body")
	const frameIds = uniqueIds(model.frames, "frame")
	const mateIds = uniqueIds(model.mates, "mate")
	uniqueIds(model.servos, "servo")

	if (!componentIds.has(model.id)) {
		throw new Error(`Model root component "${model.id}" is missing.`)
	}
	for (const instance of model.components) {
		assertIdPath(instance.id)
		if (instance.parentId !== null && !componentIds.has(instance.parentId)) {
			throw new Error(`Component "${instance.id}" references missing parent "${instance.parentId}".`)
		}
		validateResolvedTransform(instance.localTransform, `Component "${instance.id}" local transform`)
		validateResolvedTransform(instance.worldTransform, `Component "${instance.id}" world transform`)
	}
	for (const body of model.bodies) {
		assertIdPath(body.id)
		if (!componentIds.has(body.componentId)) {
			throw new Error(`Body "${body.id}" references missing component "${body.componentId}".`)
		}
		assertPositiveNumber(body.depth, `Body "${body.id}" depth`)
		if (body.outline.length < 3) {
			throw new Error(`Body "${body.id}" outline must contain at least three points.`)
		}
		body.outline.forEach((point, index) => assertFinitePoint(point, `Body "${body.id}" outline point ${index + 1}`))
	}
	for (const frame of model.frames) {
		assertIdPath(frame.id)
		if (!componentIds.has(frame.componentId)) {
			throw new Error(`Frame "${frame.id}" references missing component "${frame.componentId}".`)
		}
		if (frame.bodyId !== null && !bodyIds.has(frame.bodyId)) {
			throw new Error(`Frame "${frame.id}" references missing body "${frame.bodyId}".`)
		}
		assertFiniteVector(frame.position, `Frame "${frame.id}" position`)
		assertFiniteNumber(frame.rotationDeg, `Frame "${frame.id}" rotation`)
	}
	for (const mate of model.mates) {
		assertIdPath(mate.id)
		if (!componentIds.has(mate.componentId)) {
			throw new Error(`Mate "${mate.id}" references missing component "${mate.componentId}".`)
		}
		if (!frameIds.has(mate.parentFrameId) || !frameIds.has(mate.childFrameId)) {
			throw new Error(`Mate "${mate.id}" references a missing frame.`)
		}
		if (mate.type === "revolute") {
			assertFiniteVector(mate.axis, `Revolute mate "${mate.id}" axis`)
			if (Math.hypot(mate.axis.x, mate.axis.y, mate.axis.z) <= Number.EPSILON) {
				throw new Error(`Revolute mate "${mate.id}" axis must be non-zero.`)
			}
			if (mate.limits) {
				assertFiniteNumber(mate.limits.minDeg, `Revolute mate "${mate.id}" minimum limit`)
				assertFiniteNumber(mate.limits.maxDeg, `Revolute mate "${mate.id}" maximum limit`)
				if (mate.limits.minDeg > mate.limits.maxDeg) {
					throw new Error(`Revolute mate "${mate.id}" minimum limit cannot exceed its maximum limit.`)
				}
			}
		}
	}
	for (const servo of model.servos) {
		assertIdPath(servo.id)
		if (!componentIds.has(servo.componentId)) {
			throw new Error(`Servo "${servo.id}" references missing component "${servo.componentId}".`)
		}
		if (!mateIds.has(servo.jointId) || model.mates.find((mate) => mate.id === servo.jointId)?.type !== "revolute") {
			throw new Error(`Servo "${servo.id}" must reference a revolute mate.`)
		}
		assertFiniteNumber(servo.homeDeg, `Servo "${servo.id}" home command`)
		assertFiniteNumber(servo.commandRange.minDeg, `Servo "${servo.id}" minimum command`)
		assertFiniteNumber(servo.commandRange.maxDeg, `Servo "${servo.id}" maximum command`)
		if (servo.commandRange.minDeg > servo.commandRange.maxDeg) {
			throw new Error(`Servo "${servo.id}" minimum command cannot exceed its maximum command.`)
		}
		if (servo.homeDeg < servo.commandRange.minDeg || servo.homeDeg > servo.commandRange.maxDeg) {
			throw new Error(`Servo "${servo.id}" home command must be within its command range.`)
		}
		if (servo.maxTorqueNcm !== undefined) {
			assertPositiveNumber(servo.maxTorqueNcm, `Servo "${servo.id}" maximum torque`)
		}
		if (servo.maxSpeedDegPerSec !== undefined) {
			assertPositiveNumber(servo.maxSpeedDegPerSec, `Servo "${servo.id}" maximum speed`)
		}
	}
}

export function compileModel(model: ModelDefinition): CompiledModel {
	validateModel(model)
	const items = [...model.bodies.map(bodyToProjectNode), modelToAssemblyProjectNode(model)]
	return {
		graph: model,
		project: createProjectFile({
			items,
			selectedPath: [items.length - 1],
			revision: 0
		})
	}
}

export function serializeModelGraph(model: ModelDefinition): string {
	validateModel(model)
	return JSON.stringify(model, null, 2)
}

export function v2(x: number, y: number): Point2D {
	assertFiniteNumber(x, "Vector x")
	assertFiniteNumber(y, "Vector y")
	return { x, y }
}

export function add2(a: Point2D, b: Point2D): Point2D {
	return v2(a.x + b.x, a.y + b.y)
}

export function subtract2(a: Point2D, b: Point2D): Point2D {
	return v2(a.x - b.x, a.y - b.y)
}

export function scale2(vector: Point2D, factor: number): Point2D {
	assertFiniteNumber(factor, "Vector scale")
	return v2(vector.x * factor, vector.y * factor)
}

export function rotate2(vector: Point2D, angleDeg: number): Point2D {
	assertFiniteNumber(angleDeg, "Vector rotation")
	const angle = degreesToRadians(angleDeg)
	const cos = Math.cos(angle)
	const sin = Math.sin(angle)
	return v2(vector.x * cos - vector.y * sin, vector.x * sin + vector.y * cos)
}

export function normalize2(vector: Point2D): Point2D {
	const magnitude = Math.hypot(vector.x, vector.y)
	if (magnitude <= Number.EPSILON) {
		throw new Error("Cannot normalize a zero-length vector.")
	}
	return scale2(vector, 1 / magnitude)
}

export function directionFromXAxis(angleDeg: number): Point2D {
	return rotate2(v2(1, 0), angleDeg)
}

export function directionFromYAxis(angleDeg: number): Point2D {
	return rotate2(v2(0, 1), angleDeg)
}

export function rectangle(center: Point2D, width: number, height: number, rotateDeg = 0): Point2D[] {
	assertPositiveNumber(width, "Rectangle width")
	assertPositiveNumber(height, "Rectangle height")
	const halfWidth = width / 2
	const halfHeight = height / 2
	return [v2(halfWidth, halfHeight), v2(-halfWidth, halfHeight), v2(-halfWidth, -halfHeight), v2(halfWidth, -halfHeight)].map((point) => add2(center, rotate2(point, rotateDeg)))
}

export function roundedRectangle(center: Point2D, width: number, height: number, radius: number, rotateDeg = 0, cornerSegments = 4): Point2D[] {
	assertPositiveNumber(width, "Rounded rectangle width")
	assertPositiveNumber(height, "Rounded rectangle height")
	assertPositiveNumber(radius, "Rounded rectangle radius")
	assertSegmentCount(cornerSegments, "Rounded rectangle corner segments")
	const halfWidth = width / 2
	const halfHeight = height / 2
	const clampedRadius = Math.min(radius, halfWidth, halfHeight)
	const corners = [
		{ center: v2(halfWidth - clampedRadius, halfHeight - clampedRadius), start: 0 },
		{ center: v2(-halfWidth + clampedRadius, halfHeight - clampedRadius), start: Math.PI / 2 },
		{ center: v2(-halfWidth + clampedRadius, -halfHeight + clampedRadius), start: Math.PI },
		{ center: v2(halfWidth - clampedRadius, -halfHeight + clampedRadius), start: (3 * Math.PI) / 2 }
	]
	const outline: Point2D[] = []
	for (const corner of corners) {
		for (let index = 0; index <= cornerSegments; index += 1) {
			const angle = corner.start + (index / cornerSegments) * (Math.PI / 2)
			const local = v2(corner.center.x + Math.cos(angle) * clampedRadius, corner.center.y + Math.sin(angle) * clampedRadius)
			outline.push(add2(center, rotate2(local, rotateDeg)))
		}
	}
	return outline
}

export function circle(center: Point2D, radius: number, segments = 24): Point2D[] {
	assertPositiveNumber(radius, "Circle radius")
	assertSegmentCount(segments, "Circle segments", 3)
	return Array.from({ length: segments }, (_, index) => {
		const angle = (index / segments) * Math.PI * 2
		return v2(center.x + Math.cos(angle) * radius, center.y + Math.sin(angle) * radius)
	})
}

export function capsule(from: Point2D, to: Point2D, width: number, arcSegments = 7): Point2D[] {
	assertPositiveNumber(width, "Capsule width")
	assertSegmentCount(arcSegments, "Capsule arc segments")
	const axis = normalize2(subtract2(to, from))
	const axisAngle = Math.atan2(axis.y, axis.x)
	const radius = width / 2
	const outline: Point2D[] = []
	for (let index = 0; index <= arcSegments; index += 1) {
		const angle = axisAngle - Math.PI / 2 + (index / arcSegments) * Math.PI
		outline.push(v2(to.x + Math.cos(angle) * radius, to.y + Math.sin(angle) * radius))
	}
	for (let index = 0; index <= arcSegments; index += 1) {
		const angle = axisAngle + Math.PI / 2 + (index / arcSegments) * Math.PI
		outline.push(v2(from.x + Math.cos(angle) * radius, from.y + Math.sin(angle) * radius))
	}
	return outline
}

function bodyToProjectNode(body: BodyNode): ProjectNode {
	const sketchId = `${body.id}/outline`
	const sketch = materializeSketch({
		type: "sketch",
		id: sketchId,
		name: `${body.name} outline`,
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: body.outline.map((p0, index) => ({
			id: `${sketchId}/edge-${index + 1}`,
			type: "line" as const,
			p0: { ...p0 },
			p1: { ...(body.outline[(index + 1) % body.outline.length] as Point2D) }
		})),
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	} satisfies Sketch)
	const profile = sketch.profiles[0]
	if (!profile) {
		throw new Error(`Body "${body.id}" did not produce a closed PuppyCAD profile.`)
	}
	const extrude: SolidExtrude = {
		type: "extrude",
		id: `${body.id}/extrude`,
		name: `${body.name} solid`,
		target: {
			type: "profileRef",
			sketchId,
			profileId: profile.id
		},
		depth: body.depth
	}
	return {
		id: body.id,
		type: "part",
		name: body.name,
		visible: true,
		data: {
			features: [sketch, extrude]
		}
	}
}

function modelToAssemblyProjectNode(model: ModelDefinition): ProjectNode {
	const frameById = new Map(model.frames.map((frame) => [frame.id, frame] as const))
	const mates = model.mates.map((mate) => modelMateToAssemblyMate(mate, frameById))
	const data: Assembly = {
		id: `${model.id}/assembly`,
		name: model.name,
		instances: model.bodies.map((body) => ({
			id: body.id,
			partId: body.id
		})),
		connectors: model.frames.map((frame) => ({
			id: frame.id,
			name: frame.name,
			instanceId: frame.bodyId,
			position: { ...frame.position },
			rotation: { x: 0, y: 0, z: frame.rotationDeg }
		})),
		...(mates.length > 0 ? { mates } : {}),
		...(model.servos.length > 0
			? {
					actuators: model.servos.map((servo) => ({
						id: servo.id,
						name: servo.name,
						type: "servo" as const,
						mateId: servo.jointId,
						homeDeg: servo.homeDeg,
						commandRange: { ...servo.commandRange },
						...(servo.maxTorqueNcm === undefined ? {} : { maxTorqueNcm: servo.maxTorqueNcm }),
						...(servo.maxSpeedDegPerSec === undefined ? {} : { maxSpeedDegPerSec: servo.maxSpeedDegPerSec })
					}))
				}
			: {}),
		...(model.metadata ? { variables: metadataToVariables(model.metadata) } : {})
	}
	return {
		id: data.id,
		type: "assembly",
		name: data.name,
		visible: true,
		data
	}
}

function metadataToVariables(metadata: ModelMetadata): Variables {
	const variables: Variables = {}
	for (const [key, value] of Object.entries(metadata)) {
		if (value !== null) {
			variables[key] = value
		}
	}
	return variables
}

function modelMateToAssemblyMate(mate: MateNode, frameById: ReadonlyMap<string, FrameNode>): AssemblyMate {
	const parent = frameById.get(mate.parentFrameId)
	const child = frameById.get(mate.childFrameId)
	if (!parent || !child) {
		throw new Error(`Mate "${mate.id}" references a frame that is not present in the model.`)
	}
	const params: Variables = {
		componentId: mate.componentId
	}
	if (mate.type === "revolute") {
		params.axisX = mate.axis.x
		params.axisY = mate.axis.y
		params.axisZ = mate.axis.z
		if (mate.limits) {
			params.minDeg = mate.limits.minDeg
			params.maxDeg = mate.limits.maxDeg
		}
	}
	return {
		id: mate.id,
		name: mate.name,
		type: mate.type === "fixed" ? "fasten" : "revolute",
		a: {
			instanceId: parent.bodyId,
			connectorId: parent.id
		},
		b: {
			instanceId: child.bodyId,
			connectorId: child.id
		},
		params
	}
}

function point3(point: Point2D | Vector3D): Vector3D {
	return { x: point.x, y: point.y, z: "z" in point ? point.z : 0 }
}

function transformPoint<T extends Point2D>(point: T, transform: ResolvedPlanarTransform): Point2D {
	return add2(rotate2(point, transform.rotateDeg), transform.translate)
}

function rotateVector3(vector: Vector3D, angleDeg: number): Vector3D {
	const rotated = rotate2(vector, angleDeg)
	return { x: rotated.x, y: rotated.y, z: vector.z }
}

function resolveTransform(transform: PlanarTransform): ResolvedPlanarTransform {
	const translate = transform.translate ?? { x: 0, y: 0 }
	assertFinitePoint(translate, "Component translation")
	const rotateDeg = transform.rotateDeg ?? 0
	assertFiniteNumber(rotateDeg, "Component rotation")
	return {
		translate: { ...translate },
		rotateDeg
	}
}

function composeTransforms(parent: ResolvedPlanarTransform, child: ResolvedPlanarTransform): ResolvedPlanarTransform {
	return {
		translate: transformPoint(child.translate, parent),
		rotateDeg: parent.rotateDeg + child.rotateDeg
	}
}

function cloneTransform(transform: ResolvedPlanarTransform): ResolvedPlanarTransform {
	return {
		translate: { ...transform.translate },
		rotateDeg: transform.rotateDeg
	}
}

function validateResolvedTransform(transform: ResolvedPlanarTransform, label: string): void {
	assertFinitePoint(transform.translate, `${label} translation`)
	assertFiniteNumber(transform.rotateDeg, `${label} rotation`)
}

function uniqueIds(nodes: readonly { id: string }[], kind: string): Set<string> {
	const ids = new Set<string>()
	for (const node of nodes) {
		if (ids.has(node.id)) {
			throw new Error(`Duplicate ${kind} id "${node.id}".`)
		}
		ids.add(node.id)
	}
	return ids
}

function reserveId(ids: Set<string>, id: string, kind: string): void {
	if (ids.has(id)) {
		throw new Error(`Duplicate ${kind} id "${id}".`)
	}
	ids.add(id)
}

function assertIdSegment(id: string): void {
	if (!ID_SEGMENT_PATTERN.test(id)) {
		throw new Error(`Invalid model id segment "${id}". Use letters, numbers, dots, underscores, or hyphens.`)
	}
}

function assertIdPath(id: string): void {
	if (!ID_PATH_PATTERN.test(id)) {
		throw new Error(`Invalid model id path "${id}".`)
	}
}

function assertFinitePoint(point: Point2D, label: string): void {
	assertFiniteNumber(point.x, `${label} x`)
	assertFiniteNumber(point.y, `${label} y`)
}

function assertFiniteVector(vector: Vector3D, label: string): void {
	assertFinitePoint(vector, label)
	assertFiniteNumber(vector.z, `${label} z`)
}

function assertFiniteNumber(value: number, label: string): void {
	if (!Number.isFinite(value)) {
		throw new Error(`${label} must be a finite number.`)
	}
}

function assertPositiveNumber(value: number, label: string): void {
	if (!Number.isFinite(value) || value <= 0) {
		throw new Error(`${label} must be a positive finite number.`)
	}
}

function assertSegmentCount(value: number, label: string, minimum = 1): void {
	if (!Number.isInteger(value) || value < minimum) {
		throw new Error(`${label} must be an integer greater than or equal to ${minimum}.`)
	}
}

function humanizeId(id: string): string {
	const value = id.replaceAll(/[-_.]+/g, " ")
	return `${value.charAt(0).toUpperCase()}${value.slice(1)}`
}

function degreesToRadians(value: number): number {
	return (value * Math.PI) / 180
}
