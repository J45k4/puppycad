import { expect, it } from "bun:test"
import { interpolatePeriodicBezierSpline, cubicBezierDerivative, type CubicBezier } from "../src/sketch-bezier"
import { requireValue } from "../src/required"

it("interpolates a periodic loop with first and second derivative continuity at every join", () => {
	for (const count of [3, 4, 7, 64, 4096]) {
		const points = Array.from({ length: count }, (_, i) => ({ x: 13 * Math.cos((2 * Math.PI * i) / count), y: 8 * Math.sin((2 * Math.PI * i) / count) }))
		const before = structuredClone(points)
		const curves = interpolatePeriodicBezierSpline(points)
		expect(curves).toHaveLength(count)
		const second = (c: CubicBezier, end: boolean, axis: "x" | "y") => (end ? 6 * (c[3][axis] - 2 * c[2][axis] + c[1][axis]) : 6 * (c[2][axis] - 2 * c[1][axis] + c[0][axis]))
		for (let i = 0; i < count; i++) {
			const a = requireValue(curves[i])
			const b = requireValue(curves[(i + 1) % count])
			expect(a[0]).toEqual(requireValue(points[i]))
			expect(a[3]).toEqual(b[0])
			for (const axis of ["x", "y"] as const) {
				expect(cubicBezierDerivative(a, 1)[axis]).toBeCloseTo(cubicBezierDerivative(b, 0)[axis], 9)
				expect(second(a, true, axis)).toBeCloseTo(second(b, false, axis), 9)
			}
		}
		expect(points).toEqual(before)
	}
})
it("rejects insufficient, non-finite and repeated seam points", () => {
	expect(() =>
		interpolatePeriodicBezierSpline([
			{ x: 0, y: 0 },
			{ x: 1, y: 1 }
		])
	).toThrow("3–4096")
	expect(() =>
		interpolatePeriodicBezierSpline([
			{ x: 0, y: 0 },
			{ x: 1, y: 1 },
			{ x: 0, y: 0 }
		])
	).toThrow("seam")
	expect(() =>
		interpolatePeriodicBezierSpline([
			{ x: 0, y: 0 },
			{ x: Number.POSITIVE_INFINITY, y: 1 },
			{ x: 3, y: 0 }
		])
	).toThrow("finite")
})

it("retains native closure and excludes closed curves from endpoint operations", async () => {
	const { normalizeSpline, sampleSpline, splitSpline } = await import("../src/sketch-spline")
	const { sketchSnap } = await import("../src/sketch-snap")
	const { sketchEndpointDirection } = await import("../src/sketch-tangent-arc")
	const spline = normalizeSpline({
		id: "s",
		type: "spline",
		mode: "fit",
		closed: true,
		points: [
			{ x: 0, y: 0 },
			{ x: 20, y: 0 },
			{ x: 10, y: 20 }
		]
	})
	expect(spline.closed).toBe(true)
	const samples = sampleSpline(spline, 0.01)
	expect(samples[0]?.point).toEqual(samples.at(-1)?.point)
	expect(sketchSnap([spline], { x: 0, y: 0 }, 3, { endpointsOnly: true })).toBeNull()
	expect(() => sketchEndpointDirection(spline, "point0")).toThrow("no endpoints")
	expect(() => splitSpline(spline, 0, 0.5, "other")).toThrow("periodic")
	expect(() => normalizeSpline({ ...spline, closed: "yes" })).toThrow("closure")
})

