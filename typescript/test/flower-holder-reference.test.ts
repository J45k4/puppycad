import { describe, expect, it } from "bun:test"
import { readFileSync } from "node:fs"
import { PartBuilder } from "../src/sdk"
import { createPartGeometries } from "../src/part-mesh"
import { flowerHolderParts, referencePartPosition } from "../examples/flower-holder-parts"
import { compareSurfaces, readReferenceMeshes, type TriangleMesh } from "./helpers/mesh-comparison"
import { requireValue } from "../src/required"
const reference = readReferenceMeshes(readFileSync(new URL("./fixtures/bowl_with_flowerholder.gltf", import.meta.url), "utf8"))
function volume(mesh: TriangleMesh) {
	let result = 0
	for (let i = 0; i < mesh.triangles.length; i += 3) {
		const a = requireValue(mesh.triangles[i]) * 3
		const b = requireValue(mesh.triangles[i + 1]) * 3
		const c = requireValue(mesh.triangles[i + 2]) * 3
		const p = mesh.positions
		result +=
			(requireValue(p[a]) * (requireValue(p[b + 1]) * requireValue(p[c + 2]) - requireValue(p[b + 2]) * requireValue(p[c + 1])) +
				requireValue(p[a + 1]) * (requireValue(p[b + 2]) * requireValue(p[c]) - requireValue(p[b]) * requireValue(p[c + 2])) +
				requireValue(p[a + 2]) * (requireValue(p[b]) * requireValue(p[c + 1]) - requireValue(p[b + 1]) * requireValue(p[c]))) /
			6
	}
	return result
}
function closed(mesh: TriangleMesh) {
	const edges = new Map<string, number>()
	const key = (id: number) => [0, 1, 2].map((axis) => Math.round(requireValue(mesh.positions[id * 3 + axis]) * 1e5)).join(",")
	for (let i = 0; i < mesh.triangles.length; i += 3)
		for (let j = 0; j < 3; j++) {
			const a = key(requireValue(mesh.triangles[i + j]))
			const b = key(requireValue(mesh.triangles[i + ((j + 1) % 3)]))
			if (a === b) continue
			const k = [a, b].sort().join("/")
			edges.set(k, (edges.get(k) ?? 0) + 1)
		}
	return [...edges.values()].every((count) => count === 2)
}
describe("flower holder reference fidelity", () => {
	for (const definition of flowerHolderParts)
		it(`matches the glTF ${definition.id} surface, volume and assembly position`, () => {
			const builder = new PartBuilder()
			definition.build(builder)
			const geometries = createPartGeometries(builder.document)
			try {
				const positions = Array.from(requireValue(geometries[0]).getAttribute("position").array)
				const mesh = {
					positions: positions.map((n, i) => (i % 3 === 2 ? n + definition.localZOffset : n)),
					triangles: Array.from({ length: positions.length / 3 }, (_, i) => i)
				}
				const original = requireValue(reference[definition.referenceMesh])
				const difference = compareSurfaces(original.mesh, mesh)
				// The original glTF approximates 113 mm arcs with ~0.12 mm chord error.
				// Test every unique mesh vertex plus 2,000 area-weighted samples in BOTH directions.
				expect(difference.aToB.max).toBeLessThan(0.13)
				expect(difference.bToA.max).toBeLessThan(0.13)
				expect(Math.abs(volume(mesh) / volume(original.mesh) - 1)).toBeLessThan(0.006)
				expect(volume(mesh)).toBeGreaterThan(0)
				expect(closed(mesh)).toBe(true)
				const position = referencePartPosition(definition.id)
				expect(position.x).toBeCloseTo(original.translation.x, 3)
				expect(position.y).toBeCloseTo(original.translation.y, 3)
				expect(position.z).toBeCloseTo(original.translation.z + definition.localZOffset, 3)
			} finally {
				for (const g of geometries) g.dispose()
			}
		}, 30000)
})
