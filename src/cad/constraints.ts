import type { AnchorRef, CenterPointCircle, LineEntity, SketchConstraint, SketchDimension, SketchEntity, SketchEntityId, SketchFeature } from "../contract"
import type { Point2D } from "../types"

const DEFAULT_ITERATIONS = 8

export interface SolveSketchConstraintsOptions {
	iterations?: number
}

export interface SolveSketchConstraintsResult {
	sketch: SketchFeature
	unsupportedConstraints: SketchConstraint[]
	unsupportedDimensions: SketchDimension[]
}

type SketchConstraintInput = Pick<SketchFeature, "id" | "type" | "target" | "entities" | "profiles" | "constraints" | "dimensions">

type ResolvedAnchor = {
	entity: SketchEntity
	anchor: string
	point: Point2D
}

export function solveSketchConstraints(sketch: SketchConstraintInput, options: SolveSketchConstraintsOptions = {}): SolveSketchConstraintsResult {
	const solvedSketch = structuredClone(sketch) as SketchFeature
	const unsupportedConstraints: SketchConstraint[] = []
	const unsupportedDimensions: SketchDimension[] = []
	const iterations = normalizeIterations(options.iterations)

	solvedSketch.constraints ??= []
	solvedSketch.dimensions ??= []
	solvedSketch.entities = solvedSketch.entities.map((entity, index) => ({
		...entity,
		id: entity.id ?? `${solvedSketch.id}-entity-${index + 1}`
	}))

	for (let iteration = 0; iteration < iterations; iteration += 1) {
		const entityMap = buildEntityMap(solvedSketch.entities)

		for (const constraint of solvedSketch.constraints) {
			if (!applyConstraint(constraint, entityMap) && !unsupportedConstraints.includes(constraint)) {
				unsupportedConstraints.push(constraint)
			}
		}

		for (const dimension of solvedSketch.dimensions) {
			if (!applyDimension(dimension, entityMap) && !unsupportedDimensions.includes(dimension)) {
				unsupportedDimensions.push(dimension)
			}
		}
	}

	return {
		sketch: solvedSketch,
		unsupportedConstraints,
		unsupportedDimensions
	}
}

function buildEntityMap(entities: SketchEntity[]): Map<SketchEntityId, SketchEntity> {
	const entityMap = new Map<SketchEntityId, SketchEntity>()
	for (const entity of entities) {
		if (entity.id) {
			entityMap.set(entity.id, entity)
		}
	}
	return entityMap
}

function applyConstraint(constraint: SketchConstraint, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	switch (constraint.type) {
		case "coincident":
			return applyCoincidentConstraint(constraint.a, constraint.b, entityMap)
		case "horizontal":
			return applyHorizontalConstraint(constraint.e, entityMap)
		case "vertical":
			return applyVerticalConstraint(constraint.e, entityMap)
		case "equal":
			return applyEqualConstraint(constraint.eA, constraint.eB, entityMap)
		case "concentric":
			return applyConcentricConstraint(constraint.a, constraint.b, entityMap)
		case "midpoint":
			return applyMidpointConstraint(constraint.point, constraint.on, entityMap)
		case "parallel":
		case "perpendicular":
		case "tangent":
		case "symmetry":
			return false
		default:
			return assertNever(constraint)
	}
}

function applyDimension(dimension: SketchDimension, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	switch (dimension.type) {
		case "distance":
			return applyDistanceDimension(dimension.between[0], dimension.between[1], dimension.value, entityMap)
		case "radius":
			return applyCircleRadiusDimension(dimension.of, dimension.value, entityMap)
		case "diameter":
			return applyCircleRadiusDimension(dimension.of, dimension.value / 2, entityMap)
		case "angle":
			return false
		default:
			return assertNever(dimension)
	}
}

function applyCoincidentConstraint(sourceRef: AnchorRef, targetRef: AnchorRef, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	const source = resolveAnchor(sourceRef, entityMap)
	const target = resolveAnchor(targetRef, entityMap)
	if (!source || !target) {
		return false
	}

	target.point.x = source.point.x
	target.point.y = source.point.y
	return true
}

function applyHorizontalConstraint(entityId: SketchEntityId, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	const entity = entityMap.get(entityId)
	if (!isLineEntity(entity)) {
		return false
	}

	entity.p1.y = entity.p0.y
	return true
}

function applyVerticalConstraint(entityId: SketchEntityId, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	const entity = entityMap.get(entityId)
	if (!isLineEntity(entity)) {
		return false
	}

	entity.p1.x = entity.p0.x
	return true
}

function applyEqualConstraint(sourceEntityId: SketchEntityId, targetEntityId: SketchEntityId, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	const sourceEntity = entityMap.get(sourceEntityId)
	const targetEntity = entityMap.get(targetEntityId)

	if (isLineEntity(sourceEntity) && isLineEntity(targetEntity)) {
		const sourceLength = getPointDistance(sourceEntity.p0, sourceEntity.p1)
		setLineLength(targetEntity, sourceLength)
		return true
	}

	if (isCenterPointCircle(sourceEntity) && isCenterPointCircle(targetEntity)) {
		targetEntity.radius = sourceEntity.radius
		return true
	}

	return false
}

