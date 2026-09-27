import { expect, it } from "bun:test"
import type { Ellipse, Line, Sketch } from "../src/schema"
import { lineEllipseIntersections, circleEllipseIntersections, ellipseEllipseIntersections } from "../src/sketch-ellipse-intersections"
import { pointOnEllipseResidual, ellipsePoint } from "../src/sketch-ellipse"
import { editSketchCurve } from "../src/sketch-edit"
const ellipse: Ellipse = { id: "e", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 8 }
it("finds exact crossings independent of ellipse sampling and line length", () => {
	for (const extent of [20, 1e12]) {
		const line: Line = { id: "l", type: "line", p0: { x: -extent, y: 2.5 }, p1: { x: extent, y: 2.5 } }
		const points = lineEllipseIntersections(line, ellipse)
		expect(points).toHaveLength(2)
		expect(points[0]?.x).toBeCloseTo(-5 * Math.sqrt(3), 8)
		expect(points[1]?.x).toBeCloseTo(5 * Math.sqrt(3), 8)
		for (const point of points) expect(pointOnEllipseResidual(ellipse, point)).toBeCloseTo(0, 10)
	}
})
it("handles rotated tangency, misses and finite segment limits", () => {
	const e = { ...ellipse, rotation: 37, center: { x: 10, y: -5 } }
	const contact = ellipsePoint(e, Math.PI / 2)
	const angle = (e.rotation * Math.PI) / 180
	const line: Line = {
		id: "l",
		type: "line",
		p0: { x: contact.x - 20 * Math.cos(angle), y: contact.y - 20 * Math.sin(angle) },
		p1: { x: contact.x + 20 * Math.cos(angle), y: contact.y + 20 * Math.sin(angle) }
	}
	const points = lineEllipseIntersections(line, e)
	expect(points).toHaveLength(1)
	for (const point of points) expect(Math.hypot(point.x - contact.x, point.y - contact.y)).toBeLessThan(1e-6)
	expect(lineEllipseIntersections({ ...line, p0: { x: -20, y: 6 }, p1: { x: 20, y: 6 } }, ellipse)).toEqual([])
	expect(lineEllipseIntersections({ ...line, p0: { x: -20, y: 0 }, p1: { x: -15, y: 0 } }, ellipse, true)).toEqual([])
	expect(lineEllipseIntersections({ ...line, p0: { x: -20, y: 0 }, p1: { x: 0, y: 0 } }, ellipse, true)).toHaveLength(1)
})
it("trims and extends a line to the analytic ellipse boundary", () => {
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [ellipse, { id: "l", type: "line", p0: { x: -20, y: 2.5 }, p1: { x: 20, y: 2.5 } }],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const trimmed = editSketchCurve(sketch, "l", { x: -15, y: 2.5 }, "Trim").sketch.entities.find((e) => e.id === "l")
	if (trimmed?.type !== "line") throw Error("Missing line")
	expect(trimmed.p0.x).toBeCloseTo(-5 * Math.sqrt(3), 8)
	const short = structuredClone(sketch)
	short.entities[1] = { id: "l", type: "line", p0: { x: -20, y: 2.5 }, p1: { x: -15, y: 2.5 } }
	const extended = editSketchCurve(short, "l", { x: -15, y: 2.5 }, "Extend").sketch.entities.find((e) => e.id === "l")
	if (extended?.type !== "line") throw Error("Missing line")
	expect(extended.p1.x).toBeCloseTo(-5 * Math.sqrt(3), 8)
	expect(sketch.entities[1]).toMatchObject({ p0: { x: -20, y: 2.5 } })
})

