import { entityAnchorPoint } from "./sketch-curves"
import type { Sketch, SketchEntity } from "./schema"
import type { Point2D } from "./types"
import type { SketchAnchor, SketchRelation } from "./sketch-solver"
import { sketchRelationEntityIds } from "./sketch-relations"
import { requireValue } from "./required"

export type CircularPatternDefinition = { type: "circularPattern"; sources: string[]; instances: string[][]; center: Point2D; centerAnchor?: SketchAnchor; angle: number }
export function circularPatternCenter(pattern: CircularPatternDefinition, entities: readonly SketchEntity[]): Point2D {
	if (!pattern.centerAnchor) return { ...pattern.center }
	if (pattern.instances.flat().includes(pattern.centerAnchor.entityId)) throw Error("A pattern center cannot reference its own copy.")
	const entity = requireValue(entities.find((e) => e.id === pattern.centerAnchor?.entityId))
	return entityAnchorPoint(entity, pattern.centerAnchor.point)
}
export function setCircularPatternCenter(input: Sketch, id: string, anchor: SketchAnchor | null) {
	const sketch = structuredClone(input)
	const relation = sketch.relations?.find((r) => r.id === id)
	if (relation?.type !== "circularPattern") throw Error("Missing circular pattern.")
	const current = circularPatternCenter(relation, sketch.entities)
	relation.centerAnchor = anchor ? { ...anchor } : undefined
	relation.center = current
	const center = circularPatternCenter(relation, sketch.entities)
	return resizeCircularSketchPattern(sketch, id, relation.instances.length + 1, center, relation.angle)
}
/** Pointer angles are radians; retain the sweep when crossing the atan2 seam. */
export function circularPatternDragAngle(angle: number, previous: number, current: number): number {
	const delta = (Math.atan2(Math.sin(current - previous), Math.cos(current - previous)) * 180) / Math.PI
	return Math.max(-360, Math.min(360, angle + delta))
}
/** Full circles exclude the duplicate end instance; open patterns include both ends. */
export function circularPatternAngle(pattern: CircularPatternDefinition, index: number): number {
	const count = pattern.instances.length + 1
	return (pattern.angle * index) / (Math.abs(pattern.angle) === 360 ? count : count - 1)
}
export function rotateSketchEntity(source: SketchEntity, id: string, center: Point2D, degrees: number): SketchEntity {
	const angle = (degrees * Math.PI) / 180
	const c = Math.cos(angle)
	const s = Math.sin(angle)
	const rotate = (p: Point2D): Point2D => ({ x: center.x + c * (p.x - center.x) - s * (p.y - center.y), y: center.y + s * (p.x - center.x) + c * (p.y - center.y) })
	const result = { ...structuredClone(source), id }
	switch (result.type) {
		case "spline":
			return { ...result, points: result.points.map(rotate) }
		case "line":
			return { ...result, p0: rotate(result.p0), p1: rotate(result.p1) }
		case "point":
		case "circle":
			return { ...result, center: rotate(result.center) }
		case "arc":
			return { ...result, center: rotate(result.center), startAngle: result.startAngle + angle }
		case "capsule":
			return { ...result, from: rotate(result.from), to: rotate(result.to) }
		case "polygon":
		case "ellipticArc":
		case "ellipse":
		case "rectangle":
			return { ...result, center: rotate(result.center), rotation: result.rotation + degrees }
		case "cornerRectangle":
			return {
				id,
				type: "rectangle",
				center: rotate({ x: (result.p0.x + result.p1.x) / 2, y: (result.p0.y + result.p1.y) / 2 }),
				width: Math.abs(result.p1.x - result.p0.x),
				height: Math.abs(result.p1.y - result.p0.y),
				rotation: degrees,
				construction: result.construction
			}
	}
}
function fresh(sketch: Sketch, prefix: string): string {
	const ids = new Set([...sketch.entities, ...(sketch.relations ?? []), ...sketch.dimensions].map((e) => e.id))
	let i = 1
	while (ids.has(`${prefix}-${i}`)) i++
	return `${prefix}-${i}`
}
export function createCircularSketchPattern(input: Sketch, sources: readonly string[], count: number, center: Point2D, angle = 360) {
	if (!sources.length || new Set(sources).size !== sources.length || sources.some((id) => !input.entities.some((e) => e.id === id))) throw Error("Select source geometry for the pattern.")
	const sketch = structuredClone(input)
	const relation: SketchRelation = { id: fresh(sketch, "circular-pattern"), type: "circularPattern", sources: [...sources], instances: [], center: { ...center }, angle }
	sketch.relations ??= []
	sketch.relations.push(relation)
	return resizeCircularSketchPattern(sketch, relation.id, count, center, angle)
}
export function resizeCircularSketchPattern(input: Sketch, id: string, count: number, center: Point2D, angle: number): { sketch: Sketch; patternId: string; removedRelations: string[] } {
	const sketch = structuredClone(input)
	const relation = sketch.relations?.find((r) => r.id === id)
	if (relation?.type !== "circularPattern") throw Error("Missing circular pattern.")
	if (!Number.isInteger(count) || count < 2 || count > 32) throw Error("Pattern count must be an integer from 2 to 32, including the source.")
	if ((count - 1) * relation.sources.length > 64) throw Error("This sketch solver currently supports at most 64 copies per pattern.")
	if (!Number.isFinite(center.x) || !Number.isFinite(center.y)) throw Error("Pattern center must be finite.")
	if (!Number.isFinite(angle) || Math.abs(angle) < 1e-9 || Math.abs(angle) > 360) throw Error("Pattern angle must be nonzero and between -360 and 360 degrees.")
	const removed = new Set(relation.instances.slice(count - 1).flat())
	relation.instances = relation.instances.slice(0, count - 1)
	sketch.entities = sketch.entities.filter((e) => !removed.has(e.id))
	const removedRelations: string[] = []
	sketch.relations = sketch.relations?.filter((r) => {
		const keep = !sketchRelationEntityIds(r).some((id) => removed.has(id))
		if (!keep) removedRelations.push(r.id)
		return keep
	})
	sketch.dimensions = sketch.dimensions.filter((d) => {
		if (!removed.has(d.entityId)) return true
		removedRelations.push(d.id)
		return false
	})
	relation.center = { ...center }
	relation.angle = angle
	const resolvedCenter = circularPatternCenter(relation, sketch.entities)
	relation.center = { ...resolvedCenter }
	while (relation.instances.length < count - 1) {
		const instance: string[] = []
		for (const sourceId of relation.sources) {
			const source = requireValue(sketch.entities.find((e) => e.id === sourceId))
			const copyId = fresh(sketch, "circular-copy")
			sketch.entities.push(rotateSketchEntity(source, copyId, resolvedCenter, 0))
			instance.push(copyId)
		}
		relation.instances.push(instance)
	}
	relation.instances.forEach((instance, index) =>
		instance.forEach((copyId, sourceIndex) => {
			const source = requireValue(sketch.entities.find((e) => e.id === relation.sources[sourceIndex]))
			const targetIndex = sketch.entities.findIndex((e) => e.id === copyId)
			const target = requireValue(sketch.entities[targetIndex])
			sketch.entities[targetIndex] = { ...rotateSketchEntity(source, copyId, resolvedCenter, circularPatternAngle(relation, index + 1)), construction: target.construction }
		})
	)
	sketch.vertices = []
	sketch.loops = []
	sketch.profiles = []
	return { sketch, patternId: id, removedRelations }
}
