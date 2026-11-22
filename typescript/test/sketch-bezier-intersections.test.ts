import { expect, it } from "bun:test"
import { intersectCubicBeziers } from "../src/sketch-bezier-intersections"
import { cubicBezierPoint, type CubicBezier } from "../src/sketch-bezier"
const horizontal: CubicBezier = [
	{ x: -2, y: 0 },
	{ x: -2 / 3, y: 0 },
	{ x: 2 / 3, y: 0 },
	{ x: 2, y: 0 }
]
it("locates three crossings with parameters on both native cubics", () => {
	const curve: CubicBezier = [
		{ x: 0, y: -0.08 },
		{ x: 1 / 3, y: 0.14 },
		{ x: 2 / 3, y: -0.14 },
		{ x: 1, y: 0.08 }
	]
	const hits = intersectCubicBeziers(curve, horizontal)
	expect(hits).toHaveLength(3)
	for (const [i, t] of [0.2, 0.5, 0.8].entries()) {
		expect(hits[i]?.t).toBeCloseTo(t, 5)
		const hit = hits[i]
		if (!hit) throw Error("Missing contact")
		const a = cubicBezierPoint(curve, hit.t)
		const b = cubicBezierPoint(horizontal, hit.u)
		expect(Math.hypot(a.x - b.x, a.y - b.y)).toBeLessThan(1e-7)
	}
})
it("rejects separated control hulls and identical overlapping curves", () => {
	expect(intersectCubicBeziers(horizontal, horizontal)).toHaveLength(0)
	const above: CubicBezier = [
		{ x: -2, y: 1 },
		{ x: -1, y: 2 },
		{ x: 1, y: 2 },
		{ x: 2, y: 1 }
	]
	expect(intersectCubicBeziers(horizontal, above)).toHaveLength(0)
	expect(() => intersectCubicBeziers(horizontal, above, 0)).toThrow("tolerance")
})

it("returns one tangent contact and one shared endpoint", () => {
	const tangent: CubicBezier = [
		{ x: 0, y: 0.25 },
		{ x: 1 / 3, y: -1 / 12 },
		{ x: 2 / 3, y: -1 / 12 },
		{ x: 1, y: 0.25 }
	]
	const hits = intersectCubicBeziers(tangent, horizontal)
	expect(hits).toHaveLength(1)
	expect(hits[0]?.t).toBeCloseTo(0.5, 5)
	const separated: CubicBezier = [tangent[0], tangent[1], tangent[2], tangent[3]].map((p) => ({ x: p.x, y: p.y + 1e-6 })) as unknown as CubicBezier
	expect(intersectCubicBeziers(separated, horizontal)).toHaveLength(0)
	const endpoint: CubicBezier = [
		{ x: 2, y: 0 },
		{ x: 2, y: 1 },
		{ x: 3, y: 1 },
		{ x: 3, y: 2 }
	]
	const ends = intersectCubicBeziers(horizontal, endpoint)
	expect(ends).toHaveLength(1)
	expect(ends[0]?.t).toBeCloseTo(1, 5)
	expect(ends[0]?.u).toBeCloseTo(0, 5)
})

it("excludes partial curved overlaps including reversed subcurves", async () => {
	const { splitCubicBezier } = await import("../src/sketch-bezier")
	const curve: CubicBezier = [
		{ x: 0, y: 0 },
		{ x: 1, y: 3 },
		{ x: 3, y: -1 },
		{ x: 4, y: 2 }
	]
	const left = splitCubicBezier(curve, 0.7)[0]
	const right = splitCubicBezier(curve, 0.3)[1]
	expect(intersectCubicBeziers(left, right)).toHaveLength(0)
	const reversed: CubicBezier = [right[3], right[2], right[1], right[0]]
	expect(intersectCubicBeziers(left, reversed)).toHaveLength(0)
	expect(intersectCubicBeziers(curve, right)).toHaveLength(0)
})

it("excludes collinear overlap with different parameter speeds and retracing", () => {
	const nonlinear: CubicBezier = [
		{ x: -1, y: 0 },
		{ x: -1, y: 0 },
		{ x: 1, y: 0 },
		{ x: 1, y: 0 }
	]
	expect(intersectCubicBeziers(horizontal, nonlinear)).toHaveLength(0)
	const retracing: CubicBezier = [
		{ x: -1, y: 0 },
		{ x: 10, y: 0 },
		{ x: -10, y: 0 },
		{ x: 1, y: 0 }
	]
	expect(intersectCubicBeziers(horizontal, retracing)).toHaveLength(0)
})

it("preserves crossing parameters after a large coordinate translation", () => {
	const a: CubicBezier = [
		{ x: 0, y: 0 },
		{ x: 1 / 3, y: 0 },
		{ x: 2 / 3, y: 0 },
		{ x: 1, y: 0 }
	]
	const b: CubicBezier = [
		{ x: 0, y: -0.001 },
		{ x: 1 / 3, y: -0.000333333333 },
		{ x: 2 / 3, y: 0.000333333333 },
		{ x: 1, y: 0.001 }
	]
	const move = (c: CubicBezier): CubicBezier => {
		const point = (p: { x: number; y: number }) => ({ x: p.x + 1e12, y: p.y })
		return [point(c[0]), point(c[1]), point(c[2]), point(c[3])]
	}
	const result = intersectCubicBeziers(move(a), move(b))
	expect(result).toHaveLength(1)
	expect(result[0]?.t).toBeCloseTo(0.5, 5)
	expect(result[0]?.u).toBeCloseTo(0.5, 5)
})
