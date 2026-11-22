import { intersectCubicBeziers } from "./sketch-bezier-intersections"
import type { Ellipse, Line, Spline } from "./schema"
import type { Point2D } from "./types"
import { splineBezierSegments } from "./sketch-spline"
import { cubicBezierPoint } from "./sketch-bezier"
import { boundedPolynomialRoots } from "./sketch-ellipse-intersections"
import { requireValue } from "./required"

/** Isolated finite line contacts, retaining cubic parameters for later curve editing. */
export function lineSplineIntersections(line: Line, spline: Spline, finite = true): { point: Point2D; segment: number; t: number }[] {
	const dx = line.p1.x - line.p0.x
	const dy = line.p1.y - line.p0.y
	const length = Math.hypot(dx, dy)
	if (!Number.isFinite(length) || length < 1e-12) return []
	const ux = dx / length
	const uy = dy / length
	const result: { point: Point2D; segment: number; t: number }[] = []
	const curves = splineBezierSegments(spline)
	for (const [segment, curve] of curves.entries()) {
		const h = curve.map((p) => (p.x - line.p0.x) * uy - (p.y - line.p0.y) * ux)
		const a = requireValue(h[0])
		const b = requireValue(h[1])
		const c = requireValue(h[2])
		const d = requireValue(h[3])
		for (const root of boundedPolynomialRoots([a, 3 * (b - a), 3 * (a - 2 * b + c), d - a + 3 * (b - c)])) {
			if (root < -1e-10 || root > 1 + 1e-10) continue
			const t = Math.max(0, Math.min(1, root))
			const point = cubicBezierPoint(curve, t)
			const along = (point.x - line.p0.x) * ux + (point.y - line.p0.y) * uy
			if (finite && (along < -1e-8 || along > length + 1e-8)) continue
			if (
				!result.some((hit) => {
					const distance = Math.abs(hit.segment + hit.t - segment - t)
					return distance < 1e-9 || (spline.closed && Math.abs(distance - curves.length) < 1e-9)
				})
			)
				result.push({ point, segment, t })
		}
	}
	return result
}

/** Supporting-circle contacts from the degree-six squared-distance polynomial. */
export function circleSplineIntersections(circle: { center: Point2D; radius: number }, spline: Spline): { point: Point2D; segment: number; t: number }[] {
	if (!Number.isFinite(circle.radius) || circle.radius <= 0 || !Number.isFinite(circle.center.x + circle.center.y)) throw Error("Circle geometry must be finite with positive radius.")
	const result: { point: Point2D; segment: number; t: number }[] = []
	const curves = splineBezierSegments(spline)
	for (const [segment, curve] of curves.entries()) {
		const scale = Math.max(circle.radius, ...curve.flatMap((p) => [Math.abs(p.x - circle.center.x), Math.abs(p.y - circle.center.y)]))
		const coefficients = Array<number>(7).fill(0)
		for (const axis of ["x", "y"] as const) {
			const a = (curve[0][axis] - circle.center[axis]) / scale
			const b = (curve[1][axis] - circle.center[axis]) / scale
			const c = (curve[2][axis] - circle.center[axis]) / scale
			const d = (curve[3][axis] - circle.center[axis]) / scale
			const polynomial = [a, 3 * (b - a), 3 * (a - 2 * b + c), d - a + 3 * (b - c)]
			for (let i = 0; i < 4; i++) for (let j = 0; j < 4; j++) coefficients[i + j] = requireValue(coefficients[i + j]) + requireValue(polynomial[i]) * requireValue(polynomial[j])
		}
		coefficients[0] = requireValue(coefficients[0]) - (circle.radius / scale) ** 2
		for (const root of boundedPolynomialRoots(coefficients)) {
			if (root < -1e-10 || root > 1 + 1e-10) continue
			const t = Math.max(0, Math.min(1, root))
			const point = cubicBezierPoint(curve, t)
			if (
				!result.some((hit) => {
					const distance = Math.abs(hit.segment + hit.t - segment - t)
					return distance < 1e-9 || (spline.closed && Math.abs(distance - curves.length) < 1e-9)
				})
			)
				result.push({ point, segment, t })
		}
	}
	return result
}

/** Affine ellipse normalization preserves cubic parameters and periodic interpolation. */
export function ellipseSplineIntersections(ellipse: Ellipse, spline: Spline): { point: Point2D; segment: number; t: number }[] {
	const rx = ellipse.width / 2
	const ry = ellipse.height / 2
	const angle = (ellipse.rotation * Math.PI) / 180
	if (!(rx > 0 && ry > 0) || ![rx, ry, angle, ellipse.center.x, ellipse.center.y].every(Number.isFinite)) throw Error("Ellipse geometry must be finite with positive axes.")
	const c = Math.cos(angle)
	const s = Math.sin(angle)
	const transformed: Spline = {
		...spline,
		points: spline.points.map((point) => {
			const x = point.x - ellipse.center.x
			const y = point.y - ellipse.center.y
			return { x: (c * x + s * y) / rx, y: (-s * x + c * y) / ry }
		})
	}
	const curves = splineBezierSegments(spline)
	return circleSplineIntersections({ center: { x: 0, y: 0 }, radius: 1 }, transformed).map((hit) => ({ ...hit, point: cubicBezierPoint(requireValue(curves[hit.segment]), hit.t) }))
}

/** Whole-spline contacts, retaining both curve parameters and distinct retracing passes. */
export function splineSplineIntersections(a: Spline, b: Spline): { point: Point2D; segment: number; t: number; otherSegment: number; u: number }[] {
	const first = splineBezierSegments(a)
	const second = splineBezierSegments(b)
	const result: { point: Point2D; segment: number; t: number; otherSegment: number; u: number }[] = []
	const box = (points: readonly Point2D[]) => ({
		x0: Math.min(...points.map((p) => p.x)),
		x1: Math.max(...points.map((p) => p.x)),
		y0: Math.min(...points.map((p) => p.y)),
		y1: Math.max(...points.map((p) => p.y))
	})
	const boxes = second.map(box)
	const equivalent = (x: number, y: number, count: number, closed?: boolean) => Math.abs(x - y) < 1e-7 || (closed === true && Math.abs(Math.abs(x - y) - count) < 1e-7)
	let candidates = 0
	for (const [segment, curve] of first.entries()) {
		const left = box(curve)
		for (const [otherSegment, other] of second.entries()) {
			const right = requireValue(boxes[otherSegment])
			if (left.x1 < right.x0 || right.x1 < left.x0 || left.y1 < right.y0 || right.y1 < left.y0) continue
			if (++candidates > 4096) throw Error("Spline intersections exceed the candidate-pair limit.")
			for (const hit of intersectCubicBeziers(curve, other)) {
				if (
					result.some(
						(prior) =>
							equivalent(prior.segment + prior.t, segment + hit.t, first.length, a.closed) &&
							equivalent(prior.otherSegment + prior.u, otherSegment + hit.u, second.length, b.closed)
					)
				)
					continue
				result.push({ ...hit, segment, otherSegment })
			}
		}
	}
	return result
}
