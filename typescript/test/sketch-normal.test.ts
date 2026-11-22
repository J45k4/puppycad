import { expect, it } from "bun:test"
import type { Sketch, Ellipse, Line } from "../src/schema"
import { solveSketch, remapSketchRelations, normalizeSketchRelations, type SketchRelation } from "../src/sketch-solver"
import { sketchNormalResidual } from "../src/sketch-normal"
import { ellipsePoint } from "../src/sketch-ellipse"
import { editSketchCurve } from "../src/sketch-edit"
const normal: SketchRelation = { id: "normal", type: "normal", a: { entityId: "line", point: "p0" }, b: "curve" }
it("constrains a line normal to a fixed circle while retaining sliding and length freedom", () => {
	const solved = solveSketch(
		[
			{ id: "curve", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 64 },
			{ id: "line", type: "line", p0: { x: 10, y: 0 }, p1: { x: 20, y: 2 } }
		],
		[normal, { id: "center", type: "fixed", anchor: { entityId: "curve", point: "center" }, position: { x: 0, y: 0 } }, { id: "radius", type: "radius", entityId: "curve", value: 10 }]
	)
	expect(solved.status).toBe("underconstrained")
	expect(solved.degreesOfFreedom).toBe(2)
	const line = solved.entities[1]
	const curve = solved.entities[0]
	if (line?.type !== "line" || !curve) throw Error("Missing geometry")
	for (const residual of sketchNormalResidual(line, "p0", curve)) expect(residual).toBeCloseTo(0, 5)
})
it("uses the analytic normal of a rotated ellipse and persists the relation", async () => {
	const curve: Ellipse = { id: "curve", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 30, segments: 64 }
	const p = ellipsePoint(curve, Math.PI / 4)
	const relations: SketchRelation[] = [
		normal,
		{ id: "center", type: "fixed", anchor: { entityId: "curve", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "width", type: "width", entityId: "curve", value: 20 },
		{ id: "height", type: "height", entityId: "curve", value: 10 },
		{ id: "rotation", type: "rotation", entityId: "curve", value: 30 },
		{ id: "contact", type: "fixed", anchor: { entityId: "line", point: "p0" }, position: p },
		{ id: "length", type: "length", entityId: "line", value: 10 }
	]
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [curve, { id: "line", type: "line", p0: p, p1: { x: p.x + 5, y: p.y + 8 } }],
		relations,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const result = solveSketch(sketch.entities, relations)
	expect(result.status).toBe("fully-constrained")
	const line = result.entities[1]
	if (line?.type !== "line") throw Error("Missing line")
	for (const residual of sketchNormalResidual(line, "p0", curve)) expect(residual).toBeCloseTo(0, 5)
	// A radial line is not the normal of a noncircular ellipse away from its axes.
	expect(Math.abs((line.p1.x - line.p0.x) * p.y - (line.p1.y - line.p0.y) * p.x)).toBeGreaterThan(1)
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations).toContainEqual(normal)
	expect(
		remapSketchRelations(
			[normal],
			new Map([
				["line", "copy-line"],
				["curve", "copy-curve"]
			])
		)?.[0]
	).toMatchObject({ a: { entityId: "copy-line" }, b: "copy-curve" })
	expect(
		editSketchCurve({ ...sketch, entities: result.entities }, "line", { x: (line.p0.x + line.p1.x) / 2, y: (line.p0.y + line.p1.y) / 2 }, "Split").sketch.relations?.find(
			(r) => r.id === "normal"
		)
	).toMatchObject({ a: { entityId: "line", point: "p0" } })
})
it("checks finite arc membership and rejects invalid normal geometry", () => {
	const line: Line = { id: "line", type: "line", p0: { x: -10, y: 0 }, p1: { x: -20, y: 0 } }
	const arc = { id: "curve", type: "arc" as const, center: { x: 0, y: 0 }, radius: 10, startAngle: 0, sweep: Math.PI / 2, segments: 64 }
	expect(Math.max(...sketchNormalResidual(line, "p0", arc).map(Math.abs))).toBeGreaterThan(1)
	expect(() => sketchNormalResidual(line, "p0", line)).toThrow()
	expect(() => normalizeSketchRelations([{ ...normal, a: { entityId: "line", point: "center" } }])).toThrow()
})
it("constrains an endpoint normal to a finite straight line with sliding freedom", () => {
	const curve: Line = { id: "curve", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }
	const line: Line = { id: "line", type: "line", p0: { x: 3, y: 1 }, p1: { x: 4, y: 6 } }
	const fixed: SketchRelation[] = [
		{ id: "start", type: "fixed", anchor: { entityId: "curve", point: "p0" }, position: curve.p0 },
		{ id: "end", type: "fixed", anchor: { entityId: "curve", point: "p1" }, position: curve.p1 }
	]
	const result = solveSketch([curve, line], [...fixed, normal])
	expect(result.status).toBe("underconstrained")
	expect(result.degreesOfFreedom).toBe(2)
	const moved = result.entities[1]
	if (moved?.type !== "line") throw Error("Missing line")
	for (const residual of sketchNormalResidual(moved, "p0", curve)) expect(residual).toBeCloseTo(0, 5)
	const outside = solveSketch([curve, line], [...fixed, normal, { id: "outside", type: "fixed", anchor: { entityId: "line", point: "p0" }, position: { x: 15, y: 0 } }])
	expect(outside.status).toBe("conflicting")
	expect(() => sketchNormalResidual(curve, "p0", curve)).toThrow("separate")
	expect(() => sketchNormalResidual(line, "p0", { ...curve, p1: curve.p0 })).toThrow("nonzero")
})
it("constrains line endpoints to native spline normals with sliding and length freedom", () => {
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
		const fixed: SketchRelation[] = curve.points.map((position, i) => ({ id: `fix${i}`, type: "fixed", anchor: { entityId: curve.id, point: `point${i}` }, position }))
		const line: Line = { id: "line", type: "line", p0: { x: 5, y: 9 }, p1: { x: 6, y: 20 } }
		const result = solveSketch([curve, line], [...fixed, normal])
		expect(result.status).toBe("underconstrained")
		expect(result.degreesOfFreedom).toBe(2)
		const solved = result.entities[1]
		if (solved?.type !== "line") throw Error("Missing line")
		for (const residual of sketchNormalResidual(solved, "p0", curve)) expect(residual).toBeCloseTo(0, 5)
		expect(line.p0).toEqual({ x: 5, y: 9 })
	}
})
it("preserves spline normal references across split and project serialization", async () => {
	const { cubicBezierPoint, cubicBezierDerivative } = await import("../src/sketch-bezier")
	const { splineBezierSegments } = await import("../src/sketch-spline")
	const { requireValue } = await import("../src/required")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const { createProjectFile, serializeProjectFile, normalizeProjectFile } = await import("../src/project-file")
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
		const segment = requireValue(splineBezierSegments(curve)[0])
		const point = cubicBezierPoint(segment, 0.8)
		const tangent = cubicBezierDerivative(segment, 0.8)
		const length = Math.hypot(tangent.x, tangent.y)
		const line: Line = { id: "line", type: "line", p0: point, p1: { x: point.x - (tangent.y / length) * 5, y: point.y + (tangent.x / length) * 5 } }
		const input: Sketch = {
			id: "sketch",
			type: "sketch",
			dirty: false,
			target: { type: "plane", plane: "XY" },
			entities: [curve, line],
			relations: [normal],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}
		const edited = editSketchCurve(input, "curve", cubicBezierPoint(segment, 0.3), "Split")
		expect(edited.removedRelations).toEqual([])
		expect(edited.sketch.relations).toContainEqual({ ...normal, b: "curve-split-1" })
		const runtime = createPartRuntimeState({ features: [edited.sketch] })
		const file = createProjectFile({
			items: [{ id: "p", name: "Normal split", type: "part", data: { features: [edited.sketch], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
			selectedPath: null
		})
		const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = restored.items[0] as import("../src/contract").ProjectPartDocument
		const state = createPartRuntimeState(requireValue(part.data))
		const saved = materializePartFeatures(state.cad, state.tree)[0]
		if (saved?.type !== "sketch") throw Error("Missing sketch")
		expect(saved.relations).toEqual(edited.sketch.relations)
		const target = requireValue(saved.entities.find((e) => e.id === "curve-split-1"))
		for (const value of sketchNormalResidual(line, "p0", target)) expect(value).toBeCloseTo(0, 5)
		expect(solveSketch(saved.entities, saved.relations ?? []).status).not.toBe("conflicting")
		expect(input.relations).toEqual([normal])
	}
})
it("rejects ambiguous spline normals at sharp joins and stationary handles", () => {
	const curve: import("../src/schema").Spline = {
		id: "curve",
		type: "spline",
		mode: "control",
		points: [
			{ x: 0, y: 0 },
			{ x: 1, y: 0 },
			{ x: 2, y: 0 },
			{ x: 3, y: 0 },
			{ x: 3, y: 1 },
			{ x: 3, y: 2 },
			{ x: 3, y: 3 }
		]
	}
	const line: Line = { id: "line", type: "line", p0: { x: 3, y: 0 }, p1: { x: 3, y: -5 } }
	expect(() => sketchNormalResidual(line, "p0", curve)).toThrow("smooth spline join")
	const smooth = { ...curve, points: [...curve.points.slice(0, 4), { x: 4, y: 0 }, { x: 5, y: 1 }, { x: 6, y: 1 }] }
	for (const residual of sketchNormalResidual(line, "p0", smooth)) expect(residual).toBeCloseTo(0, 8)
	const stationary = {
		...curve,
		points: [
			{ x: 0, y: 0 },
			{ x: 0, y: 0 },
			{ x: 1, y: 1 },
			{ x: 2, y: 1 }
		]
	}
	expect(() => sketchNormalResidual({ ...line, p0: { x: 0, y: 0 } }, "p0", stationary)).toThrow("nonzero tangent")
	const reversal = { ...curve, points: [...curve.points.slice(0, 4), { x: 2, y: 0 }, { x: 1, y: 1 }, { x: 0, y: 1 }] }
	expect(() => sketchNormalResidual(line, "p0", reversal)).toThrow("smooth spline join")
})
it("solves a smooth line-to-spline endpoint join while preserving the fixed spline", () => {
	const curve: import("../src/schema").Spline = {
		id: "curve",
		type: "spline",
		mode: "control",
		points: [
			{ x: 0, y: 0 },
			{ x: 1, y: 0 },
			{ x: 2, y: 1 },
			{ x: 3, y: 1 }
		]
	}
	const fixed: SketchRelation[] = curve.points.map((position, i) => ({ id: `fix${i}`, type: "fixed", anchor: { entityId: "curve", point: `point${i}` }, position }))
	const line: Line = { id: "line", type: "line", p0: { x: 3, y: 1 }, p1: { x: 6, y: 2 } }
	const join: SketchRelation = { id: "join", type: "smoothJoin", a: { entityId: "curve", point: "point3" }, b: { entityId: "line", point: "p0" } }
	const result = solveSketch([curve, line], [...fixed, join])
	expect(result.status).toBe("underconstrained")
	expect(result.degreesOfFreedom).toBe(1)
	const solved = result.entities[1]
	if (solved?.type !== "line") throw Error("Missing line")
	expect(solved.p0.x).toBeCloseTo(3, 5)
	expect(solved.p0.y).toBeCloseTo(1, 5)
	expect(solved.p1.y).toBeCloseTo(1, 5)
	expect(solved.p1.x).toBeGreaterThan(3)
	expect(() => solveSketch([curve, line], [{ ...join, a: { entityId: "curve", point: "point1" } }])).toThrow("interior handle")
})
it("retains fit and control spline smooth joins through project reload", async () => {
	const { sketchEndpointDirection } = await import("../src/sketch-tangent-arc")
	const { requireValue } = await import("../src/required")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const { createProjectFile, serializeProjectFile, normalizeProjectFile } = await import("../src/project-file")
	for (const mode of ["fit", "control"] as const) {
		const curve: import("../src/schema").Spline = {
			id: "curve",
			type: "spline",
			mode,
			points: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 2, y: 1 },
				{ x: 3, y: 1 }
			]
		}
		const fixed: SketchRelation[] = curve.points.map((position, i) => ({ id: `fix${i}`, type: "fixed", anchor: { entityId: "curve", point: `point${i}` }, position }))
		const relations: SketchRelation[] = [...fixed, { id: "join", type: "smoothJoin", a: { entityId: "curve", point: "point3" }, b: { entityId: "line", point: "p0" } }]
		const result = solveSketch([curve, { id: "line", type: "line", p0: { x: 3, y: 1 }, p1: { x: 6, y: 2 } }], relations)
		expect(result.degreesOfFreedom).toBe(1)
		const sketch: Sketch = {
			id: "sketch",
			type: "sketch",
			dirty: false,
			target: { type: "plane", plane: "XY" },
			entities: result.entities,
			relations,
			dimensions: [],
			loops: [],
			vertices: [],
			profiles: []
		}
		const runtime = createPartRuntimeState({ features: [sketch] })
		const file = createProjectFile({
			items: [{ id: "p", type: "part", name: "Smooth join", data: { features: [sketch], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
			selectedPath: null
		})
		const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = restored.items[0] as import("../src/contract").ProjectPartDocument
		const state = createPartRuntimeState(requireValue(part.data))
		const saved = materializePartFeatures(state.cad, state.tree)[0]
		if (saved?.type !== "sketch") throw Error("Missing sketch")
		expect(saved.relations).toEqual(relations)
		const a = sketchEndpointDirection(requireValue(saved.entities[0]), "point3")
		const b = sketchEndpointDirection(requireValue(saved.entities[1]), "p0")
		expect(a.x + b.x).toBeCloseTo(0, 5)
		expect(a.y + b.y).toBeCloseTo(0, 5)
		expect(solveSketch(saved.entities, saved.relations ?? []).degreesOfFreedom).toBe(1)
	}
})
it("joins the start of fit and control splines without reversing the continuation", async () => {
	const { sketchEndpointDirection } = await import("../src/sketch-tangent-arc")
	for (const mode of ["fit", "control"] as const) {
		const curve: import("../src/schema").Spline = {
			id: "curve",
			type: "spline",
			mode,
			points: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 2, y: 1 },
				{ x: 3, y: 1 }
			]
		}
		const fixed: SketchRelation[] = curve.points.map((position, i) => ({ id: `fix${i}`, type: "fixed", anchor: { entityId: "curve", point: `point${i}` }, position }))
		for (const endpoint of ["p0", "p1"] as const) {
			const line: Line = { id: "line", type: "line", p0: endpoint === "p0" ? { x: 0, y: 0 } : { x: -4, y: 1 }, p1: endpoint === "p1" ? { x: 0, y: 0 } : { x: -4, y: 1 } }
			const join: SketchRelation = { id: "join", type: "smoothJoin", a: { entityId: "curve", point: "point0" }, b: { entityId: "line", point: endpoint } }
			const solved = solveSketch([curve, line], [...fixed, join])
			expect(solved.status).toBe("underconstrained")
			expect(solved.degreesOfFreedom).toBe(1)
			const result = solved.entities[1]
			if (result?.type !== "line") throw Error("Missing line")
			expect(result[endpoint].x).toBeCloseTo(0, 5)
			expect(result[endpoint].y).toBeCloseTo(0, 5)
			expect(result[endpoint === "p0" ? "p1" : "p0"].x).toBeLessThan(0)
			const a = sketchEndpointDirection(curve, "point0")
			const b = sketchEndpointDirection(result, endpoint)
			expect(a.x + b.x).toBeCloseTo(0, 5)
			expect(a.y + b.y).toBeCloseTo(0, 5)
		}
	}
})
it("remaps a spline endpoint tangent join to the surviving split endpoint", async () => {
	const { splineBezierSegments } = await import("../src/sketch-spline")
	const { cubicBezierPoint } = await import("../src/sketch-bezier")
	const { sketchEndpointDirection } = await import("../src/sketch-tangent-arc")
	const { requireValue } = await import("../src/required")
	for (const mode of ["fit", "control"] as const) {
		const curve: import("../src/schema").Spline = {
			id: "curve",
			type: "spline",
			mode,
			points: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 2, y: 1 },
				{ x: 3, y: 1 }
			]
		}
		const direction = sketchEndpointDirection(curve, "point3")
		const line: Line = { id: "line", type: "line", p0: { x: 3, y: 1 }, p1: { x: 3 + direction.x * 5, y: 1 + direction.y * 5 } }
		const join: SketchRelation = { id: "join", type: "smoothJoin", a: { entityId: "curve", point: "point3" }, b: { entityId: "line", point: "p0" } }
		const input: Sketch = {
			id: "s",
			type: "sketch",
			dirty: false,
			target: { type: "plane", plane: "XY" },
			entities: [curve, line],
			relations: [join],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}
		const edited = editSketchCurve(input, "curve", cubicBezierPoint(requireValue(splineBezierSegments(curve)[0]), 0.4), "Split")
		expect(edited.removedRelations).toEqual([])
		const piece = edited.sketch.entities.find((e) => e.id === "curve-split-1")
		if (piece?.type !== "spline") throw Error("Missing split spline")
		expect(edited.sketch.relations).toContainEqual({ ...join, a: { entityId: piece.id, point: `point${piece.points.length - 1}` } })
		const tangent = sketchEndpointDirection(piece, `point${piece.points.length - 1}`)
		expect(tangent.x).toBeCloseTo(direction.x, 8)
		expect(tangent.y).toBeCloseTo(direction.y, 8)
		expect(solveSketch(edited.sketch.entities, edited.sketch.relations ?? []).status).not.toBe("conflicting")
		expect(input.relations).toEqual([join])
	}
})

