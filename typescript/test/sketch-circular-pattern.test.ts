import { expect, it } from "bun:test"
import { circularPatternDragAngle, circularPatternCenter, setCircularPatternCenter, createCircularSketchPattern, resizeCircularSketchPattern, rotateSketchEntity } from "../src/sketch-circular-pattern"
import { solveSketch, normalizeSketchRelations, remapSketchRelations } from "../src/sketch-solver"
import { materializeSketch } from "../src/cad/sketch"
import { editSketchCurve } from "../src/sketch-edit"
import type { Sketch, SketchEntity } from "../src/schema"
const sketch = (): Sketch => ({
	id: "s",
	type: "sketch",
	dirty: false,
	target: { type: "plane", plane: "XY" },
	entities: [{ id: "c", type: "circle", center: { x: 20, y: 0 }, radius: 2, segments: 64 }],
	relations: [
		{ id: "center", type: "fixed", anchor: { entityId: "c", point: "center" }, position: { x: 20, y: 0 } },
		{ id: "radius", type: "radius", entityId: "c", value: 4 }
	],
	dimensions: [],
	vertices: [],
	loops: [],
	profiles: []
})
it("links source dimensions around a closed circle without duplicating the source", () => {
	const created = createCircularSketchPattern(sketch(), ["c"], 4, { x: 0, y: 0 })
	const solved = solveSketch(created.sketch.entities, created.sketch.relations ?? [])
	expect(solved.status).toBe("fully-constrained")
	const positions = [
		[20, 0],
		[0, 20],
		[-20, 0],
		[0, -20]
	]
	solved.entities.forEach((e, i) => {
		if (e.type !== "circle") throw Error("Missing circle")
		expect(e.radius).toBeCloseTo(4, 5)
		expect(e.center.x).toBeCloseTo(positions[i]?.[0] ?? Number.NaN, 5)
		expect(e.center.y).toBeCloseTo(positions[i]?.[1] ?? Number.NaN, 5)
	})
	expect(materializeSketch(created.sketch).profiles).toHaveLength(4)
})
it("keeps open clockwise patterns open and preserves IDs when the count changes", () => {
	const created = createCircularSketchPattern(sketch(), ["c"], 3, { x: 10, y: 0 }, -180)
	const grown = resizeCircularSketchPattern(created.sketch, created.patternId, 5, { x: 10, y: 0 }, -180)
	expect(grown.sketch.entities.slice(0, 3).map((e) => e.id)).toEqual(created.sketch.entities.map((e) => e.id))
	const last = grown.sketch.entities[4]
	if (last?.type !== "circle") throw Error("Missing circle")
	expect(last.center.x).toBeCloseTo(0, 8)
	expect(last.center.y).toBeCloseTo(0, 8)
	const first = grown.sketch.entities[1]
	if (first?.type !== "circle") throw Error("Missing circle")
	expect(first.center.x).toBeCloseTo(10 + Math.sqrt(50), 8)
	expect(first.center.y).toBeCloseTo(-Math.sqrt(50), 8)
	grown.sketch.relations?.push({ id: "copy-radius", type: "radius", entityId: last.id, value: 4 })
	const shrunk = resizeCircularSketchPattern(grown.sketch, created.patternId, 2, { x: 10, y: 0 }, -180)
	expect(shrunk.removedRelations).toEqual(["copy-radius"])
	expect(shrunk.sketch.entities).toHaveLength(2)
	expect(created.sketch.entities).toHaveLength(3)
})
it("rotates every native geometry without losing curve shape or construction", () => {
	const entities: SketchEntity[] = [
		{ id: "line", type: "line", p0: { x: 10, y: 0 }, p1: { x: 12, y: 0 } },
		{ id: "arc", type: "arc", center: { x: 10, y: 0 }, radius: 2, startAngle: 0, sweep: -Math.PI, segments: 64 },
		{ id: "slot", type: "capsule", from: { x: 10, y: 0 }, to: { x: 12, y: 0 }, width: 2, arcSegments: 48 },
		{ id: "rectangle", type: "rectangle", center: { x: 10, y: 0 }, width: 2, height: 4, rotation: 30 },
		{ id: "corner", type: "cornerRectangle", p0: { x: 9, y: -2 }, p1: { x: 11, y: 2 }, construction: true }
	]
	const rotated = entities.map((e) => rotateSketchEntity(e, `copy-${e.id}`, { x: 0, y: 0 }, 90))
	expect(rotated[0]).toMatchObject({ p0: { x: expect.closeTo(0), y: 10 }, p1: { x: expect.closeTo(0), y: 12 } })
	expect(rotated[1]).toMatchObject({ startAngle: Math.PI / 2, sweep: -Math.PI, radius: 2 })
	expect(rotated[2]).toMatchObject({ from: { x: expect.closeTo(0), y: 10 }, to: { x: expect.closeTo(0), y: 12 }, width: 2 })
	expect(rotated[3]).toMatchObject({ rotation: 120, width: 2, height: 4 })
	expect(rotated[4]).toMatchObject({ type: "rectangle", rotation: 90, width: 2, height: 4, construction: true })
	const input = sketch()
	input.entities = entities
	input.relations = []
	const created = createCircularSketchPattern(
		input,
		entities.map((e) => e.id),
		4,
		{ x: 0, y: 0 }
	)
	expect(solveSketch(created.sketch.entities, created.sketch.relations ?? []).status).toBe("underconstrained")
	expect(entities[1]).toMatchObject({ startAngle: 0 })
})
it("persists circular profiles and remaps all references through PCad", async () => {
	const created = createCircularSketchPattern(sketch(), ["c"], 4, { x: 1, y: 2 }, 270)
	const normalized = normalizeSketchRelations(JSON.parse(JSON.stringify(created.sketch.relations)))
	const remapped = remapSketchRelations(
		normalized,
		new Map([
			["c", "source"],
			["circular-copy-1", "copy"]
		])
	)
	expect(remapped?.find((r) => r.type === "circularPattern")).toMatchObject({
		sources: ["source"],
		instances: [["copy"], ["circular-copy-2"], ["circular-copy-3"]],
		angle: 270,
		center: { x: 1, y: 2 }
	})
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [created.sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations?.find((r) => r.type === "circularPattern")).toMatchObject({ angle: 270, center: { x: 1, y: 2 } })
	expect(saved.profiles).toHaveLength(4)
	expect(solveSketch(saved.entities, saved.relations ?? []).status).toBe("fully-constrained")
	expect(editSketchCurve(created.sketch, "c", { x: 22, y: 0 }, "Split", { x: 18, y: 0 }).removedRelations).toContain(created.patternId)
})
it("rejects invalid circular pattern metadata and edits without changing the input", () => {
	const input = sketch()
	for (const angle of [0, Number.NaN, Number.POSITIVE_INFINITY, 361, -361]) expect(() => createCircularSketchPattern(input, ["c"], 4, { x: 0, y: 0 }, angle)).toThrow("angle")
	expect(() => createCircularSketchPattern(input, ["c"], 33, { x: 0, y: 0 })).toThrow("32")
	expect(() => createCircularSketchPattern(input, ["c"], 4, { x: Number.NaN, y: 0 })).toThrow("center")
	const created = createCircularSketchPattern(input, ["c"], 4, { x: 0, y: 0 })
	const relation = created.sketch.relations?.find((r) => r.type === "circularPattern")
	if (!relation) throw Error("Missing pattern")
	expect(() => normalizeSketchRelations([{ ...relation, angle: 0 }])).toThrow()
	expect(() => normalizeSketchRelations([{ ...relation, instances: [["c"]] }])).toThrow()
	expect(input.entities).toHaveLength(1)
})

