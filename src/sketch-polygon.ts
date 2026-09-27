import type { Polygon } from "./schema"
import type { Point2D } from "./types"
export function polygonVertices(polygon: Pick<Polygon, "center" | "radius" | "rotation" | "sides">): Point2D[] {
	if (!Number.isInteger(polygon.sides) || polygon.sides < 3 || polygon.sides > 64) throw Error("Polygon sides must be an integer from 3 to 64.")
	if (![polygon.center.x, polygon.center.y, polygon.radius, polygon.rotation].every(Number.isFinite) || polygon.radius <= 0) throw Error("Polygon size must be finite and positive.")
	return Array.from({ length: polygon.sides }, (_, i) => {
		const angle = (polygon.rotation * Math.PI) / 180 + (i * 2 * Math.PI) / polygon.sides
		return { x: polygon.center.x + polygon.radius * Math.cos(angle), y: polygon.center.y + polygon.radius * Math.sin(angle) }
	})
}
export function regularPolygon(id: string, center: Point2D, vertex: Point2D, sides: number, circumscribed = false): Polygon {
	const radius = Math.hypot(vertex.x - center.x, vertex.y - center.y)
	const polygon: Polygon = {
		id,
		type: "polygon",
		center: { ...center },
		radius: circumscribed ? radius / Math.cos(Math.PI / sides) : radius,
		rotation: (Math.atan2(vertex.y - center.y, vertex.x - center.x) * 180) / Math.PI - (circumscribed ? 180 / sides : 0),
		sides
	}
	polygonVertices(polygon)
	return polygon
}

import type { Sketch, SketchEntity } from "./schema"
import type { SketchRelation } from "./sketch-solver"
function copyLinks(relations: readonly SketchRelation[]): [string, string][] {
	return relations.flatMap((r): [string, string][] => {
		if (r.type === "linearPattern" || r.type === "circularPattern") return r.instances.flatMap((group) => group.map((id, index): [string, string] => [r.sources[index] ?? "", id]))
		if (r.type === "mirror" || r.type === "offset") return [[r.a, r.b]]
		return []
	})
}
export function synchronizePolygonSides(entities: SketchEntity[], relations: readonly SketchRelation[]) {
	const byId = new Map(entities.map((entity) => [entity.id, entity]))
	const links = copyLinks(relations)
	for (let pass = 0; pass < entities.length; pass++) {
		let changed = false
		for (const [sourceId, targetId] of links) {
			const source = byId.get(sourceId)
			const target = byId.get(targetId)
			if (source?.type === "polygon" && target?.type === "polygon" && source.sides !== target.sides) {
				target.sides = source.sides
				changed = true
			}
		}
		if (!changed) break
	}
}
export function setPolygonSideCount(input: Sketch, id: string, sides: number) {
	const sketch = structuredClone(input)
	const polygon = sketch.entities.find((entity) => entity.id === id)
	if (polygon?.type !== "polygon") throw Error("Missing polygon.")
	if (copyLinks(sketch.relations ?? []).some(([, target]) => target === id)) throw Error("Edit the source polygon's side count.")
	polygon.sides = sides
	polygonVertices(polygon)
	synchronizePolygonSides(sketch.entities, sketch.relations ?? [])
	const validAnchor = (value: unknown) => {
		if (!value || typeof value !== "object" || !("entityId" in value) || !("point" in value) || typeof value.point !== "string" || !/^vertex\d+$/.test(value.point)) return true
		const entity = sketch.entities.find((entity) => entity.id === value.entityId)
		return entity?.type === "polygon" && Number(value.point.slice(6)) < entity.sides
	}
	const removedRelations: string[] = []
	sketch.relations = sketch.relations?.filter((r) => {
		const keep = Object.values(r).every(validAnchor)
		if (!keep) removedRelations.push(r.id)
		return keep
	})
	sketch.vertices = []
	sketch.loops = []
	sketch.profiles = []
	return { sketch, removedRelations }
}
