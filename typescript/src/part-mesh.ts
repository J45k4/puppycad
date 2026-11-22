import { requireValue } from "./required"
import { BufferGeometry, Float32BufferAttribute, Vector3 } from "three"
import modeling from "@jscad/modeling"
import { PCadPart } from "./pcad/project"
import { evaluateSolid } from "./solid-model"
import type { PartDocument } from "./schema"
/** One evaluated boundary shared by previews and STL export. */
export function createPartGeometries(document: PartDocument): BufferGeometry[] {
	const part = new PCadPart(document).getDocument()
	const solid = evaluateSolid(part)
	const positions = triangulateBoundary(modeling.geometries.geom3.toPolygons(solid).map((p) => p.vertices))
	const geometry = new BufferGeometry()
	geometry.setAttribute("position", new Float32BufferAttribute(positions, 3))
	geometry.computeVertexNormals()
	return [geometry]
}
export function exportPartStl(document: PartDocument): string {
	const geometries = createPartGeometries(document)
	try {
		if (geometries.length !== 1) throw new Error("Part must produce one evaluated mesh.")
		const positions = requireValue(geometries[0]).getAttribute("position")
		const lines = ["solid puppycad"]
		for (let i = 0; i < positions.count; i += 3) {
			const a = new Vector3().fromBufferAttribute(positions, i)
			const b = new Vector3().fromBufferAttribute(positions, i + 1)
			const c = new Vector3().fromBufferAttribute(positions, i + 2)
			const normal = new Vector3().subVectors(b, a).cross(new Vector3().subVectors(c, a)).normalize()
			lines.push(`facet normal ${normal.x} ${normal.y} ${normal.z}`, "outer loop", ...[a, b, c].map((p) => `vertex ${p.x} ${p.y} ${p.z}`), "endloop", "endfacet")
		}
		lines.push("endsolid puppycad")
		return lines.join("\n")
	} finally {
		for (const geometry of geometries) geometry.dispose()
	}
}

/** Split T-junction edges before triangulating convex kernel faces; keep centroids on-plane. */
export function triangulateBoundary(polygons: readonly (readonly (readonly number[])[])[]): number[] {
	const tolerance = 1e-6
	const unique = new Map<string, Vector3>()
	const faces = polygons.map((polygon) =>
		polygon.map((p) => {
			const key = p.map((v) => Math.round(v / tolerance)).join(",")
			let vertex = unique.get(key)
			if (!vertex) {
				vertex = new Vector3(p[0], p[1], p[2])
				unique.set(key, vertex)
			}
			return vertex
		})
	)
	const points = [...unique.values()].sort((a, b) => a.x - b.x)
	const lower = (x: number) => {
		let lo = 0
		let hi = points.length
		while (lo < hi) {
			const mid = (lo + hi) >>> 1
			if (requireValue(points[mid]).x < x) lo = mid + 1
			else hi = mid
		}
		return lo
	}
	const result: number[] = []
	for (const face of faces) {
		const perimeter: Vector3[] = []
		for (let i = 0; i < face.length; i++) {
			const a = requireValue(face[i])
			const b = requireValue(face[(i + 1) % face.length])
			const edge = b.clone().sub(a)
			const length2 = edge.lengthSq()
			if (length2 < tolerance * tolerance) continue
			const splits: { t: number; point: Vector3 }[] = [{ t: 0, point: a }]
			for (let j = lower(Math.min(a.x, b.x) - tolerance); j < points.length; j++) {
				const p = requireValue(points[j])
				if (p.x > Math.max(a.x, b.x) + tolerance) break
				if (
					p === a ||
					p === b ||
					p.y < Math.min(a.y, b.y) - tolerance ||
					p.y > Math.max(a.y, b.y) + tolerance ||
					p.z < Math.min(a.z, b.z) - tolerance ||
					p.z > Math.max(a.z, b.z) + tolerance
				)
					continue
				const delta = p.clone().sub(a)
				const t = delta.dot(edge) / length2
				if (t > 1e-8 && t < 1 - 1e-8 && delta.addScaledVector(edge, -t).lengthSq() < tolerance * tolerance) splits.push({ t, point: p })
			}
			perimeter.push(...splits.sort((a, b) => a.t - b.t).map((s) => s.point))
		}
		if (perimeter.length < 3) continue
		const center = perimeter.reduce((sum, p) => sum.add(p), new Vector3()).divideScalar(perimeter.length)
		for (let i = 0; i < perimeter.length; i++) {
			const a = requireValue(perimeter[i])
			const b = requireValue(perimeter[(i + 1) % perimeter.length])
			if (a.clone().sub(center).cross(b.clone().sub(center)).lengthSq() < 1e-20) continue
			for (const p of [center, a, b]) result.push(p.x, p.y, p.z)
		}
	}
	return result
}

/** Triangle topology for CLI geometry inspection and PNG rendering of SDK solids. */
export function createPartSolid(document: PartDocument): import("./schema").Solid {
	const geometry = requireValue(createPartGeometries(document)[0])
	try {
		const positions = geometry.getAttribute("position")
		const solid: import("./schema").Solid = { id: "evaluated-solid", featureId: "solidSteps", vertices: [], edges: [], faces: [] }
		const vertices = new Map<string, string>()
		const edges = new Map<string, string>()
		for (let i = 0; i < positions.count; i += 3) {
			const ids: string[] = []
			for (let j = 0; j < 3; j++) {
				const p = new Vector3().fromBufferAttribute(positions, i + j)
				const key = [p.x, p.y, p.z].join(",")
				let id = vertices.get(key)
				if (!id) {
					id = `v${vertices.size}`
					vertices.set(key, id)
					solid.vertices.push({ id, position: { x: p.x, y: p.y, z: p.z } })
				}
				ids.push(id)
			}
			const edgeIds: string[] = []
			for (let j = 0; j < 3; j++) {
				const pair = [requireValue(ids[j]), requireValue(ids[(j + 1) % 3])]
				const key = [...pair].sort().join("/")
				let id = edges.get(key)
				if (!id) {
					id = `e${edges.size}`
					edges.set(key, id)
					solid.edges.push({ id, vertexIds: pair })
				}
				edgeIds.push(id)
			}
			solid.faces.push({ id: `f${solid.faces.length}`, edgeIds })
		}
		return solid
	} finally {
		geometry.dispose()
	}
}
