import { rotateSketchEntity } from "./sketch-circular-pattern"
import { entityAnchorPoint } from "./sketch-curves"
import type { SketchAnchorName } from "./schema"
import type { Sketch, SketchEntity } from "./schema"
import type { Point2D } from "./types"
import { translateSketchEntity } from "./sketch-pattern"
import { sketchRelationEntityIds } from "./sketch-relations"
import { remapSketchRelations } from "./sketch-solver"
import { requireValue } from "./required"

export function scaleSketchEntity(source: SketchEntity, id: string, center: Point2D, factor: number): SketchEntity {
	if (!Number.isFinite(factor) || factor <= 0 || !Number.isFinite(center.x) || !Number.isFinite(center.y)) throw Error("Scale requires a positive finite factor and finite center.")
	const result = structuredClone(source)
	result.id = id
	const point = (p: Point2D) => ({ x: center.x + (p.x - center.x) * factor, y: center.y + (p.y - center.y) * factor })
	if (result.type === "line" || result.type === "cornerRectangle") {
		result.p0 = point(result.p0)
		result.p1 = point(result.p1)
	} else if (result.type === "capsule") {
		result.from = point(result.from)
		result.to = point(result.to)
	} else if (result.type === "spline") result.points = result.points.map(point)
	else result.center = point(result.center)
	if ("radius" in result) result.radius *= factor
	if ("width" in result) result.width *= factor
	if ("height" in result) result.height *= factor
	return result
}
export function translateSketchSelection(input: Sketch, selected: readonly string[], delta: Point2D, copy = false) {
	return transformSketchSelection(input, selected, delta, copy)
}
export function scaleSketchSelection(input: Sketch, selected: readonly string[], center: Point2D, factor: number, copy = false) {
	if (!Number.isFinite(factor) || factor <= 0 || !Number.isFinite(center.x) || !Number.isFinite(center.y)) throw Error("Scale requires a positive finite factor and finite center.")
	if (factor === 1 && !copy) throw Error("Choose a scale factor other than one.")
	return transformSketchSelection(input, selected, { x: 0, y: 0 }, copy, { center, factor })
}
export function rotateSketchSelection(input: Sketch, selected: readonly string[], center: Point2D, degrees: number, copy = false) {
	if (![center.x, center.y, degrees].every(Number.isFinite)) throw Error("Rotation requires finite center coordinates and angle.")
	if (!copy && degrees % 360 === 0) throw Error("Enter a nonzero rotation.")
	return transformSketchSelection(input, selected, { x: 0, y: 0 }, copy, undefined, { center, degrees })
}
/** Transform a group or independent copy, preserving only internal relations. */
function transformSketchSelection(
	input: Sketch,
	selected: readonly string[],
	delta: Point2D,
	copy: boolean,
	scale?: { center: Point2D; factor: number },
	rotation?: { center: Point2D; degrees: number }
): { sketch: Sketch; selected: string[]; removedRelations: string[] } {
	if (!selected.length || new Set(selected).size !== selected.length || selected.some((id) => !input.entities.some((e) => e.id === id))) throw Error("Select existing geometry to transform.")
	if (!Number.isFinite(delta.x) || !Number.isFinite(delta.y)) throw Error("Move distances must be finite.")
	if (!copy && !scale && !rotation && delta.x === 0 && delta.y === 0) throw Error("Enter a nonzero move distance.")
	const factor = scale?.factor ?? 1

	const sketch = structuredClone(input)
	const group = new Set(selected)
	const used = new Set([...sketch.entities, ...(sketch.relations ?? []), ...sketch.dimensions].map((e) => e.id))
	const fresh = (prefix: string) => {
		let n = 1
		while (used.has(`${prefix}-${n}`)) n++
		const id = `${prefix}-${n}`
		used.add(id)
		return id
	}
	const mapping = new Map(selected.map((id) => [id, copy ? fresh(`copy-${id}`) : id]))
	const moved = sketch.entities
		.filter((e) => group.has(e.id))
		.map((e) =>
			rotation
				? rotateSketchEntity(e, requireValue(mapping.get(e.id)), rotation.center, rotation.degrees)
				: scale
					? scaleSketchEntity(e, requireValue(mapping.get(e.id)), scale.center, scale.factor)
					: translateSketchEntity(e, requireValue(mapping.get(e.id)), delta)
		)
	const removedRelations: string[] = []
	const internal = (sketch.relations ?? []).filter((r) => sketchRelationEntityIds(r).every((id) => group.has(id)))
	const anchorNames = new Map<string, Map<SketchAnchorName, SketchAnchorName>>()
	const angle = ((rotation?.degrees ?? 0) * Math.PI) / 180
	const vector = (p: Point2D) => ({ x: p.x * Math.cos(angle) - p.y * Math.sin(angle), y: p.x * Math.sin(angle) + p.y * Math.cos(angle) })
	const rotatePoint = (p: Point2D) => {
		const center = rotation?.center ?? { x: 0, y: 0 }
		const v = vector({ x: p.x - center.x, y: p.y - center.y })
		return { x: center.x + v.x, y: center.y + v.y }
	}
	if (rotation)
		for (const e of sketch.entities.filter((e) => group.has(e.id))) {
			if (e.type !== "cornerRectangle") continue
			const target = requireValue(moved.find((m) => m.id === mapping.get(e.id)))
			const names = new Map<SketchAnchorName, SketchAnchorName>()
			for (const name of ["p0", "p1"] as const) {
				const expected = rotatePoint(e[name])
				const closest = (["p0", "p1", "p2", "p3"] as const)
					.map((point) => ({ point, position: entityAnchorPoint(target, point) }))
					.sort((a, b) => Math.hypot(a.position.x - expected.x, a.position.y - expected.y) - Math.hypot(b.position.x - expected.x, b.position.y - expected.y))[0]
				names.set(name, requireValue(closest).point)
			}
			anchorNames.set(e.id, names)
		}
	const translated = (remapSketchRelations(internal, mapping, anchorNames) ?? []).map((r) => {
		if (rotation && (r.type === "horizontal" || r.type === "vertical")) {
			const line = requireValue(moved.find((e) => e.id === r.entityId))
			if (line.type !== "line") throw Error("Orientation constraints require lines.")
			return { ...r, type: "rotation" as const, value: (Math.atan2(line.p1.y - line.p0.y, line.p1.x - line.p0.x) * 180) / Math.PI }
		}
		if (rotation && r.type === "rotation") {
			r.value += rotation.degrees
			if (rotation.degrees !== 0) r.expression = undefined
		}
		if (rotation && r.type === "distance" && (r.axis || r.direction)) {
			r.direction = vector(r.direction ?? (r.axis === "x" ? { x: 1, y: 0 } : { x: 0, y: 1 }))
			r.axis = undefined
		}
		return r
	})
	const translate = (p: Point2D) =>
		rotation
			? rotatePoint(p)
			: scale
				? { x: scale.center.x + (p.x - scale.center.x) * factor, y: scale.center.y + (p.y - scale.center.y) * factor }
				: { x: p.x + delta.x, y: p.y + delta.y }
	for (const relation of translated) {
		if (copy) relation.id = fresh("copy-constraint")
		if (relation.type === "fixed") relation.position = translate(relation.position)
		if (relation.type === "circularPattern") relation.center = translate(relation.center)
		if (relation.labelPosition) relation.labelPosition = translate(relation.labelPosition)
		if ("value" in relation && !["angle", "rotation", "arcSweep"].includes(relation.type)) {
			relation.value *= factor
			if (factor !== 1) relation.expression = undefined
		}
		if (relation.type === "chamfer" && relation.mode !== "distance-angle") relation.secondValue *= factor
		if (relation.type === "linearPattern") {
			relation.step = vector({ x: relation.step.x * factor, y: relation.step.y * factor })
			if (relation.rowStep) relation.rowStep = vector({ x: relation.rowStep.x * factor, y: relation.rowStep.y * factor })
		}
	}
	if (rotation) for (const [id] of anchorNames) translated.push({ id: fresh("rotation"), type: "rotation", entityId: requireValue(mapping.get(id)), value: rotation.degrees })
	if (copy) {
		sketch.entities.push(...moved)
		sketch.relations = [...(sketch.relations ?? []), ...translated]
		sketch.dimensions.push(
			...sketch.dimensions
				.filter((d) => group.has(d.entityId))
				.map((d) => ({ ...d, id: fresh("copy-dimension"), entityId: requireValue(mapping.get(d.entityId)), value: d.value * factor }))
		)
	} else {
		sketch.dimensions = sketch.dimensions.map((d) => (group.has(d.entityId) ? { ...d, value: d.value * factor } : d))
		const movedById = new Map(moved.map((e) => [e.id, e]))
		sketch.entities = sketch.entities.map((e) => movedById.get(e.id) ?? e)
		const internalById = new Map(translated.map((r) => [r.id, r]))
		sketch.relations = (sketch.relations ?? []).flatMap((r) => {
			const replacement = internalById.get(r.id)
			if (replacement) return [replacement]
			if (sketchRelationEntityIds(r).some((id) => group.has(id))) {
				removedRelations.push(r.id)
				return []
			}
			return [r]
		})
		const existing = new Set(sketch.relations.map((r) => r.id))
		sketch.relations.push(...translated.filter((r) => !existing.has(r.id)))
	}
	sketch.vertices = []
	sketch.loops = []
	sketch.profiles = []
	return { sketch, selected: selected.map((id) => requireValue(mapping.get(id))), removedRelations }
}
