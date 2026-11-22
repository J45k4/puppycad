import { normalizeEllipticArc } from "./sketch-elliptic-arc"
import { evaluateSketchVariables, type SketchVariable } from "./sketch-variables"
import { parseSketchValue } from "./sketch-value"
import { normalizeSpline } from "./sketch-spline"
import { sketchRelationEntityIds } from "./sketch-relations"
import { analyzeSketchJacobian } from "./sketch-redundancy"
import { sketchNormalResidual } from "./sketch-normal"
import { chamferSetbacks, chamferSecondSetback, type ChamferMode } from "./sketch-chamfer"
import { isMeasuredDimension, measureSketchDimension } from "./sketch-dimensions"
import { ellipseSupportPoint } from "./sketch-ellipse"
import { synchronizePolygonSides } from "./sketch-polygon"
import { sketchEndpointDirection } from "./sketch-tangent-arc"
import { circularPatternCenter, circularPatternAngle, rotateSketchEntity, type CircularPatternDefinition } from "./sketch-circular-pattern"
import { patternCellOffset, translateSketchEntity, type LinearPatternDefinition } from "./sketch-pattern"
import { mirrorSketchEntity } from "./sketch-mirror"
import { pointOnSketchCurveResidual, sketchCurveMidpoint } from "./sketch-point-constraints"
import { directedOffsetLines, offsetLineChain, type OffsetChainDefinition } from "./sketch-offset-chain"
import { offsetSketchEntity, offsetParameters } from "./sketch-offset"
import { entityAnchorPoint } from "./sketch-curves"
import { requireValue } from "./required"
import type { SketchAnchorName, SketchEntity } from "./schema"
import type { Point2D } from "./types"

export type SketchAnchor = { entityId: string; point: SketchAnchorName }
export type SketchRelation = { id: string; fixation?: string; labelPosition?: Point2D; reference?: boolean; expression?: string } & (
	| LinearPatternDefinition
	| CircularPatternDefinition
	| ({ type: "offsetChain"; targets: string[]; value: number } & OffsetChainDefinition)
	| { type: "midpoint"; a: SketchAnchor; b: string }
	| { type: "endpointTangent"; a: SketchAnchor; b: string; orientation?: { lineSign: 1 | -1; sweepSign: 1 | -1 } }
	| { type: "normal"; a: SketchAnchor; b: string }
	| { type: "pointOnCurve"; a: SketchAnchor; b: string }
	| { type: "mirror"; a: string; b: string; symmetryLine: string }
	| { type: "symmetric"; a: SketchAnchor; b: SketchAnchor; symmetryLine: string }
	| { type: "chamfer"; mode?: ChamferMode; chamferId: string; a: SketchAnchor; b: SketchAnchor; value: number; secondValue: number }
	| { type: "smoothJoin"; a: SketchAnchor; b: SketchAnchor }
	| { type: "coincident"; a: SketchAnchor; b: SketchAnchor }
	| { type: "fixed"; anchor: SketchAnchor; position: Point2D }
	| { type: "distance"; a: SketchAnchor; b: SketchAnchor; value: number; axis?: "x" | "y"; direction?: Point2D }
	| { type: "horizontal" | "vertical"; entityId: string }
	| { type: "length" | "diameter" | "radius" | "width" | "height" | "arcSweep" | "rotation"; entityId: string; value: number }
	| { type: "parallel" | "collinear" | "perpendicular" | "equal" | "sameEllipse" | "concentric" | "tangent" | "internalTangent"; a: string; b: string }
	| { type: "angle" | "radiusDifference" | "offset"; a: string; b: string; value: number }
)
export type SketchEntityConstraintState = "fully-constrained" | "underconstrained" | "conflicting" | "unknown"
export type SketchSolveResult = {
	entityStates: Record<string, SketchEntityConstraintState>
	entities: SketchEntity[]
	status: "underconstrained" | "fully-constrained" | "conflicting"
	degreesOfFreedom: number
	redundantEquations: number
	redundantRelations: string[]
	conflicts: string[]
	maxResidual: number
	iterations: number
}

type Parameter = { read: () => number; write: (value: number) => void }
const norm = (p: Point2D) => Math.hypot(p.x, p.y)
const subtract = (a: Point2D, b: Point2D): Point2D => ({ x: a.x - b.x, y: a.y - b.y })
const cross = (a: Point2D, b: Point2D) => a.x * b.y - a.y * b.x
const dot = (a: Point2D, b: Point2D) => a.x * b.x + a.y * b.y

