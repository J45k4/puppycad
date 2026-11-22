import { expect, it } from "bun:test"
import { lineSplineIntersections } from "../src/sketch-spline-intersections"
import { sketchSnap } from "../src/sketch-snap"
import type { Line, Spline } from "../src/schema"
const line: Line = { id: "l", type: "line", p0: { x: -1, y: 0 }, p1: { x: 2, y: 0 } }
const curve: Spline = {
	id: "s",
	type: "spline",
	mode: "control",
	points: [
		{ x: 0, y: -0.08 },
		{ x: 1 / 3, y: 0.14 },
		{ x: 2 / 3, y: -0.14 },
		{ x: 1, y: 0.08 }
	]
}
it("finds all three cubic crossings and respects finite line bounds", () => {
	const hits = lineSplineIntersections(line, curve)
	expect(hits).toHaveLength(3)
	for (const [i, t] of [0.2, 0.5, 0.8].entries()) {
		expect(hits[i]?.t).toBeCloseTo(t, 9)
		expect(hits[i]?.point.x).toBeCloseTo(t, 9)
		expect(hits[i]?.point.y).toBeCloseTo(0, 9)
	}
	expect(lineSplineIntersections({ ...line, p0: { x: 0.3, y: 0 }, p1: { x: 0.7, y: 0 } }, curve)).toHaveLength(1)
})
it("finds a repeated tangent root and does not invent contacts along coincident curves", () => {
	const tangent: Spline = { ...curve, points: curve.points.map((p, i) => ({ ...p, y: i === 0 || i === 3 ? 0.25 : -1 / 12 })) }
	const hits = lineSplineIntersections(line, tangent)
	expect(hits).toHaveLength(1)
	expect(hits[0]?.t).toBeCloseTo(0.5, 9)
	expect(lineSplineIntersections(line, { ...curve, points: curve.points.map((p) => ({ ...p, y: 0 })) })).toHaveLength(0)
})
it("offers spline-line crossing snaps with both dependency references", () => {
	const hit = sketchSnap([line, curve], { x: 0.2, y: 0.002 }, 1000, { grid: false })
	expect(hit?.kind).toBe("intersection")
	expect(hit?.curves).toEqual(["l", "s"])
	expect(hit?.position.x).toBeCloseTo(0.2, 8)
})

it("trims a line between spline crossings and extends it to a finite spline", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [line, curve],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const trimmed = editSketchCurve(source, "l", { x: 0.35, y: 0 }, "Trim")
	const pieces = trimmed.sketch.entities.filter((e) => e.type === "line")
	expect(pieces).toHaveLength(2)
	expect(pieces[0]?.p1.x).toBeCloseTo(0.2, 8)
	expect(pieces[1]?.p0.x).toBeCloseTo(0.5, 8)
	const short = { ...source, entities: [{ ...line, p0: { x: -1, y: 0 }, p1: { x: 0, y: 0 } }, curve] }
	const extended = editSketchCurve(short, "l", { x: 0, y: 0 }, "Extend")
	expect(extended.sketch.entities[0]).toMatchObject({ type: "line", p1: { x: expect.closeTo(0.2, 8), y: expect.closeTo(0, 8) } })
	expect(source.entities[0]).toEqual(line)
	expect(trimmed.sketch.entities.find((e) => e.id === "s")).toEqual(curve)
})

it("trims the picked spline interval between line crossings without flattening the retained portions", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { cubicBezierPoint } = await import("../src/sketch-bezier")
	const { splineBezierSegments, nearestSplinePoint } = await import("../src/sketch-spline")
	const { requireValue } = await import("../src/required")
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [line, curve],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const bezier = requireValue(splineBezierSegments(curve)[0])
	const trimmed = editSketchCurve(source, "s", cubicBezierPoint(bezier, 0.35), "Trim")
	const pieces = trimmed.sketch.entities.filter((e) => e.type === "spline")
	expect(pieces).toHaveLength(2)
	for (const t of [0, 0.1, 0.2, 0.5, 0.7, 1]) {
		const point = cubicBezierPoint(bezier, t)
		expect(Math.min(...pieces.map((piece) => nearestSplinePoint(piece, point).distance))).toBeLessThan(1e-6)
	}
	const removed = cubicBezierPoint(bezier, 0.35)
	expect(Math.min(...pieces.map((piece) => nearestSplinePoint(piece, removed).distance))).toBeGreaterThan(0.01)
	expect(trimmed.sketch.relations).toHaveLength(0)
})

it("trims a periodic spline across its seam into one open retained portion", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const periodic: Spline = {
		id: "s",
		type: "spline",
		mode: "fit",
		closed: true,
		points: [
			{ x: 20, y: 0 },
			{ x: 0, y: 20 },
			{ x: -20, y: 0 },
			{ x: 0, y: -20 }
		]
	}
	const boundary: Line = { id: "l", type: "line", p0: { x: 10, y: -30 }, p1: { x: 10, y: 30 } }
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [boundary, periodic],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const result = editSketchCurve(source, "s", { x: 20, y: 0 }, "Trim")
	const pieces = result.sketch.entities.filter((e) => e.type === "spline")
	expect(pieces).toHaveLength(1)
	expect(pieces[0]).toMatchObject({ closed: false, mode: "control" })
	expect(pieces[0]?.points[0]?.x).toBeCloseTo(10, 6)
	expect(pieces[0]?.points.at(-1)?.x).toBeCloseTo(10, 6)
	expect(pieces[0]?.points.some((p) => p.x < -19)).toBe(true)
})

