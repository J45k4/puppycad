import { expect, it } from "bun:test"
import type { SketchEntity } from "../src/schema"
import { solveSketch, type SketchRelation } from "../src/sketch-solver"
import { pointOnSketchCurveResidual } from "../src/sketch-point-constraints"

function fixed(entity: SketchEntity): SketchRelation[] {
	if (entity.type === "line")
		return [
			{ id: `${entity.id}-p0`, type: "fixed", anchor: { entityId: entity.id, point: "p0" }, position: entity.p0 },
			{ id: `${entity.id}-p1`, type: "fixed", anchor: { entityId: entity.id, point: "p1" }, position: entity.p1 }
		]
	if (entity.type !== "circle" && entity.type !== "arc") throw Error("Unsupported fixture")
	return [
		{ id: `${entity.id}-center`, type: "fixed", anchor: { entityId: entity.id, point: "center" }, position: entity.center },
		{ id: `${entity.id}-radius`, type: "radius", entityId: entity.id, value: entity.radius },
		...(entity.type === "arc"
			? [
					{
						id: `${entity.id}-start`,
						type: "fixed" as const,
						anchor: { entityId: entity.id, point: "p0" as const },
						position: { x: entity.center.x + entity.radius * Math.cos(entity.startAngle), y: entity.center.y + entity.radius * Math.sin(entity.startAngle) }
					},
					{ id: `${entity.id}-sweep`, type: "arcSweep" as const, entityId: entity.id, value: (entity.sweep * 180) / Math.PI }
				]
			: [])
	]
}
const arc: SketchEntity = { id: "a", type: "arc", center: { x: 0, y: 0 }, radius: 10, startAngle: 0, sweep: Math.PI, segments: 32 }
it("accepts visible tangent contacts and rejects contacts on the missing half of a fixed arc", () => {
	for (const y of [10, -10]) {
		const line: SketchEntity = { id: "l", type: "line", p0: { x: -20, y }, p1: { x: 20, y } }
		const result = solveSketch([arc, line], [...fixed(arc), ...fixed(line), { id: "touch", type: "tangent", a: "a", b: "l" }])
		expect(result.status === "conflicting").toBe(y < 0)
	}
})
it("checks both finite arcs for external tangent contacts including clockwise sweeps", () => {
	for (const startAngle of [Math.PI, 0]) {
		const other: SketchEntity = { id: "b", type: "arc", center: { x: 15, y: 0 }, radius: 5, startAngle, sweep: -Math.PI / 2, segments: 16 }
		const result = solveSketch([arc, other], [...fixed(arc), ...fixed(other), { id: "touch", type: "tangent", a: "a", b: "b" }])
		expect(result.status === "conflicting").toBe(startAngle === 0)
	}
})
it("moves a free arc into contact instead of accepting only its supporting circle", () => {
	const circle: SketchEntity = { id: "b", type: "circle", center: { x: 0, y: -15 }, radius: 5, segments: 32 }
	const result = solveSketch([arc, circle], [...fixed(circle), { id: "radius", type: "radius", entityId: "a", value: 10 }, { id: "touch", type: "tangent", a: "a", b: "b" }])
	expect(result.status).not.toBe("conflicting")
	const solved = result.entities[0]
	if (solved?.type !== "arc") throw Error("Missing arc")
	const dx = circle.center.x - solved.center.x
	const dy = circle.center.y - solved.center.y
	const distance = Math.hypot(dx, dy)
	expect(distance).toBeCloseTo(15, 5)
	const contact = { x: solved.center.x + (dx * 10) / distance, y: solved.center.y + (dy * 10) / distance }
	for (const residual of pointOnSketchCurveResidual(solved, contact)) expect(residual).toBeCloseTo(0, 5)
})
it("preserves finite-arc tangency through PCad serialization", async () => {
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const line: SketchEntity = { id: "l", type: "line", p0: { x: -20, y: 10 }, p1: { x: 20, y: 10 } }
	const relations: SketchRelation[] = [...fixed(arc), ...fixed(line), { id: "touch", type: "tangent", a: "l", b: "a" }]
	const runtime = createPartRuntimeState({
		features: [{ id: "s", type: "sketch", dirty: false, target: { type: "plane", plane: "XY" }, entities: [arc, line], relations, dimensions: [], vertices: [], loops: [], profiles: [] }]
	})
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations).toEqual(relations)
	expect(solveSketch(saved.entities, saved.relations ?? []).status).not.toBe("conflicting")
})
it("requires internal tangent contacts on both finite sweeps", () => {
	for (const outerMissing of [false, true])
		for (const innerMissing of [false, true]) {
			const outer: SketchEntity = { ...arc, startAngle: outerMissing ? Math.PI / 2 : -Math.PI / 2 }
			const inner: SketchEntity = { ...arc, id: "inner", center: { x: 5, y: 0 }, radius: 5, startAngle: innerMissing ? Math.PI / 2 : -Math.PI / 2 }
			const result = solveSketch([outer, inner], [...fixed(outer), ...fixed(inner), { id: "touch", type: "internalTangent", a: "a", b: "inner" }])
			expect(result.status === "conflicting").toBe(outerMissing || innerMissing)
		}
})
it("solves internal tangency for an initially concentric arc and circle", () => {
	const outer: SketchEntity = { id: "outer", type: "circle", center: { x: 0, y: 0 }, radius: 20, segments: 32 }
	const inner: SketchEntity = { ...arc, radius: 5, startAngle: -Math.PI / 2 }
	const result = solveSketch([outer, inner], [...fixed(outer), { id: "radius", type: "radius", entityId: "a", value: 5 }, { id: "touch", type: "internalTangent", a: "outer", b: "a" }])
	expect(result.status).not.toBe("conflicting")
	const solved = result.entities[1]
	if (solved?.type !== "arc") throw Error("Missing arc")
	expect(Math.hypot(solved.center.x, solved.center.y)).toBeCloseTo(15, 5)
	const contact = { x: (solved.center.x * 20) / 15, y: (solved.center.y * 20) / 15 }
	for (const residual of pointOnSketchCurveResidual(solved, contact)) expect(residual).toBeCloseTo(0, 5)
})

it("preserves internal arc tangency through PCad serialization", async () => {
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const line: SketchEntity = { id: "l", type: "circle", center: { x: 5, y: 0 }, radius: 5, segments: 32 }
	const relations: SketchRelation[] = [...fixed(arc), ...fixed(line), { id: "touch", type: "internalTangent", a: "a", b: "l" }]
	const runtime = createPartRuntimeState({
		features: [{ id: "s", type: "sketch", dirty: false, target: { type: "plane", plane: "XY" }, entities: [arc, line], relations, dimensions: [], vertices: [], loops: [], profiles: [] }]
	})
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations).toEqual(relations)
	expect(solveSketch(saved.entities, saved.relations ?? []).status).not.toBe("conflicting")
})
