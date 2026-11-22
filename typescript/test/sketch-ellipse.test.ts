import { expect, it } from "bun:test"
import { threePointEllipse, ellipsePoints } from "../src/sketch-ellipse"
import { entityAnchorPoint, entityAnchorNames } from "../src/sketch-curves"
import { solveSketch } from "../src/sketch-solver"
import { createCircularSketchPattern } from "../src/sketch-circular-pattern"
import { mirrorSketchEntity } from "../src/sketch-mirror"
import { offsetSketchEntity } from "../src/sketch-offset"
import { materializeSketch } from "../src/cad/sketch"
import type { Sketch } from "../src/schema"
const input = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [threePointEllipse("e", { x: 30, y: 0 }, { x: 40, y: 0 }, { x: 30, y: 5 })],
	relations: [],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("constructs rotated native ellipses from the center and perpendicular axes", () => {
	const ellipse = threePointEllipse("e", { x: 3, y: 4 }, { x: 3, y: 14 }, { x: -2, y: 9 })
	expect(ellipse).toMatchObject({ width: 20, height: 10, rotation: 90 })
	expect(entityAnchorNames(ellipse)).toEqual(["p0", "p1", "p2", "p3", "center"])
	expect(entityAnchorPoint(ellipse, "p0")).toMatchObject({ x: expect.closeTo(3, 6), y: 14 })
	expect(entityAnchorPoint(ellipse, "p1")).toMatchObject({ x: -2, y: expect.closeTo(4, 6) })
	const points = ellipsePoints(ellipse)
	const area =
		Math.abs(
			points.reduce((sum, p, i) => {
				const next = points[(i + 1) % points.length]
				return sum + p.x * (next?.y ?? 0) - p.y * (next?.x ?? 0)
			}, 0)
		) / 2
	expect(Math.abs(area / (Math.PI * 10 * 5) - 1)).toBeLessThan(0.001)
	expect(() => threePointEllipse("bad", { x: 0, y: 0 }, { x: 0, y: 0 }, { x: 2, y: 3 })).toThrow("first axis")
	expect(() => threePointEllipse("bad", { x: 0, y: 0 }, { x: 10, y: 0 }, { x: 5, y: 0 })).toThrow("positive")
})
it("drives ellipse axes and rotation while retaining an attached axis endpoint", () => {
	const sketch = input()
	sketch.relations = [
		{ id: "anchor", type: "fixed", anchor: { entityId: "e", point: "p0" }, position: { x: 40, y: 0 } },
		{ id: "width", type: "width", entityId: "e", value: 30 },
		{ id: "height", type: "height", entityId: "e", value: 12 },
		{ id: "rotation", type: "rotation", entityId: "e", value: 30 }
	]
	const result = solveSketch(sketch.entities, sketch.relations)
	expect(result.status).toBe("fully-constrained")
	const ellipse = result.entities[0]
	if (ellipse?.type !== "ellipse") throw Error("Missing ellipse")
	expect(ellipse.width).toBeCloseTo(30, 5)
	expect(ellipse.height).toBeCloseTo(12, 5)
	expect(ellipse.rotation).toBeCloseTo(30, 5)
	expect(entityAnchorPoint(ellipse, "p0").x).toBeCloseTo(40, 5)
	expect(entityAnchorPoint(ellipse, "p0").y).toBeCloseTo(0, 5)
	expect(materializeSketch({ ...sketch, entities: result.entities }).profiles).toHaveLength(1)
})
it("patterns and mirrors native ellipses and persists their dimensions through PCad", async () => {
	const sketch = input()
	sketch.relations = [{ id: "width", type: "width", entityId: "e", value: 24 }]
	const created = createCircularSketchPattern(sketch, ["e"], 4, { x: 0, y: 0 })
	const solved = solveSketch(created.sketch.entities, created.sketch.relations ?? [])
	expect(solved.entities[1]).toMatchObject({ type: "ellipse", width: expect.closeTo(24, 5), rotation: expect.closeTo(90, 5) })
	const source = sketch.entities[0]
	if (!source) throw Error("Missing ellipse")
	expect(mirrorSketchEntity(source, { id: "axis", type: "line", p0: { x: 0, y: 0 }, p1: { x: 0, y: 1 } }, "copy")).toMatchObject({
		type: "ellipse",
		center: { x: -30, y: 0 },
		width: 20,
		height: 10
	})
	expect(() => offsetSketchEntity(source, 5, "offset")).toThrow("general offset curve")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [created.sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.entities.every((e) => e.type === "ellipse")).toBe(true)
	expect(saved.profiles).toHaveLength(4)
	expect(saved.relations?.some((r) => r.type === "width")).toBe(true)
})
