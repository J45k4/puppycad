import { requireValue } from "./required"
import type { Point2D } from "./types"

/** A native cubic segment, retaining its four editable control points. */
export type CubicBezier = readonly [Point2D, Point2D, Point2D, Point2D]
const lerp = (a: Point2D, b: Point2D, t: number): Point2D => ({ x: a.x * (1 - t) + b.x * t, y: a.y * (1 - t) + b.y * t })
function validate(curve: CubicBezier, t = 0) {
	if (!curve.every((p) => Number.isFinite(p.x) && Number.isFinite(p.y)) || !Number.isFinite(t) || t < 0 || t > 1)
		throw Error("Bezier coordinates must be finite and parameter must be in [0, 1].")
}
function subdivide(curve: CubicBezier, t: number): [CubicBezier, CubicBezier] {
	const a = lerp(curve[0], curve[1], t)
	const b = lerp(curve[1], curve[2], t)
	const c = lerp(curve[2], curve[3], t)
	const d = lerp(a, b, t)
	const e = lerp(b, c, t)
	const point = lerp(d, e, t)
	return [
		[{ ...curve[0] }, a, d, point],
		[{ ...point }, e, c, { ...curve[3] }]
	]
}
export function splitCubicBezier(curve: CubicBezier, t: number): [CubicBezier, CubicBezier] {
	validate(curve, t)
	return subdivide(curve, t)
}
export function cubicBezierPoint(curve: CubicBezier, t: number): Point2D {
	validate(curve, t)
	return subdivide(curve, t)[0][3]
}
export function cubicBezierDerivative(curve: CubicBezier, t: number): Point2D {
	validate(curve, t)
	const u = 1 - t
	return {
		x: 3 * (u * u * (curve[1].x - curve[0].x) + 2 * u * t * (curve[2].x - curve[1].x) + t * t * (curve[3].x - curve[2].x)),
		y: 3 * (u * u * (curve[1].y - curve[0].y) + 2 * u * t * (curve[2].y - curve[1].y) + t * t * (curve[3].y - curve[2].y))
	}
}
function segmentDistance(p: Point2D, a: Point2D, b: Point2D): number {
	const dx = b.x - a.x
	const dy = b.y - a.y
	const length = dx * dx + dy * dy
	const t = length === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / length))
	return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy))
}
/** Convex-hull flatness bounds curve-to-chord error, including collinear reversals. */
export function sampleCubicBezier(curve: CubicBezier, tolerance: number): { point: Point2D; t: number }[] {
	validate(curve)
	if (!Number.isFinite(tolerance) || tolerance <= 0) throw Error("Bezier tolerance must be finite and positive.")
	const points = [{ point: { ...curve[0] }, t: 0 }]
	const visit = (segment: CubicBezier, depth: number, from: number, to: number) => {
		if (points.length >= 65536) throw Error("Bezier tessellation exceeds the segment limit.")
		const flatness = Math.max(segmentDistance(segment[1], segment[0], segment[3]), segmentDistance(segment[2], segment[0], segment[3]))
		if (flatness <= tolerance) {
			points.push({ point: { ...segment[3] }, t: to })
			return
		}
		if (depth >= 20) throw Error("Bezier tessellation cannot meet the requested tolerance.")
		const [left, right] = subdivide(segment, 0.5)
		const middle = (from + to) / 2
		visit(left, depth + 1, from, middle)
		visit(right, depth + 1, middle, to)
	}
	visit(curve, 0, 0, 1)
	return points
}
export function tessellateCubicBezier(curve: CubicBezier, tolerance: number): Point2D[] {
	return sampleCubicBezier(curve, tolerance).map((sample) => sample.point)
}
/** Tight axis-aligned bounds from endpoints and analytic derivative roots. */
export function cubicBezierBounds(curve: CubicBezier): { min: Point2D; max: Point2D } {
	validate(curve)
	const parameters = new Set([0, 1])
	for (const axis of ["x", "y"] as const) {
		const d0 = curve[1][axis] - curve[0][axis]
		const d1 = curve[2][axis] - curve[1][axis]
		const d2 = curve[3][axis] - curve[2][axis]
		const scale = Math.max(Math.abs(d0), Math.abs(d1), Math.abs(d2))
		if (scale === 0) continue
		const c = d0 / scale
		const b = 2 * (d1 / scale - c)
		const a = c - (2 * d1) / scale + d2 / scale
		const roots: number[] = []
		if (a === 0) {
			if (b !== 0) roots.push(-c / b)
		} else {
			const discriminant = b * b - 4 * a * c
			if (discriminant >= 0) {
				const q = -0.5 * (b + (b < 0 ? -1 : 1) * Math.sqrt(discriminant))
				if (q === 0) roots.push(-b / (2 * a))
				else roots.push(q / a, c / q)
			}
		}
		for (const t of roots) if (t > 0 && t < 1) parameters.add(t)
	}
	const points = [...parameters].map((t) => cubicBezierPoint(curve, t))
	return {
		min: { x: Math.min(...points.map((p) => p.x)), y: Math.min(...points.map((p) => p.y)) },
		max: { x: Math.max(...points.map((p) => p.x)), y: Math.max(...points.map((p) => p.y)) }
	}
}
/** Natural interpolating spline with uniform parameter intervals and free end curvature. */
export function interpolateBezierSpline(points: readonly Point2D[]): CubicBezier[] {
	if (points.length < 2 || points.length > 4096 || !points.every((p) => Number.isFinite(p.x) && Number.isFinite(p.y))) throw Error("A fit spline requires 2–4096 finite points.")
	for (let i = 1; i < points.length; i++) {
		const a = requireValue(points[i - 1])
		const b = requireValue(points[i])
		if (a.x === b.x && a.y === b.y) throw Error("Adjacent spline fit points must be distinct.")
	}
	const count = points.length
	const second = points.map(() => ({ x: 0, y: 0 }))
	const diagonal = Array<number>(count).fill(4)
	// Solve the tridiagonal natural-cubic system independently in x and y.
	for (const axis of ["x", "y"] as const) {
		const rhs = Array<number>(count).fill(0)
		for (let i = 1; i < count - 1; i++) rhs[i] = 6 * (requireValue(points[i + 1])[axis] - 2 * requireValue(points[i])[axis] + requireValue(points[i - 1])[axis])
		diagonal.fill(4)
		for (let i = 2; i < count - 1; i++) {
			const factor = 1 / requireValue(diagonal[i - 1])
			diagonal[i] = requireValue(diagonal[i]) - factor
			rhs[i] = requireValue(rhs[i]) - factor * requireValue(rhs[i - 1])
		}
		for (let i = count - 2; i >= 1; i--) requireValue(second[i])[axis] = (requireValue(rhs[i]) - requireValue(second[i + 1])[axis]) / requireValue(diagonal[i])
	}
	const segments: CubicBezier[] = []
	for (let i = 0; i < count - 1; i++) {
		const a = requireValue(points[i])
		const b = requireValue(points[i + 1])
		const m0 = requireValue(second[i])
		const m1 = requireValue(second[i + 1])
		const start = { x: b.x - a.x - (2 * m0.x + m1.x) / 6, y: b.y - a.y - (2 * m0.y + m1.y) / 6 }
		const end = { x: b.x - a.x + (m0.x + 2 * m1.x) / 6, y: b.y - a.y + (m0.y + 2 * m1.y) / 6 }
		segments.push([{ ...a }, { x: a.x + start.x / 3, y: a.y + start.y / 3 }, { x: b.x - end.x / 3, y: b.y - end.y / 3 }, { ...b }])
	}
	return segments
}