it("uses finite elliptic-arc normals and rejects contact on the removed supporting ellipse", async () => {
	const { ellipticArcPoint } = await import("../src/sketch-elliptic-arc")
	for (const sign of [-1, 1]) {
		const curve: import("../src/sketch-elliptic-arc").EllipticArc = {
			id: "curve",
			type: "ellipticArc",
			center: { x: 3, y: 4 },
			width: 20,
			height: 8,
			rotation: 35,
			startAngle: 0.8,
			sweep: sign * 0.6,
			segments: 64
		}
		const p = ellipticArcPoint(curve, 0.5)
		const angle = curve.startAngle + curve.sweep / 2
		const rotation = (curve.rotation * Math.PI) / 180
		const nx = Math.cos(angle) / curve.width
		const ny = Math.sin(angle) / curve.height
		const x = nx * Math.cos(rotation) - ny * Math.sin(rotation)
		const y = nx * Math.sin(rotation) + ny * Math.cos(rotation)
		const line: Line = { id: "line", type: "line", p0: p, p1: { x: p.x + 100 * x, y: p.y + 100 * y } }
		for (const r of sketchNormalResidual(line, "p0", curve)) expect(r).toBeCloseTo(0, 7)
		const solved = solveSketch([curve, { ...line, p1: { x: line.p1.x + 0.2, y: line.p1.y - 0.1 } }], [normal])
		expect(solved.status).toBe("underconstrained")
		const a = solved.entities[0]
		const b = solved.entities[1]
		if (a?.type !== "ellipticArc" || b?.type !== "line") throw Error("Missing geometry")
		for (const r of sketchNormalResidual(b, "p0", a)) expect(r).toBeCloseTo(0, 5)
		const outside = ellipticArcPoint({ ...curve, startAngle: curve.startAngle + Math.PI }, 0)
		expect(Math.hypot(...sketchNormalResidual({ ...line, p0: outside }, "p0", curve))).toBeGreaterThan(1)
	}
})

