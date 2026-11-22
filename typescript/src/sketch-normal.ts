import { nearestEllipticArcPoint } from "./sketch-elliptic-arc"
import { nearestSplinePoint, splineBezierSegments } from "./sketch-spline"
import { cubicBezierDerivative } from "./sketch-bezier"
import { requireValue } from "./required"
import type { Line, SketchEntity } from "./schema"
import { pointOnSketchCurveResidual } from "./sketch-point-constraints"

/** The line endpoint lies on the finite curve and its direction follows the analytic normal. */
export function sketchNormalResidual(line: Line, endpoint: "p0" | "p1", curve: SketchEntity): number[] {
	if (curve.type !== "line" && curve.type !== "circle" && curve.type !== "arc" && curve.type !== "ellipse" && curve.type !== "spline" && curve.type !== "ellipticArc")
		throw Error("Normal requires a line endpoint and a line, circle, arc, ellipse, elliptic arc or spline.")
	const point = line[endpoint]
	const dx = line.p1.x - line.p0.x
	const dy = line.p1.y - line.p0.y
	const length = Math.hypot(dx, dy)
	if (length < 1e-8) throw Error("Normal requires a nonzero line.")
	if (curve.type === "ellipticArc") {
		const hit = nearestEllipticArcPoint(curve, point)
		const angle = curve.startAngle + hit.t * curve.sweep
		const rotation = (curve.rotation * Math.PI) / 180
		const x = -curve.width * Math.sin(angle)
		const y = curve.height * Math.cos(angle)
		const tx = x * Math.cos(rotation) - y * Math.sin(rotation)
		const ty = x * Math.sin(rotation) + y * Math.cos(rotation)
		return [...pointOnSketchCurveResidual(curve, point), (dx * tx + dy * ty) / (length * Math.hypot(tx, ty))]
	}
	if (curve.type === "spline") {
		const hit = nearestSplinePoint(curve, point)
		const segments = splineBezierSegments(curve)
		const tangent = cubicBezierDerivative(requireValue(segments[hit.segment]), hit.t)
		const magnitude = Math.hypot(tangent.x, tangent.y)
		if (magnitude < 1e-12) throw Error("Normal requires a regular spline point with a nonzero tangent.")
		const adjacent =
			hit.t === 0 && hit.segment > 0
				? cubicBezierDerivative(requireValue(segments[hit.segment - 1]), 1)
				: hit.t === 1 && hit.segment + 1 < segments.length
					? cubicBezierDerivative(requireValue(segments[hit.segment + 1]), 0)
					: null
		if (adjacent) {
			const otherLength = Math.hypot(adjacent.x, adjacent.y)
			if (otherLength < 1e-12 || (adjacent.x * tangent.x + adjacent.y * tangent.y) / (otherLength * magnitude) < 1 - 1e-8)
				throw Error("Normal requires a smooth spline join with a unique tangent.")
		}
		return [...pointOnSketchCurveResidual(curve, point), (dx * tangent.x + dy * tangent.y) / (length * magnitude)]
	}
	if (curve.type === "line") {
		const tx = curve.p1.x - curve.p0.x
		const ty = curve.p1.y - curve.p0.y
		const targetLength = Math.hypot(tx, ty)
		if (curve.id === line.id || targetLength < 1e-8) throw Error("Normal requires a separate nonzero target line.")
		return [...pointOnSketchCurveResidual(curve, point), (dx * tx + dy * ty) / (length * targetLength)]
	}
	let nx = point.x - curve.center.x
	let ny = point.y - curve.center.y
	if (curve.type === "ellipse") {
		const angle = (curve.rotation * Math.PI) / 180
		const c = Math.cos(angle)
		const s = Math.sin(angle)
		const x = (nx * c + ny * s) / (curve.width * curve.width)
		const y = (-nx * s + ny * c) / (curve.height * curve.height)
		nx = x * c - y * s
		ny = x * s + y * c
	}
	const magnitude = Math.hypot(nx, ny)
	if (magnitude < 1e-15) throw Error("Normal endpoint cannot be at the curve center.")
	return [...pointOnSketchCurveResidual(curve, point), (dx * ny - dy * nx) / (length * magnitude)]
}
