import { expect, it } from "bun:test"
import type { Line, Sketch, SketchEntity } from "../src/schema"
import { solveSketch, normalizeSketchRelations, remapSketchRelations, type SketchRelation } from "../src/sketch-solver"
import { closestSketchCurvePoint, sketchCurveMidpoint } from "../src/sketch-point-constraints"
import { editSketchCurve } from "../src/sketch-edit"
const line: Line = { id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 0 } }
const marker: SketchEntity = { id: "marker", type: "circle", center: { x: 3, y: 7 }, radius: 1, segments: 32, construction: true }
const fixed: SketchRelation[] = [
	{ id: "start", type: "fixed", anchor: { entityId: "line", point: "p0" }, position: line.p0 },
	{ id: "end", type: "fixed", anchor: { entityId: "line", point: "p1" }, position: line.p1 },
	{ id: "radius", type: "radius", entityId: "marker", value: 1 }
]
it("tracks a line midpoint and retains one sliding freedom for a point on the line", () => {
	const midpoint: SketchRelation = { id: "mid", type: "midpoint", a: { entityId: "marker", point: "center" }, b: "line" }
	const result = solveSketch([line, marker], [...fixed, midpoint])
	expect(result.status).toBe("fully-constrained")
	expect(result.entities[1]).toMatchObject({ center: { x: expect.closeTo(10, 5), y: expect.closeTo(0, 5) } })
	const on = solveSketch([line, marker], [...fixed, { ...midpoint, type: "pointOnCurve" }])
	expect(on.degreesOfFreedom).toBe(1)
	expect(on.status).toBe("underconstrained")
	expect(on.entities[1]).toMatchObject({ center: { x: expect.closeTo(3, 5), y: expect.closeTo(0, 5) } })
})
it("uses finite line and clockwise arc extents", () => {
	expect(closestSketchCurvePoint(line, { x: 30, y: 4 })).toEqual({ x: 20, y: 0 })
	const arc: SketchEntity = { id: "arc", type: "arc", center: { x: 0, y: 0 }, radius: 10, startAngle: Math.PI, sweep: -Math.PI / 2, segments: 64 }
	expect(sketchCurveMidpoint(arc).x).toBeCloseTo(-Math.SQRT1_2 * 10, 8)
	expect(closestSketchCurvePoint(arc, { x: 10, y: 0 }).y).toBeCloseTo(10, 8)
	expect(closestSketchCurvePoint(arc, { x: -20, y: 0 }).x).toBeCloseTo(-10, 8)
})
it("solves a circle incidence even when the point starts at its center", () => {
	const circle: SketchEntity = { id: "curve", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 64 }
	const point = { ...marker, center: { x: 0, y: 0 } }
	const relations: SketchRelation[] = [
		{ id: "center", type: "fixed", anchor: { entityId: "curve", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "r", type: "radius", entityId: "curve", value: 10 },
		{ id: "size", type: "radius", entityId: "marker", value: 1 },
		{ id: "on", type: "pointOnCurve", a: { entityId: "marker", point: "center" }, b: "curve" }
	]
	const result = solveSketch([circle, point], relations)
	expect(result.status).toBe("underconstrained")
	expect(result.degreesOfFreedom).toBe(1)
	const solved = result.entities[1]
	if (solved?.type !== "circle") throw Error("Missing marker")
	expect(Math.hypot(solved.center.x, solved.center.y)).toBeCloseTo(10, 5)
})
it("reports a conflict when a fixed point lies beyond a finite segment", () => {
	const point = { ...marker, center: { x: 30, y: 0 } }
	const relations: SketchRelation[] = [
		...fixed,
		{ id: "point", type: "fixed", anchor: { entityId: "marker", point: "center" }, position: point.center },
		{ id: "on", type: "pointOnCurve", a: { entityId: "marker", point: "center" }, b: "line" }
	]
	expect(solveSketch([line, point], relations).status).toBe("conflicting")
})
it("persists and remaps point-to-curve relations and removes an invalidated trimmed reference", async () => {
	const relation: SketchRelation = { id: "mid", type: "midpoint", a: { entityId: "marker", point: "center" }, b: "line" }
	const normalized = normalizeSketchRelations(JSON.parse(JSON.stringify([relation])))
	expect(
		remapSketchRelations(
			normalized,
			new Map([
				["marker", "point-copy"],
				["line", "line-copy"]
			])
		)
	).toEqual([{ ...relation, a: { entityId: "point-copy", point: "center" }, b: "line-copy" }])
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [line, marker],
		relations: [...fixed, relation],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	expect(editSketchCurve(sketch, "line", { x: 10, y: 0 }, "Split").removedRelations).toContain("mid")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing sketch")
	expect(result.relations).toContainEqual(relation)
	expect(solveSketch(result.entities, result.relations ?? []).status).toBe("fully-constrained")
})
it("constrains points to native splines with interior sliding freedom and finite endpoint limits", () => {
	for (const mode of ["fit", "control"] as const) {
		const curve: import("../src/schema").Spline = {
			id: "curve",
			type: "spline",
			mode,
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			]
		}
		const locked: SketchRelation[] = curve.points.map((position, i) => ({ id: `fixed-${i}`, type: "fixed", anchor: { entityId: curve.id, point: `point${i}` }, position }))
		const point: SketchEntity = { id: "p", type: "point", center: { x: 5, y: 12 } }
		const relation: SketchRelation = { id: "on", type: "pointOnCurve", a: { entityId: "p", point: "center" }, b: "curve" }
		const result = solveSketch([curve, point], [...locked, relation])
		expect(result.status).toBe("underconstrained")
		expect(result.degreesOfFreedom).toBe(1)
		const p = result.entities[1]
		if (p?.type !== "point") throw Error("Missing point")
		const projected = closestSketchCurvePoint(curve, p.center)
		expect(p.center.x).toBeCloseTo(projected.x, 5)
		expect(p.center.y).toBeCloseTo(projected.y, 5)
		expect(point.center).toEqual({ x: 5, y: 12 })
		const outside = solveSketch([curve, { ...point, center: { x: -5, y: -5 } }], [...locked, relation])
		expect(outside.status).not.toBe("conflicting")
		const endpoint = outside.entities[1]
		if (endpoint?.type !== "point") throw Error("Missing point")
		expect(endpoint.center.x).toBeCloseTo(0, 4)
		expect(endpoint.center.y).toBeCloseTo(0, 4)
	}
})