it("retains elliptic normal and midpoint dependencies through both project formats and dimension changes", async () => {
	const { requireValue } = await import("../src/required")
	const { materializeSketch } = await import("../src/cad/sketch")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const { createProjectFile, normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: [],
		entities: [
			{ id: "curve", type: "ellipticArc", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI, segments: 64 },
			{ id: "line", type: "line", p0: { x: 0, y: 5 }, p1: { x: 0, y: 10 } }
		],
		relations: [
			normal,
			{ id: "center", type: "fixed", anchor: { entityId: "curve", point: "center" }, position: { x: 0, y: 0 } },
			{ id: "width", type: "width", entityId: "curve", value: 20 },
			{ id: "height", type: "height", entityId: "curve", value: 10 },
			{ id: "rotation", type: "rotation", entityId: "curve", value: 0 },
			{ id: "sweep", type: "arcSweep", entityId: "curve", value: 180 },
			{ id: "contact", type: "midpoint", a: { entityId: "line", point: "p0" }, b: "curve" },
			{ id: "length", type: "length", entityId: "line", value: 5 },
			{ id: "vertical", type: "vertical", entityId: "line" }
		]
	}
	for (const native of [false, true]) {
		const runtime = createPartRuntimeState({ features: [materializeSketch(sketch)] })
		const file = createProjectFile({
			items: [{ id: "p", type: "part", name: "Normal", data: { features: [sketch], ...(native ? { cad: serializePCadState(runtime.cad), tree: runtime.tree } : {}) } }],
			selectedPath: null
		})
		const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = restored.items[0] as import("../src/contract").ProjectPartDocument
		const state = createPartRuntimeState(requireValue(part.data))
		const saved = materializePartFeatures(state.cad, state.tree)[0]
		if (saved?.type !== "sketch") throw Error("Missing sketch")
		expect(saved.relations).toContainEqual(normal)
		const height = requireValue(saved.relations?.find((r) => r.id === "height"))
		if (height.type !== "height") throw Error("Missing height")
		height.value = 20
		const result = materializeSketch(saved)
		const curve = result.entities[0]
		const line = result.entities[1]
		if (curve?.type !== "ellipticArc" || line?.type !== "line") throw Error("Missing native entities")
		expect(curve.height).toBeCloseTo(20, 5)
		expect(line.p0.y).toBeCloseTo(10, 5)
		expect(line.p1.y).toBeCloseTo(15, 5)
		for (const residual of sketchNormalResidual(line, "p0", curve)) expect(residual).toBeCloseTo(0, 5)
		expect(sketch.entities[0]).toMatchObject({ height: 10 })
	}
})

