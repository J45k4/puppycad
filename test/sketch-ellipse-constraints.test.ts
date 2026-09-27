import { expect, it } from "bun:test"
import { ellipseSupportPoint, pointOnEllipseResidual } from "../src/sketch-ellipse"
import { solveSketch, type SketchRelation } from "../src/sketch-solver"
import type { Ellipse, Sketch } from "../src/schema"
const ellipse: Ellipse = { id: "e", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 30, segments: 128 }
const fixedEllipse = (): SketchRelation[] => [
	{ id: "center", type: "fixed", anchor: { entityId: "e", point: "center" }, position: { x: 0, y: 0 } },
	{ id: "width", type: "width", entityId: "e", value: 20 },
	{ id: "height", type: "height", entityId: "e", value: 10 },
	{ id: "rotation", type: "rotation", entityId: "e", value: 30 }
]
it("finds analytic support contacts on rotated ellipses", () => {
	for (const direction of [
		{ x: 1, y: 0 },
		{ x: 0, y: -1 },
		{ x: 2, y: 3 }
	]) {
		const point = ellipseSupportPoint(ellipse, direction)
		expect(pointOnEllipseResidual(ellipse, point)).toBeCloseTo(0, 10)
		const angle = (ellipse.rotation * Math.PI) / 180
		const x = point.x * Math.cos(angle) + point.y * Math.sin(angle)
		const y = -point.x * Math.sin(angle) + point.y * Math.cos(angle)
		const nx = (x / 100) * Math.cos(angle) - (y / 25) * Math.sin(angle)
		const ny = (x / 100) * Math.sin(angle) + (y / 25) * Math.cos(angle)
		expect(nx * direction.y - ny * direction.x).toBeCloseTo(0, 10)
		expect(nx * direction.x + ny * direction.y).toBeGreaterThan(0)
	}
})
it("keeps one sliding degree of freedom for a point placed at the ellipse center", () => {
	const solved = solveSketch(
		[ellipse, { id: "p", type: "point", center: { x: 0, y: 0 } }],
		[...fixedEllipse(), { id: "on", type: "pointOnCurve", a: { entityId: "p", point: "center" }, b: "e" }]
	)
	expect(solved.status).toBe("underconstrained")
	expect(solved.degreesOfFreedom).toBe(1)
	const point = solved.entities[1]
	if (point?.type !== "point") throw Error("Missing point")
	expect(pointOnEllipseResidual(ellipse, point.center)).toBeCloseTo(0, 5)
	expect(ellipse.center).toEqual({ x: 0, y: 0 })
})
it("solves line tangency from both sides and from a line through the center", () => {
	for (const y of [-2, 0, 2]) {
		const solved = solveSketch(
			[ellipse, { id: "line", type: "line", p0: { x: -30, y }, p1: { x: 30, y } }],
			[
				...fixedEllipse(),
				{ id: "h", type: "horizontal", entityId: "line" },
				{ id: "length", type: "length", entityId: "line", value: 60 },
				{ id: "x", type: "distance", a: { entityId: "e", point: "center" }, b: { entityId: "line", point: "p0" }, axis: "x", value: -30 },
				{ id: "tangent", type: "tangent", a: "line", b: "e" }
			]
		)
		expect(solved.status).toBe("fully-constrained")
		const line = solved.entities[1]
		if (line?.type !== "line") throw Error("Missing line")
		const contact = ellipseSupportPoint(ellipse, { x: 0, y: Math.sign(line.p0.y) })
		expect(line.p0.y).toBeCloseTo(contact.y, 5)
		expect(line.p1.y).toBeCloseTo(contact.y, 5)
	}
})
it("persists ellipse tangency and follows a changed axis dimension", async () => {
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [ellipse, { id: "line", type: "line", p0: { x: -30, y: 2 }, p1: { x: 30, y: 2 } }],
		relations: [...fixedEllipse(), { id: "h", type: "horizontal", entityId: "line" }, { id: "tangent", type: "tangent", a: "e", b: "line" }],
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
	const width = saved.relations?.find((r) => r.id === "width")
	if (width?.type !== "width") throw Error("Missing dimension")
	width.value = 30
	const solved = solveSketch(saved.entities, saved.relations ?? [])
	expect(solved.status).toBe("underconstrained")
	const e = solved.entities[0]
	const line = solved.entities[1]
	if (e?.type !== "ellipse" || line?.type !== "line") throw Error("Missing geometry")
	const contact = ellipseSupportPoint(e, { x: 0, y: Math.sign(line.p0.y) })
	expect(line.p0.y).toBeCloseTo(contact.y, 5)
	expect(line.p1.y).toBeCloseTo(contact.y, 5)
})

it("aligns ellipse and circle centers without changing ellipse axes or rotation", () => {
	const result = solveSketch(
		[ellipse, { id: "circle", type: "circle", center: { x: 40, y: 50 }, radius: 7, segments: 32 }],
		[...fixedEllipse(), { id: "same-center", type: "concentric", a: "e", b: "circle" }]
	)
	expect(result.status).toBe("underconstrained")
	expect(result.degreesOfFreedom).toBe(1)
	const circle = result.entities[1]
	if (circle?.type !== "circle") throw Error("Missing circle")
	expect(circle.center.x).toBeCloseTo(0, 5)
	expect(circle.center.y).toBeCloseTo(0, 5)
	expect(circle.radius).toBeCloseTo(7, 5)
	expect(result.entities[0]).toMatchObject({ width: 20, height: 10, rotation: 30 })
})
it("persists mixed concentric constraints and follows a changed ellipse center after reload", async () => {
	const sketch: Sketch = {
		id: "sketch",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [ellipse, { id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 7, segments: 32 }],
		relations: [...fixedEllipse(), { id: "concentric", type: "concentric", a: "e", b: "circle" }],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations).toContainEqual({ id: "concentric", type: "concentric", a: "e", b: "circle" })
	const center = saved.relations?.find((r) => r.id === "center")
	if (center?.type !== "fixed") throw Error("Missing center constraint")
	center.position = { x: 25, y: -15 }
	const solved = solveSketch(saved.entities, saved.relations ?? [])
	expect(solved.status).toBe("underconstrained")
	for (const entity of solved.entities) {
		if (entity.type !== "circle" && entity.type !== "ellipse") throw Error("Missing curve")
		expect(entity.center.x).toBeCloseTo(25, 5)
		expect(entity.center.y).toBeCloseTo(-15, 5)
	}
	const moved = solved.entities[0]
	if (moved?.type !== "ellipse") throw Error("Missing ellipse")
	expect(moved.width).toBeCloseTo(20, 5)
	expect(moved.height).toBeCloseTo(10, 5)
	expect(moved.rotation).toBeCloseTo(30, 5)
	expect(ellipse.center).toEqual({ x: 0, y: 0 })
})
