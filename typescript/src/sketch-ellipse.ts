import type { Ellipse } from "./schema"
import type { Point2D } from "./types"
export function ellipsePoint(ellipse: Pick<Ellipse, "center" | "width" | "height" | "rotation">, angle: number): Point2D {
	const rotation = (ellipse.rotation * Math.PI) / 180
	const x = (ellipse.width / 2) * Math.cos(angle)
	const y = (ellipse.height / 2) * Math.sin(angle)
	return { x: ellipse.center.x + x * Math.cos(rotation) - y * Math.sin(rotation), y: ellipse.center.y + x * Math.sin(rotation) + y * Math.cos(rotation) }
}
export function ellipsePoints(ellipse: Omit<Ellipse, "id">): Point2D[] {
	if (
		![ellipse.center.x, ellipse.center.y, ellipse.width, ellipse.height, ellipse.rotation].every(Number.isFinite) ||
		ellipse.width <= 0 ||
		ellipse.height <= 0 ||
		!Number.isInteger(ellipse.segments) ||
		ellipse.segments < 8 ||
		ellipse.segments > 4096
	)
		throw Error("Ellipse axes must be finite and positive, with 8–4096 segments.")
	return Array.from({ length: ellipse.segments }, (_, i) => ellipsePoint(ellipse, (i * 2 * Math.PI) / ellipse.segments))
}
export function threePointEllipse(id: string, center: Point2D, firstAxis: Point2D, secondAxis: Point2D): Ellipse {
	const dx = firstAxis.x - center.x
	const dy = firstAxis.y - center.y
	const radius = Math.hypot(dx, dy)
	if (radius < 1e-9) throw Error("Choose a first axis endpoint away from the center.")
	const height = 2 * Math.abs(((secondAxis.x - center.x) * -dy + (secondAxis.y - center.y) * dx) / radius)
	const ellipse: Ellipse = { id, type: "ellipse", center: { ...center }, width: 2 * radius, height, rotation: (Math.atan2(dy, dx) * 180) / Math.PI, segments: 128 }
	ellipsePoints(ellipse)
	return ellipse
}

/** Exact implicit ellipse equation, scaled to sketch distance units. */
export function pointOnEllipseResidual(ellipse: Pick<Ellipse, "center" | "width" | "height" | "rotation">, point: Point2D): number {
	const angle = (ellipse.rotation * Math.PI) / 180
	const dx = point.x - ellipse.center.x
	const dy = point.y - ellipse.center.y
	const x = (dx * Math.cos(angle) + dy * Math.sin(angle)) / (ellipse.width / 2)
	const y = (-dx * Math.sin(angle) + dy * Math.cos(angle)) / (ellipse.height / 2)
	return ((Math.hypot(x, y) - 1) * Math.min(ellipse.width, ellipse.height)) / 2
}
/** Point on the analytic ellipse furthest along a nonzero world direction. */
export function ellipseSupportPoint(ellipse: Pick<Ellipse, "center" | "width" | "height" | "rotation">, direction: Point2D): Point2D {
	const angle = (ellipse.rotation * Math.PI) / 180
	const nx = direction.x * Math.cos(angle) + direction.y * Math.sin(angle)
	const ny = -direction.x * Math.sin(angle) + direction.y * Math.cos(angle)
	const a = ellipse.width / 2
	const b = ellipse.height / 2
	const h = Math.hypot(a * nx, b * ny)
	if (h < 1e-12) throw Error("Ellipse tangent direction must be nonzero.")
	const x = (a * a * nx) / h
	const y = (b * b * ny) / h
	return { x: ellipse.center.x + x * Math.cos(angle) - y * Math.sin(angle), y: ellipse.center.y + x * Math.sin(angle) + y * Math.cos(angle) }
}