/** Solve all driving relations together. Input geometry is never mutated, including on failure. */
export function solveSketch(entities: readonly SketchEntity[], inputRelations: readonly SketchRelation[], options: { tolerance?: number; maxIterations?: number } = {}): SketchSolveResult {
	const tolerance = options.tolerance ?? 1e-6
	for (const relation of inputRelations)
		if (relation.reference) {
			if (!isMeasuredDimension(relation)) throw Error("Only dimensional constraints can be reference dimensions.")
			measureSketchDimension(entities, relation)
		}
	const relations = inputRelations.filter((relation) => !relation.reference)

	const geometry = structuredClone(entities) as SketchEntity[]
	for (const e of geometry) {
		if (e.type === "spline") normalizeSpline(e)
		if (e.type === "ellipticArc") normalizeEllipticArc(e)
	}
	synchronizePolygonSides(geometry, relations)
	const byId = new Map(geometry.map((e) => [e.id, e]))
	if (byId.size !== geometry.length) throw Error("Sketch entity IDs must be unique.")
	const parameters: Parameter[] = []
	const entityParameters = new Map<string, { start: number; end: number }>()
	const scalar = <T, K extends keyof T>(object: T, key: K) =>
		parameters.push({
			read: () => Number(object[key]),
			write: (value) => {
				object[key] = value as T[K]
			}
		})
	const pointParameters = (point: Point2D) => {
		scalar(point, "x")
		scalar(point, "y")
	}
	for (const e of geometry) {
		const start = parameters.length
		switch (e.type) {
			case "spline":
				e.points.forEach(pointParameters)
				break
			case "line":
			case "cornerRectangle":
				pointParameters(e.p0)
				pointParameters(e.p1)
				break
			case "arc":
				pointParameters(e.center)
				scalar(e, "radius")
				scalar(e, "startAngle")
				scalar(e, "sweep")
				break
			case "polygon":
				pointParameters(e.center)
				scalar(e, "radius")
				scalar(e, "rotation")
				break
			case "point":
				pointParameters(e.center)
				break
			case "circle":
				pointParameters(e.center)
				scalar(e, "radius")
				break
			case "ellipticArc":
				scalar(e, "startAngle")
				scalar(e, "sweep")
				pointParameters(e.center)
				scalar(e, "width")
				scalar(e, "height")
				scalar(e, "rotation")
				break
			case "ellipse":
			case "rectangle":
				pointParameters(e.center)
				scalar(e, "width")
				scalar(e, "height")
				scalar(e, "rotation")
				break
			case "capsule":
				pointParameters(e.from)
				pointParameters(e.to)
				scalar(e, "width")
				break
		}
		entityParameters.set(e.id, { start, end: parameters.length })
	}
	const entity = (id: string): SketchEntity => {
		const e = byId.get(id)
		if (!e) throw Error(`Missing sketch entity: ${id}`)
		return e
	}
	const point = (ref: SketchAnchor): Point2D => entityAnchorPoint(entity(ref.entityId), ref.point)
	const line = (id: string) => {
		const e = entity(id)
		if (e.type !== "line") throw Error(`${id} must be a line.`)
		return e
	}
	const direction = (id: string) => {
		const e = line(id)
		const d = subtract(e.p1, e.p0)
		const l = norm(d)
		return { x: d.x / Math.max(l, 1e-12), y: d.y / Math.max(l, 1e-12) }
	}
	const circle = (id: string) => {
		const e = entity(id)
		if (e.type !== "circle" && e.type !== "arc") throw Error(`${id} must be a circle or arc.`)
		return e
	}
	const size = (id: string) => {
		const e = entity(id)
		if (e.type === "circle" || e.type === "arc") return e.radius
		if (e.type === "line") return norm(subtract(e.p1, e.p0))
		throw Error("Equal requires two lines or two circles.")
	}
	const relationIds = new Set<string>()
	for (const r of relations) {
		if (!r.id || relationIds.has(r.id)) throw Error("Sketch relation IDs must be unique and nonempty.")
		relationIds.add(r.id)
		if ("value" in r && (!Number.isFinite(r.value) || (["length", "diameter", "radius", "width", "height"].includes(r.type) && r.value <= 0))) throw Error(`Invalid dimension: ${r.id}`)
		if (r.type === "fixed" && (!Number.isFinite(r.position.x) || !Number.isFinite(r.position.y))) throw Error(`Invalid fixed point: ${r.id}`)
	}
	const residual = (r: SketchRelation): number[] => {
		switch (r.type) {
			case "fixed": {
				const p = subtract(point(r.anchor), r.position)
				return [p.x, p.y]
			}
			case "chamfer": {
				line(r.chamferId)
				if (![r.value, r.secondValue].every((value) => Number.isFinite(value) && value > 0)) throw Error("Chamfer setbacks must be positive.")
				if ((r.a.point !== "p0" && r.a.point !== "p1") || (r.b.point !== "p0" && r.b.point !== "p1")) throw Error("Chamfer requires line endpoints.")
				const values = chamferSetbacks(line(r.a.entityId), r.a.point, line(r.b.entityId), r.b.point)
				const second = r.mode === "distance-angle" ? chamferSecondSetback(line(r.a.entityId), r.a.point, line(r.b.entityId), r.b.point, r.value, r.secondValue) : r.secondValue
				return [values[0] - r.value, values[1] - second]
			}
			case "smoothJoin": {
				const a = sketchEndpointDirection(entity(r.a.entityId), r.a.point)
				const b = sketchEndpointDirection(entity(r.b.entityId), r.b.point)
				const d = subtract(point(r.a), point(r.b))
				return [d.x, d.y, a.x + b.x, a.y + b.y]
			}
			case "endpointTangent": {
				const arc = entity(r.a.entityId)
				if (arc.type !== "arc" || (r.a.point !== "p0" && r.a.point !== "p1")) throw Error("Endpoint tangency requires an arc endpoint and a line.")
				const radial = subtract(point(r.a), arc.center)
				const unit = direction(r.b)
				const residuals = [dot(radial, unit)]
				if (r.orientation) {
					const tangent = { x: (-radial.y * Math.sign(arc.sweep)) / arc.radius, y: (radial.x * Math.sign(arc.sweep)) / arc.radius }
					residuals.push(Math.min(0, r.orientation.lineSign * dot(tangent, unit)) * arc.radius)
				}
				return residuals
			}
			case "normal": {
				if (r.a.point !== "p0" && r.a.point !== "p1") throw Error("Normal requires a line endpoint.")
				return sketchNormalResidual(line(r.a.entityId), r.a.point, entity(r.b))
			}
			case "midpoint":
			case "pointOnCurve": {
				const p = point(r.a)
				const curve = entity(r.b)
				if (r.type === "pointOnCurve") return pointOnSketchCurveResidual(curve, p)
				const target = sketchCurveMidpoint(curve)
				return [p.x - target.x, p.y - target.y]
			}
			case "symmetric": {
				const axis = line(r.symmetryLine)
				if (norm(subtract(axis.p1, axis.p0)) < 1e-9) throw Error("Symmetry requires a nonzero axis line.")
				const unit = direction(r.symmetryLine)
				const a = point(r.a)
				const b = point(r.b)
				const midpoint = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }
				return [cross(subtract(midpoint, axis.p0), unit), dot(subtract(a, b), unit)]
			}
			case "coincident": {
				const p = subtract(point(r.a), point(r.b))
				return [p.x, p.y]
			}
			case "distance": {
				const d = subtract(point(r.b), point(r.a))
				return [(r.direction ? dot(d, r.direction) : r.axis ? d[r.axis] : norm(d)) - r.value]
			}
			case "horizontal": {
				const e = line(r.entityId)
				return [e.p1.y - e.p0.y]
			}
			case "vertical": {
				const e = line(r.entityId)
				return [e.p1.x - e.p0.x]
			}
			case "length": {
				const e = line(r.entityId)
				return [norm(subtract(e.p1, e.p0)) - r.value]
			}
			case "rotation": {
				const rectangle = entity(r.entityId)
				if (rectangle.type === "line") {
					const angle = Math.atan2(rectangle.p1.y - rectangle.p0.y, rectangle.p1.x - rectangle.p0.x) - (r.value * Math.PI) / 180
					return [Math.atan2(Math.sin(angle), Math.cos(angle))]
				}
				if (rectangle.type !== "ellipticArc" && rectangle.type !== "ellipse" && rectangle.type !== "rectangle" && rectangle.type !== "polygon")
					throw Error("Rotation requires a native rectangle, polygon or ellipse.")
				return [rectangle.rotation - r.value]
			}
			case "arcSweep": {
				const e = entity(r.entityId)
				if ((e.type !== "arc" && e.type !== "ellipticArc") || Math.abs(r.value) <= 0 || Math.abs(r.value) >= 360)
					throw Error("Arc sweep must be between -360 and 360 degrees, excluding zero.")
				return [e.sweep - (r.value * Math.PI) / 180]
			}
			case "radius":
				return [(entity(r.entityId).type === "polygon" ? (entity(r.entityId) as Extract<SketchEntity, { type: "polygon" }>).radius : circle(r.entityId).radius) - r.value]
			case "diameter":
				return [circle(r.entityId).radius * 2 - r.value]
			case "width":
			case "height": {
				const e = entity(r.entityId)
				if (e.type === "ellipse" || e.type === "rectangle" || e.type === "ellipticArc") return [e[r.type] - r.value]
				if (e.type === "cornerRectangle") return [Math.abs(e.p1[r.type === "width" ? "x" : "y"] - e.p0[r.type === "width" ? "x" : "y"]) - r.value]
				if (e.type === "capsule" && r.type === "width") return [e.width - r.value]
				throw Error(`${r.type} requires a compatible rectangle or slot.`)
			}
			case "sameEllipse": {
				const a = entity(r.a)
				const b = entity(r.b)
				if ((a.type !== "ellipse" && a.type !== "ellipticArc") || (b.type !== "ellipse" && b.type !== "ellipticArc")) throw Error("Shared ellipse requires elliptical curves.")
				const rotation = ((a.rotation - b.rotation) * Math.PI) / 180
				return [a.center.x - b.center.x, a.center.y - b.center.y, a.width - b.width, a.height - b.height, Math.atan2(Math.sin(rotation), Math.cos(rotation))]
			}
			case "concentric": {
				const a = entity(r.a)
				const b = entity(r.b)
				if (
					(a.type !== "circle" && a.type !== "arc" && a.type !== "ellipse" && a.type !== "ellipticArc") ||
					(b.type !== "circle" && b.type !== "arc" && b.type !== "ellipse" && b.type !== "ellipticArc")
				)
					throw Error("Concentric requires circles, arcs, ellipses or elliptic arcs.")
				const d = subtract(a.center, b.center)
				return [d.x, d.y]
			}
			case "circularPattern":
				return r.instances.flatMap((instance, index) =>
					instance.flatMap((id, sourceIndex) => {
						const source = entity(requireValue(r.sources[sourceIndex]))
						const target = entity(id)
						const expectedEntity = rotateSketchEntity(source, id, circularPatternCenter(r, geometry), circularPatternAngle(r, index + 1))
						if (
							expectedEntity.type === "spline" &&
							target.type === "spline" &&
							(Boolean(expectedEntity.closed) !== Boolean(target.closed) ||
								expectedEntity.mode !== target.mode ||
								expectedEntity.points.length !== target.points.length)
						)
							throw Error("Spline copies must retain their source mode and point count.")
						if (expectedEntity.type !== target.type) throw Error("Circular pattern copy type must match its rotated source.")
						const expected = offsetParameters(expectedEntity)
						return offsetParameters(target).map((value, i) => value - requireValue(expected[i]))
					})
				)
			case "linearPattern":
				return r.instances.flatMap((instance, index) =>
					instance.flatMap((id, sourceIndex) => {
						const source = entity(requireValue(r.sources[sourceIndex]))
						const target = entity(id)
						if (
							source.type === "spline" &&
							target.type === "spline" &&
							(Boolean(source.closed) !== Boolean(target.closed) || source.mode !== target.mode || source.points.length !== target.points.length)
						)
							throw Error("Spline copies must retain their source mode and point count.")
						if (source.type !== target.type) throw Error("Pattern copy type must match its source.")
						const expected = offsetParameters(translateSketchEntity(source, id, patternCellOffset(r, index + 1)))
						return offsetParameters(target).map((value, i) => value - requireValue(expected[i]))
					})
				)
			case "offsetChain": {
				const expected = offsetLineChain(geometry, r, r.value, r.targets, false)
				const source = directedOffsetLines(geometry, r.sources)
				const result = expected.flatMap((line, i) => {
					const target = entity(requireValue(r.targets[i]))
					if (target.type !== "line") throw Error("Offset chain targets must be lines.")
					const parameters = offsetParameters(line)
					return offsetParameters(target).map((value, j) => value - requireValue(parameters[j]))
				})
				for (let i = 0; i < source.length - (r.closed ? 0 : 1); i++) {
					const delta = subtract(requireValue(source[i]).p1, requireValue(source[(i + 1) % source.length]).p0)
					result.push(delta.x, delta.y)
				}
				return result
			}
			case "mirror": {
				const expected = mirrorSketchEntity(entity(r.a), line(r.symmetryLine), r.b)
				const target = entity(r.b)
				if (
					expected.type === "spline" &&
					target.type === "spline" &&
					(Boolean(expected.closed) !== Boolean(target.closed) || expected.mode !== target.mode || expected.points.length !== target.points.length)
				)
					throw Error("Spline copies must retain their source mode and point count.")
				if (target.type !== expected.type) throw Error("Mirrored geometry type does not match its source.")
				const parameters = offsetParameters(expected)
				return offsetParameters(target).map((value, i) => {
					const difference = value - requireValue(parameters[i])
					const period =
						((target.type === "rectangle" || target.type === "ellipse") && i === 4) || (target.type === "polygon" && i === 3)
							? 360
							: target.type === "arc" && i === 3
								? 2 * Math.PI
								: 0
					return period ? (Math.atan2(Math.sin((difference / period) * 2 * Math.PI), Math.cos((difference / period) * 2 * Math.PI)) * period) / (2 * Math.PI) : difference
				})
			}
			case "offset": {
				const source = entity(r.a)
				const target = entity(r.b)
				if (
					source.type === "spline" &&
					target.type === "spline" &&
					(Boolean(source.closed) !== Boolean(target.closed) || source.mode !== target.mode || source.points.length !== target.points.length)
				)
					throw Error("Spline copies must retain their source mode and point count.")
				if (source.type !== target.type) throw Error("Offset entities must have the same type.")
				const expected = offsetParameters(offsetSketchEntity(source, r.value, target.id, false))
				return offsetParameters(target).map((value, i) => value - requireValue(expected[i]))
			}
			case "radiusDifference":
				return [circle(r.a).radius - circle(r.b).radius - r.value]
			case "collinear":
				return [cross(direction(r.a), direction(r.b)), cross(subtract(line(r.b).p0, line(r.a).p0), direction(r.a))]
			case "parallel":
				return [cross(direction(r.a), direction(r.b))]
			case "perpendicular":
				return [dot(direction(r.a), direction(r.b))]
			case "angle": {
				const a = direction(r.a)
				const b = direction(r.b)
				const d = Math.atan2(cross(a, b), dot(a, b)) - (r.value * Math.PI) / 180
				return [Math.atan2(Math.sin(d), Math.cos(d))]
			}
			case "equal": {
				const a = entity(r.a)
				const b = entity(r.b)
				if ((a.type === "ellipse" || a.type === "ellipticArc") && (b.type === "ellipse" || b.type === "ellipticArc")) return [a.width - b.width, a.height - b.height]
				if (entity(r.a).type !== entity(r.b).type && !(["arc", "circle"].includes(entity(r.a).type) && ["arc", "circle"].includes(entity(r.b).type)))
					throw Error("Equal requires matching entity types.")
				return [size(r.a) - size(r.b)]
			}
			case "internalTangent": {
				const outer = circle(r.a)
				const inner = circle(r.b)
				const delta = subtract(inner.center, outer.center)
				const distance = norm(delta)
				const unit = distance > 1e-12 ? { x: delta.x / distance, y: delta.y / distance } : { x: 1, y: 0 }
				return [
					distance + inner.radius - outer.radius,
					...(outer.type === "arc"
						? pointOnSketchCurveResidual(outer, { x: outer.center.x + unit.x * outer.radius, y: outer.center.y + unit.y * outer.radius }).slice(1)
						: []),
					...(inner.type === "arc"
						? pointOnSketchCurveResidual(inner, { x: inner.center.x + unit.x * inner.radius, y: inner.center.y + unit.y * inner.radius }).slice(1)
						: [])
				]
			}
			case "tangent": {
				const a = entity(r.a)
				const b = entity(r.b)
				const ellipse = a.type === "ellipse" || a.type === "ellipticArc" ? a : b.type === "ellipse" || b.type === "ellipticArc" ? b : null
				const straight = a.type === "line" ? a : b.type === "line" ? b : null
				if (ellipse && straight) {
					const unit = direction(straight.id)
					const normal = { x: -unit.y, y: unit.x }
					const support = subtract(ellipseSupportPoint(ellipse, normal), ellipse.center)
					const signedDistance = dot(subtract(ellipse.center, straight.p0), normal)
					const side = signedDistance > 0 ? -1 : 1
					const contact = { x: ellipse.center.x + side * support.x, y: ellipse.center.y + side * support.y }
					return [Math.abs(signedDistance) - dot(support, normal), ...(ellipse.type === "ellipticArc" ? pointOnSketchCurveResidual(ellipse, contact) : [])]
				}
				if ((a.type === "circle" || a.type === "arc") && (b.type === "circle" || b.type === "arc")) {
					const delta = subtract(b.center, a.center)
					const distance = norm(delta)
					const unit = distance > 1e-12 ? { x: delta.x / distance, y: delta.y / distance } : { x: 1, y: 0 }
					const contactA = { x: a.center.x + unit.x * a.radius, y: a.center.y + unit.y * a.radius }
					const contactB = { x: b.center.x - unit.x * b.radius, y: b.center.y - unit.y * b.radius }
					return [
						distance - a.radius - b.radius,
						...(a.type === "arc" ? pointOnSketchCurveResidual(a, contactA).slice(1) : []),
						...(b.type === "arc" ? pointOnSketchCurveResidual(b, contactB).slice(1) : [])
					]
				}
				const c = a.type === "circle" || a.type === "arc" ? a : b.type === "circle" || b.type === "arc" ? b : null
				const l = a.type === "line" ? a : b.type === "line" ? b : null
				if (!c || !l) throw Error("Tangent requires a line and circle, or two circles.")
				const unit = direction(l.id)
				const along = dot(subtract(c.center, l.p0), unit)
				const contact = { x: l.p0.x + along * unit.x, y: l.p0.y + along * unit.y }
				return [Math.abs(cross(subtract(c.center, l.p0), unit)) - c.radius, ...(c.type === "arc" ? pointOnSketchCurveResidual(c, contact).slice(1) : [])]
			}
		}
	}
	const values = () => parameters.map((p) => p.read())
	const assign = (x: number[]) => parameters.forEach((p, i) => p.write(requireValue(x[i])))
	const evaluate = () => relations.flatMap(residual)
	const valid = () =>
		parameters.every((p) => Number.isFinite(p.read())) &&
		geometry.every((e) =>
			e.type === "spline"
				? e.mode !== "fit" ||
					e.points.every(
						(p, i) =>
							(i === 0 && !e.closed) ||
							p.x !== e.points[(i + e.points.length - 1) % e.points.length]?.x ||
							p.y !== e.points[(i + e.points.length - 1) % e.points.length]?.y
					)
				: e.type === "ellipticArc"
					? e.width > 0 && e.height > 0 && Math.abs(e.sweep) > 1e-9 && Math.abs(e.sweep) <= 2 * Math.PI
					: e.type === "arc"
						? e.radius > 0 && Math.abs(e.sweep) > 1e-9 && Math.abs(e.sweep) < 2 * Math.PI
						: e.type === "circle" || e.type === "polygon"
							? e.radius > 0
							: e.type === "rectangle" || e.type === "ellipse"
								? e.width > 0 && e.height > 0
								: e.type === "capsule"
									? e.width > 0
									: true
		)
	if (!valid()) throw Error("Sketch coordinates and sizes must be finite; sizes must be positive.")
	// Break the radial singularity when a constrained point starts at the curve center.
	// Central differences of distance have zero slope there in every direction.
	for (const relation of relations)
		if (relation.type === "pointOnCurve" || relation.type === "normal") {
			const curve = entity(relation.b)
			const p = point(relation.a)
			if ((curve.type === "ellipse" || curve.type === "circle" || curve.type === "arc") && norm(subtract(p, curve.center)) < 1e-9) {
				const angle = curve.type === "ellipse" ? (curve.rotation * Math.PI) / 180 : curve.type === "arc" ? curve.startAngle + curve.sweep / 2 : 0
				const step = Math.max(1e-3, (curve.type === "ellipse" ? Math.min(curve.width, curve.height) / 2 : curve.radius) * 1e-3)
				const owner = entity(relation.a.entityId)
				const position =
					(owner.type === "arc" && (relation.a.point === "p0" || relation.a.point === "p1")) ||
					((owner.type === "ellipse" || owner.type === "rectangle" || owner.type === "polygon") && relation.a.point !== "center")
						? owner.center
						: p
				position.x += Math.cos(angle) * step
				position.y += Math.sin(angle) * step
			}
		}
	for (const relation of relations)
		if (relation.type === "internalTangent") {
			const outer = entity(relation.a)
			const inner = entity(relation.b)
			if ((outer.type === "circle" || outer.type === "arc") && (inner.type === "circle" || inner.type === "arc") && norm(subtract(outer.center, inner.center)) < 1e-9)
				inner.center.x += Math.max(1e-3, outer.radius * 1e-3)
		}
	for (const relation of relations)
		if (relation.type === "tangent" || relation.type === "internalTangent") {
			const a = entity(relation.a)
			const b = entity(relation.b)
			// At the midpoint of the missing arc, the equally near endpoints create a
			// discontinuity. Pick a nearby angular branch before taking derivatives.
			for (const [curve, other] of [
				[a, b],
				[b, a]
			]) {
				if (curve?.type !== "arc" || !other) continue
				let contact: Point2D
				if (other.type === "circle" || other.type === "arc") {
					contact =
						relation.type === "internalTangent" && curve.id === relation.b
							? { x: 2 * curve.center.x - other.center.x, y: 2 * curve.center.y - other.center.y }
							: other.center
				} else if (other.type === "line") {
					const unit = direction(other.id)
					const along = dot(subtract(curve.center, other.p0), unit)
					contact = { x: other.p0.x + along * unit.x, y: other.p0.y + along * unit.y }
				} else continue
				const angle = Math.atan2(contact.y - curve.center.y, contact.x - curve.center.x) - curve.startAngle - curve.sweep / 2
				if (Math.abs(Math.abs(Math.atan2(Math.sin(angle), Math.cos(angle))) - Math.PI) < 1e-8) curve.startAngle += 1e-4
			}
			const ellipse = a.type === "ellipse" ? a : b.type === "ellipse" ? b : null
			const straight = a.type === "line" ? a : b.type === "line" ? b : null
			if (ellipse && straight) {
				const unit = direction(straight.id)
				const normal = { x: -unit.y, y: unit.x }
				if (Math.abs(dot(subtract(ellipse.center, straight.p0), normal)) < 1e-9) {
					const step = Math.max(1e-3, Math.min(ellipse.width, ellipse.height) * 1e-3)
					ellipse.center.x += normal.x * step
					ellipse.center.y += normal.y * step
				}
			}
		}
	// At an exactly wrong right angle, cross/dot residuals have zero angular slope.
	// Nudge the initial direction so the least-squares solver can leave that stationary point.
	for (const relation of relations)
		if (relation.type === "parallel" || relation.type === "collinear" || relation.type === "perpendicular") {
			const a = direction(relation.a)
			const b = direction(relation.b)
			const stationary = relation.type === "perpendicular" ? Math.abs(cross(a, b)) < 1e-8 : Math.abs(dot(a, b)) < 1e-8
			if (stationary) {
				const target = line(relation.b)
				const delta = subtract(target.p1, target.p0)
				const angle = 1e-3
				target.p1.x = target.p0.x + delta.x * Math.cos(angle) - delta.y * Math.sin(angle)
				target.p1.y = target.p0.y + delta.x * Math.sin(angle) + delta.y * Math.cos(angle)
			}
		}
	let x = values()
	let f = evaluate()
	let damping = 1e-4
	let iterations = 0
	const squared = (v: number[]) => v.reduce((sum, value) => sum + value * value, 0)
	const jacobian = (): number[][] => {
		const rows = f.map(() => Array(parameters.length).fill(0) as number[])
		for (let j = 0; j < parameters.length; j++) {
			const h = 1e-5 * Math.max(1, Math.abs(requireValue(x[j])))
			requireValue(parameters[j]).write(requireValue(x[j]) + h)
			const plus = evaluate()
			requireValue(parameters[j]).write(requireValue(x[j]) - h)
			const minus = evaluate()
			requireValue(parameters[j]).write(requireValue(x[j]))
			for (let i = 0; i < f.length; i++) requireValue(rows[i])[j] = (requireValue(plus[i]) - requireValue(minus[i])) / (2 * h)
		}
		return rows
	}
	for (; iterations < (options.maxIterations ?? 100) && Math.max(0, ...f.map(Math.abs)) > tolerance; iterations++) {
		const j = jacobian()
		const n = x.length
		const matrix = Array.from({ length: n }, (_, a) =>
			Array.from({ length: n }, (_, b) => j.reduce((sum, row) => sum + requireValue(row[a]) * requireValue(row[b]), 0) + (a === b ? damping : 0))
		)
		const rhs = Array.from({ length: n }, (_, a) => -j.reduce((sum, row, i) => sum + requireValue(row[a]) * requireValue(f[i]), 0))
		const delta = solveLinear(matrix, rhs)
		if (!delta) break
		const candidate = x.map((value, i) => value + requireValue(delta[i]))
		assign(candidate)
		const next = valid() ? evaluate() : null
		if (next && squared(next) < squared(f)) {
			x = candidate
			f = next
			damping = Math.max(1e-10, damping / 3)
		} else {
			assign(x)
			damping *= 10
			if (damping > 1e12) break
		}
	}
	assign(x)
	for (const relation of relations) if (relation.type === "offset") offsetSketchEntity(entity(relation.a), relation.value, relation.b)
	for (const relation of relations) if (relation.type === "offsetChain") offsetLineChain(geometry, relation, relation.value, relation.targets)
	const finalJacobian = jacobian()
	const rank = matrixRank(finalJacobian)
	const conflicts = relations.filter((r) => residual(r).some((v) => !Number.isFinite(v) || Math.abs(v) > tolerance)).map((r) => r.id)
	for (const relation of relations)
		if (relation.type === "internalTangent") {
			const outer = circle(relation.a)
			const inner = circle(relation.b)
			if ((outer.radius - inner.radius <= tolerance || norm(subtract(outer.center, inner.center)) <= tolerance) && !conflicts.includes(relation.id)) conflicts.push(relation.id)
		}
	for (const relation of relations)
		if (relation.type === "endpointTangent") {
			const arc = entity(relation.a.entityId)
			const target = line(relation.b)
			const collapsed = norm(subtract(target.p1, target.p0)) <= tolerance
			const reversed = relation.orientation && arc.type === "arc" && arc.sweep * relation.orientation.sweepSign <= 0
			if ((collapsed || reversed) && !conflicts.includes(relation.id)) conflicts.push(relation.id)
		}
	let rowOffset = 0
	const groups = relations.map((r) => {
		const count = residual(r).length
		const rows = finalJacobian.slice(rowOffset, rowOffset + count)
		rowOffset += count
		return { id: r.id, rows }
	})
	const analysis = analyzeSketchJacobian(groups, rank, parameters.length)
	const redundantRelations = conflicts.length ? [] : analysis.redundantRelations
	const involved = new Set(relations.filter((r) => conflicts.includes(r.id)).flatMap(sketchRelationEntityIds))
	const entityStates: Record<string, SketchEntityConstraintState> = {}
	for (const e of geometry) {
		const range = requireValue(entityParameters.get(e.id))
		entityStates[e.id] = involved.has(e.id)
			? "conflicting"
			: conflicts.length || !analysis.fixedParameters
				? "unknown"
				: analysis.fixedParameters.slice(range.start, range.end).every(Boolean)
					? "fully-constrained"
					: "underconstrained"
	}
	return {
		entities: geometry,
		entityStates,
		status: conflicts.length ? "conflicting" : rank === parameters.length ? "fully-constrained" : "underconstrained",
		degreesOfFreedom: parameters.length - rank,
		redundantEquations: f.length - rank,
		redundantRelations,
		conflicts,
		maxResidual: Math.max(0, ...f.map(Math.abs)),
		iterations
	}
}

