import { expect, it } from "bun:test"
import { offsetSketchEntity } from "../src/sketch-offset"
import { solveSketch, normalizeSketchRelations, remapSketchRelations, type SketchRelation } from "../src/sketch-solver"
import type { SketchEntity } from "../src/schema"
const source: SketchEntity = { id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 20, segments: 64 }
it("drives a concentric offset from both its source dimension and signed distance", () => {
	const target = offsetSketchEntity(source, 5, "copy")
	const relations: SketchRelation[] = [
		{ id: "fixed", type: "fixed", anchor: { entityId: "c", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "radius", type: "radius", entityId: "c", value: 30 },
		{ id: "offset", type: "offset", a: "c", b: "copy", value: -8 }
	]
	const result = solveSketch([source, target], relations)
	expect(result.status).toBe("fully-constrained")
	expect(result.entities[1]).toMatchObject({ radius: expect.closeTo(22, 5), center: { x: expect.closeTo(0, 5), y: expect.closeTo(0, 5) } })
	expect(source.radius).toBe(20)
	const normalized = normalizeSketchRelations(JSON.parse(JSON.stringify(relations)))
	expect(
		remapSketchRelations(
			normalized,
			new Map([
				["c", "new-c"],
				["copy", "new-copy"]
			])
		)?.[2]
	).toMatchObject({ a: "new-c", b: "new-copy", value: -8 })
})
it("offsets directed lines, clockwise arcs and reversed rectangles without losing native geometry", () => {
	expect(offsetSketchEntity({ id: "l", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }, 3, "b")).toMatchObject({ p0: { x: 0, y: 3 }, p1: { x: 10, y: 3 } })
	expect(offsetSketchEntity({ id: "a", type: "arc", center: { x: 0, y: 0 }, radius: 10, startAngle: 1, sweep: -2, segments: 64 }, -3, "b")).toMatchObject({ radius: 7, startAngle: 1, sweep: -2 })
	expect(offsetSketchEntity({ id: "r", type: "cornerRectangle", p0: { x: 10, y: 20 }, p1: { x: 0, y: 0 } }, 2, "b")).toMatchObject({ p0: { x: 12, y: 22 }, p1: { x: -2, y: -2 } })
	expect(() => offsetSketchEntity(source, -20, "bad")).toThrow("positive")
})
it("round-trips linked offsets through PCad materialization", async () => {
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const sketch: import("../src/schema").Sketch = {
		id: "sketch",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [source, offsetSketchEntity(source, 5, "copy")],
		relations: [{ id: "o", type: "offset", a: "c", b: "copy", value: 5 }],
		dimensions: [],
		loops: [],
		vertices: [],
		profiles: []
	}
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing saved sketch")
	expect(result.relations).toContainEqual(expect.objectContaining({ type: "offset", value: 5 }))
	expect(result.entities).toHaveLength(2)
	expect(result.profiles.length).toBeGreaterThan(0)
})
