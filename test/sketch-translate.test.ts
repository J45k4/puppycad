import { entityAnchorPoint } from "../src/sketch-curves"
import { measureSketchDimension } from "../src/sketch-dimensions"
import { createLinearSketchPattern } from "../src/sketch-pattern"
import { chamferSketchCorner } from "../src/sketch-chamfer"
import { expect, it } from "bun:test"
import type { Sketch } from "../src/schema"
import { translateSketchSelection, scaleSketchSelection, rotateSketchSelection } from "../src/sketch-translate"
import { materializeSketch } from "../src/cad/sketch"
import { createCircularSketchPattern } from "../src/sketch-circular-pattern"
const input = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [
		{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 64 },
		{ id: "p", type: "point", center: { x: 0, y: 0 }, construction: true }
	],
	relations: [
		{ id: "fixed", type: "fixed", anchor: { entityId: "c", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "radius", type: "radius", entityId: "c", value: 5, labelPosition: { x: 6, y: 6 } },
		{ id: "join", type: "coincident", a: { entityId: "p", point: "center" }, b: { entityId: "c", point: "center" } }
	],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("moves internal constraints and detaches only cross-group dependencies", () => {
	const original = input()
	const result = translateSketchSelection(original, ["c"], { x: 20, y: -5 })
	expect(result.removedRelations).toEqual(["join"])
	const moved = materializeSketch(result.sketch)
	expect(moved.entities[0]).toMatchObject({ center: { x: 20, y: -5 }, radius: 5 })
	expect(moved.entities[1]).toMatchObject({ center: { x: 0, y: 0 } })
	expect(moved.relations?.find((r) => r.id === "radius")).toMatchObject({ labelPosition: { x: 26, y: 1 } })
	expect(original.entities[0]).toMatchObject({ center: { x: 0, y: 0 } })
	expect(original.relations).toHaveLength(3)
})
it("copies a constrained group with independent IDs and preserves it through PCad", async () => {
	const original = input()
	const result = translateSketchSelection(original, ["c", "p"], { x: 20, y: 0 }, true)
	expect(result.removedRelations).toEqual([])
	expect(result.sketch.entities).toHaveLength(4)
	expect(new Set(result.sketch.relations?.map((r) => r.id)).size).toBe(6)
	const copy = result.sketch.relations?.find((r) => r.type === "radius" && r.entityId === result.selected[0])
	if (copy?.type !== "radius") throw Error("Missing copied radius")
	copy.value = 7
	const moved = materializeSketch(result.sketch)
	expect(moved.profiles).toHaveLength(2)
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [moved] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.entities.find((e) => e.id === result.selected[0])).toMatchObject({ center: { x: 20, y: 0 }, radius: expect.closeTo(7, 5) })
	expect(saved.entities.find((e) => e.id === "c")).toMatchObject({ center: { x: 0, y: 0 }, radius: 5 })
	expect(saved.entities.find((e) => e.id === result.selected[1])).toMatchObject({ center: { x: 20, y: 0 }, construction: true })
})
it("translates complete circular patterns and rejects invalid selections atomically", () => {
	const sketch = input()
	sketch.entities = [{ id: "c", type: "circle", center: { x: 10, y: 0 }, radius: 2, segments: 64 }]
	sketch.relations = []
	const pattern = createCircularSketchPattern(sketch, ["c"], 3, { x: 0, y: 0 }, 360).sketch
	const moved = translateSketchSelection(
		pattern,
		pattern.entities.map((e) => e.id),
		{ x: 20, y: 30 }
	)
	const saved = materializeSketch(moved.sketch)
	expect(saved.relations?.find((r) => r.type === "circularPattern")).toMatchObject({ center: { x: 20, y: 30 } })
	for (let i = 0; i < pattern.entities.length; i++) {
		const a = pattern.entities[i]
		const b = saved.entities[i]
		if (a?.type !== "circle" || b?.type !== "circle") throw Error("Missing circles")
		expect(b.center.x - a.center.x).toBeCloseTo(20, 5)
		expect(b.center.y - a.center.y).toBeCloseTo(30, 5)
	}
	for (const ids of [[], ["missing"], ["c", "c"]]) expect(() => translateSketchSelection(sketch, ids, { x: 1, y: 0 })).toThrow()
	expect(() => translateSketchSelection(sketch, ["c"], { x: Number.NaN, y: 0 })).toThrow()
})

