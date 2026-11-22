import type { Capsule } from "./schema"
import type { Point2D } from "./types"

/** The first two points are end-cap centers; the third sets the side-wall distance. */
export function threePointSlot(id: string, from: Point2D, to: Point2D, side: Point2D): Capsule {
	const length = Math.hypot(to.x - from.x, to.y - from.y)
	if (!Number.isFinite(length) || length < 1e-9) throw Error("Pick two different slot centers.")
	const width = (2 * Math.abs((to.x - from.x) * (side.y - from.y) - (to.y - from.y) * (side.x - from.x))) / length
	if (!Number.isFinite(width) || width < 1e-9) throw Error("Move away from the centerline to give the slot a width.")
	return { id, type: "capsule", from: { ...from }, to: { ...to }, width, arcSegments: 48 }
}
