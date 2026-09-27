import { expect, it } from "bun:test"
import { fixedEntityRelations } from "../src/sketch-fix"
import { solveSketch } from "../src/sketch-solver"
import type { SketchEntity } from "../src/schema"

it("fully constrains native curves including dimensions not fixed by their anchors", () => {
	const entities: SketchEntity[] = [
		{ id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 10 } },
		{ id: "circle", type: "circle", center: { x: 4, y: 5 }, radius: 10, segments: 64 },
		{ id: "ellipse", type: "ellipse", center: { x: 3, y: 7 }, width: 20, height: 10, rotation: 25, segments: 64 },
		{ id: "arc", type: "arc", center: { x: 2, y: 3 }, radius: 10, startAngle: 0.2, sweep: 1.5, segments: 64 },
		{ id: "elliptic", type: "ellipticArc", center: { x: 2, y: 3 }, width: 20, height: 10, rotation: 35, startAngle: 0.2, sweep: -1.5, segments: 64 },
		{ id: "slot", type: "capsule", from: { x: 0, y: 0 }, to: { x: 20, y: 10 }, width: 5, arcSegments: 16 },
		{ id: "rectangle", type: "rectangle", center: { x: 4, y: 5 }, width: 20, height: 10, rotation: 35 },
		{ id: "polygon", type: "polygon", center: { x: 2, y: 3 }, radius: 10, rotation: 15, sides: 5 },
		{
			id: "spline",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 3, y: 4 },
				{ x: 6, y: -4 },
				{ x: 10, y: 0 }
			]
		}
	]
	for (const entity of entities) {
		const before = structuredClone(entity)
		const relations = fixedEntityRelations(entity, "fix")
		const result = solveSketch([entity], relations)
		expect(result.status).toBe("fully-constrained")
		expect(result.degreesOfFreedom).toBe(0)
		expect(result.entities[0]).toEqual(before)
		expect(entity).toEqual(before)
		expect(new Set(relations.map((r) => r.id)).size).toBe(relations.length)
	}
})

it("validates and remaps fixation ownership with copied geometry", async () => {
	const { normalizeSketchRelations, remapSketchRelations } = await import("../src/sketch-solver")
	const relations = normalizeSketchRelations([{ id: "fix", type: "radius", entityId: "a", value: 10, fixation: "a" }])
	expect(remapSketchRelations(relations, new Map([["a", "copy"]]))?.[0]).toMatchObject({ entityId: "copy", fixation: "copy" })
	expect(() => normalizeSketchRelations([{ id: "fix", type: "radius", entityId: "a", value: 10, fixation: "other" }])).toThrow("Invalid sketch relation")
})

it("moves fixation ownership with a surviving endpoint when a fixed line is split", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { normalizeSketchRelations } = await import("../src/sketch-solver")
	const entity: SketchEntity = { id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [entity],
		relations: fixedEntityRelations(entity, "fix").map((r) => ({ ...r, fixation: entity.id })),
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const result = editSketchCurve(sketch, entity.id, { x: 5, y: 0 }, "Split").sketch
	const fixed = result.relations?.filter((r) => r.type === "fixed") ?? []
	expect(fixed).toHaveLength(2)
	for (const relation of fixed) {
		if (relation.type !== "fixed") throw Error("Missing fixed point")
		expect(relation.fixation).toBe(relation.anchor.entityId)
	}
	expect(() => normalizeSketchRelations(result.relations)).not.toThrow()
})

it("reuses matching driving dimensions and fixed anchors but not reference dimensions", () => {
	const entity: SketchEntity = { id: "circle", type: "circle", center: { x: 4, y: 5 }, radius: 10, segments: 64 }
	const fixed: import("../src/sketch-solver").SketchRelation = { id: "center", type: "fixed", anchor: { entityId: entity.id, point: "center" }, position: entity.center }
	const radius: import("../src/sketch-solver").SketchRelation = { id: "radius", type: "radius", entityId: entity.id, value: 10 }
	expect(fixedEntityRelations(entity, "fix", [fixed, radius])).toEqual([])
	expect(fixedEntityRelations(entity, "fix", [{ ...radius, reference: true }])).toHaveLength(2)
	expect(fixedEntityRelations(entity, "fix", [radius])).toHaveLength(1)
})