it("drives a referenced pattern center from another entity's dimensions and detaches at its current position", async () => {
	const input = sketch()
	input.entities.push({ id: "axis", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 }, construction: true })
	input.relations?.push(
		{ id: "base", type: "fixed", anchor: { entityId: "axis", point: "p0" }, position: { x: 0, y: 0 } },
		{ id: "horizontal", type: "horizontal", entityId: "axis" },
		{ id: "axis-length", type: "length", entityId: "axis", value: 5 }
	)
	const created = createCircularSketchPattern(input, ["c"], 2, { x: 0, y: 0 })
	const linked = setCircularPatternCenter(created.sketch, created.patternId, { entityId: "axis", point: "p1" }).sketch
	const solved = solveSketch(linked.entities, linked.relations ?? [])
	expect(solved.status).toBe("fully-constrained")
	const copy = solved.entities.find((e) => e.id === "circular-copy-1")
	if (copy?.type !== "circle") throw Error("Missing copy")
	expect(copy.center.x).toBeCloseTo(-10, 5)
	linked.entities = solved.entities
	const detached = setCircularPatternCenter(linked, created.patternId, null).sketch
	const detachedPattern = detached.relations?.find((r) => r.type === "circularPattern")
	if (detachedPattern?.type !== "circularPattern") throw Error("Missing pattern")
	expect(detachedPattern.centerAnchor).toBeUndefined()
	expect(detachedPattern.center.x).toBeCloseTo(5, 5)
	const remapped = remapSketchRelations(linked.relations, new Map([["axis", "new-axis"]]))
	expect(remapped?.find((r) => r.type === "circularPattern")).toMatchObject({ centerAnchor: { entityId: "new-axis", point: "p1" } })
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [linked] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	const dimension = saved.relations?.find((r) => r.id === "axis-length")
	if (dimension?.type !== "length") throw Error("Missing dimension")
	dimension.value = 7
	const changed = solveSketch(saved.entities, saved.relations ?? [])
	expect(changed.status).toBe("fully-constrained")
	const changedCopy = changed.entities.find((e) => e.id === "circular-copy-1")
	if (changedCopy?.type !== "circle") throw Error("Missing copy")
	expect(changedCopy.center.x).toBeCloseTo(-6, 5)
})
it("remaps surviving center anchors when splitting the reference curve", () => {
	const input = sketch()
	input.entities.push({ id: "axis", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 }, construction: true })
	const created = createCircularSketchPattern(input, ["c"], 3, { x: 0, y: 0 })
	const linked = setCircularPatternCenter(created.sketch, created.patternId, { entityId: "axis", point: "p1" }).sketch
	const split = editSketchCurve(linked, "axis", { x: 5, y: 0 }, "Split")
	const pattern = split.sketch.relations?.find((r) => r.type === "circularPattern")
	if (pattern?.type !== "circularPattern") throw Error("Missing pattern")
	expect(pattern.centerAnchor).toEqual({ entityId: "axis-split-1", point: "p1" })
	expect(circularPatternCenter(pattern, split.sketch.entities)).toEqual({ x: 10, y: 0 })
	expect(split.removedRelations).not.toContain(created.patternId)
	expect(editSketchCurve(linked, "axis", { x: 5, y: 0 }, "Trim").removedRelations).toContain(created.patternId)
	expect(() => setCircularPatternCenter(created.sketch, created.patternId, { entityId: "circular-copy-1", point: "center" })).toThrow("own copy")
	expect(() => normalizeSketchRelations([{ ...pattern, centerAnchor: { entityId: "circular-copy-1", point: "center" } }])).toThrow()
})

it("tracks circular pattern angle drags across the atan2 seam and clamps full turns", () => {
	const radians = (value: number) => (value * Math.PI) / 180
	expect(circularPatternDragAngle(180, radians(179), radians(-179))).toBeCloseTo(182, 8)
	expect(circularPatternDragAngle(-180, radians(-179), radians(179))).toBeCloseTo(-182, 8)
	expect(circularPatternDragAngle(360, 0, radians(-90))).toBeCloseTo(270, 8)
	expect(circularPatternDragAngle(-360, 0, radians(90))).toBeCloseTo(-270, 8)
	expect(circularPatternDragAngle(350, 0, radians(90))).toBe(360)
})
