import { requireValue } from "../src/required"
import { expect, it } from "bun:test"
import { sampleCubicBezier, interpolateBezierSpline, cubicBezierBounds, cubicBezierPoint, cubicBezierDerivative, splitCubicBezier, tessellateCubicBezier, type CubicBezier } from "../src/sketch-bezier"
const curve: CubicBezier = [
	{ x: 0, y: 0 },
	{ x: 0, y: 10 },
	{ x: 10, y: 10 },
	{ x: 10, y: 0 }
]
it("evaluates native cubic positions and endpoint tangents", () => {
	expect(cubicBezierPoint(curve, 0)).toEqual(curve[0])
	expect(cubicBezierPoint(curve, 1)).toEqual(curve[3])
	expect(cubicBezierPoint(curve, 0.5)).toEqual({ x: 5, y: 7.5 })
	expect(cubicBezierDerivative(curve, 0)).toEqual({ x: 0, y: 30 })
	expect(cubicBezierDerivative(curve, 1)).toEqual({ x: 0, y: -30 })
})
it("splits without changing the curve and without sharing mutable control points", () => {
	const [left, right] = splitCubicBezier(curve, 0.3)
	for (let i = 0; i <= 20; i++) {
		const t = i / 20
		for (const [piece, sourceT] of [
			[left, t * 0.3],
			[right, 0.3 + t * 0.7]
		] as const) {
			const p = cubicBezierPoint(piece, t)
			const q = cubicBezierPoint(curve, sourceT)
			expect(p.x).toBeCloseTo(q.x, 10)
			expect(p.y).toBeCloseTo(q.y, 10)
		}
	}
	left[3].x = 100
	expect(right[0].x).not.toBe(100)
	expect(curve[0]).toEqual({ x: 0, y: 0 })
})
it("adapts tessellation and preserves loops and collinear reversals", () => {
	expect(tessellateCubicBezier(curve, 0.01).length).toBeGreaterThan(tessellateCubicBezier(curve, 1).length)
	const loop: CubicBezier = [
		{ x: 0, y: 0 },
		{ x: 10, y: 20 },
		{ x: -10, y: 20 },
		{ x: 0, y: 0 }
	]
	expect(tessellateCubicBezier(loop, 0.01).length).toBeGreaterThan(10)
	const reverse: CubicBezier = [
		{ x: 0, y: 0 },
		{ x: 20, y: 0 },
		{ x: -20, y: 0 },
		{ x: 1, y: 0 }
	]
	const points = tessellateCubicBezier(reverse, 0.01)
	expect(Math.max(...points.map((p) => p.x))).toBeGreaterThan(1)
	expect(Math.min(...points.map((p) => p.x))).toBeLessThan(0)
	expect(() => tessellateCubicBezier(curve, 0)).toThrow()
	expect(() => cubicBezierPoint(curve, 2)).toThrow()
})

it("finds tight bounds at interior extrema, including degenerate and reversed cubics", () => {
	expect(cubicBezierBounds(curve)).toEqual({ min: { x: 0, y: 0 }, max: { x: 10, y: 7.5 } })
	const cases: CubicBezier[] = [
		[
			{ x: 0, y: 0 },
			{ x: 20, y: 0 },
			{ x: -20, y: 0 },
			{ x: 1, y: 0 }
		],
		[
			{ x: 2, y: 3 },
			{ x: 2, y: 3 },
			{ x: 2, y: 3 },
			{ x: 2, y: 3 }
		],
		[
			{ x: 0, y: 0 },
			{ x: 1, y: 1 },
			{ x: 2, y: 1 },
			{ x: 3, y: 0 }
		]
	]
	for (const cubic of cases) {
		const bounds = cubicBezierBounds(cubic)
		const reversed: CubicBezier = [cubic[3], cubic[2], cubic[1], cubic[0]]
		const reverseBounds = cubicBezierBounds(reversed)
		for (const axis of ["x", "y"] as const) {
			expect(bounds.min[axis]).toBeCloseTo(reverseBounds.min[axis], 10)
			expect(bounds.max[axis]).toBeCloseTo(reverseBounds.max[axis], 10)
		}
		for (let i = 0; i <= 100; i++) {
			const p = cubicBezierPoint(cubic, i / 100)
			expect(p.x).toBeGreaterThanOrEqual(bounds.min.x - 1e-12)
			expect(p.x).toBeLessThanOrEqual(bounds.max.x + 1e-12)
			expect(p.y).toBeGreaterThanOrEqual(bounds.min.y - 1e-12)
			expect(p.y).toBeLessThanOrEqual(bounds.max.y + 1e-12)
		}
	}
})