it("joins either elliptic endpoint smoothly to a line without reversing continuation", async () => {
	const { sketchEndpointDirection } = await import("../src/sketch-tangent-arc")
	const { ellipticArcPoint } = await import("../src/sketch-elliptic-arc")
	for (const sign of [-1, 1])
		for (const endpoint of ["p0", "p1"] as const) {
			const curve: import("../src/sketch-elliptic-arc").EllipticArc = {
				id: "curve",
				type: "ellipticArc",
				center: { x: 4, y: 7 },
				width: 30,
				height: 8,
				rotation: 35,
				startAngle: 5.8,
				sweep: sign * 1.2,
				segments: 64
			}
			const p = ellipticArcPoint(curve, endpoint === "p0" ? 0 : 1)
			const direction = sketchEndpointDirection(curve, endpoint)
			const line: Line = { id: "line", type: "line", p0: p, p1: { x: p.x + direction.x * 10 + 0.2, y: p.y + direction.y * 10 - 0.1 } }
			const result = solveSketch([curve, line], [{ id: "join", type: "smoothJoin", a: { entityId: curve.id, point: endpoint }, b: { entityId: line.id, point: "p0" } }])
			expect(result.status).toBe("underconstrained")
			const arc = result.entities[0]
			const straight = result.entities[1]
			if (arc?.type !== "ellipticArc" || straight?.type !== "line") throw Error("Missing joined geometry")
			const a = sketchEndpointDirection(arc, endpoint)
			const b = sketchEndpointDirection(straight, "p0")
			expect(a.x + b.x).toBeCloseTo(0, 5)
			expect(a.y + b.y).toBeCloseTo(0, 5)
			const joined = ellipticArcPoint(arc, endpoint === "p0" ? 0 : 1)
			expect(joined.x).toBeCloseTo(straight.p0.x, 5)
			expect(joined.y).toBeCloseTo(straight.p0.y, 5)
		}
})