const cell = (rows: number[][], r: number, c: number) => requireValue(requireValue(rows[r])[c])
function solveLinear(matrix: number[][], rhs: number[]): number[] | null {
	const rows = matrix.map((row, i) => [...row, requireValue(rhs[i])])
	const n = rows.length
	for (let c = 0; c < n; c++) {
		let pivot = c
		for (let r = c + 1; r < n; r++) if (Math.abs(cell(rows, r, c)) > Math.abs(cell(rows, pivot, c))) pivot = r
		if (Math.abs(cell(rows, pivot, c)) < 1e-15) return null
		const old = requireValue(rows[c])
		rows[c] = requireValue(rows[pivot])
		rows[pivot] = old
		const d = cell(rows, c, c)
		for (let k = c; k <= n; k++) requireValue(rows[c])[k] = cell(rows, c, k) / d
		for (let r = 0; r < n; r++)
			if (r !== c) {
				const factor = cell(rows, r, c)
				for (let k = c; k <= n; k++) requireValue(rows[r])[k] = cell(rows, r, k) - factor * cell(rows, c, k)
			}
	}
	return rows.map((row) => requireValue(row[n]))
}
function matrixRank(input: number[][]): number {
	const rows = input.map((row) => [...row])
	let rank = 0
	const columns = rows[0]?.length ?? 0
	for (let c = 0; c < columns && rank < rows.length; c++) {
		let pivot = rank
		for (let r = rank + 1; r < rows.length; r++) if (Math.abs(cell(rows, r, c)) > Math.abs(cell(rows, pivot, c))) pivot = r
		if (Math.abs(cell(rows, pivot, c)) < 1e-7) continue
		const old = requireValue(rows[rank])
		rows[rank] = requireValue(rows[pivot])
		rows[pivot] = old
		const d = cell(rows, rank, c)
		for (let k = c; k < columns; k++) requireValue(rows[rank])[k] = cell(rows, rank, k) / d
		for (let r = rank + 1; r < rows.length; r++) {
			const factor = cell(rows, r, c)
			for (let k = c; k < columns; k++) requireValue(rows[r])[k] = cell(rows, r, k) - factor * cell(rows, rank, k)
		}
		rank++
	}
	return rank
}

