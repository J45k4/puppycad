import { expect, it } from "bun:test"
import { sketchSnap } from "../src/sketch-snap"
import type { SketchEntity } from "../src/schema"
it("chooses the nearest anchor in a screen-space tolerance before falling back to origin or grid", () => {
	const entities: SketchEntity[] = [
		{ id: "a", type: "point", center: { x: 10, y: 0 } },
		{ id: "b", type: "point", center: { x: 11, y: 0 } }
	]
	expect(sketchSnap(entities, { x: 10.9, y: 0 }, 3)).toMatchObject({ kind: "anchor", anchor: { entityId: "b", point: "center" }, position: { x: 11, y: 0 } })
	expect(sketchSnap(entities, { x: 13.1, y: 0 }, 3)?.kind).toBe("anchor")
	expect(sketchSnap(entities, { x: 13.1, y: 0 }, 10)).toEqual({ kind: "grid", position: { x: 13, y: 0 } })
	expect(sketchSnap([], { x: 1, y: 0 }, 3)).toEqual({ kind: "origin", position: { x: 0, y: 0 } })
	expect(sketchSnap([], { x: 5.6, y: 6.2 }, 3)).toEqual({ kind: "grid", position: { x: 6, y: 6 } })
})
it("restricts tangent-arc start candidates to line and arc endpoints", () => {
	const entities: SketchEntity[] = [
		{ id: "point", type: "point", center: { x: 10, y: 0 } },
		{ id: "line", type: "line", p0: { x: 11, y: 0 }, p1: { x: 20, y: 0 } }
	]
	expect(sketchSnap(entities, { x: 10, y: 0 }, 3, { endpointsOnly: true })?.anchor).toEqual({ entityId: "line", point: "p0" })
	expect(sketchSnap(entities, { x: 0, y: 0 }, 3, { endpointsOnly: true })).toBeNull()
})
it("controls geometry and grid snaps independently", () => {
	const entities: SketchEntity[] = [{ id: "p", type: "point", center: { x: 10.4, y: 0.2 } }]
	const raw = { x: 10.8, y: 0.4 }
	expect(sketchSnap(entities, raw, 3, { geometry: false })).toEqual({ position: { x: 11, y: 0 }, kind: "grid" })
	expect(sketchSnap(entities, raw, 3, { grid: false })?.position).toEqual({ x: 10.4, y: 0.2 })
	expect(sketchSnap(entities, raw, 3, { grid: false, geometry: false })).toBeNull()
	expect(sketchSnap(entities, { x: 30, y: 30 }, 3, { grid: false })).toBeNull()
	expect(sketchSnap([], { x: 0.1, y: 0.1 }, 3, { grid: false })?.kind).toBe("origin")
})
it("snaps to analytic ellipse crossings while respecting geometry and endpoint-only controls", () => {
	const a = { id: "a", type: "ellipse" as const, center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 8 }
	const b = { ...a, id: "b", rotation: 90 }
	const expected = Math.sqrt(20)
	const raw = { x: expected + 0.1, y: expected - 0.1 }
	const snap = sketchSnap([a, b], raw, 10, { grid: false })
	expect(snap?.kind).toBe("intersection")
	expect(snap?.position.x).toBeCloseTo(expected, 8)
	expect(snap?.position.y).toBeCloseTo(expected, 8)
	expect(sketchSnap([a, b], raw, 10, { grid: false, geometry: false })).toBeNull()
	expect(sketchSnap([a, b], raw, 10, { endpointsOnly: true })).toBeNull()
})
it("snaps to finite line-circle and line-line crossings without snapping to their extensions", () => {
	const circle: SketchEntity = { id: "c", type: "circle", center: { x: 10, y: 10 }, radius: 5, segments: 8 }
	const line: SketchEntity = { id: "l", type: "line", p0: { x: 0, y: 13 }, p1: { x: 20, y: 13 } }
	expect(sketchSnap([circle, line], { x: 14.1, y: 13.1 }, 10, { grid: false })).toMatchObject({ kind: "intersection", position: { x: 14, y: 13 } })
	const short = { ...line, p1: { x: 11, y: 13 } }
	expect(sketchSnap([circle, short], { x: 14.1, y: 13.1 }, 10, { grid: false })).toBeNull()
	const diagonal: SketchEntity = { id: "d", type: "line", p0: { x: 10, y: 9 }, p1: { x: 20, y: 19 } }
	expect(sketchSnap([line, diagonal], { x: 14.1, y: 13.1 }, 10, { grid: false })?.kind).toBe("intersection")
})
it("filters circle-arc contacts to clockwise and counterclockwise sweeps", () => {
	const circle: SketchEntity = { id: "c", type: "circle", center: { x: 10, y: 10 }, radius: 5, segments: 8 }
	for (const sweep of [Math.PI, -Math.PI]) {
		const arc: SketchEntity = { id: "a", type: "arc", center: { x: 16, y: 10 }, radius: 5, startAngle: 0, sweep, segments: 8 }
		const snap = sketchSnap([circle, arc], { x: 13.1, y: 14.1 }, 10, { grid: false })
		if (sweep > 0) expect(snap).toMatchObject({ kind: "intersection", position: { x: 13, y: 14 } })
		else expect(snap).toBeNull()
	}
})
it("does not invent isolated intersections for overlapping collinear lines", () => {
	const a: SketchEntity = { id: "a", type: "line", p0: { x: 10, y: 10 }, p1: { x: 30, y: 10 } }
	const b: SketchEntity = { ...a, id: "b", p0: { x: 15, y: 10 } }
	expect(sketchSnap([a, b], { x: 18, y: 10 }, 10, { grid: false })).toBeNull()
})
it("snaps to line and signed-arc midpoints while respecting endpoint-only and geometry controls", () => {
	const line: SketchEntity = { id: "line", type: "line", p0: { x: 10, y: 10 }, p1: { x: 30, y: 20 } }
	expect(sketchSnap([line], { x: 20.1, y: 15.1 }, 10, { grid: false })).toMatchObject({ kind: "midpoint", curve: "line", position: { x: 20, y: 15 } })
	expect(sketchSnap([line], { x: 20.1, y: 15.1 }, 10, { endpointsOnly: true })).toBeNull()
	expect(sketchSnap([line], { x: 20.1, y: 15.1 }, 10, { grid: false, geometry: false })).toBeNull()
	for (const sweep of [Math.PI, -Math.PI]) {
		const arc: SketchEntity = { id: "arc", type: "arc", center: { x: 10, y: 10 }, radius: 5, startAngle: 0, sweep, segments: 8 }
		const y = 10 + Math.sign(sweep) * 5
		const snap = sketchSnap([arc], { x: 10.1, y: y + 0.1 }, 10, { grid: false })
		expect(snap?.kind).toBe("midpoint")
		expect(snap?.position.x).toBeCloseTo(10, 8)
		expect(snap?.position.y).toBeCloseTo(y, 8)
	}
})

it("snaps to finite elliptic parameter midpoints with a persistent curve reference", async () => {
	const { ellipticArcPoint } = await import("../src/sketch-elliptic-arc")
	for (const sign of [-1, 1]) {
		const arc: import("../src/sketch-elliptic-arc").EllipticArc = {
			id: "elliptic",
			type: "ellipticArc",
			center: { x: 30, y: 40 },
			width: 40,
			height: 20,
			rotation: 25,
			startAngle: 0.3,
			sweep: sign * 1.5,
			segments: 64
		}
		const point = ellipticArcPoint(arc, 0.5)
		const raw = { x: point.x + 0.02, y: point.y + 0.02 }
		expect(sketchSnap([arc], raw, 100, { grid: false })).toEqual({ kind: "midpoint", curve: arc.id, position: point })
		expect(sketchSnap([arc], raw, 100, { grid: false, endpointsOnly: true })).toBeNull()
		expect(sketchSnap([arc], raw, 100, { grid: false, geometry: false })).toBeNull()
	}
})