function applyConcentricConstraint(sourceEntityId: SketchEntityId, targetEntityId: SketchEntityId, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	const sourceEntity = entityMap.get(sourceEntityId)
	const targetEntity = entityMap.get(targetEntityId)
	if (!hasCenterPoint(sourceEntity) || !hasCenterPoint(targetEntity)) {
		return false
	}

	targetEntity.center.x = sourceEntity.center.x
	targetEntity.center.y = sourceEntity.center.y
	return true
}

function applyMidpointConstraint(pointRef: AnchorRef, lineEntityId: SketchEntityId, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	const targetAnchor = resolveAnchor(pointRef, entityMap)
	const lineEntity = entityMap.get(lineEntityId)
	if (!targetAnchor || !isLineEntity(lineEntity)) {
		return false
	}

	targetAnchor.point.x = (lineEntity.p0.x + lineEntity.p1.x) / 2
	targetAnchor.point.y = (lineEntity.p0.y + lineEntity.p1.y) / 2
	return true
}

function applyDistanceDimension(sourceRef: AnchorRef, targetRef: AnchorRef, value: number, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	if (!Number.isFinite(value) || value < 0) {
		return false
	}

	const source = resolveAnchor(sourceRef, entityMap)
	const target = resolveAnchor(targetRef, entityMap)
	if (!source || !target) {
		return false
	}

	const dx = target.point.x - source.point.x
	const dy = target.point.y - source.point.y
	const currentLength = Math.hypot(dx, dy)
	const directionX = currentLength > 0 ? dx / currentLength : 1
	const directionY = currentLength > 0 ? dy / currentLength : 0

	target.point.x = source.point.x + directionX * value
	target.point.y = source.point.y + directionY * value
	return true
}

function applyCircleRadiusDimension(entityId: SketchEntityId, radius: number, entityMap: Map<SketchEntityId, SketchEntity>): boolean {
	if (!Number.isFinite(radius) || radius < 0) {
		return false
	}

	const entity = entityMap.get(entityId)
	if (!isCenterPointCircle(entity)) {
		return false
	}

	entity.radius = radius
	return true
}

function resolveAnchor(anchorRef: AnchorRef, entityMap: Map<SketchEntityId, SketchEntity>): ResolvedAnchor | null {
	const separatorIndex = anchorRef.indexOf(":")
	if (separatorIndex <= 0 || separatorIndex === anchorRef.length - 1) {
		return null
	}

	const entityId = anchorRef.slice(0, separatorIndex)
	const anchor = anchorRef.slice(separatorIndex + 1)
	const entity = entityMap.get(entityId)
	if (!entity) {
		return null
	}

	const point = getAnchorPoint(entity, anchor)
	if (!point) {
		return null
	}

	return { entity, anchor, point }
}

function getAnchorPoint(entity: SketchEntity, anchor: string): Point2D | null {
	switch (entity.type) {
		case "line":
		case "cornerRectangle":
		case "alignedRectangle":
			if (anchor === "p0") {
				return entity.p0
			}

			if (anchor === "p1") {
				return entity.p1
			}

			return null
		case "centeredRectangle":
		case "centerPointCircle":
			return anchor === "center" ? entity.center : null
		case "midpointLine":
			return anchor === "midpoint" ? entity.midpoint : null
		default:
			return assertNever(entity)
	}
}

function hasCenterPoint(entity: SketchEntity | undefined): entity is Extract<SketchEntity, { center: Point2D }> {
	return !!entity && "center" in entity
}

function isLineEntity(entity: SketchEntity | undefined): entity is LineEntity {
	return !!entity && entity.type === "line"
}

function isCenterPointCircle(entity: SketchEntity | undefined): entity is CenterPointCircle {
	return !!entity && entity.type === "centerPointCircle"
}

function getPointDistance(a: Point2D, b: Point2D): number {
	return Math.hypot(b.x - a.x, b.y - a.y)
}

function setLineLength(line: LineEntity, length: number): void {
	const dx = line.p1.x - line.p0.x
	const dy = line.p1.y - line.p0.y
	const currentLength = Math.hypot(dx, dy)
	const directionX = currentLength > 0 ? dx / currentLength : 1
	const directionY = currentLength > 0 ? dy / currentLength : 0

	line.p1.x = line.p0.x + directionX * length
	line.p1.y = line.p0.y + directionY * length
}

function normalizeIterations(value: number | undefined): number {
	if (!Number.isFinite(value) || value === undefined) {
		return DEFAULT_ITERATIONS
	}

	return Math.max(1, Math.round(value))
}

function assertNever(value: never): never {
	throw new Error(`Unsupported sketch constraint value: ${JSON.stringify(value)}`)
}
