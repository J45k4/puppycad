import { expect, it } from "bun:test"
import { centerRectangle, threePointRectangle, threePointCenterRectangle } from "../src/sketch-rectangle"
import { solveSketch, normalizeSketchRelations } from "../src/sketch-solver"
import { primitivePoints } from "../src/sketch-primitives"
it("creates centered rectangles in either corner direction and rejects flat dimensions", () => {
	expect(centerRectangle("r", { x: 2, y: 3 }, { x: -4, y: 8 })).toMatchObject({ center: { x: 2, y: 3 }, width: 12, height: 10, rotation: 0 })
	expect(() => centerRectangle("r", { x: 2, y: 3 }, { x: 2, y: 8 })).toThrow("nonzero")
})
it("creates rotated rectangles on either side of an edge", () => {
	const above = threePointRectangle("r", { x: 0, y: 0 }, { x: 3, y: 4 }, { x: -4, y: 3 })
	expect(above.width).toBeCloseTo(5, 8)
	expect(above.height).toBeCloseTo(5, 8)
	expect(above.rotation).toBeCloseTo((Math.atan2(4, 3) * 180) / Math.PI, 8)
	expect(above.center).toEqual({ x: -0.5, y: 3.5 })
	const below = threePointRectangle("r", { x: 0, y: 0 }, { x: 3, y: 4 }, { x: 4, y: -3 })
	expect(below.center).toEqual({ x: 3.5, y: 0.5 })
	expect(primitivePoints(above)).toHaveLength(4)
	expect(() => threePointRectangle("r", { x: 0, y: 0 }, { x: 3, y: 4 }, { x: 6, y: 8 })).toThrow("height")
})
it("drives rotation together with rectangle size and fixed center", () => {
	const rectangle = centerRectangle("r", { x: 0, y: 0 }, { x: 5, y: 3 })
	const relations =
		normalizeSketchRelations([
			{ id: "center", type: "fixed", anchor: { entityId: "r", point: "center" }, position: { x: 0, y: 0 } },
			{ id: "w", type: "width", entityId: "r", value: 20 },
			{ id: "h", type: "height", entityId: "r", value: 10 },
			{ id: "angle", type: "rotation", entityId: "r", value: 45 }
		]) ?? []
	const solved = solveSketch([rectangle], relations)
	expect(solved.status).toBe("fully-constrained")
	expect(solved.entities[0]).toMatchObject({ width: expect.closeTo(20, 5), height: expect.closeTo(10, 5), rotation: expect.closeTo(45, 5) })
	expect(solveSketch([{ ...rectangle, rotation: 405 }], relations).status).not.toBe("conflicting")
})

it("can drive an exact half-turn without a wrapped-angle discontinuity", () => {
	const rectangle = centerRectangle("r", { x: 0, y: 0 }, { x: 5, y: 3 })
	const result = solveSketch([rectangle], [{ id: "rotation", type: "rotation", entityId: "r", value: 180 }])
	expect(result.status).not.toBe("conflicting")
	expect(result.entities[0]).toMatchObject({ rotation: expect.closeTo(180, 5) })
})
it("computes all four anchors in native outline order and solves a fixed rotated corner", async () => {
	const { entityAnchorPoint, entityAnchorNames } = await import("../src/sketch-curves")
	const rectangle = { ...centerRectangle("r", { x: 3, y: 4 }, { x: 8, y: 7 }), rotation: 37 }
	const names = entityAnchorNames(rectangle).filter((p) => p !== "center")
	const outline = primitivePoints(rectangle)
	expect(names).toEqual(["p0", "p1", "p2", "p3"])
	names.forEach((name, i) => {
		const p = entityAnchorPoint(rectangle, name)
		expect(p.x).toBeCloseTo(outline[i]?.x ?? Number.NaN, 8)
		expect(p.y).toBeCloseTo(outline[i]?.y ?? Number.NaN, 8)
	})
	const relations =
		normalizeSketchRelations([
			{ id: "corner", type: "fixed", anchor: { entityId: "r", point: "p2" }, position: { x: 0, y: 0 } },
			{ id: "width", type: "width", entityId: "r", value: 20 },
			{ id: "height", type: "height", entityId: "r", value: 10 },
			{ id: "rotation", type: "rotation", entityId: "r", value: 90 }
		]) ?? []
	const solved = solveSketch([rectangle], relations)
	expect(solved.status).toBe("fully-constrained")
	const result = solved.entities[0]
	if (result?.type !== "rectangle") throw Error("Missing rectangle")
	expect(entityAnchorPoint(result, "p2").x).toBeCloseTo(0, 5)
	expect(entityAnchorPoint(result, "p2").y).toBeCloseTo(0, 5)
	expect(result.center.x).toBeCloseTo(-5, 5)
	expect(result.center.y).toBeCloseTo(10, 5)
})
it("keeps a line attached to a resized rectangle corner through PCad persistence", async () => {
	const { entityAnchorPoint } = await import("../src/sketch-curves")
	const rectangle = centerRectangle("r", { x: 0, y: 0 }, { x: 5, y: 3 })
	const line: import("../src/schema").Line = { id: "line", type: "line", p0: entityAnchorPoint(rectangle, "p3"), p1: { x: 30, y: 0 } }
	const relations =
		normalizeSketchRelations([
			{ id: "join", type: "coincident", a: { entityId: "line", point: "p0" }, b: { entityId: "r", point: "p3" } },
			{ id: "width", type: "width", entityId: "r", value: 20 }
		]) ?? []
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [rectangle, line],
		relations,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing sketch")
	const box = result.entities.find((e) => e.id === "r")
	const edge = result.entities.find((e) => e.id === "line")
	if (box?.type !== "rectangle" || edge?.type !== "line") throw Error("Missing linked geometry")
	const corner = entityAnchorPoint(box, "p3")
	expect(edge.p0.x).toBeCloseTo(corner.x, 5)
	expect(edge.p0.y).toBeCloseTo(corner.y, 5)
	expect(box.width).toBeCloseTo(20, 5)
})

