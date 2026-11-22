import { sketchRelationEntityIds } from "../src/sketch-relations"
import { editSketchCurve } from "../src/sketch-edit"
import { expect, it } from "bun:test"
import type { Sketch } from "../src/schema"
import { chamferSketchCorner, chamferSetbacks, chamferAngle, setChamferMode } from "../src/sketch-chamfer"
import { materializeSketch } from "../src/cad/sketch"
import { normalizeSketchRelations, remapSketchRelations } from "../src/sketch-solver"
const corner = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [
		{ id: "a", type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 0 } },
		{ id: "b", type: "line", p0: { x: 0, y: 0 }, p1: { x: 0, y: 20 } }
	],
	relations: [
		{ id: "join", type: "coincident", a: { entityId: "a", point: "p0" }, b: { entityId: "b", point: "p0" } },
		{ id: "h", type: "horizontal", entityId: "a" },
		{ id: "v", type: "vertical", entityId: "b" },
		{ id: "fa", type: "fixed", anchor: { entityId: "a", point: "p1" }, position: { x: 20, y: 0 } },
		{ id: "fb", type: "fixed", anchor: { entityId: "b", point: "p1" }, position: { x: 0, y: 20 } }
	],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("creates unequal chamfer setbacks and drives both after PCad persistence", async () => {
	const input = corner()
	const result = chamferSketchCorner(input, "a", "b", 3, 5)
	expect(result.removedRelations).toEqual(["join"])
	expect(input.entities).toHaveLength(2)
	let sketch = materializeSketch(result.sketch)
	const dim = sketch.relations?.find((r) => r.type === "chamfer")
	if (dim?.type !== "chamfer") throw Error("Missing chamfer")
	dim.value = 6
	dim.secondValue = 8
	sketch = materializeSketch(sketch)
	expect(structuredClone(sketch.entities.find((e) => e.id === result.lineId))).toMatchObject({
		p0: { x: expect.closeTo(6, 5), y: expect.closeTo(0, 5) },
		p1: { x: expect.closeTo(0, 5), y: expect.closeTo(8, 5) }
	})
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const loaded = materializePartFeatures(restored.cad, restored.tree)[0]
	if (loaded?.type !== "sketch") throw Error("Missing sketch")
	expect(loaded.relations?.find((r) => r.type === "chamfer")).toMatchObject({ value: 6, secondValue: 8 })
	expect(remapSketchRelations([dim], new Map([["a", "copy-a"]]))?.[0]).toMatchObject({ a: { entityId: "copy-a" } })
})
it("handles reversed endpoints and oblique corners, and rejects invalid cuts atomically", () => {
	const input = corner()
	input.relations = []
	input.entities = [
		{ id: "a", type: "line", p1: { x: 0, y: 0 }, p0: { x: 20, y: 0 } },
		{ id: "b", type: "line", p0: { x: 0, y: 0 }, p1: { x: -10, y: 20 } }
	]
	const result = chamferSketchCorner(input, "a", "b", 3, 4)
	const [a, b] = result.sketch.entities
	if (a?.type !== "line" || b?.type !== "line") throw Error("Missing lines")
	const measured = chamferSetbacks(a, "p1", b, "p0")
	expect(measured[0]).toBeCloseTo(3, 8)
	expect(measured[1]).toBeCloseTo(4, 8)
	for (const value of [0, -1, Number.POSITIVE_INFINITY, 25]) expect(() => chamferSketchCorner(input, "a", "b", value)).toThrow()
	expect(input.entities[0]).toMatchObject({ p1: { x: 0, y: 0 } })
	expect(() => normalizeSketchRelations([{ id: "bad", type: "chamfer", a: { entityId: "a", point: "center" }, b: { entityId: "b", point: "p0" }, value: 1, secondValue: 2 }])).toThrow()
})
it("keeps a closed profile and rejects a setback that consumes a constrained edge", () => {
	const input = corner()
	input.entities.push({ id: "c", type: "line", p0: { x: 20, y: 0 }, p1: { x: 20, y: 20 } }, { id: "d", type: "line", p0: { x: 20, y: 20 }, p1: { x: 0, y: 20 } })
	const result = chamferSketchCorner(input, "a", "b", 3, 5)
	expect(materializeSketch(result.sketch).profiles).toHaveLength(1)
	const dimension = result.sketch.relations?.find((r) => r.type === "chamfer")
	if (dimension?.type !== "chamfer") throw Error("Missing dimension")
	dimension.value = 25
	expect(() => materializeSketch(result.sketch)).toThrow()
})

it("tracks the chamfer edge for deletion and dissolves its operation when the edge is split", () => {
	const result = chamferSketchCorner(corner(), "a", "b", 3, 5)
	const relation = result.sketch.relations?.find((r) => r.type === "chamfer")
	if (!relation) throw Error("Missing relation")
	expect(sketchRelationEntityIds(relation)).toContain(result.lineId)
	expect(remapSketchRelations([relation], new Map([[result.lineId, "copy-edge"]]))?.[0]).toMatchObject({ chamferId: "copy-edge" })
	expect(editSketchCurve(result.sketch, result.lineId, { x: 1.5, y: 2.5 }, "Split").removedRelations).toContain(relation.id)
})

it("drives a distance-angle chamfer and converts modes without changing its geometry", async () => {
	const result = chamferSketchCorner(corner(), "a", "b", 3, 30, "distance-angle")
	let sketch = materializeSketch(result.sketch)
	const r = sketch.relations?.find((r) => r.type === "chamfer")
	if (r?.type !== "chamfer") throw Error("Missing chamfer")
	r.secondValue = 60
	sketch = materializeSketch(sketch)
	const a = sketch.entities.find((e) => e.id === "a")
	const b = sketch.entities.find((e) => e.id === "b")
	if (a?.type !== "line" || b?.type !== "line") throw Error("Missing lines")
	expect(chamferAngle(a, "p0", b, "p0")).toBeCloseTo(60, 5)
	expect(chamferSetbacks(a, "p0", b, "p0")[1]).toBeCloseTo(3 * Math.sqrt(3), 5)
	const converted = setChamferMode(sketch, r.id, "two-distances")
	expect(converted.entities).toEqual(sketch.entities)
	expect(converted.relations?.find((q) => q.id === r.id)).toMatchObject({ mode: "two-distances", secondValue: expect.closeTo(3 * Math.sqrt(3), 5) })
	const again = setChamferMode(sketch, r.id, "distance-angle")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [again] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const loaded = materializePartFeatures(restored.cad, restored.tree)[0]
	if (loaded?.type !== "sketch") throw Error("Missing sketch")
	expect(loaded.relations?.find((q) => q.id === r.id)).toMatchObject({ mode: "distance-angle", secondValue: expect.closeTo(60, 5) })
})
it("uses the actual oblique corner angle and rejects angles outside its triangle", () => {
	const input = corner()
	input.relations = []
	const b = input.entities[1]
	if (b?.type !== "line") throw Error("Missing line")
	b.p1 = { x: -10, y: 20 }
	const result = chamferSketchCorner(input, "a", "b", 3, 40, "distance-angle")
	const aCut = result.sketch.entities[0]
	const bCut = result.sketch.entities[1]
	if (aCut?.type !== "line" || bCut?.type !== "line") throw Error("Missing lines")
	expect(chamferAngle(aCut, "p0", bCut, "p0")).toBeCloseTo(40, 8)
	for (const angle of [0, -5, 70, 180]) expect(() => chamferSketchCorner(input, "a", "b", 3, angle, "distance-angle")).toThrow()
})