it("keeps point-on-curve sliding freedom across a periodic seam after deformation", async () => {
	const { solveSketch } = await import("../src/sketch-solver")
	const { nearestSplinePoint } = await import("../src/sketch-spline")
	const curve: import("../src/schema").Spline = {
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
	const fixed: import("../src/sketch-solver").SketchRelation[] = curve.points.map((position, i) => ({ id: `f${i}`, type: "fixed", anchor: { entityId: "s", point: `point${i}` }, position }))
	const on: import("../src/sketch-solver").SketchRelation = { id: "on", type: "pointOnCurve", a: { entityId: "p", point: "center" }, b: "s" }
	for (const y of [-0.01, 0, 0.01]) {
		const result = solveSketch([curve, { id: "p", type: "point", center: { x: 23, y } }], [...fixed, on])
		expect(result.status).toBe("underconstrained")
		expect(result.degreesOfFreedom).toBe(1)
		const point = result.entities[1]
		if (point?.type !== "point") throw Error("Missing point")
		expect(nearestSplinePoint(curve, point.center).distance).toBeLessThan(1e-6)
		const changed = fixed.map((r) => (r.id === "f0" && r.type === "fixed" ? { ...r, position: { x: 25, y: 0 } } : r))
		const deformed = solveSketch(result.entities, [...changed, on])
		expect(deformed.status).toBe("underconstrained")
		expect(deformed.degreesOfFreedom).toBe(1)
		const spline = deformed.entities[0]
		const moved = deformed.entities[1]
		if (spline?.type !== "spline" || moved?.type !== "point") throw Error("Missing geometry")
		expect(nearestSplinePoint(spline, moved.center).distance).toBeLessThan(1e-6)
		const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
		const sketch: import("../src/schema").Sketch = {
			id: "sk",
			type: "sketch",
			dirty: false,
			target: { type: "plane", plane: "XY" },
			entities: deformed.entities,
			relations: [...changed, on],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}
		const runtime = createPartRuntimeState({ features: [sketch] })
		const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
		const loaded = materializePartFeatures(restored.cad, restored.tree)[0]
		if (loaded?.type !== "sketch") throw Error("Missing sketch")
		expect(loaded.entities[0]).toMatchObject({ closed: true })
		expect(loaded.relations).toContainEqual(on)
		expect(solveSketch(loaded.entities, loaded.relations ?? []).degreesOfFreedom).toBe(1)
	}
})

it("opens periodic curves at a segment or seam without altering their locus", async () => {
	const { openPeriodicSpline, splineBezierSegments, nearestSplinePoint } = await import("../src/sketch-spline")
	const { cubicBezierPoint } = await import("../src/sketch-bezier")
	const source: import("../src/schema").Spline = {
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
	for (const t of [0, 0.3, 1]) {
		const opened = openPeriodicSpline(source, 2, t)
		expect(opened).toMatchObject({ id: "s", mode: "control", closed: false })
		expect(opened.points[0]).toEqual(opened.points.at(-1))
		const picked = cubicBezierPoint(requireValue(splineBezierSegments(source)[2]), t)
		expect(opened.points[0]).toEqual(picked)
		for (const curve of splineBezierSegments(source)) for (const u of [0, 0.2, 0.5, 0.8, 1]) expect(nearestSplinePoint(opened, cubicBezierPoint(curve, u)).distance).toBeLessThan(1e-6)
	}
	expect(source.closed).toBe(true)
})

it("preserves fit-point and contact references through periodic split and reload", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { splineBezierSegments } = await import("../src/sketch-spline")
	const { cubicBezierPoint } = await import("../src/sketch-bezier")
	const { entityAnchorPoint } = await import("../src/sketch-curves")
	const { solveSketch } = await import("../src/sketch-solver")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const spline: import("../src/schema").Spline = {
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
	const contact = cubicBezierPoint(requireValue(splineBezierSegments(spline)[3]), 0.4)
	const relations: import("../src/sketch-solver").SketchRelation[] = [
		...spline.points.map((position, i): import("../src/sketch-solver").SketchRelation => ({ id: `f${i}`, type: "fixed", anchor: { entityId: "s", point: `point${i}` }, position })),
		{ id: "contact", type: "pointOnCurve", a: { entityId: "p", point: "center" }, b: "s" }
	]
	const source: import("../src/schema").Sketch = {
		id: "sk",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [spline, { id: "p", type: "point", center: contact }],
		relations,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const before = structuredClone(source)
	for (const t of [0, 0.5]) {
		const cut = cubicBezierPoint(requireValue(splineBezierSegments(spline)[1]), t)
		const result = editSketchCurve(source, "s", cut, "Split")
		expect(result.removedRelations).toHaveLength(0)
		const runtime = createPartRuntimeState({ features: [result.sketch] })
		const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
		const loaded = materializePartFeatures(restored.cad, restored.tree)[0]
		if (loaded?.type !== "sketch") throw Error("Missing sketch")
		const curve = requireValue(loaded.entities[0])
		expect(curve).toMatchObject({ type: "spline", mode: "control", closed: false })
		for (const relation of loaded.relations ?? [])
			if (relation.type === "fixed") {
				const actual = entityAnchorPoint(curve, relation.anchor.point)
				expect(actual.x).toBeCloseTo(relation.position.x, 6)
				expect(actual.y).toBeCloseTo(relation.position.y, 6)
			}
		expect(loaded.relations).toContainEqual(relations.at(-1))
		expect(solveSketch(loaded.entities, loaded.relations ?? []).status).not.toBe("conflicting")
	}
	expect(source).toEqual(before)
})
