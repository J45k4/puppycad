import { expect, it } from "bun:test"
import { threePointSlot } from "../src/sketch-slot"
import { primitivePoints } from "../src/sketch-primitives"
import { solveSketch } from "../src/sketch-solver"
it("creates a rotated native slot whose width is perpendicular to the centerline", () => {
	const from = { x: 0, y: 0 }
	const to = { x: 30, y: 40 }
	const slot = threePointSlot("slot", from, to, { x: 11, y: 23 })
	expect(slot.width).toBeCloseTo(10, 8)
	expect(slot.from).toEqual(from)
	expect(slot.to).toEqual(to)
	expect(primitivePoints(slot).length).toBeGreaterThan(90)
	const result = solveSketch(
		[slot],
		[
			{ id: "w", type: "width", entityId: "slot", value: 12 },
			{ id: "d", type: "distance", a: { entityId: "slot", point: "from" }, b: { entityId: "slot", point: "to" }, value: 60 }
		]
	)
	expect(result.status).toBe("underconstrained")
	const solved = result.entities[0]
	if (solved?.type !== "capsule") throw Error("Missing native slot")
	expect(solved.width).toBeCloseTo(12, 6)
	expect(Math.hypot(solved.to.x - solved.from.x, solved.to.y - solved.from.y)).toBeCloseTo(60, 6)
	expect(from).toEqual({ x: 0, y: 0 })
})
it("rejects duplicate centers and zero-width slots", () => {
	expect(() => threePointSlot("s", { x: 1, y: 2 }, { x: 1, y: 2 }, { x: 2, y: 3 })).toThrow("different")
	expect(() => threePointSlot("s", { x: 0, y: 0 }, { x: 20, y: 0 }, { x: 10, y: 0 })).toThrow("width")
})
