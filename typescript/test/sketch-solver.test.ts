import { expect, it } from "bun:test"
import { solveSketch, normalizeSketchRelations, type SketchRelation } from "../src/sketch-solver"
import type { Sketch, SketchEntity, PartDocument } from "../src/schema"
import { materializeSketch } from "../src/cad/sketch"
import { createPartRuntimeState, materializePartFeatures, serializePCadState } from "../src/pcad/part-state"
import { createProjectFile, normalizeProjectFile, serializeProjectFile } from "../src/project-file"
import { requireValue } from "../src/required"
import { extrudeSolidFeature } from "../src/cad/extrude"

const center = (entityId: string) => ({ entityId, point: "center" as const })
const p0 = (entityId: string) => ({ entityId, point: "p0" as const })
const p1 = (entityId: string) => ({ entityId, point: "p1" as const })
function ring(): Sketch {
	return {
		id: "ring-sketch",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: [],
		entities: [
			{ id: "pitch", type: "circle", center: { x: 1, y: 2 }, radius: 100, segments: 96, construction: true },
			{ id: "outer", type: "circle", center: { x: -2, y: 1 }, radius: 110, segments: 96 },
			{ id: "inner", type: "circle", center: { x: 2, y: -1 }, radius: 95, segments: 96 }
		],
		relations: [
			{ id: "origin", type: "fixed", anchor: center("pitch"), position: { x: 0, y: 0 } },
			{ id: "outer-center", type: "concentric", a: "pitch", b: "outer" },
			{ id: "inner-center", type: "concentric", a: "pitch", b: "inner" },
			{ id: "pitch-diameter", type: "diameter", entityId: "pitch", value: 210, labelPosition: { x: 120, y: -100 } },
			{ id: "outer-offset", type: "radiusDifference", a: "outer", b: "pitch", value: 8 },
			{ id: "inner-offset", type: "radiusDifference", a: "pitch", b: "inner", value: 8 }
		]
	}
}
it("solves the reference annulus from a construction circle and driving dimensions", () => {
	const sketch = ring()
	const original = structuredClone(sketch)
	const result = solveSketch(sketch.entities, sketch.relations ?? [])
	expect(result.status).toBe("fully-constrained")
	expect(result.degreesOfFreedom).toBe(0)
	expect(result.conflicts).toEqual([])
	for (const [index, radius] of [105, 113, 97].entries()) {
		const e = result.entities[index]
		if (e?.type !== "circle") throw Error("Missing circle")
		expect(e.radius).toBeCloseTo(radius, 5)
		expect(e.center.x).toBeCloseTo(0, 5)
		expect(e.center.y).toBeCloseTo(0, 5)
	}
	expect(sketch).toEqual(original)
	const materialized = materializeSketch(sketch)
	expect(materialized.loops).toHaveLength(2)
	expect(materialized.profiles).toHaveLength(1)
	expect(materialized.profiles[0]?.holeLoopIds).toHaveLength(1)
})
it("rebuilds dependent extrusion geometry when a driving dimension changes", () => {
	const sketch = materializeSketch(ring())
	const extrusion = {
		type: "extrude" as const,
		id: "ring",
		dirty: false,
		depth: 25,
		operation: "new" as const,
		target: { type: "profileRef" as const, sketchId: sketch.id, profileId: requireValue(sketch.profiles[0]).id }
	}
	const doc: PartDocument = { features: [sketch, extrusion] }
	const before = extrudeSolidFeature(doc, extrusion)
	const diameter = sketch.relations?.find((r) => r.id === "pitch-diameter")
	if (!diameter || !("value" in diameter)) throw Error("Missing driving dimension")
	diameter.value = 230
	const after = extrudeSolidFeature(doc, extrusion)
	const maxRadius = (loops: { x: number; y: number }[][]) => Math.max(...loops.flat().map((p) => Math.hypot(p.x, p.y)))
	expect(maxRadius(before.profileLoops)).toBeCloseTo(113, 4)
	expect(maxRadius(after.profileLoops)).toBeCloseTo(123, 4)
})
it("reports remaining freedom and distinguishes redundant dimensions from conflicts", () => {
	const entities: SketchEntity[] = [{ id: "c", type: "circle", center: { x: 10, y: 20 }, radius: 5, segments: 32 }]
	const constraints: SketchRelation[] = [{ id: "d", type: "diameter", entityId: "c", value: 20 }]
	const free = solveSketch(entities, constraints)
	expect(free.status).toBe("underconstrained")
	expect(free.degreesOfFreedom).toBe(2)
	expect(free.entities[0]).toMatchObject({ center: { x: 10, y: 20 } })
	const redundant = solveSketch(entities, [...constraints, { ...requireValue(constraints[0]), id: "d2" }])
	expect(redundant.status).toBe("underconstrained")
	expect(redundant.redundantEquations).toBe(1)
	const conflicting = solveSketch(entities, [...constraints, { id: "r", type: "radius", entityId: "c", value: 20 }])
	expect(conflicting.status).toBe("conflicting")
	expect(conflicting.conflicts).toContain("r")
	expect(entities[0]).toMatchObject({ radius: 5 })
})
it("solves a joined horizontal and vertical pair with lengths and fixed origin", () => {
	const entities: SketchEntity[] = [
		{ id: "a", type: "line", p0: { x: 1, y: 1 }, p1: { x: 9, y: 2 } },
		{ id: "b", type: "line", p0: { x: 10, y: 0 }, p1: { x: 11, y: 7 } }
	]
	const result = solveSketch(entities, [
		{ id: "origin", type: "fixed", anchor: p0("a"), position: { x: 0, y: 0 } },
		{ id: "h", type: "horizontal", entityId: "a" },
		{ id: "v", type: "vertical", entityId: "b" },
		{ id: "join", type: "coincident", a: p1("a"), b: p0("b") },
		{ id: "len-a", type: "length", entityId: "a", value: 20 },
		{ id: "len-b", type: "length", entityId: "b", value: 10 }
	])
	expect(result.status).toBe("fully-constrained")
	const b = result.entities[1]
	if (b?.type !== "line") throw Error("Missing line")
	expect(b.p0.x).toBeCloseTo(20, 5)
	expect(b.p0.y).toBeCloseTo(0, 5)
	expect(b.p1.y).toBeCloseTo(10, 5)
})
it("supports angle, equal length, signed distances and line-circle tangency", () => {
	const entities: SketchEntity[] = [
		{ id: "a", type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 0 } },
		{ id: "b", type: "line", p0: { x: 0, y: 0 }, p1: { x: 8, y: 12 } },
		{ id: "c", type: "circle", center: { x: 10, y: 4 }, radius: 3, segments: 32 }
	]
	const result = solveSketch(entities, [
		{ id: "base0", type: "fixed", anchor: p0("a"), position: { x: 0, y: 0 } },
		{ id: "base1", type: "fixed", anchor: p1("a"), position: { x: 20, y: 0 } },
		{ id: "b0", type: "coincident", a: p0("a"), b: p0("b") },
		{ id: "angle", type: "angle", a: "a", b: "b", value: 60 },
		{ id: "equal", type: "equal", a: "a", b: "b" },
		{ id: "radius", type: "radius", entityId: "c", value: 5 },
		{ id: "x", type: "distance", a: p0("a"), b: center("c"), axis: "x", value: -10 },
		{ id: "touch", type: "tangent", a: "a", b: "c" }
	])
	expect(result.status).toBe("fully-constrained")
	const c = result.entities[2]
	if (c?.type !== "circle") throw Error("Missing circle")
	expect(c.center.x).toBeCloseTo(-10, 4)
	expect(c.center.y).toBeCloseTo(5, 4)
})
it("retains construction geometry, relation labels and dimensions through project and graph round trips", () => {
	const doc: PartDocument = { features: [materializeSketch(ring())] }
	const runtime = createPartRuntimeState(doc)
	doc.cad = serializePCadState(runtime.cad)
	doc.tree = runtime.tree
	const file = createProjectFile({ items: [{ id: "part", type: "part", name: "Ring", data: doc }], selectedPath: null })
	const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
	const item = restored.items[0]
	if (!item || !("type" in item) || item.type !== "part") throw Error("Missing part")
	const data = requireValue((item as import("../src/contract").ProjectPartDocument).data)
	const rebuilt = createPartRuntimeState(data)
	const features = materializePartFeatures(rebuilt.cad, rebuilt.tree)
	const sketch = features[0]
	if (sketch?.type !== "sketch") throw Error("Missing sketch")
	expect(sketch.entities.find((e) => e.id === "pitch")?.construction).toBe(true)
	expect(sketch.relations).toEqual(ring().relations)
	expect(sketch.loops).toHaveLength(2)
})
it("rejects malformed relations and missing references instead of discarding constraints", () => {
	expect(() => normalizeSketchRelations([{ id: "bad", type: "diameter", entityId: "c", value: -1 }])).toThrow()
	expect(() => normalizeSketchRelations([{ id: "bad", type: "unknown" }])).toThrow()
	expect(() => solveSketch([], [{ id: "missing", type: "radius", entityId: "c", value: 3 }])).toThrow("Missing sketch entity")
	const sketch = ring()
	sketch.relations?.push({ id: "conflict", type: "diameter", entityId: "pitch", value: 100 })
	expect(() => materializeSketch(sketch)).toThrow("Conflicting sketch constraints")
})

