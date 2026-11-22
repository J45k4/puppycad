import { PuppyCad, type LiveProject, type PartRef } from "../src/sdk"
import { requireValue } from "../src/required"
import { FLOWER_HOLDER, flowerHolderParts, holderHub, holderPinLocations, referencePartPosition, ringLegLocations } from "./flower-holder-parts"
/** Rebuild the five original glTF parts and preserve its partially exploded assembly pose. */
export async function buildFlowerHolder(project: LiveProject) {
	const parts = new Map<string, PartRef>()
	for (const definition of flowerHolderParts) parts.set(definition.id, await project.part(definition.id, definition.build, definition.name))
	await project.assembly(
		"flower-holder",
		(assembly) => {
			const instances = new Map(
				flowerHolderParts.map((part) => [part.id, assembly.instance(part.id, requireValue(parts.get(part.id)), { translation: referencePartPosition(part.id) })])
			)
			const lower = requireValue(instances.get("lower-ring"))
			const upper = requireValue(instances.get("upper-ring"))
			const holder = requireValue(instances.get("centre-holder"))
			const stem = requireValue(instances.get("stem"))
			assembly.fasten(
				"lower-ring-ground",
				assembly.connector("reference-origin", null, { ...FLOWER_HOLDER.lowerPosition }),
				assembly.connector("lower-origin", lower, { x: 0, y: 0, z: 0 })
			)
			ringLegLocations.forEach((point, index) =>
				assembly.fasten(
					`leg-in-socket-${index + 1}`,
					assembly.connector(`lower-socket-${index + 1}`, lower, { ...point, z: 20 }),
					assembly.connector(`upper-foot-${index + 1}`, upper, { ...point, z: 0 })
				)
			)
			holderPinLocations.forEach((point, index) =>
				assembly.fasten(
					`holder-pin-${index + 1}`,
					assembly.connector(`upper-hole-${index + 1}`, upper, { ...point, z: 65 }),
					assembly.connector(`holder-seat-${index + 1}`, holder, { ...point, z: 10 })
				)
			)
			assembly.fasten("stem-in-holder", assembly.connector("holder-stem-seat", holder, { ...holderHub, z: 18 }), assembly.connector("stem-shoulder", stem, { x: 0, y: 0, z: 14 }))
		},
		"Bowl with flower holder — reference layout"
	)
}
if (import.meta.main) {
	const cad = new PuppyCad({ serverUrl: process.env.PUPPYCAD_SERVER_URL })
	const project = process.argv[2] ? await cad.openProject(process.argv[2]) : await cad.createProject()
	console.log(`Open ${project.viewerUrl}`)
	await buildFlowerHolder(project)
	console.log(`Saved reference reconstruction ${project.id}`)
}
