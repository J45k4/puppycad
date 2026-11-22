import type { SketchEntity } from "./schema"

/** Positive distances expand closed shapes; lines move to their directed left side. */
export function offsetSketchEntity(source: SketchEntity, distance: number, id: string, validate = true): SketchEntity {
	const result = structuredClone(source)
	result.id = id
	if (!Number.isFinite(distance)) throw Error("Offset distance must be finite.")
	switch (result.type) {
		case "spline":
			throw Error("Spline offset is not supported yet.")
		case "ellipticArc":
		case "ellipse":
			throw Error("Ellipse offset requires a general offset curve and is not supported yet.")
		case "polygon":
			result.radius += distance / Math.cos(Math.PI / result.sides)
			if (validate && result.radius <= 0) throw Error("Offset would collapse the polygon.")
			break
		case "point":
			throw Error("A sketch point has no offset curve.")
		case "circle":
		case "arc":
			result.radius += distance
			if (validate && result.radius <= 0) throw Error("Offset radius must stay positive.")
			break
		case "rectangle":
			result.width += 2 * distance
			result.height += 2 * distance
			if (validate && Math.min(result.width, result.height) <= 0) throw Error("Offset would collapse the rectangle.")
			break
		case "capsule":
			result.width += 2 * distance
			if (validate && result.width <= 0) throw Error("Offset would collapse the slot.")
			break
		case "cornerRectangle": {
			const sx = Math.sign(result.p1.x - result.p0.x)
			const sy = Math.sign(result.p1.y - result.p0.y)
			if (validate && (Math.abs(result.p1.x - result.p0.x) + 2 * distance <= 0 || Math.abs(result.p1.y - result.p0.y) + 2 * distance <= 0))
				throw Error("Offset would collapse the rectangle.")
			result.p0.x -= sx * distance
			result.p1.x += sx * distance
			result.p0.y -= sy * distance
			result.p1.y += sy * distance
			break
		}
		case "line": {
			const dx = result.p1.x - result.p0.x
			const dy = result.p1.y - result.p0.y
			const length = Math.hypot(dx, dy)
			if (length < 1e-10) throw Error("Cannot offset a zero-length line.")
			for (const p of [result.p0, result.p1]) {
				p.x -= (dy / length) * distance
				p.y += (dx / length) * distance
			}
		}
	}
	return result
}

export function offsetParameters(e: SketchEntity): number[] {
	if (e.type === "ellipticArc") return [e.center.x, e.center.y, e.width, e.height, e.rotation, e.startAngle, e.sweep]
	switch (e.type) {
		case "spline":
			return e.points.flatMap((p) => [p.x, p.y])
		case "polygon":
			return [e.center.x, e.center.y, e.radius, e.rotation]
		case "point":
			return [e.center.x, e.center.y]
		case "line":
		case "cornerRectangle":
			return [e.p0.x, e.p0.y, e.p1.x, e.p1.y]
		case "circle":
			return [e.center.x, e.center.y, e.radius]
		case "arc":
			return [e.center.x, e.center.y, e.radius, e.startAngle, e.sweep]
		case "ellipse":
		case "rectangle":
			return [e.center.x, e.center.y, e.width, e.height, e.rotation]
		case "capsule":
			return [e.from.x, e.from.y, e.to.x, e.to.y, e.width]
	}
}