/** Validate persisted relations without silently discarding a driving constraint. */
export function normalizeSketchRelations(input: unknown, variables: readonly SketchVariable[] = []): SketchRelation[] | undefined {
	if (input === undefined) return undefined
	if (!Array.isArray(input)) throw Error("Sketch relations must be an array.")
	const values = evaluateSketchVariables(variables)
	const resolve = (name: string) => {
		const value = values.get(name)
		if (!value) throw Error(`Unknown sketch variable: #${name}`)
		return value
	}
	const str = (v: unknown): v is string => typeof v === "string" && v.length > 0
	const point = (v: unknown): v is Point2D => !!v && typeof v === "object" && Number.isFinite((v as Point2D).x) && Number.isFinite((v as Point2D).y)
	const anchor = (v: unknown): v is SketchAnchor =>
		!!v &&
		typeof v === "object" &&
		str((v as SketchAnchor).entityId) &&
		(["p0", "p1", "p2", "p3", "center", "from", "to"].includes((v as SketchAnchor).point) ||
			(/^point(?:0|[1-9][0-9]{0,3})$/.test((v as SketchAnchor).point) && Number((v as SketchAnchor).point.slice(5)) < 4096) ||
			/^vertex(?:[0-9]|[1-5][0-9]|6[0-3])$/.test((v as SketchAnchor).point))
	return input.map((value) => {
		if (!value || typeof value !== "object" || !str(value.id)) throw Error("Invalid sketch relation ID.")
		const r = value as Record<string, unknown>
		let valid = false
		switch (r.type) {
			case "circularPattern":
				valid =
					Array.isArray(r.sources) &&
					r.sources.length > 0 &&
					r.sources.every(str) &&
					Array.isArray(r.instances) &&
					r.instances.length > 0 &&
					r.instances.length < 32 &&
					r.instances.every((row) => Array.isArray(row) && row.length === (r.sources as unknown[]).length && row.every(str)) &&
					r.instances.flat().length <= 64 &&
					new Set([...r.sources, ...r.instances.flat()]).size === r.sources.length + r.instances.flat().length &&
					point(r.center) &&
					(r.centerAnchor === undefined || (anchor(r.centerAnchor) && !r.instances.flat().includes(r.centerAnchor.entityId))) &&
					typeof r.angle === "number" &&
					Number.isFinite(r.angle) &&
					Math.abs(r.angle) >= 1e-9 &&
					Math.abs(r.angle) <= 360
				break
			case "linearPattern":
				valid =
					Array.isArray(r.sources) &&
					r.sources.length > 0 &&
					r.sources.every(str) &&
					Array.isArray(r.instances) &&
					r.instances.length > 0 &&
					r.instances.length < 32 &&
					r.instances.every((row) => Array.isArray(row) && row.length === (r.sources as unknown[]).length && row.every(str)) &&
					r.instances.flat().length <= 64 &&
					new Set([...r.sources, ...r.instances.flat()]).size === r.sources.length + r.instances.flat().length &&
					point(r.step) &&
					Math.hypot(r.step.x, r.step.y) > 1e-9 &&
					(r.columns === undefined || (typeof r.columns === "number" && Number.isInteger(r.columns) && r.columns >= 1 && (r.instances.length + 1) % r.columns === 0)) &&
					(r.rowStep === undefined || (point(r.rowStep) && Math.hypot(r.rowStep.x, r.rowStep.y) > 1e-9))
				break
			case "offsetChain":
				valid =
					Array.isArray(r.sources) &&
					r.sources.length >= 2 &&
					r.sources.every((s) => s && typeof s === "object" && str(s.entityId) && typeof s.reversed === "boolean") &&
					Array.isArray(r.targets) &&
					r.targets.length === r.sources.length &&
					r.targets.every(str) &&
					new Set([...r.sources.map((s) => s.entityId), ...r.targets]).size === r.sources.length * 2 &&
					typeof r.closed === "boolean" &&
					(r.side === 1 || r.side === -1) &&
					typeof r.value === "number" &&
					Number.isFinite(r.value)
				break
			case "fixed":
				valid = anchor(r.anchor) && point(r.position)
				break
			case "normal":
				valid = anchor(r.a) && str(r.b) && r.a.entityId !== r.b && ["p0", "p1"].includes(r.a.point)
				break
			case "midpoint":
			case "endpointTangent":
			case "pointOnCurve":
				valid = anchor(r.a) && str(r.b) && (r.a as SketchAnchor).entityId !== r.b
				break
			case "mirror":
				valid = str(r.a) && str(r.b) && str(r.symmetryLine) && r.a !== r.b && r.a !== r.symmetryLine && r.b !== r.symmetryLine
				break
			case "symmetric":
				valid =
					anchor(r.a) &&
					anchor(r.b) &&
					str(r.symmetryLine) &&
					((r.a as SketchAnchor).entityId !== (r.b as SketchAnchor).entityId || (r.a as SketchAnchor).point !== (r.b as SketchAnchor).point)
				break
			case "chamfer":
				valid =
					(r.mode === undefined || r.mode === "two-distances" || r.mode === "distance-angle") &&
					(r.mode !== "distance-angle" || (typeof r.secondValue === "number" && r.secondValue < 180)) &&
					str(r.chamferId) &&
					anchor(r.a) &&
					anchor(r.b) &&
					r.chamferId !== r.a.entityId &&
					r.chamferId !== r.b.entityId &&
					r.a.entityId !== r.b.entityId &&
					["p0", "p1"].includes(r.a.point) &&
					["p0", "p1"].includes(r.b.point) &&
					typeof r.value === "number" &&
					Number.isFinite(r.value) &&
					r.value > 0 &&
					typeof r.secondValue === "number" &&
					Number.isFinite(r.secondValue) &&
					r.secondValue > 0
				break
			case "smoothJoin":
				valid = anchor(r.a) && anchor(r.b) && r.a.entityId !== r.b.entityId && /^(p[01]|point\d+)$/.test(r.a.point) && /^(p[01]|point\d+)$/.test(r.b.point)
				break
			case "coincident":
				valid = anchor(r.a) && anchor(r.b)
				break
			case "distance":
				valid =
					anchor(r.a) &&
					anchor(r.b) &&
					Number.isFinite(r.value) &&
					(r.axis === undefined || r.axis === "x" || r.axis === "y") &&
					(r.direction === undefined || (r.axis === undefined && point(r.direction) && Math.abs(Math.hypot(r.direction.x, r.direction.y) - 1) < 1e-6))
				break
			case "horizontal":
			case "vertical":
				valid = str(r.entityId)
				break
			case "rotation":
				valid = str(r.entityId) && typeof r.value === "number" && Number.isFinite(r.value)
				break
			case "arcSweep":
				valid = str(r.entityId) && typeof r.value === "number" && Number.isFinite(r.value) && Math.abs(r.value) > 0 && Math.abs(r.value) < 360
				break
			case "length":
			case "diameter":
			case "radius":
			case "width":
			case "height":
				valid = str(r.entityId) && typeof r.value === "number" && Number.isFinite(r.value) && (r.reference === true ? Number(r.value) >= 0 : Number(r.value) > 0)
				break
			case "collinear":
			case "parallel":
			case "perpendicular":
			case "equal":
			case "sameEllipse":
			case "concentric":
			case "tangent":
			case "internalTangent":
				valid = str(r.a) && str(r.b)
				break
			case "offset":
			case "angle":
			case "radiusDifference":
				valid = str(r.a) && str(r.b) && Number.isFinite(r.value)
				break
		}
		if (r.type === "endpointTangent" && r.orientation !== undefined) {
			const orientation = r.orientation as { lineSign?: unknown; sweepSign?: unknown } | null
			valid = valid && !!orientation && (orientation.lineSign === 1 || orientation.lineSign === -1) && (orientation.sweepSign === 1 || orientation.sweepSign === -1)
		}
		if (r.reference !== undefined && (typeof r.reference !== "boolean" || (r.reference && !isMeasuredDimension(value as SketchRelation)))) valid = false
		if (r.expression !== undefined) {
			if (typeof r.expression !== "string" || r.reference || !isMeasuredDimension(value as SketchRelation)) valid = false
			else {
				const evaluated = parseSketchValue(r.expression, ["angle", "arcSweep", "rotation"].includes(String(r.type)) ? "angle" : "length", resolve)
				if (typeof r.value !== "number" || Math.abs(evaluated - r.value) > 1e-12 * Math.max(1, Math.abs(evaluated))) valid = false
			}
		}
		if (r.fixation !== undefined)
			valid =
				valid &&
				str(r.fixation) &&
				((r.type === "fixed" && (r.anchor as SketchAnchor).entityId === r.fixation) ||
					(["radius", "width", "height", "rotation"].includes(String(r.type)) && r.entityId === r.fixation))
		if (!valid || (r.labelPosition !== undefined && !point(r.labelPosition))) throw Error(`Invalid sketch relation: ${value.id}`)
		return structuredClone(value) as SketchRelation
	})
}

