import type { Rectangle } from "./schema"
import type { Point2D } from "./types"

export function centerRectangle(id: string, center: Point2D, corner: Point2D): Rectangle {
	const width = 2 * Math.abs(corner.x - center.x)
	const height = 2 * Math.abs(corner.y - center.y)
	if (!Number.isFinite(width + height) || Math.min(width, height) < 1e-9) throw Error("Choose a corner with nonzero width and height.")
	return { id, type: "rectangle", center: { ...center }, width, height, rotation: 0 }
}
export function threePointRectangle(id: string, first: Point2D, second: Point2D, side: Point2D): Rectangle {
	const dx = second.x - first.x
	const dy = second.y - first.y
	const width = Math.hypot(dx, dy)
	if (!Number.isFinite(width) || width < 1e-9) throw Error("Choose two different points for the first edge.")
	const signedHeight = (dx * (side.y - first.y) - dy * (side.x - first.x)) / width
	if (!Number.isFinite(signedHeight) || Math.abs(signedHeight) < 1e-9) throw Error("Move away from the first edge to set the height.")
	return {
		id,
		type: "rectangle",
		center: { x: (first.x + second.x) / 2 - ((dy / width) * signedHeight) / 2, y: (first.y + second.y) / 2 + ((dx / width) * signedHeight) / 2 },
		width,
		height: Math.abs(signedHeight),
		rotation: (Math.atan2(dy, dx) * 180) / Math.PI
	}
}
/** Center, midpoint of a side, then a point fixing the perpendicular half-height. */
export function threePointCenterRectangle(id: string, center: Point2D, sideMidpoint: Point2D, adjacentSide: Point2D): Rectangle {
	const dx = sideMidpoint.x - center.x
	const dy = sideMidpoint.y - center.y
	const halfWidth = Math.hypot(dx, dy)
	if (!Number.isFinite(halfWidth) || halfWidth < 1e-9) throw Error("Choose a side midpoint away from the center.")
	const halfHeight = Math.abs((dx * (adjacentSide.y - center.y) - dy * (adjacentSide.x - center.x)) / halfWidth)
	if (!Number.isFinite(halfHeight) || halfHeight < 1e-9) throw Error("Move away from the center axis to set the height.")
	return { id, type: "rectangle", center: { ...center }, width: 2 * halfWidth, height: 2 * halfHeight, rotation: (Math.atan2(dy, dx) * 180) / Math.PI }
}
