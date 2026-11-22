import { splineBezierSegments } from "./sketch-spline"
import { cubicBezierDerivative } from "./sketch-bezier"
import { requireValue } from "./required"
import type { Arc, SketchEntity, SketchAnchorName } from "./schema"
import type { Point2D } from "./types"
import { entityAnchorPoint, normalizeArc } from "./sketch-curves"

/** Unit tangent pointing outward from a finite curve endpoint. */
export function sketchEndpointDirection(entity: SketchEntity, endpoint: SketchAnchorName): Point2D {
	if (entity.type === "spline") {
		if (entity.closed) throw Error("A closed spline has no endpoints.")
		const start = endpoint === "point0"
		if (!start && endpoint !== `point${entity.points.length - 1}`) throw Error("Choose a spline endpoint, not an interior handle.")
		const segments = splineBezierSegments(entity)
		const tangent = cubicBezierDerivative(requireValue(start ? segments[0] : segments.at(-1)), start ? 0 : 1)
		const magnitude = Math.hypot(tangent.x, tangent.y)
		if (magnitude < 1e-12) throw Error("Tangency requires a nonzero spline endpoint tangent.")
		const sign = start ? -1 : 1
		return { x: (sign * tangent.x) / magnitude, y: (sign * tangent.y) / magnitude }
	}
	if (endpoint !== "p0" && endpoint !== "p1") throw Error("Choose a line or arc endpoint.")
	if (entity.type === "line") {
		const dx = entity.p1.x - entity.p0.x
		const dy = entity.p1.y - entity.p0.y
		const length = Math.hypot(dx, dy)
		if (length < 1e-9) throw Error("Tangency requires a nonzero line.")
		const sign = endpoint === "p1" ? 1 : -1
		return { x: (sign * dx) / length, y: (sign * dy) / length }
	}
	if (entity.type === "ellipticArc") {
		if (Math.abs(entity.sweep) >= 2 * Math.PI) throw Error("A closed elliptic arc has no endpoints.")
		const angle = entity.startAngle + (endpoint === "p1" ? entity.sweep : 0)
		const rotation = (entity.rotation * Math.PI) / 180
		const sign = Math.sign(entity.sweep) * (endpoint === "p1" ? 1 : -1)
		const x = -entity.width * Math.sin(angle)
		const y = entity.height * Math.cos(angle)
		const magnitude = Math.hypot(x, y)
		return { x: (sign * (x * Math.cos(rotation) - y * Math.sin(rotation))) / magnitude, y: (sign * (x * Math.sin(rotation) + y * Math.cos(rotation))) / magnitude }
	}
	if (entity.type !== "arc") throw Error("Choose a line or arc endpoint.")
	const angle = entity.startAngle + (endpoint === "p1" ? entity.sweep : 0)
	const sign = Math.sign(entity.sweep) * (endpoint === "p1" ? 1 : -1)
	return { x: -Math.sin(angle) * sign, y: Math.cos(angle) * sign }
}
export function tangentArc(id: string, source: SketchEntity, endpoint: SketchAnchorName, end: Point2D): Arc {
	const start = entityAnchorPoint(source, endpoint)
	const tangent = sketchEndpointDirection(source, endpoint)
	const normal = { x: -tangent.y, y: tangent.x }
	const dx = end.x - start.x
	const dy = end.y - start.y
	const projection = dx * normal.x + dy * normal.y
	if (Math.hypot(dx, dy) < 1e-8 || Math.abs(projection) < 1e-9) throw Error("Choose an endpoint away from the tangent line.")
	const offset = (dx * dx + dy * dy) / (2 * projection)
	const center = { x: start.x + normal.x * offset, y: start.y + normal.y * offset }
	const startAngle = Math.atan2(start.y - center.y, start.x - center.x)
	const endAngle = Math.atan2(end.y - center.y, end.x - center.x)
	const tau = 2 * Math.PI
	const sweep = offset > 0 ? (((endAngle - startAngle) % tau) + tau) % tau : -((((startAngle - endAngle) % tau) + tau) % tau)
	const result = normalizeArc({ type: "arc", center, radius: Math.abs(offset), startAngle, sweep, segments: 128 }, id)
	if (!result) throw Error("Cannot create a tangent arc at those points.")
	return result
}
