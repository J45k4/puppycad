import { expect, it } from "bun:test"
import { solveSketch, normalizeSketchRelations, type SketchRelation } from "../src/sketch-solver"
import type { Sketch, SketchEntity } from "../src/schema"
import { editSketchCurve } from "../src/sketch-edit"
const entities: SketchEntity[] = [
	{ id: "outer", type: "circle", center: { x: 0, y: 0 }, radius: 20, segments: 64 },
	{ id: "inner", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 64 }
]
const tangent: SketchRelation = { id: "touch", type: "internalTangent", a: "outer", b: "inner" }
const fixed: SketchRelation[] = [
	{ id: "center", type: "fixed", anchor: { entityId: "outer", point: "center" }, position: { x: 0, y: 0 } },
	{ id: "ro", type: "radius", entityId: "outer", value: 20 },
	{ id: "ri", type: "radius", entityId: "inner", value: 5 }
]
it("solves internal tangency from concentric starting circles and retains sliding freedom", () => {
	const result = solveSketch(entities, [...fixed, tangent])
	expect(result.status).toBe("underconstrained")
	expect(result.degreesOfFreedom).toBe(1)
	const outer = result.entities[0]
	const inner = result.entities[1]
	if (outer?.type !== "circle" || inner?.type !== "circle") throw Error("Missing circles")
	expect(Math.hypot(inner.center.x - outer.center.x, inner.center.y - outer.center.y)).toBeCloseTo(15, 5)
	expect(entities[1]).toMatchObject({ center: { x: 0, y: 0 } })
})
it("rejects coincident equal circles and an inner circle larger than its container", () => {
	const same = fixed.map((r) => (r.id === "ri" ? { ...r, value: 20 } : r))
	expect(solveSketch(entities, [...same, tangent]).status).toBe("conflicting")
	const larger = fixed.map((r) => (r.id === "ri" ? { ...r, value: 25 } : r))
	expect(solveSketch(entities, [...larger, tangent]).status).toBe("conflicting")
})
it("persists containment direction and removes the constraint if a circle is split", async () => {
	const relations = [...fixed, tangent]
	expect(normalizeSketchRelations(JSON.parse(JSON.stringify(relations)))).toEqual(relations)
	const result = solveSketch(entities, relations)
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: result.entities,
		relations,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	expect(editSketchCurve(sketch, "outer", { x: 20, y: 0 }, "Split", { x: -20, y: 0 }).removedRelations).toContain("touch")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations).toContainEqual(tangent)
	expect(solveSketch(saved.entities, saved.relations ?? []).status).not.toBe("conflicting")
})