/** Reevaluate driving expressions before geometry solving; preserve the input transaction. */
export function resolveSketchDimensionExpressions(relations: readonly SketchRelation[] | undefined, variables: readonly SketchVariable[] = []): SketchRelation[] | undefined {
	const values = evaluateSketchVariables(variables)
	const resolved = relations?.map((relation) => {
		if (relation.expression === undefined) return relation
		if (!("value" in relation) || relation.reference || !isMeasuredDimension(relation)) throw Error(`Invalid expression dimension: ${relation.id}`)
		const value = parseSketchValue(relation.expression, ["angle", "arcSweep", "rotation"].includes(relation.type) ? "angle" : "length", (name) => {
			const quantity = values.get(name)
			if (!quantity) throw Error(`Unknown sketch variable: #${name}`)
			return quantity
		})
		return { ...relation, value }
	})
	return normalizeSketchRelations(resolved, variables)
}

export function remapSketchRelations(
	relations: readonly SketchRelation[] | undefined,
	ids: ReadonlyMap<string, string>,
	anchorNames?: ReadonlyMap<string, ReadonlyMap<SketchAnchorName, SketchAnchorName>>
): SketchRelation[] | undefined {
	return relations?.map((relation) => {
		const r = structuredClone(relation)
		if (r.fixation) r.fixation = ids.get(r.fixation) ?? r.fixation
		if (r.type === "chamfer") r.chamferId = ids.get(r.chamferId) ?? r.chamferId
		const anchor = (p: SketchAnchor): SketchAnchor => ({ ...p, entityId: ids.get(p.entityId) ?? p.entityId, point: anchorNames?.get(p.entityId)?.get(p.point) ?? p.point })
		if ("entityId" in r) r.entityId = ids.get(r.entityId) ?? r.entityId
		if (r.type === "symmetric" || r.type === "mirror") r.symmetryLine = ids.get(r.symmetryLine) ?? r.symmetryLine
		if (r.type === "linearPattern" || r.type === "circularPattern") {
			r.sources = r.sources.map((id) => ids.get(id) ?? id)
			r.instances = r.instances.map((row) => row.map((id) => ids.get(id) ?? id))
			if (r.type === "circularPattern" && r.centerAnchor) r.centerAnchor = anchor(r.centerAnchor)
		} else if (r.type === "offsetChain") {
			r.sources = r.sources.map((s) => ({ ...s, entityId: ids.get(s.entityId) ?? s.entityId }))
			r.targets = r.targets.map((id) => ids.get(id) ?? id)
		} else if (r.type === "fixed") r.anchor = anchor(r.anchor)
		else if ("a" in r && "b" in r) {
			if (r.type === "normal" || r.type === "midpoint" || r.type === "pointOnCurve" || r.type === "endpointTangent") {
				r.a = anchor(r.a)
				r.b = ids.get(r.b) ?? r.b
			} else if (r.type === "chamfer" || r.type === "coincident" || r.type === "smoothJoin" || r.type === "distance" || r.type === "symmetric") {
				r.a = anchor(r.a)
				r.b = anchor(r.b)
			} else {
				r.a = ids.get(r.a) ?? r.a
				r.b = ids.get(r.b) ?? r.b
			}
		}
		return r
	})
}
