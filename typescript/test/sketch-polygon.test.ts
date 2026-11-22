import { expect, it } from "bun:test"
import { regularPolygon, polygonVertices, setPolygonSideCount } from "../src/sketch-polygon"
import { entityAnchorPoint, entityAnchorNames } from "../src/sketch-curves"
import { solveSketch, normalizeSketchRelations, remapSketchRelations } from "../src/sketch-solver"
import { createLinearSketchPattern } from "../src/sketch-pattern"
import { materializeSketch } from "../src/cad/sketch"
import type { Sketch } from "../src/schema"
const input = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [regularPolygon("p", { x: 0, y: 0 }, { x: 10, y: 0 }, 6)],
	relations: [],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("constructs inscribed and circumscribed polygons and exposes every vertex", () => {
	const polygon = regularPolygon("p", { x: 0, y: 0 }, { x: 10, y: 0 }, 6)
	expect(polygonVertices(polygon)).toHaveLength(6)
	expect(entityAnchorNames(polygon)).toHaveLength(7)
	expect(entityAnchorPoint(polygon, "vertex5").x).toBeCloseTo(5, 6)
	expect(entityAnchorPoint(polygon, "vertex0")).toEqual({ x: 10, y: 0 })
	const outer = regularPolygon("p", { x: 0, y: 0 }, { x: 10, y: 0 }, 6, true)
	const vertices = polygonVertices(outer)
	expect(((vertices[0]?.x ?? Number.NaN) + (vertices[1]?.x ?? Number.NaN)) / 2).toBeCloseTo(10, 6)
	expect(((vertices[0]?.y ?? Number.NaN) + (vertices[1]?.y ?? Number.NaN)) / 2).toBeCloseTo(0, 6)
	expect(() => regularPolygon("p", { x: 0, y: 0 }, { x: 10, y: 0 }, 2)).toThrow("3 to 64")
	expect(() => entityAnchorPoint(polygon, "vertex6")).toThrow("no vertex6")
})
it("solves polygon size and rotation while keeping a referenced vertex fixed", () => {
	const polygon = input().entities[0]
	if (!polygon) throw Error("Missing polygon")
	const solved = solveSketch(
		[polygon],
		[
			{ id: "anchor", type: "fixed", anchor: { entityId: "p", point: "vertex5" }, position: { x: 5, y: -Math.sqrt(75) } },
			{ id: "radius", type: "radius", entityId: "p", value: 15 },
			{ id: "rotation", type: "rotation", entityId: "p", value: 30 }
		]
	)
	expect(solved.status).toBe("fully-constrained")
	const result = solved.entities[0]
	if (result?.type !== "polygon") throw Error("Missing polygon")
	expect(result.radius).toBeCloseTo(15, 5)
	expect(result.rotation).toBeCloseTo(30, 5)
	expect(entityAnchorPoint(result, "vertex5").x).toBeCloseTo(5, 5)
	expect(entityAnchorPoint(result, "vertex5").y).toBeCloseTo(-Math.sqrt(75), 5)
})
it("propagates side counts into copies and removes constraints on deleted vertices", () => {
	const created = createLinearSketchPattern(input(), ["p"], 2, { x: 30, y: 0 })
	created.sketch.relations?.push({ id: "deleted", type: "fixed", anchor: { entityId: "pattern-copy-1", point: "vertex5" }, position: { x: 35, y: -Math.sqrt(75) } })
	const resized = setPolygonSideCount(created.sketch, "p", 4)
	expect(resized.removedRelations).toEqual(["deleted"])
	expect(resized.sketch.entities.every((entity) => entity.type === "polygon" && entity.sides === 4)).toBe(true)
	expect(materializeSketch(resized.sketch).profiles).toHaveLength(2)
	expect(() => setPolygonSideCount(resized.sketch, "pattern-copy-1", 5)).toThrow("source polygon")
	expect(() => setPolygonSideCount(created.sketch, "p", 2)).toThrow()
})
it("persists polygons and their vertex constraints through PCad and remapping", async () => {
	const sketch = input()
	sketch.relations = [{ id: "fixed", type: "fixed", anchor: { entityId: "p", point: "vertex5" }, position: { x: 5, y: -Math.sqrt(75) } }]
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.entities[0]).toMatchObject({ type: "polygon", sides: 6 })
	expect(saved.profiles).toHaveLength(1)
	expect(remapSketchRelations(normalizeSketchRelations(saved.relations), new Map([["p", "copy"]]))?.[0]).toMatchObject({ anchor: { entityId: "copy", point: "vertex5" } })
})
