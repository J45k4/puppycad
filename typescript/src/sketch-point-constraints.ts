import { nearestEllipticArcPoint, ellipticArcPoint } from "./sketch-elliptic-arc"
import { nearestSplinePoint, splineBezierSegments } from "./sketch-spline"
import { cubicBezierDerivative } from "./sketch-bezier"
import { requireValue } from "./required"
import { pointOnEllipseResidual } from "./sketch-ellipse"
import type { SketchEntity } from "./schema"
import type { Point2D } from "./types"
import { arcPoint } from "./sketch-curves"

export function sketchCurveMidpoint(curve: SketchEntity): Point2D {
	if (curve.type === "ellipticArc") return ellipticArcPoint(curve, 0.5)
	if (curve.type === "line") return { x: (curve.p0.x + curve.p1.x) / 2, y: (curve.p0.y + curve.p1.y) / 2 }
	if (curve.type === "arc") return arcPoint(curve, 0.5)
	throw Error("Midpoint requires a line or an arc.")
}

/** Projection onto the finite curve, including arc/line endpoints. */
export function closestSketchCurvePoint(curve: SketchEntity, point: Point2D): Point2D {
	if (curve.type === "ellipticArc") return nearestEllipticArcPoint(curve, point).point
	if (curve.type === "spline") return nearestSplinePoint(curve, point).point
	if (curve.type === "line") {
		const dx = curve.p1.x - curve.p0.x
		const dy = curve.p1.y - curve.p0.y
		const lengthSquared = dx * dx + dy * dy
		if (lengthSquared < 1e-20) throw Error("Point on curve requires a nonzero line.")
		const t = Math.max(0, Math.min(1, ((point.x - curve.p0.x) * dx + (point.y - curve.p0.y) * dy) / lengthSquared))
		return { x: curve.p0.x + t * dx, y: curve.p0.y + t * dy }
	}
	if (curve.type !== "circle" && curve.type !== "arc") throw Error("Point on curve requires a line, circle or arc.")
	const angle = Math.atan2(point.y - curve.center.y, point.x - curve.center.x)
	const radial = { x: curve.center.x + curve.radius * Math.cos(angle), y: curve.center.y + curve.radius * Math.sin(angle) }
	if (curve.type === "circle") return radial
	const period = 2 * Math.PI
	const delta = curve.sweep > 0 ? angle - curve.startAngle : curve.startAngle - angle
	const wrapped = ((delta % period) + period) % period
	if (wrapped <= Math.abs(curve.sweep)) return radial
	const start = arcPoint(curve, 0)
	const end = arcPoint(curve, 1)
	return Math.hypot(point.x - start.x, point.y - start.y) <= Math.hypot(point.x - end.x, point.y - end.y) ? start : end
}

/** Independent normal and endpoint residuals keep interior sliding freedom measurable. */
export function pointOnSketchCurveResidual(curve: SketchEntity, point: Point2D): number[] {
	if (curve.type === "ellipticArc") {
		const hit = nearestEllipticArcPoint(curve, point)
		const angle = curve.startAngle + curve.sweep * hit.t
		const rotation = (curve.rotation * Math.PI) / 180
		const tx = (-curve.width / 2) * Math.sin(angle)
		const ty = (curve.height / 2) * Math.cos(angle)
		const x = tx * Math.cos(rotation) - ty * Math.sin(rotation)
		const y = tx * Math.sin(rotation) + ty * Math.cos(rotation)
		const length = Math.hypot(x, y)
		const dx = point.x - hit.point.x
		const dy = point.y - hit.point.y
		return [(dx * y - dy * x) / length, (dx * x + dy * y) / length]
	}
	if (curve.type === "ellipse") return [pointOnEllipseResidual(curve, point)]
	if (curve.type === "spline") {
		const hit = nearestSplinePoint(curve, point)
		const segments = splineBezierSegments(curve)
		const tangent = cubicBezierDerivative(requireValue(segments[hit.segment]), hit.t)
		const length = Math.hypot(tangent.x, tangent.y)
		const dx = point.x - hit.point.x
		const dy = point.y - hit.point.y
		if (length < 1e-12) return [dx, dy]
		return [(dx * tangent.y - dy * tangent.x) / length, (dx * tangent.x + dy * tangent.y) / length]
	}
	const projected = closestSketchCurvePoint(curve, point)
	if (curve.type === "line") {
		const dx = curve.p1.x - curve.p0.x
		const dy = curve.p1.y - curve.p0.y
		const length = Math.hypot(dx, dy)
		return [((point.x - curve.p0.x) * dy - (point.y - curve.p0.y) * dx) / length, ((point.x - projected.x) * dx + (point.y - projected.y) * dy) / length]
	}
	if (curve.type !== "circle" && curve.type !== "arc") throw Error("Unsupported curve.")
	const radial = Math.hypot(point.x - curve.center.x, point.y - curve.center.y) - curve.radius
	if (curve.type === "circle") return [radial]
	const angle = Math.atan2(point.y - curve.center.y, point.x - curve.center.x)
	const projectedAngle = Math.atan2(projected.y - curve.center.y, projected.x - curve.center.x)
	return [radial, Math.atan2(Math.sin(angle - projectedAngle), Math.cos(angle - projectedAngle)) * curve.radius]
}
