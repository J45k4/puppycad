import { expect, it } from "bun:test"
import { tangentArc, sketchEndpointDirection } from "../src/sketch-tangent-arc"
import { arcPoint } from "../src/sketch-curves"
import { solveSketch, normalizeSketchRelations, remapSketchRelations, type SketchRelation } from "../src/sketch-solver"
import { editSketchCurve } from "../src/sketch-edit"
import type { Sketch, Line, Arc } from "../src/schema"
const line: Line = { id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }
it("constructs tangent arcs from either line endpoint with signed minor and major sweeps", () => {
	for (const [point, end] of [
		["p1", { x: 15, y: 5 }],
		["p1", { x: 15, y: -5 }],
		["p0", { x: 5, y: 5 }]
	] as const) {
		const arc = tangentArc("arc", line, point, end)
		const start = arcPoint(arc, 0)
		const last = arcPoint(arc, 1)
		expect(start.x).toBeCloseTo(line[point].x, 7)
		expect(start.y).toBeCloseTo(line[point].y, 7)
		expect(last.x).toBeCloseTo(end.x, 7)
		expect(last.y).toBeCloseTo(end.y, 7)
		const a = sketchEndpointDirection(line, point)
		const b = sketchEndpointDirection(arc, "p0")
		expect(a.x + b.x).toBeCloseTo(0, 7)
		expect(a.y + b.y).toBeCloseTo(0, 7)
	}
	expect(tangentArc("arc", line, "p1", { x: 15, y: -5 }).sweep).toBeLessThan(0)
	expect(Math.abs(tangentArc("arc", line, "p0", { x: 5, y: 5 }).sweep)).toBeGreaterThan(Math.PI)
	expect(() => tangentArc("arc", line, "p1", { x: 20, y: 0 })).toThrow("tangent line")
})
it("continues both endpoints of clockwise and counterclockwise arcs", () => {
	for (const sweep of [Math.PI / 2, -Math.PI / 2])
		for (const endpoint of ["p0", "p1"] as const) {
			const source: Arc = { id: "source", type: "arc", center: { x: 0, y: 0 }, radius: 10, startAngle: 0, sweep, segments: 64 }
			const created = tangentArc("new", source, endpoint, { x: 20, y: 20 })
			const a = sketchEndpointDirection(source, endpoint)
			const b = sketchEndpointDirection(created, "p0")
			expect(a.x + b.x).toBeCloseTo(0, 7)
			expect(a.y + b.y).toBeCloseTo(0, 7)
			expect(solveSketch([source, created], [{ id: "join", type: "smoothJoin", a: { entityId: source.id, point: endpoint }, b: { entityId: created.id, point: "p0" } }]).status).toBe(
				"underconstrained"
			)
		}
})
it("retains the finite tangent join while changing the arc radius and persists it through PCad", async () => {
	const arc = tangentArc("arc", line, "p1", { x: 15, y: 5 })
	const relations: SketchRelation[] = [
		{ id: "line-start", type: "fixed", anchor: { entityId: line.id, point: "p0" }, position: line.p0 },
		{ id: "line-end", type: "fixed", anchor: { entityId: line.id, point: "p1" }, position: line.p1 },
		{ id: "join", type: "smoothJoin", a: { entityId: line.id, point: "p1" }, b: { entityId: arc.id, point: "p0" } },
		{ id: "radius", type: "radius", entityId: arc.id, value: 8 },
		{ id: "sweep", type: "arcSweep", entityId: arc.id, value: 90 }
	]
	const solved = solveSketch([line, arc], relations)
	expect(solved.status).toBe("fully-constrained")
	const changed = solved.entities[1]
	if (changed?.type !== "arc") throw Error("Missing arc")
	expect(changed.radius).toBeCloseTo(8, 5)
	expect(arcPoint(changed, 0).x).toBeCloseTo(10, 5)
	expect(arcPoint(changed, 0).y).toBeCloseTo(0, 5)
	expect(arcPoint(changed, 1).x).toBeCloseTo(18, 5)
	expect(arcPoint(changed, 1).y).toBeCloseTo(8, 5)
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
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(solveSketch(saved.entities, saved.relations ?? []).status).toBe("fully-constrained")
	expect(remapSketchRelations(normalizeSketchRelations(saved.relations), new Map([["arc", "new-arc"]]))?.find((r) => r.type === "smoothJoin")).toMatchObject({
		b: { entityId: "new-arc", point: "p0" }
	})
	const split = editSketchCurve(saved, "line", { x: 5, y: 0 }, "Split")
	expect(split.sketch.relations?.find((r) => r.type === "smoothJoin")).toMatchObject({ a: { entityId: "line-split-1", point: "p1" } })
	expect(() => normalizeSketchRelations([{ id: "bad", type: "smoothJoin", a: { entityId: "line", point: "center" }, b: { entityId: "arc", point: "p0" } }])).toThrow()
})
it("constructs tangent arcs from both native spline endpoints and excludes interior handles from snapping", async () => {
	const { sketchSnap } = await import("../src/sketch-snap")
	for (const mode of ["fit", "control"] as const) {
		const curve: import("../src/schema").Spline = {
			id: "spline",
			type: "spline",
			mode,
			points: [
				{ x: 0, y: 0 },
				{ x: 10, y: 0 },
				{ x: 20, y: 10 },
				{ x: 30, y: 10 }
			]
		}
		for (const endpoint of ["point0", "point3"] as const) {
			const arc = tangentArc("arc", curve, endpoint, { x: 15, y: 25 })
			const a = sketchEndpointDirection(curve, endpoint)
			const b = sketchEndpointDirection(arc, "p0")
			expect(a.x + b.x).toBeCloseTo(0, 8)
			expect(a.y + b.y).toBeCloseTo(0, 8)
			expect(arcPoint(arc, 1).x).toBeCloseTo(15, 8)
			expect(arcPoint(arc, 1).y).toBeCloseTo(25, 8)
			const snap = sketchSnap([curve], arcPoint(arc, 0), 10, { endpointsOnly: true })
			expect(snap?.anchor).toEqual({ entityId: "spline", point: endpoint })
		}
		expect(sketchSnap([curve], { x: 10, y: 0 }, 10, { endpointsOnly: true })).toBeNull()
	}
})
it("retains spline-to-arc tangency after local project reload and a radius change", async () => {
	const { requireValue } = await import("../src/required")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const { createProjectFile, serializeProjectFile, normalizeProjectFile } = await import("../src/project-file")
	for (const mode of ["fit", "control"] as const) {
		const curve: import("../src/schema").Spline = {
			id: "spline",
			type: "spline",
			mode,
			points: [
				{ x: 0, y: 0 },
				{ x: 10, y: 0 },
				{ x: 20, y: 10 },
				{ x: 30, y: 10 }
			]
		}
		const arc = tangentArc("arc", curve, "point3", { x: 45, y: 25 })
		const relations: SketchRelation[] = [
			...curve.points.map((position, i): SketchRelation => ({ id: `fix${i}`, type: "fixed", anchor: { entityId: "spline", point: `point${i}` }, position })),
			{ id: "join", type: "smoothJoin", a: { entityId: "spline", point: "point3" }, b: { entityId: "arc", point: "p0" } }
		]
		const sketch: Sketch = {
			id: "sketch",
			type: "sketch",
			dirty: false,
			target: { type: "plane", plane: "XY" },
			entities: [curve, arc],
			relations,
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}
		const runtime = createPartRuntimeState({ features: [sketch] })
		const file = createProjectFile({
			items: [{ id: "p", type: "part", name: "Tangent arc", data: { features: [sketch], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
			selectedPath: null
		})
		const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = restored.items[0] as import("../src/contract").ProjectPartDocument
		const state = createPartRuntimeState(requireValue(part.data))
		const saved = materializePartFeatures(state.cad, state.tree)[0]
		if (saved?.type !== "sketch") throw Error("Missing sketch")
		expect(saved.relations).toEqual(relations)
		const changed = solveSketch(saved.entities, [...relations, { id: "radius", type: "radius", entityId: "arc", value: arc.radius * 1.5 }])
		expect(changed.status).not.toBe("conflicting")
		const result = changed.entities.find((e) => e.id === "arc")
		if (result?.type !== "arc") throw Error("Missing arc")
		expect(result.radius).toBeCloseTo(arc.radius * 1.5, 5)
		expect(arcPoint(result, 0).x).toBeCloseTo(30, 5)
		expect(arcPoint(result, 0).y).toBeCloseTo(10, 5)
		const a = sketchEndpointDirection(curve, "point3")
		const b = sketchEndpointDirection(result, "p0")
		expect(a.x + b.x).toBeCloseTo(0, 5)
		expect(a.y + b.y).toBeCloseTo(0, 5)
	}
})
