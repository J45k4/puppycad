import { expect, it } from "bun:test"
import { arcPoint, arcPoints, threePointArc, normalizeArc } from "../src/sketch-curves"
import { solveSketch } from "../src/sketch-solver"
import { materializeSketch } from "../src/cad/sketch"
import { extrudeSolidFeature } from "../src/cad/extrude"
import type { Sketch, SolidExtrude, PartDocument } from "../src/schema"
import { createPartRuntimeState, materializePartFeatures, serializePCadState } from "../src/pcad/part-state"
import { createProjectFile, normalizeProjectFile, serializeProjectFile } from "../src/project-file"
import { requireValue } from "../src/required"
import { v2 } from "../src/sdk"
function semicircle(): Sketch {
	return {
		id: "profile",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [threePointArc("arc", v2(-10, 0), v2(10, 0), v2(0, 10)), { id: "base", type: "line", p0: v2(-10, 0), p1: v2(10, 0) }],
		dimensions: [],
		relations: [],
		vertices: [],
		loops: [],
		profiles: []
	}
}
it("constructs minor, major, clockwise and counterclockwise arcs through all three points", () => {
	for (const [start, end, through] of [
		[v2(10, 0), v2(0, 10), v2(Math.sqrt(50), Math.sqrt(50))],
		[v2(10, 0), v2(0, 10), v2(-10, 0)],
		[v2(0, 10), v2(10, 0), v2(-10, 0)]
	]) {
		const arc = threePointArc("a", requireValue(start), requireValue(end), requireValue(through))
		expect(arc.radius).toBeCloseTo(10, 7)
		expect(arcPoint(arc, 0).x).toBeCloseTo(requireValue(start).x, 7)
		expect(arcPoint(arc, 1).y).toBeCloseTo(requireValue(end).y, 7)
		expect(arcPoints(arc).length).toBeGreaterThan(2)
	}
	const major = threePointArc("a", v2(10, 0), v2(0, 10), v2(-10, 0))
	expect(major.sweep).toBeCloseTo(-Math.PI * 1.5, 7)
	expect(() => threePointArc("a", v2(0, 0), v2(1, 1), v2(2, 2))).toThrow("collinear")
	expect(normalizeArc({ ...major, sweep: Math.PI * 2 }, "a")).toBeUndefined()
})
it("materializes arc and line into a semicircle without closing an open arc on its chord", () => {
	const sketch = semicircle()
	const open = materializeSketch({ ...sketch, entities: [requireValue(sketch.entities[0])] })
	expect(open.profiles).toHaveLength(0)
	const closed = materializeSketch(sketch)
	expect(closed.profiles).toHaveLength(1)
	const extrude: SolidExtrude = { id: "pad", type: "extrude", depth: 5, target: { type: "profileRef", sketchId: sketch.id, profileId: requireValue(closed.profiles[0]).id } }
	const solid = extrudeSolidFeature({ features: [closed, extrude] }, extrude)
	const points = requireValue(solid.profileLoops[0])
	const area =
		Math.abs(
			points.reduce((sum, p, i) => {
				const next = requireValue(points[(i + 1) % points.length])
				return sum + p.x * next.y - p.y * next.x
			}, 0)
		) / 2
	expect(area).toBeCloseTo(Math.PI * 50, 0)
	expect(points.every((p) => p.y >= -1e-6)).toBe(true)
	expect(materializeSketch({ ...sketch, entities: [{ ...requireValue(sketch.entities[0]), construction: true }, requireValue(sketch.entities[1])] }).profiles).toHaveLength(0)
})
it("solves arc endpoints and driving radius together with a connected chord", () => {
	const sketch = semicircle()
	const relations: import("../src/sketch-solver").SketchRelation[] = [
		{ id: "center", type: "fixed", anchor: { entityId: "arc", point: "center" }, position: v2(0, 0) },
		{ id: "radius", type: "radius", entityId: "arc", value: 20 },
		{ id: "sweep", type: "arcSweep", entityId: "arc", value: -180 },
		{ id: "start", type: "coincident", a: { entityId: "arc", point: "p0" }, b: { entityId: "base", point: "p0" } },
		{ id: "end", type: "coincident", a: { entityId: "arc", point: "p1" }, b: { entityId: "base", point: "p1" } },
		{ id: "horizontal", type: "horizontal", entityId: "base" }
	]
	const result = solveSketch(sketch.entities, relations)
	expect(result.status).toBe("fully-constrained")
	expect(result.entities[0]).toMatchObject({ type: "arc", radius: expect.closeTo(20, 5) })
	const line = requireValue(result.entities[1])
	if (line.type !== "line") throw Error("Missing line")
	expect(Math.abs(line.p1.x - line.p0.x)).toBeCloseTo(40, 5)
	expect(result.conflicts).toEqual([])
})
it("round-trips native arcs and remapped relation IDs through project and PCad persistence", () => {
	const sketch = semicircle()
	sketch.entities[0] = { ...requireValue(sketch.entities[0]), id: "profile" }
	sketch.relations = [{ id: "r", type: "radius", entityId: "profile", value: 10 }]
	const doc: PartDocument = { features: [materializeSketch(sketch)], solidSteps: [] }
	const runtime = createPartRuntimeState(doc)
	doc.cad = serializePCadState(runtime.cad)
	doc.tree = runtime.tree
	expect(doc.cad.nodes.some((n) => n.type === "sketchArc")).toBe(true)
	const file = createProjectFile({ items: [{ id: "p", name: "Arcs", type: "part", data: doc }], selectedPath: null })
	const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
	const item = restored.items[0] as import("../src/contract").ProjectPartDocument
	const rebuilt = createPartRuntimeState(requireValue(item.data))
	const result = materializePartFeatures(rebuilt.cad, rebuilt.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing sketch")
	expect(result.entities[0]).toMatchObject({ type: "arc", radius: 10 })
	expect(result.relations?.[0]).toMatchObject({ entityId: result.entities[0]?.id })
	expect(result.profiles).toHaveLength(1)
})
it("creates center-point arcs with clockwise and counterclockwise major sweeps", async () => {
	const { centerPointArc } = await import("../src/sketch-curves")
	const center = { x: 2, y: 3 }
	const ccw = centerPointArc("a", center, { x: 12, y: 3 }, { x: 2, y: 23 })
	const clockwise = centerPointArc("b", center, { x: 12, y: 3 }, { x: 2, y: 23 }, true)
	expect(ccw.radius).toBeCloseTo(10, 8)
	expect(ccw.sweep).toBeCloseTo(Math.PI / 2, 8)
	expect(clockwise.sweep).toBeCloseTo((-3 * Math.PI) / 2, 8)
	expect(arcPoint(ccw, 1).y).toBeCloseTo(13, 8)
	expect(arcPoint(clockwise, 1).y).toBeCloseTo(13, 8)
	expect(() => centerPointArc("bad", center, center, { x: 4, y: 5 })).toThrow("center")
	expect(() => centerPointArc("bad", center, { x: 12, y: 3 }, { x: 22, y: 3 }, true)).toThrow("Circle")
	expect(center).toEqual({ x: 2, y: 3 })
})
