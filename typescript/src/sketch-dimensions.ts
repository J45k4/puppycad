import type { SketchEntity } from "./schema"
import type { SketchRelation } from "./sketch-solver"
import { entityAnchorPoint } from "./sketch-curves"
import { requireValue } from "./required"
export function isMeasuredDimension(relation: SketchRelation): boolean {
	return ["length", "diameter", "radius", "width", "height", "arcSweep", "rotation", "distance", "angle", "radiusDifference"].includes(relation.type)
}
export function measureSketchDimension(entities: readonly SketchEntity[], relation: SketchRelation): number {
	const entity = (id: string) => requireValue(entities.find((entity) => entity.id === id))
	const radius = (id: string) => {
		const e = entity(id)
		if (e.type !== "circle" && e.type !== "arc" && e.type !== "polygon") throw Error("A radius requires circular geometry or a polygon.")
		return e.radius
	}
	const lineAngle = (id: string) => {
		const e = entity(id)
		if (e.type !== "line") throw Error("An angle requires lines.")
		return Math.atan2(e.p1.y - e.p0.y, e.p1.x - e.p0.x)
	}
	switch (relation.type) {
		case "distance": {
			const a = entityAnchorPoint(entity(relation.a.entityId), relation.a.point)
			const b = entityAnchorPoint(entity(relation.b.entityId), relation.b.point)
			return relation.direction
				? (b.x - a.x) * relation.direction.x + (b.y - a.y) * relation.direction.y
				: relation.axis
					? b[relation.axis] - a[relation.axis]
					: Math.hypot(b.x - a.x, b.y - a.y)
		}
		case "angle": {
			const angle = lineAngle(relation.b) - lineAngle(relation.a)
			return (Math.atan2(Math.sin(angle), Math.cos(angle)) * 180) / Math.PI
		}
		case "radiusDifference":
			return radius(relation.a) - radius(relation.b)
		case "radius":
			return radius(relation.entityId)
		case "diameter":
			return radius(relation.entityId) * 2
		case "length": {
			const e = entity(relation.entityId)
			if (e.type !== "line") throw Error("Length requires a line.")
			return Math.hypot(e.p1.x - e.p0.x, e.p1.y - e.p0.y)
		}
		case "arcSweep": {
			const e = entity(relation.entityId)
			if (e.type !== "arc" && e.type !== "ellipticArc") throw Error("Sweep requires an arc.")
			return (e.sweep * 180) / Math.PI
		}
		case "rotation": {
			const e = entity(relation.entityId)
			if (e.type === "line") return (Math.atan2(e.p1.y - e.p0.y, e.p1.x - e.p0.x) * 180) / Math.PI
			if (e.type !== "ellipticArc" && e.type !== "ellipse" && e.type !== "rectangle" && e.type !== "polygon") throw Error("Rotation requires a rectangle, polygon or ellipse.")
			return e.rotation
		}
		case "width":
		case "height": {
			const e = entity(relation.entityId)
			if (e.type === "rectangle" || e.type === "ellipse" || e.type === "ellipticArc") return e[relation.type]
			if (e.type === "cornerRectangle") return Math.abs(e.p1[relation.type === "width" ? "x" : "y"] - e.p0[relation.type === "width" ? "x" : "y"])
			if (e.type === "capsule" && relation.type === "width") return e.width
			throw Error("Axis dimension requires a rectangle, ellipse or slot.")
		}
		default:
			throw Error("Only dimensional constraints can be reference dimensions.")
	}
}