it("interpolates fit points with continuous first and second derivatives", () => {
	const points = [
		{ x: 0, y: 0 },
		{ x: 3, y: 5 },
		{ x: 9, y: -2 },
		{ x: 15, y: 4 }
	]
	const segments = interpolateBezierSpline(points)
	expect(segments).toHaveLength(3)
	const second = (s: CubicBezier, end: boolean) =>
		end ? { x: 6 * (s[3].x - 2 * s[2].x + s[1].x), y: 6 * (s[3].y - 2 * s[2].y + s[1].y) } : { x: 6 * (s[2].x - 2 * s[1].x + s[0].x), y: 6 * (s[2].y - 2 * s[1].y + s[0].y) }
	for (const [i, segment] of segments.entries()) {
		expect(cubicBezierPoint(segment, 0)).toEqual(requireValue(points[i]))
		expect(cubicBezierPoint(segment, 1)).toEqual(requireValue(points[i + 1]))
		const next = segments[i + 1]
		if (next)
			for (const axis of ["x", "y"] as const) {
				expect(cubicBezierDerivative(segment, 1)[axis]).toBeCloseTo(cubicBezierDerivative(next, 0)[axis], 10)
				expect(second(segment, true)[axis]).toBeCloseTo(second(next, false)[axis], 10)
			}
	}
	for (const axis of ["x", "y"] as const) {
		expect(second(requireValue(segments[0]), false)[axis]).toBeCloseTo(0, 10)
		expect(second(requireValue(segments[2]), true)[axis]).toBeCloseTo(0, 10)
	}
	expect(
		interpolateBezierSpline([
			{ x: 0, y: 0 },
			{ x: 9, y: 0 }
		])[0]
	).toEqual([
		{ x: 0, y: 0 },
		{ x: 3, y: 0 },
		{ x: 6, y: 0 },
		{ x: 9, y: 0 }
	])
	expect(() => interpolateBezierSpline([{ x: 0, y: 0 }])).toThrow()
	expect(() =>
		interpolateBezierSpline([
			{ x: 0, y: 0 },
			{ x: 0, y: 0 }
		])
	).toThrow()
})

it("retains ordered native parameters and bounds tessellation deviation", () => {
	const samples = sampleCubicBezier(curve, 0.02)
	expect(samples[0]?.t).toBe(0)
	expect(samples.at(-1)?.t).toBe(1)
	for (let i = 1; i < samples.length; i++) {
		const a = requireValue(samples[i - 1])
		const b = requireValue(samples[i])
		expect(b.t).toBeGreaterThan(a.t)
		const exact = cubicBezierPoint(curve, b.t)
		expect(b.point.x).toBeCloseTo(exact.x, 12)
		expect(b.point.y).toBeCloseTo(exact.y, 12)
		for (let k = 1; k < 10; k++) {
			const p = cubicBezierPoint(curve, a.t + ((b.t - a.t) * k) / 10)
			const dx = b.point.x - a.point.x
			const dy = b.point.y - a.point.y
			const ratio = Math.max(0, Math.min(1, ((p.x - a.point.x) * dx + (p.y - a.point.y) * dy) / (dx * dx + dy * dy)))
			expect(Math.hypot(p.x - a.point.x - ratio * dx, p.y - a.point.y - ratio * dy)).toBeLessThanOrEqual(0.02)
		}
	}
})
