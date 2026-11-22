import { boundedPolynomialRoots } from "./sketch-ellipse-intersections"
import { requireValue } from "./required"
import { cubicBezierPoint, cubicBezierBounds, cubicBezierDerivative, splitCubicBezier, type CubicBezier } from "./sketch-bezier"
import type { Point2D } from "./types"

/** Identify matching polynomial portions from endpoint correspondences. */
function overlappingPortions(a: CubicBezier, b: CubicBezier): boolean {
	const scale = Math.max(1, ...[...a, ...b].flatMap((p) => [Math.abs(p.x), Math.abs(p.y)]))
	const epsilon = 128 * Number.EPSILON * scale
	const controls = [...a, ...b]
	const origin = a[0]
	const farthest = controls.reduce((best, p) => (Math.hypot(p.x - origin.x, p.y - origin.y) > Math.hypot(best.x - origin.x, best.y - origin.y) ? p : best), origin)
	const length = Math.hypot(farthest.x - origin.x, farthest.y - origin.y)
	if (length > epsilon) {
		const ux = (farthest.x - origin.x) / length
		const uy = (farthest.y - origin.y) / length
		if (controls.every((p) => Math.abs((p.x - origin.x) * uy - (p.y - origin.y) * ux) <= epsilon)) {
			const project = (p: Point2D): Point2D => ({ x: (p.x - origin.x) * ux + (p.y - origin.y) * uy, y: 0 })
			const range = (curve: CubicBezier) => cubicBezierBounds([project(curve[0]), project(curve[1]), project(curve[2]), project(curve[3])])
			const x = range(a)
			const y = range(b)
			if (Math.min(x.max.x, y.max.x) - Math.max(x.min.x, y.min.x) > epsilon) return true
		}
	}
	const parameters = (curve: CubicBezier, point: Point2D) => {
		const span = (axis: "x" | "y") => Math.max(...curve.map((p) => p[axis])) - Math.min(...curve.map((p) => p[axis]))
		const axis = span("x") >= span("y") ? "x" : "y"
		const [p, q, r, s] = curve.map((v) => v[axis] - point[axis])
		const x = requireValue(p)
		const y = requireValue(q)
		const z = requireValue(r)
		const w = requireValue(s)
		return boundedPolynomialRoots([x, 3 * (y - x), 3 * (x - 2 * y + z), w - x + 3 * (y - z)]).filter((t) => {
			if (t < 0 || t > 1) return false
			const p = cubicBezierPoint(curve, t)
			return Math.hypot(p.x - point.x, p.y - point.y) <= epsilon
		})
	}
	const pairs: { t: number; u: number }[] = []
	for (const t of [0, 1]) for (const u of parameters(b, cubicBezierPoint(a, t))) pairs.push({ t, u })
	for (const u of [0, 1]) for (const t of parameters(a, cubicBezierPoint(b, u))) pairs.push({ t, u })
	const portion = (curve: CubicBezier, from: number, to: number): CubicBezier => {
		const lo = Math.min(from, to)
		const hi = Math.max(from, to)
		let result = hi < 1 ? splitCubicBezier(curve, hi)[0] : curve
		if (lo > 0) result = splitCubicBezier(result, lo / hi)[1]
		return from <= to ? result : [result[3], result[2], result[1], result[0]]
	}
	for (let i = 0; i < pairs.length; i++)
		for (const q of pairs.slice(i + 1)) {
			const p = requireValue(pairs[i])
			if (Math.abs(p.t - q.t) < 1e-10 || Math.abs(p.u - q.u) < 1e-10) continue
			const x = portion(a, p.t, q.t)
			const y = portion(b, p.u, q.u)
			if (x.every((point, j) => Math.hypot(point.x - requireValue(y[j]).x, point.y - requireValue(y[j]).y) <= epsilon)) return true
		}
	return false
}