it("escapes stationary initial directions for parallel and perpendicular lines", () => {
	for (const type of ["parallel", "perpendicular"] as const) {
		const entities = [
			{ id: "a", type: "line" as const, p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } },
			{ id: "b", type: "line" as const, p0: { x: 20, y: 0 }, p1: type === "parallel" ? { x: 20, y: 10 } : { x: 30, y: 0 } }
		]
		const result = solveSketch(entities, [{ id: "direction", type, a: "a", b: "b" }])
		expect(result.status).not.toBe("conflicting")
		expect(result.degreesOfFreedom).toBe(7)
	}
})
it("reports incompatible fixed directions even after stationary-point recovery", () => {
	for (const type of ["parallel", "perpendicular", "collinear"] as const) {
		const entities = [
			{ id: "a", type: "line" as const, p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } },
			{ id: "b", type: "line" as const, p0: { x: 20, y: 0 }, p1: type === "perpendicular" ? { x: 30, y: 0 } : { x: 20, y: 10 } }
		]
		const before = structuredClone(entities)
		const fixed = entities.flatMap((entity) =>
			(["p0", "p1"] as const).map((point) => ({ id: `${entity.id}-${point}`, type: "fixed" as const, anchor: { entityId: entity.id, point }, position: { ...entity[point] } }))
		)
		const result = solveSketch(entities, [...fixed, { id: "direction", type, a: "a", b: "b" }])
		expect(result.status).toBe("conflicting")
		expect(result.conflicts.length).toBeGreaterThan(0)
		expect(entities).toEqual(before)
	}
})
