import type { SketchEntity } from "./schema"
import type { SketchRelation } from "./sketch-solver"
import { entityAnchorNames, entityAnchorPoint } from "./sketch-curves"

/** Capture an entity's geometry using native point and dimension constraints. */
export function fixedEntityRelations(entity: SketchEntity, prefix: string, existing: readonly SketchRelation[] = []): SketchRelation[] {
	const relations: SketchRelation[] = entityAnchorNames(entity).map((point, index) => ({
		id: `${prefix}-point-${index}`,
		type: "fixed",
		anchor: { entityId: entity.id, point },
		position: { ...entityAnchorPoint(entity, point) }
	}))
	if (entity.type === "circle") relations.push({ id: `${prefix}-radius`, type: "radius", entityId: entity.id, value: entity.radius })
	if (entity.type === "capsule") relations.push({ id: `${prefix}-width`, type: "width", entityId: entity.id, value: entity.width })
	if (entity.type === "ellipticArc") {
		for (const type of ["width", "height", "rotation"] as const) relations.push({ id: `${prefix}-${type}`, type, entityId: entity.id, value: entity[type] })
	}
	return relations.filter(
		(candidate) =>
			!existing.some((relation) => {
				if (relation.reference || relation.type !== candidate.type) return false
				if (candidate.type === "fixed" && relation.type === "fixed")
					return (
						candidate.anchor.entityId === relation.anchor.entityId &&
						candidate.anchor.point === relation.anchor.point &&
						Math.hypot(candidate.position.x - relation.position.x, candidate.position.y - relation.position.y) < 1e-8
					)
				return (
					"entityId" in candidate &&
					"entityId" in relation &&
					"value" in candidate &&
					"value" in relation &&
					candidate.entityId === relation.entityId &&
					Math.abs(candidate.value - relation.value) < 1e-8
				)
			})
	)
}