it("constrains whole-line tangency to the retained elliptic arc interior", () => {
	const curve: import("../src/sketch-elliptic-arc").EllipticArc = {
		id: "curve",
		type: "ellipticArc",
		center: { x: 0, y: 0 },
		width: 20,
		height: 10,
		rotation: 0,
		startAngle: 0,
		sweep: Math.PI,
		segments: 64
	}
	const line: Line = { id: "line", type: "line", p0: { x: -20, y: 5.2 }, p1: { x: 20, y: 5.2 } }
	const result = solveSketch(
		[curve, line],
		[
			{ id: "tangent", type: "tangent", a: curve.id, b: line.id },
			{ id: "center", type: "fixed", anchor: { entityId: curve.id, point: "center" }, position: { x: 0, y: 0 } },
			{ id: "width", type: "width", entityId: curve.id, value: 20 },
			{ id: "height", type: "height", entityId: curve.id, value: 10 },
			{ id: "rotation", type: "rotation", entityId: curve.id, value: 0 },
			{ id: "horizontal", type: "horizontal", entityId: line.id }
		]
	)
	expect(result.status).not.toBe("conflicting")
	const straight = result.entities[1]
	if (straight?.type !== "line") throw Error("Missing tangent line")
	expect(straight.p0.y).toBeCloseTo(5, 5)
	expect(straight.p1.y).toBeCloseTo(5, 5)
})