it("finds all four circle-ellipse crossings without sampled segments", () => {
	const circle = { center: { x: 0, y: 0 }, radius: 7 }
	const points = circleEllipseIntersections(circle, ellipse)
	expect(points).toHaveLength(4)
	for (const point of points) {
		expect(pointOnEllipseResidual(ellipse, point)).toBeCloseTo(0, 9)
		expect(Math.hypot(point.x, point.y)).toBeCloseTo(7, 9)
		expect(Math.abs(point.x)).toBeCloseTo(Math.sqrt(32), 8)
	}
})
it("retains tangencies and chart-boundary contacts while excluding misses and coincident curves", () => {
	expect(circleEllipseIntersections({ center: { x: 0, y: 0 }, radius: 5 }, ellipse)).toHaveLength(2)
	expect(circleEllipseIntersections({ center: { x: 0, y: 0 }, radius: 10 }, ellipse)).toHaveLength(2)
	expect(circleEllipseIntersections({ center: { x: 12, y: 0 }, radius: 2 }, ellipse)).toHaveLength(1)
	expect(circleEllipseIntersections({ center: { x: 13, y: 0 }, radius: 2 }, ellipse)).toHaveLength(0)
	expect(circleEllipseIntersections({ center: { x: 0, y: 0 }, radius: 2 }, ellipse)).toHaveLength(0)
	expect(circleEllipseIntersections({ center: { x: 0, y: 0 }, radius: 10 }, { ...ellipse, height: 20 })).toHaveLength(0)
	const rotated = { ...ellipse, center: { x: 30, y: -20 }, rotation: 67 }
	const points = circleEllipseIntersections({ center: { x: 30, y: -20 }, radius: 7 }, rotated)
	expect(points).toHaveLength(4)
	for (const p of points) {
		expect(pointOnEllipseResidual(rotated, p)).toBeCloseTo(0, 8)
		expect(Math.hypot(p.x - 30, p.y + 20)).toBeCloseTo(7, 8)
	}
})
it("trims circles and extends finite arcs to analytic ellipse contacts", () => {
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [ellipse, { id: "round", type: "circle", center: { x: 0, y: 0 }, radius: 7, segments: 8 }],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const trimmed = editSketchCurve(sketch, "round", { x: 7, y: 0 }, "Trim").sketch.entities.find((e) => e.id === "round")
	if (trimmed?.type !== "arc") throw Error("Missing trimmed arc")
	const start = { x: trimmed.center.x + trimmed.radius * Math.cos(trimmed.startAngle), y: trimmed.center.y + trimmed.radius * Math.sin(trimmed.startAngle) }
	expect(pointOnEllipseResidual(ellipse, start)).toBeCloseTo(0, 8)
	sketch.entities[1] = { id: "round", type: "arc", center: { x: 0, y: 0 }, radius: 7, startAngle: 0, sweep: 0.2, segments: 8 }
	const extended = editSketchCurve(sketch, "round", { x: 7 * Math.cos(0.2), y: 7 * Math.sin(0.2) }, "Extend").sketch.entities.find((e) => e.id === "round")
	if (extended?.type !== "arc") throw Error("Missing extended arc")
	expect(extended.sweep).toBeCloseTo(Math.acos(Math.sqrt(32) / 7), 8)
})
it("solves asymmetric circle offsets with odd polynomial coefficients", () => {
	const points = circleEllipseIntersections({ center: { x: 0, y: 3 }, radius: 5 }, ellipse)
	expect(points).toHaveLength(2)
	for (const p of points) {
		expect(p.y).toBeCloseTo(-1 + Math.sqrt(29), 8)
		expect(Math.hypot(p.x, p.y - 3)).toBeCloseTo(5, 8)
	}
})

it("finds four rotated ellipse crossings without tessellation and is symmetric", () => {
	const a = { ...ellipse, rotation: 17, center: { x: 40, y: -20 } }
	const b = { ...a, id: "b", rotation: 107 }
	const points = ellipseEllipseIntersections(a, b)
	expect(points).toHaveLength(4)
	const reverse = ellipseEllipseIntersections(b, a)
	for (const point of points) {
		expect(pointOnEllipseResidual(a, point)).toBeCloseTo(0, 8)
		expect(pointOnEllipseResidual(b, point)).toBeCloseTo(0, 8)
		expect(reverse.some((p) => Math.hypot(p.x - point.x, p.y - point.y) < 1e-8)).toBe(true)
	}
})
it("handles ellipse tangency, chart seams, containment and coincident outlines", () => {
	for (const offset of [
		{ x: 20, y: 0 },
		{ x: 0, y: 10 }
	]) {
		const points = ellipseEllipseIntersections(ellipse, { ...ellipse, center: offset })
		expect(points).toHaveLength(1)
		expect(points[0]?.x).toBeCloseTo(offset.x / 2, 8)
		expect(points[0]?.y).toBeCloseTo(offset.y / 2, 8)
	}
	expect(ellipseEllipseIntersections(ellipse, { ...ellipse, width: 4, height: 2 })).toEqual([])
	expect(ellipseEllipseIntersections(ellipse, { ...ellipse, rotation: 180 })).toEqual([])
	expect(ellipseEllipseIntersections(ellipse, { ...ellipse, width: 10, height: 20, rotation: 90 })).toEqual([])
	expect(ellipseEllipseIntersections(ellipse, { ...ellipse, center: { x: 30, y: 0 } })).toEqual([])
})
