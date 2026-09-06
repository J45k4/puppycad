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
		entities: loops.flatMap((loop, l) => loop.map((p, i) => ({ type: "line" as const, id: `${old.id}/${l}/${i}`, p0: { ...p }, p1: { ...requireValue(loop[(i + 1) % loop.length]) } })))
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