/** Isolated contacts by conservative control-hull subdivision, in sketch units. */
function intersectLocalCubics(a: CubicBezier, b: CubicBezier, tolerance = 1e-7): { point: Point2D; t: number; u: number }[] {
	if (!Number.isFinite(tolerance) || tolerance <= 0) throw Error("Intersection tolerance must be positive and finite.")
	if (![...a, ...b].every((p) => Number.isFinite(p.x) && Number.isFinite(p.y))) throw Error("Intersection coordinates must be finite.")
	const equal = (p: Point2D, q: Point2D) => p.x === q.x && p.y === q.y
	if (a.every((p, i) => equal(p, b[i] ?? p)) || a.every((p, i) => equal(p, b[3 - i] ?? p))) return []
	if (overlappingPortions(a, b)) return []
	const bounds = (curve: CubicBezier) => ({
		minX: Math.min(...curve.map((p) => p.x)),
		maxX: Math.max(...curve.map((p) => p.x)),
		minY: Math.min(...curve.map((p) => p.y)),
		maxY: Math.max(...curve.map((p) => p.y))
	})
	const results: { point: Point2D; t: number; u: number }[] = []
	let visits = 0
	const visit = (left: CubicBezier, right: CubicBezier, t0: number, t1: number, u0: number, u1: number, depth: number) => {
		if (++visits > 200000 || depth > 100) throw Error("Spline intersections exceed the subdivision limit.")
		const x = bounds(left)
		const y = bounds(right)
		const rounding = 16 * Number.EPSILON * Math.max(1, ...Object.values(x).map(Math.abs), ...Object.values(y).map(Math.abs))
		if (x.maxX + rounding < y.minX || y.maxX + rounding < x.minX || x.maxY + rounding < y.minY || y.maxY + rounding < x.minY) return
		const sizeA = Math.hypot(x.maxX - x.minX, x.maxY - x.minY)
		const sizeB = Math.hypot(y.maxX - y.minX, y.maxY - y.minY)
		if (Math.max(sizeA, sizeB) <= tolerance) {
			let t = (t0 + t1) / 2
			let u = (u0 + u1) / 2
			for (let step = 0; step < 24; step++) {
				const p = cubicBezierPoint(a, t)
				const q = cubicBezierPoint(b, u)
				const da = cubicBezierDerivative(a, t)
				const db = cubicBezierDerivative(b, u)
				const det = da.x * db.y - da.y * db.x
				if (Math.abs(det) < 1e-24) break
				const dx = q.x - p.x
				const dy = q.y - p.y
				const dt = (dx * db.y - dy * db.x) / det
				const du = (dx * da.y - dy * da.x) / det
				if (Math.max(Math.abs(dt), Math.abs(du)) > 0.1) break
				const nextT = Math.max(0, Math.min(1, t + dt))
				const nextU = Math.max(0, Math.min(1, u + du))
				if (nextT === t && nextU === u) break
				t = nextT
				u = nextU
			}
			const p = cubicBezierPoint(a, t)
			const q = cubicBezierPoint(b, u)
			if (Math.hypot(p.x - q.x, p.y - q.y) <= tolerance && !results.some((hit) => Math.abs(hit.t - t) < 1e-7 && Math.abs(hit.u - u) < 1e-7))
				results.push({ point: { x: (p.x + q.x) / 2, y: (p.y + q.y) / 2 }, t, u })
			return
		}
		if (sizeA >= sizeB) {
			const [first, second] = splitCubicBezier(left, 0.5)
			const mid = (t0 + t1) / 2
			visit(first, right, t0, mid, u0, u1, depth + 1)
			visit(second, right, mid, t1, u0, u1, depth + 1)
		} else {
			const [first, second] = splitCubicBezier(right, 0.5)
			const mid = (u0 + u1) / 2
			visit(left, first, t0, t1, u0, mid, depth + 1)
			visit(left, second, t0, t1, mid, u1, depth + 1)
		}
	}
	visit(a, b, 0, 1, 0, 1, 0)
	return results.sort((p, q) => p.t - q.t || p.u - q.u)
}

/** Translation keeps roundoff tolerances independent of distance from the model origin. */
export function intersectCubicBeziers(a: CubicBezier, b: CubicBezier, tolerance = 1e-7): { point: Point2D; t: number; u: number }[] {
	const origin = a[0]
	const move = (curve: CubicBezier): CubicBezier => {
		const point = (p: Point2D): Point2D => ({ x: p.x - origin.x, y: p.y - origin.y })
		return [point(curve[0]), point(curve[1]), point(curve[2]), point(curve[3])]
	}
	return intersectLocalCubics(move(a), move(b), tolerance).map((hit) => ({ ...hit, point: { x: hit.point.x + origin.x, y: hit.point.y + origin.y } }))
}