/** Uniform periodic cubic interpolation, including the last-to-first segment. */
export function interpolatePeriodicBezierSpline(points: readonly Point2D[]): CubicBezier[] {
	if (points.length < 3 || points.length > 4096 || !points.every((p) => Number.isFinite(p.x) && Number.isFinite(p.y))) throw Error("A periodic fit spline requires 3–4096 finite points.")
	const count = points.length
	const point = (i: number) => requireValue(points[(i + count) % count])
	for (let i = 0; i < count; i++) if (point(i).x === point(i + 1).x && point(i).y === point(i + 1).y) throw Error("Adjacent periodic fit points must be distinct, including the seam.")
	// Sherman–Morrison reduces the cyclic system to two tridiagonal solves.
	const solve = (values: number[]) => {
		const diagonal = Array<number>(count).fill(4)
		diagonal[0] = 8
		diagonal[count - 1] = 4.25
		const rhs = [...values]
		for (let i = 1; i < count; i++) {
			const factor = 1 / requireValue(diagonal[i - 1])
			diagonal[i] = requireValue(diagonal[i]) - factor
			rhs[i] = requireValue(rhs[i]) - factor * requireValue(rhs[i - 1])
		}
		const result = Array<number>(count).fill(0)
		for (let i = count - 1; i >= 0; i--) result[i] = (requireValue(rhs[i]) - (result[i + 1] ?? 0)) / requireValue(diagonal[i])
		return result
	}
	const correctionRhs = Array<number>(count).fill(0)
	correctionRhs[0] = -4
	correctionRhs[count - 1] = 1
	const correction = solve(correctionRhs)
	const second = points.map(() => ({ x: 0, y: 0 }))
	for (const axis of ["x", "y"] as const) {
		const values = solve(points.map((_, i) => 6 * (point(i + 1)[axis] - 2 * point(i)[axis] + point(i - 1)[axis])))
		const factor = (requireValue(values[0]) - requireValue(values[count - 1]) / 4) / (1 + requireValue(correction[0]) - requireValue(correction[count - 1]) / 4)
		for (let i = 0; i < count; i++) requireValue(second[i])[axis] = requireValue(values[i]) - factor * requireValue(correction[i])
	}
	return points.map((a, i) => {
		const b = point(i + 1)
		const m0 = requireValue(second[i])
		const m1 = requireValue(second[(i + 1) % count])
		return [
			{ ...a },
			{ x: a.x + (b.x - a.x - (2 * m0.x + m1.x) / 6) / 3, y: a.y + (b.y - a.y - (2 * m0.y + m1.y) / 6) / 3 },
			{ x: b.x - (b.x - a.x + (m0.x + 2 * m1.x) / 6) / 3, y: b.y - (b.y - a.y + (m0.y + 2 * m1.y) / 6) / 3 },
			{ ...b }
		]
	})
}
