import { expect, it } from "bun:test"
import { solveSketch, type SketchRelation } from "../src/sketch-solver"
import type { SketchEntity } from "../src/schema"
const entities: SketchEntity[] = [
	{ id: "fixed", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 64 },
	{ id: "free", type: "circle", center: { x: 30, y: 0 }, radius: 5, segments: 64 },
	{ id: "point", type: "point", center: { x: 0, y: 0 } }
]
const relations: SketchRelation[] = [
	{ id: "center", type: "fixed", anchor: { entityId: "fixed", point: "center" }, position: { x: 0, y: 0 } },
	{ id: "radius", type: "radius", entityId: "fixed", value: 10 },
	{ id: "join", type: "coincident", a: { entityId: "point", point: "center" }, b: { entityId: "fixed", point: "center" } }
]
it("distinguishes fixed entities from free geometry and follows coupled constraints", () => {
	const solved = solveSketch(entities, relations)
	expect(solved.status).toBe("underconstrained")
	expect(solved.entityStates).toEqual({ fixed: "fully-constrained", free: "underconstrained", point: "fully-constrained" })
	expect(
		solveSketch(
			entities,
			relations.filter((r) => r.id !== "radius")
		).entityStates
	).toEqual({ fixed: "underconstrained", free: "underconstrained", point: "fully-constrained" })
	expect(
		solveSketch(
			entities,
			relations.filter((r) => r.id !== "center")
		).entityStates.point
	).toBe("underconstrained")
})
it("does not treat a reference radius as driving or label unrelated geometry fixed during conflicts", () => {
	expect(
		solveSketch(
			entities,
			relations.map((r) => (r.id === "radius" ? { ...r, reference: true } : r))
		).entityStates.fixed
	).toBe("underconstrained")
	const result = solveSketch(entities, [...relations, { id: "bad", type: "radius", entityId: "fixed", value: 20 }])
	expect(result.entityStates.fixed).toBe("conflicting")
	expect(result.entityStates.free).toBe("unknown")
})
it("requires all line parameters to be constrained, not just one fixed endpoint", () => {
	const line: SketchEntity = { id: "l", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }
	const fixed: SketchRelation = { id: "start", type: "fixed", anchor: { entityId: "l", point: "p0" }, position: { x: 0, y: 0 } }
	expect(solveSketch([line], [fixed]).entityStates.l).toBe("underconstrained")
	expect(solveSketch([line], [fixed, { id: "length", type: "length", entityId: "l", value: 10 }, { id: "h", type: "horizontal", entityId: "l" }]).entityStates.l).toBe("fully-constrained")
})
