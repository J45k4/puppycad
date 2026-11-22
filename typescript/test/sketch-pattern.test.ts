import { expect, it } from "bun:test"
import { createLinearSketchPattern, resizeLinearSketchPattern } from "../src/sketch-pattern"
import { solveSketch, normalizeSketchRelations, remapSketchRelations } from "../src/sketch-solver"
import { materializeSketch } from "../src/cad/sketch"
import { editSketchCurve } from "../src/sketch-edit"
import type { Sketch } from "../src/schema"
const sketch = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 2, segments: 64 }],
	relations: [
		{ id: "center", type: "fixed", anchor: { entityId: "c", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "radius", type: "radius", entityId: "c", value: 4 }
	],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("drives all copies from a source dimension and one pattern step", () => {
	const result = createLinearSketchPattern(sketch(), ["c"], 3, { x: 20, y: -10 })
	const solved = solveSketch(result.sketch.entities, result.sketch.relations ?? [])
	expect(solved.status).toBe("fully-constrained")
	const last = solved.entities[2]
	if (last?.type !== "circle") throw Error("Missing copy")
	expect(last.center.x).toBeCloseTo(40, 5)
	expect(last.center.y).toBeCloseTo(-20, 5)
	expect(last.radius).toBeCloseTo(4, 5)
	expect(materializeSketch(result.sketch).profiles).toHaveLength(3)
})
it("retains surviving IDs and removes dependent constraints when shrinking a pattern", () => {
	const input = sketch()
	const first = createLinearSketchPattern(input, ["c"], 3, { x: 20, y: 0 })
	const originalIds = first.sketch.entities.map((e) => e.id)
	const grown = resizeLinearSketchPattern(first.sketch, first.patternId, 4, { x: 25, y: 5 })
	expect(grown.sketch.entities.slice(0, 3).map((e) => e.id)).toEqual(originalIds)
	const removedId = grown.sketch.entities[3]?.id
	if (!removedId) throw Error("Missing copy")
	grown.sketch.relations?.push({ id: "copy-radius", type: "radius", entityId: removedId, value: 2 })
	const shrunk = resizeLinearSketchPattern(grown.sketch, first.patternId, 2, { x: 25, y: 5 })
	expect(shrunk.sketch.entities.map((e) => e.id)).toEqual(originalIds.slice(0, 2))
	expect(shrunk.removedRelations).toContain("copy-radius")
	expect(input.entities).toHaveLength(1)
	expect(() => resizeLinearSketchPattern(first.sketch, first.patternId, 2, { x: 0, y: 0 })).toThrow("nonzero")
	expect(() => resizeLinearSketchPattern(first.sketch, first.patternId, 2.5, { x: 20, y: 0 })).toThrow("integer")
})
it("persists and remaps every source and instance reference", async () => {
	const created = createLinearSketchPattern(sketch(), ["c"], 3, { x: 20, y: 0 })
	const normalized = normalizeSketchRelations(JSON.parse(JSON.stringify(created.sketch.relations)))
	const remapped = remapSketchRelations(
		normalized,
		new Map([
			["c", "new-c"],
			["pattern-copy-1", "new-copy"]
		])
	)
	expect(remapped?.find((r) => r.type === "linearPattern")).toMatchObject({ sources: ["new-c"], instances: [["new-copy"], ["pattern-copy-2"]] })
	expect(editSketchCurve(created.sketch, "c", { x: 2, y: 0 }, "Split", { x: -2, y: 0 }).removedRelations).toContain(created.patternId)
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [created.sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations?.some((r) => r.type === "linearPattern")).toBe(true)
	expect(saved.profiles).toHaveLength(3)
})
it("patterns connected native arcs and lines without breaking their closed profiles", () => {
	const input = sketch()
	input.entities = [
		{ id: "arc", type: "arc", center: { x: 0, y: 0 }, radius: 2, startAngle: 0, sweep: Math.PI, segments: 64 },
		{ id: "chord", type: "line", p0: { x: -2, y: 0 }, p1: { x: 2, y: 0 } }
	]
	input.relations = []
	const result = createLinearSketchPattern(input, ["arc", "chord"], 3, { x: 10, y: 5 })
	expect(result.sketch.entities.filter((e) => e.type === "arc")).toHaveLength(3)
	expect(result.sketch.entities.filter((e) => e.type === "line")).toHaveLength(3)
	expect(materializeSketch(result.sketch).profiles).toHaveLength(3)
})

it("keeps grid cell IDs and linked dimensions when columns change", () => {
	const created = createLinearSketchPattern(sketch(), ["c"], 3, { x: 20, y: 0 }, 2, { x: 5, y: 30 })
	const pattern = created.sketch.relations?.find((r) => r.type === "linearPattern")
	if (pattern?.type !== "linearPattern") throw Error("Missing pattern")
	const rowStart = pattern.instances[2]?.[0]
	const removed = pattern.instances[4]?.[0]
	if (!rowStart || !removed) throw Error("Missing cells")
	created.sketch.relations?.push({ id: "kept-radius", type: "radius", entityId: rowStart, value: 4 }, { id: "removed-radius", type: "radius", entityId: removed, value: 4 })
	const resized = resizeLinearSketchPattern(created.sketch, created.patternId, 2, { x: 25, y: 0 }, 3, { x: 5, y: 30 })
	const saved = normalizeSketchRelations(JSON.parse(JSON.stringify(resized.sketch.relations)))
	const grid = saved?.find((r) => r.type === "linearPattern")
	if (grid?.type !== "linearPattern") throw Error("Missing grid")
	expect(grid.instances[1]).toEqual([rowStart])
	expect(grid.instances.flat()).not.toContain(removed)
	expect(resized.removedRelations).toEqual(["removed-radius"])
	const solved = solveSketch(resized.sketch.entities, saved ?? [])
	expect(solved.status).toBe("fully-constrained")
	const last = solved.entities.find((e) => e.id === grid.instances[4]?.[0])
	if (last?.type !== "circle") throw Error("Missing circle")
	expect(last.center.x).toBeCloseTo(35, 5)
	expect(last.center.y).toBeCloseTo(60, 5)
	expect(last.radius).toBeCloseTo(4, 5)
	expect(materializeSketch(resized.sketch).profiles).toHaveLength(6)
	expect(() => resizeLinearSketchPattern(created.sketch, created.patternId, 8, { x: 20, y: 0 }, 5)).toThrow("32")
	expect(() => normalizeSketchRelations([{ ...grid, columns: 4 }])).toThrow()
})
it("supports a single grid column and legacy patterns without grid metadata", () => {
	const created = createLinearSketchPattern(sketch(), ["c"], 1, { x: 20, y: 0 }, 3, { x: -5, y: 15 })
	expect(created.sketch.entities[2]).toMatchObject({ center: { x: -10, y: 30 } })
	const legacy = createLinearSketchPattern(sketch(), ["c"], 3, { x: 20, y: 0 })
	const relation = legacy.sketch.relations?.find((r) => r.type === "linearPattern")
	if (relation?.type !== "linearPattern") throw Error("Missing pattern")
	relation.columns = undefined
	relation.rowStep = undefined
	expect(solveSketch(legacy.sketch.entities, normalizeSketchRelations(JSON.parse(JSON.stringify(legacy.sketch.relations))) ?? []).status).toBe("fully-constrained")
})