it("retains contacts on surviving spline portions and removes a contact on the trimmed section after reload", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { cubicBezierPoint } = await import("../src/sketch-bezier")
	const { splineBezierSegments } = await import("../src/sketch-spline")
	const { requireValue } = await import("../src/required")
	const { solveSketch } = await import("../src/sketch-solver")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const bezier = requireValue(splineBezierSegments(curve)[0])
	const positions = [0.1, 0.35, 0.7].map((t) => cubicBezierPoint(bezier, t))
	const relations: import("../src/sketch-solver").SketchRelation[] = positions.map((_, i) => ({ id: `contact-${i}`, type: "pointOnCurve", a: { entityId: `p${i}`, point: "center" }, b: "s" }))
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [line, curve, ...positions.map((center, i): import("../src/schema").SketchEntity => ({ id: `p${i}`, type: "point", center }))],
		relations,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const result = editSketchCurve(source, "s", requireValue(positions[1]), "Trim")
	expect(result.removedRelations).toEqual(["contact-1"])
	const runtime = createPartRuntimeState({ features: [result.sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const loaded = materializePartFeatures(restored.cad, restored.tree)[0]
	if (loaded?.type !== "sketch") throw Error("Missing sketch")
	expect(loaded.relations).toContainEqual({ id: "contact-0", type: "pointOnCurve", a: { entityId: "p0", point: "center" }, b: "s" })
	expect(loaded.relations).toContainEqual({ id: "contact-2", type: "pointOnCurve", a: { entityId: "p2", point: "center" }, b: "s-split-1" })
	expect(loaded.relations).toHaveLength(2)
	expect(solveSketch(loaded.entities, loaded.relations ?? []).status).not.toBe("conflicting")
	expect(source.relations).toHaveLength(3)
})

it("finds circle crossings and tangencies and filters contacts outside a finite arc", async () => {
	const { circleSplineIntersections } = await import("../src/sketch-spline-intersections")
	const { finiteSketchCurveIntersections } = await import("../src/sketch-edit")
	const straight: Spline = { id: "s", type: "spline", mode: "control", points: [-2, -2 / 3, 2 / 3, 2].map((y) => ({ x: 0, y })) }
	const circle = { center: { x: 0, y: 0 }, radius: 1 }
	const hits = circleSplineIntersections(circle, straight)
	expect(hits).toHaveLength(2)
	expect(hits[0]?.point.y).toBeCloseTo(-1, 8)
	expect(hits[1]?.point.y).toBeCloseTo(1, 8)
	const tangent = circleSplineIntersections(circle, { ...straight, points: straight.points.map((p) => ({ ...p, x: 1 })) })
	expect(tangent).toHaveLength(1)
	expect(tangent[0]?.point.y).toBeCloseTo(0, 8)
	const arc: import("../src/schema").Arc = { id: "arc", type: "arc", ...circle, startAngle: 0, sweep: Math.PI, segments: 64 }
	const upper = finiteSketchCurveIntersections(straight, arc)
	expect(upper).toHaveLength(1)
	expect(upper[0]?.y).toBeCloseTo(1, 8)
})

it("trims a spline against a circle while retaining its outer portions", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const straight: Spline = { id: "s", type: "spline", mode: "control", points: [-2, -2 / 3, 2 / 3, 2].map((x) => ({ x, y: 0 })) }
	const circle: import("../src/schema").Circle = { id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 1, segments: 64 }
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [straight, circle],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const result = editSketchCurve(source, "s", { x: 0, y: 0 }, "Trim")
	const pieces = result.sketch.entities.filter((e) => e.type === "spline")
	expect(pieces).toHaveLength(2)
	expect(pieces[0]?.points.at(-1)?.x).toBeCloseTo(-1, 8)
	expect(pieces[1]?.points[0]?.x).toBeCloseTo(1, 8)
})

it("retains distinct curve parameters when a cubic repeatedly crosses the same circle points", async () => {
	const { circleSplineIntersections } = await import("../src/sketch-spline-intersections")
	const retracing: Spline = { id: "s", type: "spline", mode: "control", points: [-2, 10, -10, 2].map((x) => ({ x, y: 0 })) }
	const hits = circleSplineIntersections({ center: { x: 0, y: 0 }, radius: 1 }, retracing)
	expect(hits).toHaveLength(6)
	for (const hit of hits) expect(Math.abs(hit.point.x)).toBeCloseTo(1, 7)
	expect(new Set(hits.map((hit) => hit.t.toFixed(7))).size).toBe(6)
	const crossing: Line = { id: "l", type: "line", p0: { x: 0, y: -1 }, p1: { x: 0, y: 1 } }
	expect(lineSplineIntersections(crossing, retracing)).toHaveLength(3)
})

it("trims circles and extends finite arcs using spline boundaries", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { arcPoint } = await import("../src/sketch-curves")
	const horizontal: Spline = { id: "s", type: "spline", mode: "control", points: [-2, -2 / 3, 2 / 3, 2].map((x) => ({ x, y: 0 })) }
	const circle: import("../src/schema").Circle = { id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 1, segments: 64 }
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [circle, horizontal],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const trimmed = editSketchCurve(source, "c", { x: 0, y: 1 }, "Trim")
	const lower = trimmed.sketch.entities[0]
	if (lower?.type !== "arc") throw Error("Missing retained arc")
	expect(lower.sweep).toBeCloseTo(Math.PI, 8)
	expect(arcPoint(lower, 0.5).y).toBeCloseTo(-1, 8)
	const arc: import("../src/schema").Arc = { ...circle, type: "arc", startAngle: 0, sweep: Math.PI / 4 }
	const vertical: Spline = { ...horizontal, points: horizontal.points.map((p) => ({ x: 0, y: p.x })) }
	const extended = editSketchCurve({ ...source, entities: [arc, vertical] }, "c", arcPoint(arc, 1), "Extend")
	const upper = extended.sketch.entities[0]
	if (upper?.type !== "arc") throw Error("Missing extended arc")
	expect(upper.sweep).toBeCloseTo(Math.PI / 2, 8)
	expect(arcPoint(upper, 1).y).toBeCloseTo(1, 8)
})

