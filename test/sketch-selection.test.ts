import { ellipsePoint } from "../src/sketch-ellipse"
import { expect, it } from "bun:test"
import { sketchEntityInBox } from "../src/sketch-selection"
import type { SketchEntity } from "../src/schema"
const box = { minX: -2, minY: -2, maxX: 2, maxY: 2 }
it("distinguishes full containment from curve crossing and ignores closed interiors", () => {
	const line: SketchEntity = { id: "l", type: "line", p0: { x: -10, y: 0 }, p1: { x: 10, y: 0 } }
	expect(sketchEntityInBox(line, box, false)).toBe(false)
	expect(sketchEntityInBox(line, box, true)).toBe(true)
	const circle: SketchEntity = { id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 8 }
	expect(sketchEntityInBox(circle, box, true)).toBe(false)
	expect(sketchEntityInBox(circle, { minX: 9.9, maxX: 10.1, minY: -0.01, maxY: 0.01 }, true)).toBe(true)
	expect(sketchEntityInBox(circle, { minX: -10, maxX: 10, minY: -10, maxY: 10 }, false)).toBe(true)
	expect(sketchEntityInBox({ id: "p", type: "point", center: { x: 1, y: 1 } }, box, false)).toBe(true)
})
it("uses finite arc extents and analytic ellipse intersections independently of tessellation", () => {
	const arc: SketchEntity = { id: "a", type: "arc", center: { x: 0, y: 0 }, radius: 10, startAngle: 0, sweep: Math.PI / 2, segments: 8 }
	expect(sketchEntityInBox(arc, { minX: 7.05, maxX: 7.1, minY: 7.05, maxY: 7.1 }, true)).toBe(true)
	expect(sketchEntityInBox(arc, { minX: -10.1, maxX: -9.9, minY: -0.1, maxY: 0.1 }, true)).toBe(false)
	expect(sketchEntityInBox(arc, { minX: -0.01, maxX: 10.01, minY: -0.01, maxY: 10.01 }, false)).toBe(true)
	const ellipse: SketchEntity = { id: "e", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 45, segments: 8 }
	expect(sketchEntityInBox(ellipse, { minX: 7.05, maxX: 7.1, minY: 7.05, maxY: 7.1 }, true)).toBe(true)
	const betweenSamples = ellipsePoint(ellipse, 0.31)
	expect(sketchEntityInBox(ellipse, { minX: betweenSamples.x - 0.001, maxX: betweenSamples.x + 0.001, minY: betweenSamples.y - 0.001, maxY: betweenSamples.y + 0.001 }, true)).toBe(true)
	expect(sketchEntityInBox(ellipse, box, true)).toBe(false)
	expect(sketchEntityInBox(ellipse, { minX: -8, maxX: 8, minY: -8, maxY: 8 }, false)).toBe(true)
})
it("handles slot caps, polygon edges and rotated rectangles", () => {
	const slot: SketchEntity = { id: "slot", type: "capsule", from: { x: -5, y: 0 }, to: { x: 5, y: 0 }, width: 4, arcSegments: 1 }
	expect(sketchEntityInBox(slot, { minX: 6.9, maxX: 7.1, minY: -0.01, maxY: 0.01 }, true)).toBe(true)
	expect(sketchEntityInBox(slot, { minX: -1, maxX: 1, minY: -1, maxY: 1 }, true)).toBe(false)
	expect(sketchEntityInBox(slot, { minX: -7, maxX: 7, minY: -2, maxY: 2 }, false)).toBe(true)
	const polygon: SketchEntity = { id: "p", type: "polygon", center: { x: 0, y: 0 }, radius: 10, rotation: 0, sides: 4 }
	expect(sketchEntityInBox(polygon, { minX: 4.9, maxX: 5.1, minY: 4.9, maxY: 5.1 }, true)).toBe(true)
	const rectangle: SketchEntity = { id: "r", type: "rectangle", center: { x: 0, y: 0 }, width: 20, height: 2, rotation: 45 }
	expect(sketchEntityInBox(rectangle, box, true)).toBe(true)
	expect(sketchEntityInBox(rectangle, box, false)).toBe(false)
})

it("selects native spline crossings smaller than sampled chord error without selecting interiors", async () => {
	const { splineBezierSegments } = await import("../src/sketch-spline")
	const { cubicBezierPoint } = await import("../src/sketch-bezier")
	const curves: import("../src/schema").Spline[] = [
		{
			id: "control",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 30 },
				{ x: 30, y: -20 },
				{ x: 40, y: 0 }
			]
		},
		{
			id: "fit",
			type: "spline",
			mode: "fit",
			points: [
				{ x: 0, y: 0 },
				{ x: 10, y: 20 },
				{ x: 30, y: -10 },
				{ x: 40, y: 0 }
			]
		},
		{
			id: "closed",
			type: "spline",
			mode: "fit",
			closed: true,
			points: [
				{ x: -20, y: 0 },
				{ x: 0, y: 20 },
				{ x: 20, y: 0 },
				{ x: 0, y: -20 }
			]
		}
	]
	for (const curve of curves) {
		for (const segment of splineBezierSegments(curve)) {
			for (const t of [0.137, 0.371, 0.613, 0.891]) {
				const point = cubicBezierPoint(segment, t)
				expect(sketchEntityInBox(curve, { minX: point.x - 1e-6, maxX: point.x + 1e-6, minY: point.y - 1e-6, maxY: point.y + 1e-6 }, true)).toBe(true)
			}
		}
	}
	const closed = curves[2]
	if (!closed) throw Error("Missing closed spline")
	expect(sketchEntityInBox(closed, box, true)).toBe(false)
	expect(sketchEntityInBox(closed, { minX: -50, maxX: 50, minY: -50, maxY: 50 }, false)).toBe(true)
})
