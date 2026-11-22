import { PartBuilder } from "../src/sdk"
import { flowerHolderParts } from "./flower-holder-parts"
import { isSketchPrimitive, primitivePoints } from "../src/sketch-primitives"
import { sameLoop, stepLoops, sketchFor } from "../src/solid-edit"
import { materializeSketch } from "../src/cad/sketch"
import { createPartGeometries } from "../src/part-mesh"
import type { PartDocument, SketchEntity } from "../src/schema"
import type { Project, ProjectNode, ProjectPartDocument } from "../src/contract"
import type { SyncedProjectCommand } from "../src/project-commands"
import { requireValue } from "../src/required"

/** Restore authored primitives only where the existing outline still exactly matches. */
export function migrateFlowerHolderPart(current: PartDocument, reference: PartDocument): { document: PartDocument; converted: number } {
	const document = structuredClone(current)
	let converted = 0
	for (const step of reference.solidSteps ?? []) {
		if (step.type !== "extrusion") continue
		const desired = sketchFor(reference, step).entities.filter(isSketchPrimitive)
		if (!desired.length) continue
		const existingStep = document.solidSteps?.find((s) => s.id === step.id)
		if (!existingStep || existingStep.type !== "extrusion") throw Error(`Missing flower-holder feature ${step.id}`)
		const old = sketchFor(document, existingStep)
		const loops = stepLoops(document, existingStep)
		const replacements: { points: ReturnType<typeof primitivePoints>; entity: SketchEntity }[] = []
		for (const primitive of desired) {
			const points = primitivePoints(primitive)
			if (!loops.some((loop) => sameLoop(points, loop))) throw Error(`Feature ${step.id} differs from the authored primitive; no changes were saved.`)
			if (old.entities.some((e) => isSketchPrimitive(e) && sameLoop(primitivePoints(e), points))) continue
			replacements.push({ points, entity: structuredClone(primitive) })
			converted++
		}
		if (!replacements.length) continue
		const same = (a: { x: number; y: number }, b: { x: number; y: number }) => Math.hypot(a.x - b.x, a.y - b.y) < 1e-6
		const retained = old.entities.filter(
			(e) =>
				!replacements.some(
					(r) =>
						e.type === "line" &&
						r.points.some((p, i) => {
							const q = requireValue(r.points[(i + 1) % r.points.length])
							return (same(e.p0, p) && same(e.p1, q)) || (same(e.p1, p) && same(e.p0, q))
						})
				)
		)
		const next = materializeSketch({ ...old, entities: [...retained, ...replacements.map((r) => r.entity)] })
		// Preserve the extrusion target while changing only the source entity representation.
		const profile = next.profiles.find((p) => p.holeLoopIds.length === loops.length - 1)
		if (!profile) throw Error(`Cannot reconstruct profile for ${step.id}`)
		const feature = document.features.find((f) => f.type === "extrude" && f.id === existingStep.featureId)
		if (feature?.type !== "extrude") throw Error("Missing extrusion")
		feature.target.profileId = profile.id
		document.features[document.features.indexOf(old)] = next
	}
	if (converted) {
		document.cad = undefined
		document.tree = undefined
	}
	return { document, converted }
}
function boundsAndVolume(document: PartDocument) {
	const geometries = createPartGeometries(document)
	let volume = 0
	const vertices: number[] = []
	try {
		for (const geometry of geometries) {
			const positions = geometry.getAttribute("position")
			for (let i = 0; i < positions.count; i++) vertices.push(positions.getX(i), positions.getY(i), positions.getZ(i))
			for (let i = 0; i < positions.count; i += 3)
				volume +=
					(positions.getX(i) * (positions.getY(i + 1) * positions.getZ(i + 2) - positions.getZ(i + 1) * positions.getY(i + 2)) +
						positions.getY(i) * (positions.getZ(i + 1) * positions.getX(i + 2) - positions.getX(i + 1) * positions.getZ(i + 2)) +
						positions.getZ(i) * (positions.getX(i + 1) * positions.getY(i + 2) - positions.getY(i + 1) * positions.getX(i + 2))) /
					6
		}
		return {
			volume,
			bounds: [0, 1, 2].flatMap((axis) => {
				const values = vertices.filter((_, i) => i % 3 === axis)
				return [values.reduce((a, b) => Math.min(a, b), Number.POSITIVE_INFINITY), values.reduce((a, b) => Math.max(a, b), Number.NEGATIVE_INFINITY)]
			})
		}
	} finally {
		for (const geometry of geometries) geometry.dispose()
	}
}
function parts(nodes: ProjectNode[]): ProjectPartDocument[] {
	return nodes.flatMap((n) => ("items" in n ? parts(n.items) : n.type === "part" && "data" in n ? [n as ProjectPartDocument] : []))
}
if (import.meta.main) {
	const id = process.argv[2]
	if (!id) throw Error("Usage: bun examples/migrate-flower-holder-primitives.ts PROJECT_ID [--apply]")
	const url = `${process.env.PUPPYCAD_URL ?? "http://localhost:5337"}/api/projects/${encodeURIComponent(id)}`
	const response = await fetch(url)
	if (!response.ok) throw Error(`Project load failed: ${response.status}`)
	const payload = (await response.json()) as { project: Project }
	const commands: SyncedProjectCommand[] = []
	const report = []
	for (const definition of flowerHolderParts) {
		const current = parts(payload.project.items).find((p) => p.id === definition.id)
		if (!current?.data) throw Error(`Missing flower-holder part ${definition.id}`)
		const builder = new PartBuilder()
		definition.build(builder)
		const result = migrateFlowerHolderPart(current.data, builder.document)
		if (result.converted) {
			const before = boundsAndVolume(current.data)
			const after = boundsAndVolume(result.document)
			if (Math.abs(before.volume - after.volume) > Math.max(1e-5, Math.abs(before.volume) * 1e-7) || before.bounds.some((v, i) => Math.abs(v - requireValue(after.bounds[i])) > 1e-5))
				throw Error(`Geometry changed for ${definition.id}`)
			commands.push({ type: "upsertDocument", document: { ...current, data: result.document } })
		}
		report.push({ part: definition.id, converted: result.converted })
	}
	console.log(JSON.stringify({ projectId: id, revision: payload.project.revision, report }, null, 2))
	if (process.argv.includes("--apply") && commands.length) {
		const backup = `workdir/flower-holder-before-primitives-${id}-r${payload.project.revision}.json`
		await Bun.write(backup, JSON.stringify(payload.project, null, 2))
		const result = await fetch(`${url}/commands`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ clientId: "flower-holder-primitive-migration", baseRevision: payload.project.revision, commands })
		})
		if (!result.ok) throw Error(`Migration rejected: ${result.status} ${await result.text()}`)
		const saved = (await result.json()) as { revision: number }
		console.log(JSON.stringify({ savedRevision: saved.revision, backup }))
	}
}