it("intersects rotated ellipses in native spline parameters and uses them as trim boundaries", async () => {
	const { ellipseSplineIntersections } = await import("../src/sketch-spline-intersections")
	const { editSketchCurve } = await import("../src/sketch-edit")
	const ellipse: import("../src/schema").Ellipse = { id: "e", type: "ellipse", center: { x: 5, y: -3 }, width: 8, height: 2, rotation: 30, segments: 64 }
	const angle = Math.PI / 6
	const along = (x: number) => ({ x: 5 + x * Math.cos(angle), y: -3 + x * Math.sin(angle) })
	const spline: Spline = { id: "s", type: "spline", mode: "control", points: [-8, -8 / 3, 8 / 3, 8].map(along) }
	const hits = ellipseSplineIntersections(ellipse, spline)
	expect(hits).toHaveLength(2)
	expect(hits[0]?.t).toBeCloseTo(0.25, 8)
	expect(hits[1]?.t).toBeCloseTo(0.75, 8)
	expect(hits[0]?.point.x).toBeCloseTo(along(-4).x, 8)
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [ellipse, spline],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const trimmed = editSketchCurve(source, "s", along(0), "Trim")
	expect(trimmed.sketch.entities.filter((e) => e.type === "spline")).toHaveLength(2)
	expect(() => ellipseSplineIntersections({ ...ellipse, width: 0 }, spline)).toThrow("positive axes")
})

it("aggregates whole-spline crossings without duplicating a shared segment join", async () => {
	const { splineSplineIntersections } = await import("../src/sketch-spline-intersections")
	const a: Spline = { id: "a", type: "spline", mode: "control", points: [-2, -4 / 3, -2 / 3, 0, 2 / 3, 4 / 3, 2].map((x) => ({ x, y: 0 })) }
	const b: Spline = {
		id: "b",
		type: "spline",
		mode: "fit",
		points: [
			{ x: 0, y: -2 },
			{ x: 0, y: 0 },
			{ x: 0, y: 2 }
		]
	}
	const hits = splineSplineIntersections(a, b)
	expect(hits).toHaveLength(1)
	expect(hits[0]?.point.x).toBeCloseTo(0, 7)
	expect(hits[0]?.point.y).toBeCloseTo(0, 7)
	const reversed = splineSplineIntersections(b, a)
	expect(reversed).toHaveLength(1)
	expect(reversed[0]?.point.x).toBeCloseTo(0, 7)
})

it("snaps to spline pairs and trims one spline at the other without changing its boundary", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const a: Spline = {
		id: "a",
		type: "spline",
		mode: "fit",
		points: [
			{ x: 0, y: 0 },
			{ x: 10, y: 10 }
		]
	}
	const b: Spline = {
		id: "b",
		type: "spline",
		mode: "fit",
		points: [
			{ x: 0, y: 10 },
			{ x: 10, y: 0 }
		]
	}
	const snap = sketchSnap([a, b], { x: 5.05, y: 5 }, 100, { grid: false })
	expect(snap?.kind).toBe("intersection")
	expect(snap?.curves).toEqual(["a", "b"])
	expect(snap?.position.x).toBeCloseTo(5, 6)
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [a, b],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const result = editSketchCurve(source, "a", { x: 2, y: 2 }, "Trim")
	const retained = result.sketch.entities[0]
	if (retained?.type !== "spline") throw Error("Missing spline")
	expect(retained.points[0]?.x).toBeCloseTo(5, 6)
	expect(retained.points.at(-1)).toEqual({ x: 10, y: 10 })
	expect(result.sketch.entities[1]).toEqual(b)
	expect(source.entities[0]).toEqual(a)
})
