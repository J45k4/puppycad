import { requireValue } from "../src/required"
import { expect, it } from "bun:test"
import { redundantSketchRelations } from "../src/sketch-redundancy"
import { solveSketch, type SketchRelation } from "../src/sketch-solver"
const line = { id: "line", type: "line" as const, p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }
it("identifies later duplicate driving relations while ignoring reference measurements", () => {
	const relations: SketchRelation[] = [
		{ id: "first", type: "length", entityId: "line", value: 10 },
		{ id: "second", type: "length", entityId: "line", value: 10 },
		{ id: "ref", type: "length", entityId: "line", value: 999, reference: true }
	]
	const solved = solveSketch([line], relations)
	expect(solved.redundantRelations).toEqual(["second"])
	expect(solved.degreesOfFreedom).toBe(3)
	expect(
		solveSketch(
			[line],
			relations.filter((r) => r.id !== "second")
		).redundantRelations
	).toEqual([])
	expect(solveSketch([line], [requireValue(relations[1]), requireValue(relations[0])]).redundantRelations).toEqual(["first"])
})
it("keeps partly independent relations and ignores inactive residual rows", () => {
	expect(
		redundantSketchRelations(
			[
				{ id: "x", rows: [[1, 0]] },
				{
					id: "xy",
					rows: [
						[1, 0],
						[0, 1],
						[0, 0]
					]
				},
				{ id: "inactive", rows: [[0, 0]] },
				{ id: "diagonal", rows: [[3, 4]] }
			],
			2
		)
	).toEqual(["diagonal"])
	expect(
		redundantSketchRelations(
			[
				{ id: "x", rows: [[1, 0]] },
				{ id: "repeat", rows: [[1, 0]] }
			],
			2
		)
	).toEqual([])
})
it("does not label conflicting dimensions as redundant", () => {
	const result = solveSketch(
		[line],
		[
			{ id: "a", type: "length", entityId: "line", value: 10 },
			{ id: "b", type: "length", entityId: "line", value: 20 }
		]
	)
	expect(result.status).toBe("conflicting")
	expect(result.redundantRelations).toEqual([])
})
