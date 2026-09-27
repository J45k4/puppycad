import { expect, it } from "bun:test"
import { PartBuilder, circle, capsule, rectangle, v2 } from "../src/sdk"
import { createProjectFile, normalizeProjectFile, serializeProjectFile } from "../src/project-file"
import { createPartRuntimeState, materializePartFeatures, serializePCadState } from "../src/pcad/part-state"
import { validatePCadState } from "../src/pcad/runtime"
import { primitivePoints, normalizePrimitive } from "../src/sketch-primitives"
import { replaceStepLoops, stepLoops } from "../src/solid-edit"
import { requireValue } from "../src/required"
import { pickSolidSketchSource } from "../src/ui/solid-source"
import { Vector3 } from "three"
import type { PartDocument } from "../src/schema"
import { exportPartStl } from "../src/part-mesh"

function part(): PartDocument {
	const builder = new PartBuilder()
	builder.extrude("ring", { outline: circle(v2(0, 0), 10, 64), holes: [circle(v2(0, 0), 5, 64)], depth: 3 })
	builder.extrude("arm", { outline: capsule(v2(0, 0), v2(20, 0), 4, 16), depth: 3 })
	builder.extrude("foot", { outline: rectangle(v2(20, 0), 5, 6, 15), depth: 2 })
	return builder.document
}
it("preserves primitive parameters through project JSON and graph materialization", () => {
	const doc = part()
	const runtime = createPartRuntimeState(doc)
	validatePCadState(runtime.cad)
	doc.cad = serializePCadState(runtime.cad)
	doc.tree = runtime.tree
	const file = createProjectFile({ items: [{ id: "p", name: "Part", type: "part", data: doc }], selectedPath: null })
	const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
	const node = restored.items[0]
	if (!node || !("type" in node) || node.type !== "part") throw Error("Missing part")
	const data = requireValue((node as import("../src/contract").ProjectPartDocument).data)
	expect(
		data.features
			.filter((f) => f.type === "sketch")
			.flatMap((f) => f.entities)
			.map((e) => e.type)
	).toEqual(["circle", "circle", "capsule", "rectangle"])
	const reopened = createPartRuntimeState(data)
	validatePCadState(reopened.cad)
	expect(
		materializePartFeatures(reopened.cad, reopened.tree)
			.filter((f) => f.type === "sketch")
			.flatMap((f) => f.entities)
	).toEqual(doc.features.filter((f) => f.type === "sketch").flatMap((f) => f.entities))
	expect(exportPartStl(data)).toBe(exportPartStl(doc))
})
it("applying cached profile loops does not downgrade primitives, and source picking resolves the circle ID", () => {
	const doc = part()
	const step = requireValue(doc.solidSteps?.[0])
	const entities = structuredClone(doc.features[0])
	replaceStepLoops(doc, step, stepLoops(doc, step))
	expect(doc.features[0]).toEqual(entities)
	const source = pickSolidSketchSource(doc, new Vector3(-5, 0, 3), 0.01, "ring")
	expect(source?.entityId).toBe("ring/sketch/1")
})
it("rejects invalid primitive dimensions and samples a closed circle at its requested resolution", () => {
	expect(normalizePrimitive({ type: "circle", center: { x: 0, y: 0 }, radius: -2, segments: 64 }, "c")).toBeUndefined()
	expect(normalizePrimitive({ type: "circle", center: { x: Number.NaN, y: 0 }, radius: 2, segments: 64 }, "c")).toBeUndefined()
	expect(normalizePrimitive({ type: "circle", center: { x: 0, y: 0 }, radius: 2, segments: 2 }, "c")).toBeUndefined()
	const points = primitivePoints(circle(v2(2, 3), 5, 64))
	expect(points).toHaveLength(64)
	for (const p of points) expect(Math.hypot(p.x - 2, p.y - 3)).toBeCloseTo(5)
})

it("migrates known outlines without changing cuts or dimensions and refuses changed source geometry", async () => {
	const { migrateFlowerHolderPart } = await import("../examples/migrate-flower-holder-primitives")
	const { outlineEntities } = await import("../src/sketch-primitives")
	const reference = part()
	const legacy = structuredClone(reference)
	for (const feature of legacy.features)
		if (feature.type === "sketch")
			feature.entities = feature.entities.flatMap((e) => (e.type === "circle" || e.type === "capsule" || e.type === "rectangle" ? outlineEntities(primitivePoints(e), e.id) : [e]))
	const before = structuredClone(legacy)
	const migrated = migrateFlowerHolderPart(legacy, reference)
	expect(migrated.converted).toBe(4)
	expect(legacy).toEqual(before)
	expect(migrated.document.solidSteps).toEqual(legacy.solidSteps)
	expect(exportPartStl(migrated.document)).toBe(exportPartStl(reference))
	expect(migrateFlowerHolderPart(migrated.document, reference).converted).toBe(0)
	const sketch = legacy.features[0]
	if (sketch?.type !== "sketch" || sketch.entities[0]?.type !== "line") throw Error("Missing legacy sketch")
	sketch.entities[0].p0.x += 0.1
	expect(() => migrateFlowerHolderPart(legacy, reference)).toThrow()
})
