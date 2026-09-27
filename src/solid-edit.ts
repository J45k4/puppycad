import { isSketchPrimitive, primitivePoints, outlineEntities, type SketchOutline } from "./sketch-primitives"
import type { SketchPrimitive } from "./schema"
import type { PartDocument, Sketch, SolidExtrude } from "./schema"
import type { Point2D } from "./types"
import type { SolidStep } from "./solid-model"
import { extrudeSolidFeature } from "./cad/extrude"
import { materializeSketch } from "./cad/sketch"
import { requireValue } from "./required"
import { createPartGeometries } from "./part-mesh"
export function extrusionFor(document: PartDocument, step: SolidStep): SolidExtrude {
	const feature = document.features.find((f) => f.type === "extrude" && f.id === step.featureId)
	if (!feature || feature.type !== "extrude") throw Error(`Missing extrusion ${step.id}`)
	return feature
}
export function sketchFor(document: PartDocument, step: SolidStep): Sketch {
	const feature = extrusionFor(document, step)
	const sketch = document.features.find((f) => f.type === "sketch" && f.id === feature.target.sketchId)
	if (!sketch || sketch.type !== "sketch") throw Error(`Missing sketch ${step.id}`)
	return sketch
}
export function stepLoops(document: PartDocument, step: SolidStep): Point2D[][] {
	if (step.type === "loft" || step.type === "shell") return []
	return step.type === "revolve" ? [structuredClone(requireValue(step.outline))] : extrudeSolidFeature(document, extrusionFor(document, step)).profileLoops
}
export function replaceStepLoops(document: PartDocument, step: SolidStep, loops: Point2D[][]): void {
	if (step.type === "revolve") {
		step.outline = structuredClone(requireValue(loops[0]))
		return
	}
	const old = sketchFor(document, step)
	const feature = extrusionFor(document, step)
	const sketch = materializeSketch({
		...old,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: [],
		entities: loops.flatMap((loop, l) => {
			const primitive = primitiveForLoop(document, step, loop)
			return primitive ? [structuredClone(primitive)] : outlineEntities(loop, `${old.id}/${l}`)
		})
	})
	const profile = sketch.profiles.find((p) => p.holeLoopIds.length === loops.length - 1)
	if (!profile) throw Error("The outline and holes must form one closed profile.")
	document.features[document.features.indexOf(old)] = sketch
	feature.target.profileId = profile.id
	document.cad = undefined
	document.tree = undefined
}
export function validateSolidEdit(document: PartDocument): void {
	const ids = new Set<string>()
	for (const step of document.solidSteps ?? []) {
		if (!step.id.trim() || ids.has(step.id)) throw Error("Feature names must be unique and nonempty.")
		ids.add(step.id)
	}
	const geometries = createPartGeometries(document)
	try {
		if (!geometries.length || geometries.every((g) => g.getAttribute("position").count === 0)) throw Error("These features leave no solid.")
	} finally {
		for (const geometry of geometries) geometry.dispose()
	}
}

export function primitiveForLoop(document: PartDocument, step: SolidStep, points: readonly Point2D[]): SketchPrimitive | undefined {
	if (step.type !== "extrusion") return
	return sketchFor(document, step).entities.find((entity): entity is SketchPrimitive => isSketchPrimitive(entity) && sameLoop(primitivePoints(entity), points))
}
export function sameLoop(a: readonly Point2D[], b: readonly Point2D[]): boolean {
	if (a.length !== b.length || !a[0]) return false
	const same = (p: Point2D | undefined, q: Point2D | undefined) => p && q && Math.hypot(p.x - q.x, p.y - q.y) < 1e-6
	const start = b.findIndex((p) => same(a[0], p))
	return start >= 0 && [1, -1].some((direction) => a.every((p, i) => same(p, b[(start + direction * i + b.length) % b.length])))
}
export function replaceStepOutline(document: PartDocument, step: SolidStep, loops: Point2D[][], index: number, outline: SketchOutline): void {
	replaceStepLoops(document, step, loops)
	if (step.type === "revolve") throw Error("Primitive replacement requires an extrusion sketch")
	const old = sketchFor(document, step)
	const entities = loops.flatMap((points, i) =>
		i === index
			? outlineEntities(outline, `${old.id}/${i}`)
			: primitiveForLoop(document, step, points)
				? [structuredClone(requireValue(primitiveForLoop(document, step, points)))]
				: outlineEntities(points, `${old.id}/${i}`)
	)
	const next = materializeSketch({ ...old, entities })
	const profile = next.profiles.find((p) => p.holeLoopIds.length === loops.length - 1)
	if (!profile) throw Error("The outline and holes must form one closed profile.")
	document.features[document.features.indexOf(old)] = next
	extrusionFor(document, step).target.profileId = profile.id
}
