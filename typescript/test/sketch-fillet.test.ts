import { expect, it } from "bun:test"
import { filletSketchCorner } from "../src/sketch-fillet"
import { solveSketch } from "../src/sketch-solver"
import { materializeSketch } from "../src/cad/sketch"
import { arcPoint } from "../src/sketch-curves"
import type { Sketch, Line } from "../src/schema"
const line = (id: string, x0: number, y0: number, x1: number, y1: number): Line => ({ id, type: "line", p0: { x: x0, y: y0 }, p1: { x: x1, y: y1 } })
const sketch = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [line("a", 0, 0, 20, 0), line("b", 20, 0, 20, 20)],
	relations: [
		{ id: "horizontal", type: "horizontal", entityId: "a" },
		{ id: "vertical", type: "vertical", entityId: "b" },
		{ id: "first", type: "fixed", anchor: { entityId: "a", point: "p0" }, position: { x: 0, y: 0 } },
		{ id: "last", type: "fixed", anchor: { entityId: "b", point: "p1" }, position: { x: 20, y: 20 } }
	],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("creates a tangent native corner arc and solves a changed driving radius", () => {
	const input = sketch()
	const result = filletSketchCorner(input, "a", "b", 2)
	const arc = result.sketch.entities[2]
	if (arc?.type !== "arc") throw Error("Missing fillet arc")
	expect(arc.radius).toBe(2)
	expect(arc.center.x).toBeCloseTo(18, 8)
	expect(arcPoint(arc, 0).y).toBeCloseTo(0, 8)
	expect(arcPoint(arc, 1).x).toBeCloseTo(20, 8)
	const relations = result.sketch.relations?.map((r) => (r.id === result.radiusId ? { ...r, value: 4 } : r)) ?? []
	const solved = solveSketch(result.sketch.entities, relations)
	expect(solved.status).toBe("fully-constrained")
	expect(solved.entities[0]).toMatchObject({ p1: { x: expect.closeTo(16, 5), y: expect.closeTo(0, 5) } })
	expect(solved.entities[1]).toMatchObject({ p0: { x: expect.closeTo(20, 5), y: expect.closeTo(4, 5) } })
	expect(input.entities).toHaveLength(2)
})
it("removes obsolete corner and length constraints and rejects oversize radii atomically", () => {
	const input = sketch()
	input.relations?.push({ id: "length", type: "length", entityId: "a", value: 20 }, { id: "corner", type: "coincident", a: { entityId: "a", point: "p1" }, b: { entityId: "b", point: "p0" } })
	expect(filletSketchCorner(input, "b", "a", 3).removedRelations).toEqual(["length", "corner"])
	const before = JSON.stringify(input)
	expect(() => filletSketchCorner(input, "a", "b", 20)).toThrow("too large")
	expect(JSON.stringify(input)).toBe(before)
	const straight = sketch()
	straight.entities[1] = line("b", 20, 0, 40, 0)
	expect(() => filletSketchCorner(straight, "a", "b", 2)).toThrow("collinear")
})
it("keeps a rounded rectangle profile closed through PCad persistence", async () => {
	const input = sketch()
	input.entities.push(line("c", 20, 20, 0, 20), line("d", 0, 20, 0, 0))
	const result = filletSketchCorner(input, "a", "b", 3)
	expect(materializeSketch(result.sketch).profiles).toHaveLength(1)
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [result.sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.entities.some((e) => e.type === "arc")).toBe(true)
	expect(saved.relations?.filter((r) => r.type === "endpointTangent")).toHaveLength(2)
	expect(saved.profiles).toHaveLength(1)
})

it("rejects a driving radius edit that would reverse the trimmed lines", () => {
	const result = filletSketchCorner(sketch(), "a", "b", 2)
	for (const value of [20, 25]) {
		const relations = result.sketch.relations?.map((r) => (r.id === result.radiusId ? { ...r, value } : r)) ?? []
		expect(solveSketch(result.sketch.entities, relations).status).toBe("conflicting")
	}
})
