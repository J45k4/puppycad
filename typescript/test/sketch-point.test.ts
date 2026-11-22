import { expect, it } from "bun:test"
import { sketchPoint, normalizePrimitive } from "../src/sketch-primitives"
import { materializeSketch } from "../src/cad/sketch"
import { solveSketch } from "../src/sketch-solver"
import { createCircularSketchPattern, setCircularPatternCenter } from "../src/sketch-circular-pattern"
import { createLinearSketchPattern } from "../src/sketch-pattern"
import { mirrorSketchEntity } from "../src/sketch-mirror"
import { offsetSketchEntity } from "../src/sketch-offset"
import type { Sketch, SketchPoint } from "../src/schema"
const point: SketchPoint = { id: "p", ...sketchPoint({ x: 10, y: 0 }) }
const input = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [structuredClone(point)],
	relations: [],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("stores native points with two degrees of freedom and never produces a profile", () => {
	const solved = solveSketch([point], [])
	expect(solved.degreesOfFreedom).toBe(2)
	expect(materializeSketch(input()).profiles).toHaveLength(0)
	expect(normalizePrimitive(point, "copy")).toEqual({ id: "copy", type: "point", center: { x: 10, y: 0 } })
	expect(normalizePrimitive({ ...point, center: { x: Number.NaN, y: 0 } }, "p")).toBeUndefined()
	expect(() => sketchPoint({ x: Number.POSITIVE_INFINITY, y: 0 })).toThrow("finite")
})
it("solves native point distances and curve constraints", () => {
	const result = solveSketch(
		[point, { id: "q", type: "point", center: { x: 13, y: 4 } }],
		[
			{ id: "fixed", type: "fixed", anchor: { entityId: "p", point: "center" }, position: { x: 10, y: 0 } },
			{ id: "x", type: "distance", a: { entityId: "p", point: "center" }, b: { entityId: "q", point: "center" }, axis: "x", value: 6 },
			{ id: "y", type: "distance", a: { entityId: "p", point: "center" }, b: { entityId: "q", point: "center" }, axis: "y", value: 8 }
		]
	)
	expect(result.status).toBe("fully-constrained")
	expect(result.entities[1]).toMatchObject({ center: { x: expect.closeTo(16, 5), y: expect.closeTo(8, 5) } })
	const onCurve = solveSketch(
		[point, { id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 20, segments: 64 }],
		[
			{ id: "center", type: "fixed", anchor: { entityId: "c", point: "center" }, position: { x: 0, y: 0 } },
			{ id: "radius", type: "radius", entityId: "c", value: 20 },
			{ id: "on", type: "pointOnCurve", a: { entityId: "p", point: "center" }, b: "c" }
		]
	)
	expect(onCurve.degreesOfFreedom).toBe(1)
	expect(onCurve.status).toBe("underconstrained")
})
it("patterns and mirrors native points without manufacturing area geometry", () => {
	expect(createLinearSketchPattern(input(), ["p"], 3, { x: 10, y: 5 }).sketch.entities[2]).toMatchObject({ type: "point", center: { x: 30, y: 10 } })
	const circular = createCircularSketchPattern(input(), ["p"], 4, { x: 0, y: 0 })
	expect(structuredClone(circular.sketch.entities[1])).toMatchObject({ type: "point", center: { x: expect.closeTo(0, 5), y: 10 } })
	expect(materializeSketch(circular.sketch).profiles).toHaveLength(0)
	expect(mirrorSketchEntity(point, { id: "axis", type: "line", p0: { x: 0, y: -10 }, p1: { x: 0, y: 10 } }, "mirror")).toMatchObject({ type: "point", center: { x: -10, y: 0 } })
	expect(() => offsetSketchEntity(point, 5, "offset")).toThrow("no offset curve")
})
it("persists a native point used as a pattern center through PCad", async () => {
	const sketch = input()
	sketch.entities.push({ id: "c", type: "circle", center: { x: 30, y: 0 }, radius: 2, segments: 64 })
	const created = createCircularSketchPattern(sketch, ["c"], 4, { x: 0, y: 0 })
	const linked = setCircularPatternCenter(created.sketch, created.patternId, { entityId: "p", point: "center" }).sketch
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [linked] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.entities.find((e) => e.id === "p")).toEqual(point)
	expect(saved.relations?.find((r) => r.type === "circularPattern")).toMatchObject({ centerAnchor: { entityId: "p", point: "center" } })
	expect(saved.profiles).toHaveLength(4)
})
