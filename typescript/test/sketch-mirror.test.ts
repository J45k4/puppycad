import { expect, it } from "bun:test"
import { mirrorSketchEntity } from "../src/sketch-mirror"
import { arcPoint } from "../src/sketch-curves"
import { solveSketch, normalizeSketchRelations, remapSketchRelations, type SketchRelation } from "../src/sketch-solver"
import { materializeSketch } from "../src/cad/sketch"
import { editSketchCurve } from "../src/sketch-edit"
import type { Line, SketchEntity, Sketch } from "../src/schema"
const axis: Line = { id: "axis", type: "line", p0: { x: 0, y: -20 }, p1: { x: 0, y: 20 }, construction: true }
it("reflects native arc endpoints and reverses its sweep", () => {
	const arc: SketchEntity = { id: "arc", type: "arc", center: { x: 20, y: 3 }, radius: 5, startAngle: 0.3, sweep: 2, segments: 64 }
	const result = mirrorSketchEntity(arc, axis, "copy")
	if (result.type !== "arc") throw Error("Lost native arc")
	expect(result.sweep).toBe(-2)
	for (const t of [0, 0.25, 0.5, 1]) {
		const original = arcPoint(arc, t)
		const reflected = arcPoint(result, t)
		expect(reflected.x).toBeCloseTo(-original.x, 8)
		expect(reflected.y).toBeCloseTo(original.y, 8)
	}
})
it("reflects rotated rectangles and slot centers without flattening them", () => {
	const diagonal: Line = { ...axis, p0: { x: 0, y: 0 }, p1: { x: 10, y: 10 } }
	const box = mirrorSketchEntity({ id: "box", type: "cornerRectangle", p0: { x: 4, y: 2 }, p1: { x: 10, y: 6 } }, diagonal, "b")
	expect(box).toMatchObject({ type: "rectangle", center: { x: 4, y: 7 }, width: 6, height: 4, rotation: 90 })
	const slot = mirrorSketchEntity({ id: "slot", type: "capsule", from: { x: 2, y: 4 }, to: { x: 8, y: 10 }, width: 3, arcSegments: 32, construction: true }, diagonal, "s")
	expect(slot).toMatchObject({ type: "capsule", from: { x: 4, y: 2 }, to: { x: 10, y: 8 }, width: 3, construction: true })
	expect(() => mirrorSketchEntity(axis, { ...axis, p1: axis.p0 }, "bad")).toThrow("nonzero")
})
it("drives mirrored circles from changed source dimensions and persists the complete link", async () => {
	const source: SketchEntity = { id: "c", type: "circle", center: { x: 20, y: 0 }, radius: 3, segments: 64 }
	const target = mirrorSketchEntity(source, axis, "copy")
	const relation: SketchRelation = { id: "mirror", type: "mirror", a: "c", b: "copy", symmetryLine: "axis" }
	const relations: SketchRelation[] = [
		relation,
		{ id: "start", type: "fixed", anchor: { entityId: "axis", point: "p0" }, position: axis.p0 },
		{ id: "end", type: "fixed", anchor: { entityId: "axis", point: "p1" }, position: axis.p1 },
		{ id: "center", type: "fixed", anchor: { entityId: "c", point: "center" }, position: source.center },
		{ id: "radius", type: "radius", entityId: "c", value: 6 }
	]
	const solved = solveSketch([source, target, axis], relations)
	expect(solved.status).toBe("fully-constrained")
	expect(structuredClone(solved.entities[1])).toMatchObject({ center: { x: expect.closeTo(-20, 5), y: expect.closeTo(0, 5) }, radius: expect.closeTo(6, 5) })
	const normalized = normalizeSketchRelations(JSON.parse(JSON.stringify([relation])))
	expect(
		remapSketchRelations(
			normalized,
			new Map([
				["c", "c2"],
				["copy", "copy2"],
				["axis", "axis2"]
			])
		)
	).toEqual([{ ...relation, a: "c2", b: "copy2", symmetryLine: "axis2" }])
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: solved.entities,
		relations,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	expect(materializeSketch(sketch).profiles).toHaveLength(2)
	expect(editSketchCurve(sketch, "axis", { x: 0, y: 0 }, "Split").removedRelations).toContain("mirror")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing sketch")
	expect(result.relations).toContainEqual(relation)
	expect(result.profiles).toHaveLength(2)
})
it("accepts equivalent wrapped rectangle rotations in mirror constraints", () => {
	const source: SketchEntity = { id: "r", type: "rectangle", center: { x: 20, y: 0 }, width: 4, height: 8, rotation: 23 }
	const target = mirrorSketchEntity(source, axis, "copy")
	if (target.type !== "rectangle") throw Error("Missing rectangle")
	target.rotation += 360
	expect(solveSketch([source, target, axis], [{ id: "mirror", type: "mirror", a: "r", b: "copy", symmetryLine: "axis" }]).status).not.toBe("conflicting")
})
it("keeps an arc-and-chord profile closed after reflection", () => {
	const arc: SketchEntity = { id: "arc", type: "arc", center: { x: 20, y: 0 }, radius: 5, startAngle: -Math.PI / 2, sweep: Math.PI, segments: 96 }
	const chord: Line = { id: "chord", type: "line", p0: arcPoint(arc, 0), p1: arcPoint(arc, 1) }
	const originals = [arc, chord]
	const copies = originals.map((e) => mirrorSketchEntity(e, axis, `${e.id}-copy`))
	const relations: SketchRelation[] = originals.map((e) => ({ id: `mirror-${e.id}`, type: "mirror", a: e.id, b: `${e.id}-copy`, symmetryLine: axis.id }))
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [...originals, ...copies, axis],
		relations,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	expect(materializeSketch(sketch).profiles).toHaveLength(2)
})
it("accepts equivalent full-turn rotations for mirrored ellipses and polygons", () => {
	for (const type of ["ellipse", "polygon"] as const) {
		const source: SketchEntity =
			type === "ellipse"
				? { id: "source", type, center: { x: 2, y: 3 }, width: 12, height: 6, rotation: 20, segments: 32 }
				: { id: "source", type, center: { x: 2, y: 3 }, radius: 5, rotation: 20, sides: 5 }
		const target = mirrorSketchEntity(source, axis, "target")
		if (target.type !== "ellipse" && target.type !== "polygon") throw Error("Missing rotated shape")
		target.rotation += 360
		const relations: SketchRelation[] = [
			{ id: "mirror", type: "mirror", a: source.id, b: target.id, symmetryLine: axis.id },
			{ id: "start", type: "fixed", anchor: { entityId: axis.id, point: "p0" }, position: axis.p0 },
			{ id: "end", type: "fixed", anchor: { entityId: axis.id, point: "p1" }, position: axis.p1 }
		]
		for (const entity of [source, target]) {
			relations.push(
				{ id: `${entity.id}-center`, type: "fixed", anchor: { entityId: entity.id, point: "center" }, position: entity.center },
				{ id: `${entity.id}-rotation`, type: "rotation", entityId: entity.id, value: entity.rotation }
			)
			if (entity.type === "ellipse")
				relations.push(
					{ id: `${entity.id}-width`, type: "width", entityId: entity.id, value: entity.width },
					{ id: `${entity.id}-height`, type: "height", entityId: entity.id, value: entity.height }
				)
			else relations.push({ id: `${entity.id}-radius`, type: "radius", entityId: entity.id, value: entity.radius })
		}
		const result = solveSketch([source, target, axis], relations)
		expect(result.status).toBe("fully-constrained")
		expect(result.conflicts).toEqual([])
	}
})
