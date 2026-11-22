import { expect, it } from "bun:test"
import { measureSketchDimension } from "../src/sketch-dimensions"
import { solveSketch, normalizeSketchRelations } from "../src/sketch-solver"
import { materializeSketch } from "../src/cad/sketch"
import { editSketchCurve } from "../src/sketch-edit"
import type { Sketch } from "../src/schema"
const input = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [{ id: "l", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }],
	relations: [{ id: "ref", type: "length", entityId: "l", value: 999, reference: true }],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("measures reference dimensions without driving geometry or consuming freedom", () => {
	const sketch = input()
	const solved = solveSketch(sketch.entities, sketch.relations ?? [])
	expect(solved.status).toBe("underconstrained")
	expect(solved.degreesOfFreedom).toBe(4)
	expect(solved.entities).toEqual(sketch.entities)
	expect(solved.redundantEquations).toBe(0)
	expect(materializeSketch(sketch).relations?.[0]).toMatchObject({ value: 10, reference: true })
	expect(sketch.relations?.[0]).toMatchObject({ value: 999 })
})
it("retains live reference measurements when other dimensions move geometry and after persistence", async () => {
	const sketch = input()
	sketch.relations?.push({ id: "driving", type: "length", entityId: "l", value: 20 })
	const updated = materializeSketch(sketch)
	expect(updated.relations?.[0]).toMatchObject({ value: expect.closeTo(20, 5), reference: true })
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations?.[0]).toMatchObject({ reference: true, value: expect.closeTo(20, 5) })
	const split = editSketchCurve(input(), "l", { x: 4, y: 0 }, "Split")
	expect(split.removedRelations).not.toContain("ref")
	expect(materializeSketch(split.sketch).relations?.find((r) => r.id === "ref")).toMatchObject({ value: 4, reference: true })
})
it("measures signed distances, rotations and sweep values and validates reference flags", () => {
	const entities: Sketch["entities"] = [
		{ id: "a", type: "point" as const, center: { x: 5, y: 8 } },
		{ id: "b", type: "point" as const, center: { x: 1, y: 2 } }
	]
	expect(measureSketchDimension(entities, { id: "x", type: "distance", a: { entityId: "a", point: "center" }, b: { entityId: "b", point: "center" }, axis: "x", value: 0 })).toBe(-4)
	entities.push(
		{ id: "arc", type: "arc", center: { x: 0, y: 0 }, radius: 10, startAngle: 0, sweep: -Math.PI / 2, segments: 32 },
		{ id: "ellipse", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 230, segments: 32 }
	)
	expect(measureSketchDimension(entities, { id: "sweep", type: "arcSweep", entityId: "arc", value: 1 })).toBe(-90)
	expect(measureSketchDimension(entities, { id: "rotation", type: "rotation", entityId: "ellipse", value: 0 })).toBe(230)
	expect(() => normalizeSketchRelations([{ id: "bad", type: "coincident", a: { entityId: "a", point: "center" }, b: { entityId: "b", point: "center" }, reference: true }])).toThrow()
	expect(normalizeSketchRelations([{ id: "zero", type: "length", entityId: "line", value: 0, reference: true }])).toHaveLength(1)
	expect(() => normalizeSketchRelations([{ id: "bad", type: "length", entityId: "line", value: 0 }])).toThrow()
})

it("validates retained dimensional expressions and rejects stale numeric caches", () => {
	const relation = { id: "length", type: "length" as const, entityId: "l", value: 25.4, expression: "1 in" }
	expect(normalizeSketchRelations([relation])).toEqual([relation])
	for (const change of [{ value: 10 }, { expression: "pi rad" }, { expression: "1 / 0" }, { expression: 12 }, { reference: true }])
		expect(() => normalizeSketchRelations([{ ...relation, ...change }])).toThrow()
	expect(() => normalizeSketchRelations([{ id: "h", type: "horizontal", entityId: "l", expression: "2" }])).toThrow()
})
