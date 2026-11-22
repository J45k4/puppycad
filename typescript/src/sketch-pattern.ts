import type { Sketch, SketchEntity } from "./schema"
import type { Point2D } from "./types"
import type { SketchRelation } from "./sketch-solver"
import { sketchRelationEntityIds } from "./sketch-relations"
import { requireValue } from "./required"

export type LinearPatternDefinition = { type: "linearPattern"; sources: string[]; instances: string[][]; step: Point2D; columns?: number; rowStep?: Point2D }
export function patternLayout(pattern: LinearPatternDefinition) {
	const columns = pattern.columns ?? pattern.instances.length + 1
	return { columns, rows: (pattern.instances.length + 1) / columns, rowStep: pattern.rowStep ?? { x: 0, y: 20 } }
}
export function patternCellOffset(pattern: LinearPatternDefinition, index: number): Point2D {
	const { columns, rowStep } = patternLayout(pattern)
	return { x: pattern.step.x * (index % columns) + rowStep.x * Math.floor(index / columns), y: pattern.step.y * (index % columns) + rowStep.y * Math.floor(index / columns) }
}
export function translateSketchEntity(source: SketchEntity, id: string, step: Point2D): SketchEntity {
	const result = structuredClone(source)
	result.id = id
	const translate = (p: Point2D) => {
		p.x += step.x
		p.y += step.y
	}
	if (result.type === "line" || result.type === "cornerRectangle") {
		translate(result.p0)
		translate(result.p1)
	} else if (result.type === "capsule") {
		translate(result.from)
		translate(result.to)
	} else if (result.type === "spline") result.points.forEach(translate)
	else translate(result.center)
	return result
}
function fresh(sketch: Sketch, prefix: string): string {
	const ids = new Set([...sketch.entities, ...(sketch.relations ?? []), ...sketch.dimensions].map((e) => e.id))
	let i = 1
	while (ids.has(`${prefix}-${i}`)) i++
	return `${prefix}-${i}`
}
export function createLinearSketchPattern(input: Sketch, sources: readonly string[], count: number, step: Point2D, rows = 1, rowStep: Point2D = { x: 0, y: 20 }) {
	if (!sources.length || new Set(sources).size !== sources.length || sources.some((id) => !input.entities.some((e) => e.id === id))) throw Error("Select source geometry for the pattern.")
	const sketch = structuredClone(input)
	const relation: SketchRelation = { id: fresh(sketch, "linear-pattern"), type: "linearPattern", sources: [...sources], instances: [], step: { ...step } }
	sketch.relations ??= []
	sketch.relations.push(relation)
	return resizeLinearSketchPattern(sketch, relation.id, count, step, rows, rowStep)
}
export function resizeLinearSketchPattern(
	input: Sketch,
	id: string,
	count: number,
	step: Point2D,
	rows = 1,
	rowStep: Point2D = { x: 0, y: 20 }
): { sketch: Sketch; patternId: string; removedRelations: string[] } {
	const sketch = structuredClone(input)
	const relation = sketch.relations?.find((r) => r.id === id)
	if (relation?.type !== "linearPattern") throw Error("Missing linear pattern.")
	if (!Number.isInteger(count) || !Number.isInteger(rows) || count < 1 || rows < 1 || count * rows < 2 || count * rows > 32)
		throw Error("Pattern columns and rows must be positive integers totaling 2 to 32 instances.")
	if ((count * rows - 1) * relation.sources.length > 64) throw Error("This sketch solver currently supports at most 64 copies per pattern.")
	if (!Number.isFinite(step.x) || !Number.isFinite(step.y) || Math.hypot(step.x, step.y) < 1e-9) throw Error("Pattern step must be finite and nonzero.")
	if (!Number.isFinite(rowStep.x) || !Number.isFinite(rowStep.y) || Math.hypot(rowStep.x, rowStep.y) < 1e-9) throw Error("Row step must be finite and nonzero.")
	const oldColumns = patternLayout(relation).columns
	const oldInstances = relation.instances
	const cells = new Map(oldInstances.map((group, index) => [`${Math.floor((index + 1) / oldColumns)},${(index + 1) % oldColumns}`, group]))
	relation.columns = count
	relation.step = { ...step }
	relation.rowStep = { ...rowStep }
	relation.instances = []
	for (let row = 0; row < rows; row++)
		for (let col = 0; col < count; col++) {
			if (!row && !col) continue
			const existing = cells.get(`${row},${col}`)
			if (existing) relation.instances.push(existing)
			else {
				const group: string[] = []
				for (const sourceId of relation.sources) {
					const source = requireValue(sketch.entities.find((e) => e.id === sourceId))
					const copyId = fresh(sketch, "pattern-copy")
					sketch.entities.push(translateSketchEntity(source, copyId, patternCellOffset(relation, row * count + col)))
					group.push(copyId)
				}
				relation.instances.push(group)
			}
		}
	const retained = new Set(relation.instances.flat())
	const removed = new Set(oldInstances.flat().filter((id) => !retained.has(id)))
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
	relation.instances.forEach((instance, index) =>
		instance.forEach((id, sourceIndex) => {
			const source = requireValue(sketch.entities.find((e) => e.id === relation.sources[sourceIndex]))
			const targetIndex = sketch.entities.findIndex((e) => e.id === id)
			const target = requireValue(sketch.entities[targetIndex])
			sketch.entities[targetIndex] = { ...translateSketchEntity(source, id, patternCellOffset(relation, index + 1)), construction: target.construction }
		})
	)
	sketch.vertices = []
	sketch.loops = []
	sketch.profiles = []
	return { sketch, patternId: id, removedRelations }
}
