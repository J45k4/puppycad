import { requireValue } from "../src/required"
import { expect, it } from "bun:test"
import { threePointCircle } from "../src/sketch-curves"
import { solveSketch, type SketchRelation } from "../src/sketch-solver"
import type { Sketch } from "../src/schema"
it("creates the same native circle for either point order and at different scales", () => {
	for (const scale of [0.0001, 1, 10000])
		for (const reverse of [false, true]) {
			const a = { x: 7 + 5 * scale, y: -3 }
			const b = { x: 7, y: -3 + 5 * scale }
			const c = { x: 7 - 5 * scale, y: -3 }
			const circle = threePointCircle("circle", a, reverse ? c : b, reverse ? b : c)
			expect(circle.type).toBe("circle")
			expect(circle.center.x).toBeCloseTo(7, 6)
			expect(circle.center.y).toBeCloseTo(-3, 6)
			expect(circle.radius).toBeCloseTo(5 * scale, 6)
		}
	expect(() => threePointCircle("c", { x: 0, y: 0 }, { x: 1, y: 1 }, { x: 2, y: 2 })).toThrow("collinear")
	expect(() => threePointCircle("c", { x: 0, y: 0 }, { x: 0, y: 0 }, { x: 1, y: 1 })).toThrow("distinct")
	expect(() => threePointCircle("c", { x: Number.NaN, y: 0 }, { x: 1, y: 0 }, { x: 0, y: 1 })).toThrow("finite")
})
it("keeps three referenced points on a saved circle when a reference moves", async () => {
	const points = [
		{ x: 0, y: 0 },
		{ x: 10, y: 0 },
		{ x: 0, y: 10 }
	]
	const circle = threePointCircle("circle", requireValue(points[0]), requireValue(points[1]), requireValue(points[2]))
	const entities = [...points.map((center, i) => ({ id: `p${i}`, type: "circle" as const, center, radius: 0.2, segments: 32, construction: true })), circle]
	const relations: SketchRelation[] = points.flatMap((position, i): SketchRelation[] => [
		{ id: `fixed${i}`, type: "fixed", anchor: { entityId: `p${i}`, point: "center" }, position },
		{ id: `size${i}`, type: "radius", entityId: `p${i}`, value: 0.2 },
		{ id: `on${i}`, type: "pointOnCurve", a: { entityId: `p${i}`, point: "center" }, b: "circle" }
	])
	const sketch: Sketch = { id: "s", type: "sketch", dirty: false, target: { type: "plane", plane: "XY" }, entities, relations, dimensions: [], vertices: [], loops: [], profiles: [] }
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.profiles).toHaveLength(1)
	const fixed = saved.relations?.find((r) => r.id === "fixed2")
	if (fixed?.type !== "fixed") throw Error("Missing reference")
	fixed.position = { x: 0, y: 15 }
	const solved = solveSketch(saved.entities, saved.relations ?? [])
	expect(solved.status).toBe("fully-constrained")
	const result = solved.entities.find((e) => e.id === "circle")
	if (result?.type !== "circle") throw Error("Missing circle")
	expect(result.center.x).toBeCloseTo(5, 5)
	expect(result.center.y).toBeCloseTo(7.5, 5)
	expect(result.radius).toBeCloseTo(Math.hypot(5, 7.5), 5)
})