it("scales a dimensioned copy about an arbitrary center and persists independent dimensions", async () => {
	const original = input()
	const result = scaleSketchSelection(original, ["c", "p"], { x: -20, y: 0 }, 2, true)
	const moved = materializeSketch(result.sketch)
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [moved] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.entities.find((e) => e.id === result.selected[0])).toMatchObject({ center: { x: 20, y: 0 }, radius: 10 })
	expect(saved.entities.find((e) => e.id === "c")).toMatchObject({ center: { x: 0, y: 0 }, radius: 5 })
	expect(saved.entities.find((e) => e.id === result.selected[1])).toMatchObject({ center: { x: 20, y: 0 }, construction: true })
	expect(saved.relations?.find((r) => r.type === "radius" && r.entityId === result.selected[0])).toMatchObject({ value: 10, labelPosition: { x: 32, y: 12 } })
})
it("scales a distance-angle chamfer's linear setback without changing its angle", () => {
	const sketch = input()
	sketch.entities = [
		{ id: "a", type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 0 } },
		{ id: "b", type: "line", p0: { x: 0, y: 0 }, p1: { x: 0, y: 20 } }
	]
	sketch.relations = []
	const chamfer = chamferSketchCorner(sketch, "a", "b", 3, 30, "distance-angle").sketch
	const result = scaleSketchSelection(
		chamfer,
		chamfer.entities.map((e) => e.id),
		{ x: 0, y: 0 },
		2
	)
	const saved = materializeSketch(result.sketch)
	expect(saved.relations?.find((r) => r.type === "chamfer")).toMatchObject({ value: 6, secondValue: 30, mode: "distance-angle" })
	expect(saved.entities.find((e) => e.id === "a")).toMatchObject({ p0: { x: expect.closeTo(6, 6), y: 0 }, p1: { x: 40, y: 0 } })
	for (const factor of [0, -1, Number.POSITIVE_INFINITY, Number.NaN]) expect(() => scaleSketchSelection(sketch, ["a"], { x: 0, y: 0 }, factor)).toThrow()
})

it("scales both directions of a complete grid pattern", () => {
	const sketch = input()
	sketch.entities = sketch.entities.filter((e) => e.id === "c")
	sketch.relations = []
	const pattern = createLinearSketchPattern(sketch, ["c"], 2, { x: 20, y: 0 }, 2, { x: 0, y: 30 }).sketch
	const result = scaleSketchSelection(
		pattern,
		pattern.entities.map((e) => e.id),
		{ x: 0, y: 0 },
		2
	)
	const saved = materializeSketch(result.sketch)
	expect(saved.relations?.find((r) => r.type === "linearPattern")).toMatchObject({ step: { x: 40, y: 0 }, rowStep: { x: 0, y: 60 } })
	expect(saved.entities).toHaveLength(4)
	for (const e of saved.entities) {
		if (e.type !== "circle") throw Error("Missing circle")
		expect(e.radius).toBeCloseTo(10, 6)
	}
})

