import type { Line, SketchEntity } from "./schema"
import type { Point2D } from "./types"

export function mirrorSketchEntity(source: SketchEntity, axis: Line, id: string): SketchEntity {
	const dx = axis.p1.x - axis.p0.x
	const dy = axis.p1.y - axis.p0.y
	const squared = dx * dx + dy * dy
	if (!Number.isFinite(squared) || squared < 1e-18) throw Error("Mirror requires a nonzero axis line.")
	const reflect = (p: Point2D): Point2D => {
		const t = ((p.x - axis.p0.x) * dx + (p.y - axis.p0.y) * dy) / squared
		return { x: 2 * (axis.p0.x + t * dx) - p.x, y: 2 * (axis.p0.y + t * dy) - p.y }
	}
	const angle = Math.atan2(dy, dx)
	const result = { ...structuredClone(source), id }
	switch (result.type) {
		case "spline":
			return { ...result, points: result.points.map(reflect) }
		case "line":
			result.p0 = reflect(result.p0)
			result.p1 = reflect(result.p1)
			return result
		case "point":
		case "circle":
			result.center = reflect(result.center)
			return result
		case "capsule":
			result.from = reflect(result.from)
			result.to = reflect(result.to)
			return result
		case "arc":
			result.center = reflect(result.center)
			result.startAngle = 2 * angle - result.startAngle
			result.sweep = -result.sweep
			return result
		case "ellipticArc":
			return { ...result, center: reflect(result.center), rotation: (2 * angle * 180) / Math.PI - result.rotation, startAngle: -result.startAngle, sweep: -result.sweep }
		case "polygon":
		case "ellipse":
		case "rectangle":
			result.center = reflect(result.center)
			result.rotation = (2 * angle * 180) / Math.PI - result.rotation
			return result
		case "cornerRectangle":
			return {
				id,
				type: "rectangle",
				center: reflect({ x: (result.p0.x + result.p1.x) / 2, y: (result.p0.y + result.p1.y) / 2 }),
				width: Math.abs(result.p1.x - result.p0.x),
				height: Math.abs(result.p1.y - result.p0.y),
				rotation: (2 * angle * 180) / Math.PI,
				construction: result.construction
			}
	}
}
