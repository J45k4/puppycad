import { it, expect } from "bun:test"
import { editSketchCurve } from "../src/sketch-edit"
import { materializeSketch } from "../src/cad/sketch"
import { solveSketch } from "../src/sketch-solver"
import { arcPoint, threePointArc } from "../src/sketch-curves"
import type { Sketch, SketchEntity, Line } from "../src/schema"
import { v2 } from "../src/sdk"
import { requireValue } from "../src/required"
const line = (id: string, x0: number, y0: number, x1: number, y1: number): Line => ({ id, type: "line", p0: v2(x0, y0), p1: v2(x1, y1) })
const sketch = (entities: SketchEntity[]): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities,
	relations: [],
	dimensions: [],
	loops: [],
	vertices: [],
	profiles: []
})
it("trims only the clicked line interval and preserves untouched endpoint constraints", () => {
	const input = sketch([line("a", -10, 0, 10, 0), line("left", -3, -5, -3, 5), line("right", 4, -5, 4, 5)])
	input.relations = [
		{ id: "length", type: "length", entityId: "a", value: 20 },
		{ id: "end", type: "fixed", anchor: { entityId: "a", point: "p1" }, position: v2(10, 0) }
	]
	const result = editSketchCurve(input, "a", v2(0, 0), "Trim")
	expect(input.entities).toHaveLength(3)
	expect(result.sketch.entities).toHaveLength(4)
	expect(result.sketch.entities[0]).toMatchObject({ id: "a", p0: v2(-10, 0), p1: v2(-3, 0) })
	expect(result.sketch.entities[1]).toMatchObject({ p0: v2(4, 0), p1: v2(10, 0) })
	expect(result.removedRelations).toEqual(["length"])
	expect(result.sketch.relations?.find((r) => r.id === "end")).toMatchObject({ anchor: { entityId: result.sketch.entities[1]?.id, point: "p1" } })
	expect(solveSketch(result.sketch.entities, result.sketch.relations ?? []).status).not.toBe("conflicting")
})
it("trims a circle into a native semicircular arc that closes with its boundary", () => {
	const input = sketch([{ id: "c", type: "circle", center: v2(0, 0), radius: 10, segments: 128 }, line("chord", -10, 0, 10, 0)])
	input.relations = [
		{ id: "d", type: "diameter", entityId: "c", value: 20 },
		{ id: "fixed", type: "fixed", anchor: { entityId: "c", point: "center" }, position: v2(0, 0) }
	]
	const result = editSketchCurve(input, "c", v2(0, -10), "Trim")
	const arc = result.sketch.entities[0]
	if (arc?.type !== "arc") throw Error("Missing arc")
	expect(arcPoint(arc, 0.5).y).toBeCloseTo(10, 6)
	expect(arc.sweep).toBeCloseTo(Math.PI, 7)
	expect(result.removedRelations).toEqual([])
	expect(materializeSketch(result.sketch).profiles).toHaveLength(1)
})
it("splits an arc without flattening it and moves endpoint references to the correct piece", () => {
	const input = sketch([threePointArc("a", v2(-10, 0), v2(10, 0), v2(0, 10))])
	input.relations = [
		{ id: "r", type: "radius", entityId: "a", value: 10 },
		{ id: "end", type: "fixed", anchor: { entityId: "a", point: "p1" }, position: v2(10, 0) }
	]
	const result = editSketchCurve(input, "a", v2(0, 10), "Split")
	expect(result.sketch.entities).toHaveLength(2)
	expect(result.sketch.entities.every((e) => e.type === "arc")).toBe(true)
	expect(result.sketch.relations?.some((r) => r.type === "concentric")).toBe(true)
	expect(result.sketch.relations?.some((r) => r.type === "coincident")).toBe(true)
	expect(solveSketch(result.sketch.entities, result.sketch.relations ?? []).status).not.toBe("conflicting")
})
it("extends only the picked endpoint to the nearest finite boundary", () => {
	const input = sketch([line("a", 0, 0, 3, 0), line("b", 5, -2, 5, 2), line("c", 8, -2, 8, 2)])
	const result = editSketchCurve(input, "a", v2(2.9, 0), "Extend")
	expect(result.sketch.entities[0]).toMatchObject({ p0: v2(0, 0), p1: v2(5, 0) })
	expect(() => editSketchCurve(input, "a", v2(0.1, 0), "Extend")).toThrow("No boundary")
	const finite = sketch([line("a", 0, 0, 3, 0), line("b", 5, 1, 5, 2)])
	expect(() => editSketchCurve(finite, "a", v2(3, 0), "Extend")).toThrow("No boundary")
})
it("handles circle-circle intersections and ignores nonintersecting arc portions", () => {
	const input = sketch([
		{ id: "a", type: "circle", center: v2(0, 0), radius: 10, segments: 128 },
		{ id: "b", type: "circle", center: v2(10, 0), radius: 10, segments: 128 }
	])
	const result = editSketchCurve(input, "a", v2(10, 0), "Trim")
	const arc = requireValue(result.sketch.entities[0])
	if (arc.type !== "arc") throw Error("Missing arc")
	expect(Math.abs(arc.sweep)).toBeCloseTo((Math.PI * 4) / 3, 6)
	const upper = threePointArc("upper", v2(-10, 0), v2(10, 0), v2(0, 10))
	const finite = sketch([line("a", -20, -5, 20, -5), upper])
	expect(editSketchCurve(finite, "a", v2(0, -5), "Trim").sketch.entities).toHaveLength(1)
})
it("splits a circle at two selected points and retains a closed constrained profile", () => {
	const input = sketch([{ id: "c", type: "circle", center: v2(0, 0), radius: 10, segments: 128 }])
	input.relations = [{ id: "d", type: "diameter", entityId: "c", value: 20 }]
	const result = editSketchCurve(input, "c", v2(10, 0), "Split", v2(0, 10))
	expect(result.sketch.entities).toHaveLength(2)
	expect(materializeSketch(result.sketch).profiles).toHaveLength(1)
	expect(result.sketch.relations?.filter((r) => r.type === "coincident")).toHaveLength(2)
	expect(() => editSketchCurve(input, "c", v2(10, 0), "Split", v2(10, 0))).toThrow("distinct")
})
it("extends either end of a native arc to a line boundary without reaching a full revolution", () => {
	const arc = threePointArc("a", v2(10, 0), v2(0, 10), v2(Math.sqrt(50), Math.sqrt(50)))
	const input = sketch([arc, line("end", -10, 0, 10, 0)])
	const result = editSketchCurve(input, "a", arcPoint(arc, 0.9), "Extend")
	const extended = requireValue(result.sketch.entities[0])
	if (extended.type !== "arc") throw Error("Missing arc")
	expect(extended.sweep).toBeCloseTo(Math.PI, 7)
	const backwards = editSketchCurve(sketch([arc, line("start", 0, -10, 0, 10)]), "a", arcPoint(arc, 0.1), "Extend")
	const previous = requireValue(backwards.sketch.entities[0])
	if (previous.type !== "arc") throw Error("Missing arc")
	expect(previous.sweep).toBeCloseTo(Math.PI, 7)
	expect(arcPoint(previous, 0).y).toBeCloseTo(-10, 7)
})
it("persists split geometry and its connecting constraints through PCad and project JSON", async () => {
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const { createProjectFile, normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
	const input = sketch([line("base", 0, 0, 20, 0), line("right", 20, 0, 20, 10), line("top", 20, 10, 0, 10), line("left", 0, 10, 0, 0)])
	input.relations = [{ id: "h", type: "horizontal", entityId: "base" }]
	const edited = materializeSketch(editSketchCurve(input, "base", v2(7, 0), "Split").sketch)
	const runtime = createPartRuntimeState({ features: [edited] })
	const file = createProjectFile({
		items: [{ id: "p", type: "part", name: "Split", data: { features: [edited], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
		selectedPath: null
	})
	const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
	const part = restored.items[0] as import("../src/contract").ProjectPartDocument
	const result = createPartRuntimeState(requireValue(part.data))
	const saved = materializePartFeatures(result.cad, result.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.profiles).toHaveLength(1)
	expect(saved.entities).toHaveLength(5)
	expect(saved.relations?.some((r) => r.type === "collinear")).toBe(true)
	expect(solveSketch(saved.entities, saved.relations ?? []).status).not.toBe("conflicting")
})
it("keeps intersection point dependencies on the surviving line fragments and detaches removed contacts", () => {
	const original = sketch([
		line("base", 0, 0, 20, 0),
		line("cut1", 8, -5, 8, 5),
		line("cut2", 12, -5, 12, 5),
		{ id: "left", type: "point", center: v2(4, 0) },
		{ id: "middle", type: "point", center: v2(10, 0) },
		{ id: "right", type: "point", center: v2(16, 0) }
	])
	original.relations = ["left", "middle", "right"].map((id) => ({ id: `on-${id}`, type: "pointOnCurve", a: { entityId: id, point: "center" }, b: "base" }))
	const result = editSketchCurve(original, "base", v2(10, 0), "Trim")
	expect(result.removedRelations).toEqual(["on-middle"])
	expect(result.sketch.relations).toContainEqual({ id: "on-left", type: "pointOnCurve", a: { entityId: "left", point: "center" }, b: "base" })
	expect(result.sketch.relations).toContainEqual({ id: "on-right", type: "pointOnCurve", a: { entityId: "right", point: "center" }, b: "base-split-1" })
	expect(solveSketch(result.sketch.entities, result.sketch.relations ?? []).status).not.toBe("conflicting")
	expect(original.relations[2]).toMatchObject({ b: "base" })
})
it("remaps circle contact dependencies to the correct split arc and persists the relationship", async () => {
	const original = sketch([
		{ id: "circle", type: "circle", center: v2(0, 0), radius: 10, segments: 32 },
		{ id: "upper", type: "point", center: v2(0, 10) },
		{ id: "lower", type: "point", center: v2(0, -10) }
	])
	original.relations = ["upper", "lower"].map((id) => ({ id: `on-${id}`, type: "pointOnCurve", a: { entityId: id, point: "center" }, b: "circle" }))
	const result = editSketchCurve(original, "circle", v2(10, 0), "Split", v2(-10, 0))
	expect(result.removedRelations).toEqual([])
	expect(result.sketch.relations).toContainEqual({ id: "on-lower", type: "pointOnCurve", a: { entityId: "lower", point: "center" }, b: "circle-split-1" })
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [result.sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations).toEqual(result.sketch.relations)
	const solved = solveSketch(saved.entities, [
		...(saved.relations ?? []),
		{ id: "radius", type: "radius", entityId: "circle", value: 12 },
		{ id: "center", type: "fixed", anchor: { entityId: "circle", point: "center" }, position: v2(0, 0) }
	])
	expect(solved.status).not.toBe("conflicting")
	for (const entity of solved.entities.filter((entity) => entity.type === "point")) expect(Math.hypot(entity.center.x, entity.center.y)).toBeCloseTo(12, 5)
})

it("preserves Normal contacts on split line and circle targets and removes deleted targets", async () => {
	for (const curved of [false, true]) {
		const target: SketchEntity = curved ? { id: "target", type: "circle", center: v2(0, 0), radius: 10, segments: 32 } : line("target", -10, 0, 10, 0)
		const source = curved ? line("source", 0, -10, 0, -20) : line("source", 5, 0, 5, 10)
		const original = sketch([target, source])
		original.relations = [{ id: "normal", type: "normal", a: { entityId: "source", point: "p0" }, b: "target" }]
		const result = curved ? editSketchCurve(original, "target", v2(10, 0), "Split", v2(-10, 0)) : editSketchCurve(original, "target", v2(0, 0), "Split")
		expect(result.removedRelations).toEqual([])
		expect(result.sketch.relations).toContainEqual({ id: "normal", type: "normal", a: { entityId: "source", point: "p0" }, b: "target-split-1" })
		const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
		const state = createPartRuntimeState({ features: [result.sketch] })
		const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
		const saved = materializePartFeatures(restored.cad, restored.tree)[0]
		if (saved?.type !== "sketch") throw Error("Missing sketch")
		expect(saved.relations).toEqual(result.sketch.relations)
		expect(solveSketch(saved.entities, saved.relations ?? []).status).not.toBe("conflicting")
		expect(original.relations[0]).toEqual({ id: "normal", type: "normal", a: { entityId: "source", point: "p0" }, b: "target" })
	}
	const original = sketch([line("target", 0, 0, 20, 0), line("source", 0, 0, 0, 10)])
	original.relations = [{ id: "normal", type: "normal", a: { entityId: "source", point: "p0" }, b: "target" }]
	expect(editSketchCurve(original, "target", v2(10, 0), "Trim").removedRelations).toEqual(["normal"])
})

it("keeps an oriented tangent join on the surviving target line after splitting", () => {
	const original = sketch([line("target", -10, 0, 10, 0), { id: "arc", type: "arc", center: v2(10, 5), radius: 5, startAngle: -Math.PI / 2, sweep: Math.PI / 2, segments: 32 }])
	original.relations = [
		{ id: "join", type: "coincident", a: { entityId: "arc", point: "p0" }, b: { entityId: "target", point: "p1" } },
		{ id: "tangent", type: "endpointTangent", a: { entityId: "arc", point: "p0" }, b: "target", orientation: { lineSign: 1, sweepSign: 1 } }
	]
	const result = editSketchCurve(original, "target", v2(0, 0), "Split")
	expect(result.removedRelations).toEqual([])
	expect(result.sketch.relations).toContainEqual({ id: "tangent", type: "endpointTangent", a: { entityId: "arc", point: "p0" }, b: "target-split-1", orientation: { lineSign: 1, sweepSign: 1 } })
	expect(result.sketch.relations).toContainEqual({ id: "join", type: "coincident", a: { entityId: "arc", point: "p0" }, b: { entityId: "target-split-1", point: "p1" } })
	expect(solveSketch(result.sketch.entities, result.sketch.relations ?? []).status).not.toBe("conflicting")
	const trimmed = editSketchCurve(original, "target", v2(0, 0), "Trim")
	expect(trimmed.removedRelations).toContain("tangent")
	expect(trimmed.removedRelations).toContain("join")
})

it("splits native splines while preserving outer endpoint constraints and reporting changed handles", () => {
	const source = sketch([
		{
			id: "curve",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			]
		}
	])
	source.relations = [
		{ id: "start", type: "fixed", anchor: { entityId: "curve", point: "point0" }, position: { x: 0, y: 0 } },
		{ id: "end", type: "fixed", anchor: { entityId: "curve", point: "point3" }, position: { x: 10, y: 0 } },
		{ id: "handle", type: "fixed", anchor: { entityId: "curve", point: "point1" }, position: { x: 0, y: 10 } }
	]
	const original = structuredClone(source)
	const result = editSketchCurve(source, "curve", { x: 5, y: 7.5 }, "Split")
	expect(result.sketch.entities).toHaveLength(2)
	expect(result.removedRelations).toEqual(["handle"])
	expect(result.sketch.relations).toContainEqual({ id: "end", type: "fixed", anchor: { entityId: "curve-split-1", point: "point3" }, position: { x: 10, y: 0 } })
	expect(result.sketch.relations).toContainEqual({ id: "coincident-1", type: "coincident", a: { entityId: "curve", point: "point3" }, b: { entityId: "curve-split-1", point: "point0" } })
	expect(solveSketch(result.sketch.entities, result.sketch.relations ?? []).status).not.toBe("conflicting")
	expect(source).toEqual(original)
	expect(() => editSketchCurve(source, "curve", { x: 0, y: 0 }, "Split")).toThrow("inside")
	expect(() => editSketchCurve(source, "curve", { x: 5, y: 7.5 }, "Extend")).toThrow("extension")
	const trimmed = editSketchCurve(source, "curve", { x: 5, y: 7.5 }, "Trim")
	expect(trimmed.sketch.entities).toHaveLength(0)
	expect(trimmed.removedRelations).toEqual(["start", "end", "handle"])
})
it("retains a closed fit-spline profile and endpoint relations after splitting and project reload", async () => {
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const { createProjectFile, normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
	const input = sketch([
		{
			id: "curve",
			type: "spline",
			mode: "fit",
			points: [
				{ x: 0, y: 0 },
				{ x: 5, y: 5 },
				{ x: 10, y: 0 }
			]
		},
		line("chord", 10, 0, 0, 0)
	])
	input.relations = [
		{ id: "start", type: "coincident", a: { entityId: "curve", point: "point0" }, b: { entityId: "chord", point: "p1" } },
		{ id: "end", type: "coincident", a: { entityId: "curve", point: "point2" }, b: { entityId: "chord", point: "p0" } },
		{ id: "peak", type: "fixed", anchor: { entityId: "curve", point: "point1" }, position: { x: 5, y: 5 } }
	]
	const original = structuredClone(input)
	const edit = editSketchCurve(input, "curve", { x: 5, y: 5 }, "Split")
	expect(edit.removedRelations).toEqual([])
	const edited = materializeSketch(edit.sketch)
	expect(edited.profiles).toHaveLength(1)
	const runtime = createPartRuntimeState({ features: [edited] })
	const file = createProjectFile({
		items: [{ id: "p", type: "part", name: "Split fit curve", data: { features: [edited], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
		selectedPath: null
	})
	const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
	const part = restored.items[0] as import("../src/contract").ProjectPartDocument
	const result = createPartRuntimeState(requireValue(part.data))
	const saved = materializePartFeatures(result.cad, result.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.profiles).toHaveLength(1)
	expect(saved.entities).toEqual(edited.entities)
	expect(saved.relations).toEqual(edited.relations)
	expect(saved.entities.filter((e) => e.type === "spline").every((e) => e.mode === "control")).toBe(true)
	expect(solveSketch(saved.entities, saved.relations ?? []).status).not.toBe("conflicting")
	expect(input).toEqual(original)
})
it("remaps spline contacts to both split pieces and retains them through project reload", async () => {
	const { cubicBezierPoint } = await import("../src/sketch-bezier")
	const { splineBezierSegments } = await import("../src/sketch-spline")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const { createProjectFile, normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
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
		const segments = splineBezierSegments(curve)
		const input = sketch([curve, ...[0.2, 0.8].map((t, i): SketchEntity => ({ id: `p${i}`, type: "point", center: cubicBezierPoint(requireValue(segments[0]), t) }))])
		input.relations = [0, 1].map((i) => ({ id: `contact${i}`, type: "pointOnCurve", a: { entityId: `p${i}`, point: "center" }, b: "curve" }))
		const result = editSketchCurve(input, "curve", cubicBezierPoint(requireValue(segments[0]), 0.5), "Split")
		expect(result.removedRelations).toEqual([])
		expect(result.sketch.relations).toContainEqual({ id: "contact0", type: "pointOnCurve", a: { entityId: "p0", point: "center" }, b: "curve" })
		expect(result.sketch.relations).toContainEqual({ id: "contact1", type: "pointOnCurve", a: { entityId: "p1", point: "center" }, b: "curve-split-1" })
		const runtime = createPartRuntimeState({ features: [result.sketch] })
		const file = createProjectFile({
			items: [{ id: "p", type: "part", name: "Spline contacts", data: { features: [result.sketch], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
			selectedPath: null
		})
		const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = restored.items[0] as import("../src/contract").ProjectPartDocument
		const state = createPartRuntimeState(requireValue(part.data))
		const saved = materializePartFeatures(state.cad, state.tree)[0]
		if (saved?.type !== "sketch") throw Error("Missing sketch")
		expect(saved.relations).toEqual(result.sketch.relations)
		expect(solveSketch(saved.entities, saved.relations ?? []).status).not.toBe("conflicting")
		expect(input.relations.every((r) => r.type === "pointOnCurve" && r.b === "curve")).toBe(true)
	}
})

it("keeps coincident but independent spline endpoints distinct through split and reload", async () => {
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const source = sketch([
		{
			id: "curve",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 0, y: 0 }
			]
		}
	])
	source.relations = [
		{ id: "start", type: "fixed", anchor: { entityId: "curve", point: "point0" }, position: { x: 0, y: 0 } },
		{ id: "end", type: "fixed", anchor: { entityId: "curve", point: "point3" }, position: { x: 0, y: 0 } }
	]
	const result = editSketchCurve(source, "curve", { x: 3.75, y: 7.5 }, "Split")
	expect(result.removedRelations).toEqual([])
	expect(result.sketch.relations).toContainEqual({ id: "end", type: "fixed", anchor: { entityId: "curve-split-1", point: "point3" }, position: { x: 0, y: 0 } })
	const runtime = createPartRuntimeState({ features: [result.sketch] })
	const restored = createPartRuntimeState({ features: [], cad: serializePCadState(runtime.cad), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations).toEqual(result.sketch.relations)
	const end = requireValue(saved.relations?.find((r) => r.id === "end"))
	if (end.type !== "fixed") throw Error("Missing endpoint constraint")
	end.position = { x: 2, y: 0 }
	const solved = solveSketch(saved.entities, saved.relations ?? [])
	expect(solved.status).not.toBe("conflicting")
	const left = solved.entities.find((e) => e.id === "curve")
	const right = solved.entities.find((e) => e.id === "curve-split-1")
	if (left?.type !== "spline" || right?.type !== "spline") throw Error("Missing split splines")
	expect(requireValue(left.points[0]).x).toBeCloseTo(0, 6)
	expect(requireValue(right.points.at(-1)).x).toBeCloseTo(2, 6)
})

it("removes a trimmed spline anchor instead of transferring it to an equal-position endpoint", () => {
	const source = sketch([
		{
			id: "curve",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 0, y: 0 }
			]
		},
		line("boundary", -5, 5, 15, 5)
	])
	source.relations = [
		{ id: "start", type: "fixed", anchor: { entityId: "curve", point: "point0" }, position: { x: 0, y: 0 } },
		{ id: "end", type: "fixed", anchor: { entityId: "curve", point: "point3" }, position: { x: 0, y: 0 } }
	]
	const result = editSketchCurve(source, "curve", { x: 0.27, y: 2.7 }, "Trim")
	expect(result.removedRelations).toEqual(["start"])
	expect(result.sketch.relations).toEqual([{ id: "end", type: "fixed", anchor: { entityId: "curve", point: "point3" }, position: { x: 0, y: 0 } }])
})

it("retains distinct repeated fit points when opening a periodic spline", () => {
	const source = sketch([
		{
			id: "curve",
			type: "spline",
			mode: "fit",
			closed: true,
			points: [
				{ x: 0, y: 0 },
				{ x: 10, y: 10 },
				{ x: 0, y: 0 },
				{ x: -10, y: 10 }
			]
		}
	])
	source.relations = [
		{ id: "first", type: "fixed", anchor: { entityId: "curve", point: "point0" }, position: { x: 0, y: 0 } },
		{ id: "second", type: "fixed", anchor: { entityId: "curve", point: "point2" }, position: { x: 0, y: 0 } }
	]
	const result = editSketchCurve(source, "curve", { x: 10, y: 10 }, "Split")
	expect(result.removedRelations).toEqual([])
	expect(result.sketch.relations).toEqual([
		{ id: "first", type: "fixed", anchor: { entityId: "curve", point: "point9" }, position: { x: 0, y: 0 } },
		{ id: "second", type: "fixed", anchor: { entityId: "curve", point: "point3" }, position: { x: 0, y: 0 } }
	])
})
