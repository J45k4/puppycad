import type { SketchRelation } from "./sketch-solver"

export function sketchRelationEntityIds(relation: SketchRelation): string[] {
	if (relation.type === "circularPattern") return [...relation.sources, ...relation.instances.flat(), ...(relation.centerAnchor ? [relation.centerAnchor.entityId] : [])]
	if (relation.type === "linearPattern") return [...relation.sources, ...relation.instances.flat()]
	if (relation.type === "offsetChain") return [...relation.sources.map((s) => s.entityId), ...relation.targets]
	if ("entityId" in relation) return [relation.entityId]
	if (relation.type === "fixed") return [relation.anchor.entityId]
	const ids = [typeof relation.a === "string" ? relation.a : relation.a.entityId, typeof relation.b === "string" ? relation.b : relation.b.entityId]
	if (relation.type === "symmetric" || relation.type === "mirror") ids.push(relation.symmetryLine)
	if (relation.type === "chamfer") ids.push(relation.chamferId)
	return ids
}
