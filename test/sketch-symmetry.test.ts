import { expect, it } from "bun:test"
import type { Sketch, SketchEntity } from "../src/schema"
import { solveSketch, normalizeSketchRelations, remapSketchRelations, type SketchRelation } from "../src/sketch-solver"
import { editSketchCurve } from "../src/sketch-edit"
const entities: SketchEntity[] = [
	{ id: "a", type: "circle", center: { x: 3, y: 4 }, radius: 1, segments: 32, construction: true },
	{ id: "b", type: "circle", center: { x: -2, y: 5 }, radius: 1, segments: 32, construction: true },
	{ id: "axis", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 10 }, construction: true }
]
const symmetric: SketchRelation = { id: "sym", type: "symmetric", a: { entityId: "a", point: "center" }, b: { entityId: "b", point: "center" }, symmetryLine: "axis" }
const fixed: SketchRelation[] = [
	{ id: "first", type: "fixed", anchor: { entityId: "a", point: "center" }, position: { x: 3, y: 4 } },
	{ id: "start", type: "fixed", anchor: { entityId: "axis", point: "p0" }, position: { x: 0, y: 0 } },
	{ id: "end", type: "fixed", anchor: { entityId: "axis", point: "p1" }, position: { x: 10, y: 10 } },
	{ id: "ra", type: "radius", entityId: "a", value: 1 },
	{ id: "rb", type: "radius", entityId: "b", value: 1 }
]
it("reflects a point across a rotated line and follows a changed axis", () => {
	const result = solveSketch(entities, [...fixed, symmetric])
	expect(result.status).toBe("fully-constrained")
	expect(structuredClone(result.entities[1])).toMatchObject({ center: { x: expect.closeTo(4, 5), y: expect.closeTo(3, 5) } })
	const updated = fixed.map((r) => (r.id === "end" ? { ...r, position: { x: 0, y: 10 } } : r))
	const moved = solveSketch(result.entities, [...updated, symmetric])
	expect(moved.status).toBe("fully-constrained")
	expect(moved.entities[1]).toMatchObject({ center: { x: expect.closeTo(-3, 5), y: expect.closeTo(4, 5) } })
	expect(entities[1]).toMatchObject({ center: { x: -2, y: 5 } })
})
it("reports incompatible fixed points and rejects a zero-length symmetry axis", () => {
	expect(solveSketch(entities, [...fixed, symmetric, { id: "second", type: "fixed", anchor: { entityId: "b", point: "center" }, position: { x: 9, y: 9 } }]).status).toBe("conflicting")
	const bad = structuredClone(entities)
	const axis = bad[2]
	if (axis?.type !== "line") throw Error("Missing axis")
	axis.p1 = { ...axis.p0 }
	expect(() => solveSketch(bad, [symmetric])).toThrow("nonzero")
})
it("persists all three references and removes symmetry when the axis is split", async () => {
	const normalized = normalizeSketchRelations(JSON.parse(JSON.stringify([symmetric])))
	expect(
		remapSketchRelations(
			normalized,
			new Map([
				["a", "new-a"],
				["b", "new-b"],
				["axis", "new-axis"]
			])
		)
	).toEqual([{ ...symmetric, a: { entityId: "new-a", point: "center" }, b: { entityId: "new-b", point: "center" }, symmetryLine: "new-axis" }])
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities,
		relations: [...fixed, symmetric],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	expect(editSketchCurve(sketch, "axis", { x: 5, y: 5 }, "Split").removedRelations).toContain("sym")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing sketch")
	expect(result.relations).toContainEqual(symmetric)
	expect(solveSketch(result.entities, result.relations ?? []).status).toBe("fully-constrained")
})