it("rotates line orientation, fixed positions and signed coordinate dimensions together", () => {
	const sketch = input()
	sketch.entities = [
		{ id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } },
		{ id: "p", type: "point", center: { x: 10, y: 3 } }
	]
	sketch.relations = [
		{ id: "fixed", type: "fixed", anchor: { entityId: "line", point: "p0" }, position: { x: 0, y: 0 } },
		{ id: "horizontal", type: "horizontal", entityId: "line" },
		{ id: "length", type: "length", entityId: "line", value: 10 },
		{ id: "point", type: "fixed", anchor: { entityId: "p", point: "center" }, position: { x: 10, y: 3 } },
		{ id: "x", type: "distance", a: { entityId: "line", point: "p0" }, b: { entityId: "p", point: "center" }, value: 10, axis: "x" }
	]
	const result = materializeSketch(rotateSketchSelection(sketch, ["line", "p"], { x: 0, y: 0 }, 30).sketch)
	const orientation = result.relations?.find((r) => r.id === "horizontal")
	expect(orientation).toMatchObject({ type: "rotation", value: expect.closeTo(30, 6) })
	const dimension = result.relations?.find((r) => r.id === "x")
	if (!dimension || dimension.type !== "distance") throw Error("Missing dimension")
	expect(dimension.axis).toBeUndefined()
	expect(dimension.direction?.x).toBeCloseTo(Math.sqrt(3) / 2, 8)
	expect(dimension.direction?.y).toBeCloseTo(0.5, 8)
	expect(measureSketchDimension(result.entities, dimension)).toBeCloseTo(10, 6)
})
it("preserves both rectangle corner identities after rotation and PCad persistence", async () => {
	const sketch = input()
	sketch.entities = [{ id: "rect", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 20 } }]
	sketch.relations = [
		{ id: "a", type: "fixed", anchor: { entityId: "rect", point: "p0" }, position: { x: 0, y: 0 } },
		{ id: "b", type: "fixed", anchor: { entityId: "rect", point: "p1" }, position: { x: 10, y: 20 } }
	]
	const result = rotateSketchSelection(sketch, ["rect"], { x: 0, y: 0 }, 90, true)
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [result.sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	const copied = saved.entities.find((e) => e.id === result.selected[0])
	if (!copied) throw Error("Missing copy")
	expect(copied.type).toBe("rectangle")
	for (const relation of saved.relations ?? [])
		if (relation.type === "fixed" && relation.anchor.entityId === copied.id) {
			const actual = entityAnchorPoint(copied, relation.anchor.point)
			expect(actual.x).toBeCloseTo(relation.position.x, 6)
			expect(actual.y).toBeCloseTo(relation.position.y, 6)
		}
	expect(saved.relations?.some((r) => r.type === "rotation" && r.entityId === copied.id)).toBe(true)
})
it("rotates grid pattern directions without changing distances", () => {
	const sketch = input()
	sketch.entities = sketch.entities.filter((e) => e.id === "c")
	sketch.relations = []
	const pattern = createLinearSketchPattern(sketch, ["c"], 2, { x: 20, y: 0 }, 2, { x: 0, y: 30 }).sketch
	const saved = materializeSketch(
		rotateSketchSelection(
			pattern,
			pattern.entities.map((e) => e.id),
			{ x: 0, y: 0 },
			90
		).sketch
	)
	const r = saved.relations?.find((r) => r.type === "linearPattern")
	if (r?.type !== "linearPattern") throw Error("Missing pattern")
	expect(r.step.x).toBeCloseTo(0, 8)
	expect(r.step.y).toBeCloseTo(20, 8)
	expect(r.rowStep?.x).toBeCloseTo(-30, 8)
	expect(r.rowStep?.y).toBeCloseTo(0, 8)
})

it("retains arithmetic for translation and clears it when scaling overrides the dimension", () => {
	const source = input()
	const relation = source.relations?.find((r) => r.id === "radius")
	if (!relation) throw Error("Missing radius")
	relation.expression = "10 mm / 2"
	const moved = translateSketchSelection(source, ["c", "p"], { x: 20, y: 10 })
	expect(moved.sketch.relations?.find((r) => r.id === "radius")?.expression).toBe("10 mm / 2")
	const scaled = scaleSketchSelection(source, ["c", "p"], { x: 0, y: 0 }, 2)
	expect(scaled.sketch.relations?.find((r) => r.id === "radius")).toMatchObject({ value: 10 })
	expect(scaled.sketch.relations?.find((r) => r.id === "radius")?.expression).toBeUndefined()
	expect(relation.expression).toBe("10 mm / 2")
})