it("constructs centered rotated rectangles in either axis and side direction", () => {
	const center = { x: 2, y: 3 }
	for (const direction of [-1, 1])
		for (const side of [-1, 1]) {
			const rectangle = threePointCenterRectangle("r", center, { x: 2 + 3 * direction, y: 3 + 4 * direction }, { x: 2 - 8 * side, y: 3 + 6 * side })
			expect(rectangle.center).toEqual(center)
			expect(rectangle.width).toBeCloseTo(10, 8)
			expect(rectangle.height).toBeCloseTo(20, 8)
			const points = primitivePoints(rectangle)
			const average = points.reduce((sum, p) => ({ x: sum.x + p.x / 4, y: sum.y + p.y / 4 }), { x: 0, y: 0 })
			expect(average.x).toBeCloseTo(center.x, 8)
			expect(average.y).toBeCloseTo(center.y, 8)
			rectangle.center.x = 99
			expect(center).toEqual({ x: 2, y: 3 })
		}
	expect(() => threePointCenterRectangle("r", center, center, { x: 5, y: 8 })).toThrow("midpoint")
	expect(() => threePointCenterRectangle("r", center, { x: 5, y: 7 }, { x: 8, y: 11 })).toThrow("height")
	expect(() => threePointCenterRectangle("r", center, { x: Number.POSITIVE_INFINITY, y: 7 }, { x: 8, y: 11 })).toThrow("midpoint")
	expect(() => threePointCenterRectangle("r", center, { x: 5, y: 7 }, { x: Number.NaN, y: 11 })).toThrow("height")
})
it("keeps centered rotated rectangles editable after project reload", async () => {
	const { requireValue } = await import("../src/required")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const { createProjectFile, serializeProjectFile, normalizeProjectFile } = await import("../src/project-file")
	const rectangle = threePointCenterRectangle("r", { x: 2, y: 3 }, { x: 5, y: 7 }, { x: -6, y: 9 })
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [rectangle],
		relations: [{ id: "center", type: "fixed", anchor: { entityId: "r", point: "center" }, position: { x: 2, y: 3 } }],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const runtime = createPartRuntimeState({ features: [sketch] })
	const file = createProjectFile({
		items: [{ id: "p", type: "part", name: "Centered rectangle", data: { features: [sketch], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
		selectedPath: null
	})
	const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
	const part = restored.items[0] as import("../src/contract").ProjectPartDocument
	const state = createPartRuntimeState(requireValue(part.data))
	const saved = materializePartFeatures(state.cad, state.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.entities[0]).toEqual(rectangle)
	expect(saved.profiles).toHaveLength(1)
	const changed = solveSketch(saved.entities, [
		...(saved.relations ?? []),
		{ id: "width", type: "width", entityId: "r", value: 40 },
		{ id: "height", type: "height", entityId: "r", value: 12 },
		{ id: "rotation", type: "rotation", entityId: "r", value: 75 }
	])
	expect(changed.status).toBe("fully-constrained")
	expect(changed.entities[0]).toMatchObject({
		center: { x: expect.closeTo(2, 5), y: expect.closeTo(3, 5) },
		width: expect.closeTo(40, 5),
		height: expect.closeTo(12, 5),
		rotation: expect.closeTo(75, 5)
	})
})
