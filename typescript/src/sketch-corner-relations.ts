import type { Sketch } from "./schema"
import type { SketchAnchor } from "./sketch-solver"

export function removeCornerRelations(sketch: Sketch, firstId: string, secondId: string, pa: "p0" | "p1", pb: "p0" | "p1"): string[] {
	const cut = (anchor: SketchAnchor) => (anchor.entityId === firstId && anchor.point === pa) || (anchor.entityId === secondId && anchor.point === pb)
	const changed = (id: string) => id === firstId || id === secondId
	const removedRelations: string[] = []
	sketch.relations = (sketch.relations ?? []).filter((r) => {
		let remove = false
		if (r.type === "fixed") remove = cut(r.anchor)
		else if (r.type === "linearPattern" || r.type === "circularPattern")
			remove = r.sources.some(changed) || r.instances.flat().some(changed) || (r.type === "circularPattern" && !!r.centerAnchor && cut(r.centerAnchor))
		else if (r.type === "offsetChain") remove = r.sources.some((s) => changed(s.entityId)) || r.targets.some(changed)
		else if ("entityId" in r) remove = r.type === "length" && !r.reference && changed(r.entityId)
		else if ("a" in r) {
			if (typeof r.a !== "string") remove = cut(r.a)
			if (typeof r.b !== "string") remove = remove || cut(r.b)
			if ((r.type === "offset" || r.type === "mirror") && (changed(r.a) || changed(r.b))) remove = true
		}
		if (r.type === "chamfer" && changed(r.chamferId)) remove = true
		if (remove) removedRelations.push(r.id)
		return !remove
	})
	sketch.dimensions = sketch.dimensions.filter((d) => {
		if (!changed(d.entityId)) return true
		removedRelations.push(d.id)
		return false
	})
	return removedRelations
}