it("rejects elliptic tangency on a removed half when the arc and line are fixed", () => {
	const curve: import("../src/sketch-elliptic-arc").EllipticArc = {
		id: "curve",
		type: "ellipticArc",
		center: { x: 0, y: 0 },
		width: 20,
		height: 10,
		rotation: 0,
		startAngle: 0,
		sweep: Math.PI,
		segments: 64
	}
	const line: Line = { id: "line", type: "line", p0: { x: -20, y: -5 }, p1: { x: 20, y: -5 } }
	const result = solveSketch(
		[curve, line],
		[
			{ id: "tangent", type: "tangent", a: curve.id, b: line.id },
			{ id: "center", type: "fixed", anchor: { entityId: curve.id, point: "center" }, position: { x: 0, y: 0 } },
			{ id: "start", type: "fixed", anchor: { entityId: curve.id, point: "p0" }, position: { x: 10, y: 0 } },
			{ id: "end", type: "fixed", anchor: { entityId: curve.id, point: "p1" }, position: { x: -10, y: 0 } },
			{ id: "height", type: "height", entityId: curve.id, value: 10 },
			{ id: "rotation", type: "rotation", entityId: curve.id, value: 0 },
			{ id: "line-start", type: "fixed", anchor: { entityId: line.id, point: "p0" }, position: line.p0 },
			{ id: "line-end", type: "fixed", anchor: { entityId: line.id, point: "p1" }, position: line.p1 }
		]
	)
	expect(result.status).toBe("conflicting")
})
