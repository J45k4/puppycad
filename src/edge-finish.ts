import modeling from "@jscad/modeling"
import type { Geom3 } from "@jscad/modeling/src/geometries/types"
import type { Mat4 } from "@jscad/modeling/src/maths/types"
import { Matrix4, Quaternion, Vector3 } from "three"
import type { Solid } from "./schema"
import { requireValue } from "./required"
export type EdgeFinish = { kind: "fillet" | "chamfer"; edgeId: string; radius: number; distance2?: number; segments?: number }
/** Convex straight source edges. Each tangent half-space produces a closed boolean cut. */
export function finishSolidEdge(input: Geom3, topology: Solid, finish: EdgeFinish): Geom3 {
	let solid = input
	const edge = topology.edges.find((e) => e.id === finish.edgeId)
	if (!edge || edge.vertexIds.length !== 2) throw new Error(`Unknown source edge: ${finish.edgeId}`)
	const point = (id: string) => {
		const p = requireValue(topology.vertices.find((v) => v.id === id)).position
		return new Vector3(p.x, p.y, p.z)
	}
	const a = point(requireValue(edge.vertexIds[0]))
	const b = point(requireValue(edge.vertexIds[1]))
	const radius = finish.radius
	const d2 = finish.distance2 ?? radius
	if (![radius, d2].every((d) => Number.isFinite(d) && d > 0)) throw new Error("Edge finish distances must be positive.")
	const normals: Vector3[] = []
	for (const polygon of modeling.geometries.geom3.toPolygons(solid)) {
		const plane = modeling.geometries.poly3.plane(polygon)
		const normal = new Vector3(plane[0], plane[1], plane[2])
		if (Math.abs(normal.dot(a) - plane[3]) < 1e-6 && Math.abs(normal.dot(b) - plane[3]) < 1e-6 && !normals.some((n) => n.distanceTo(normal) < 1e-6)) normals.push(normal)
	}
	if (normals.length !== 2) throw new Error("Edge finish requires an unchanged straight edge shared by two planar faces.")
	const n1 = requireValue(normals[0])
	const n2 = requireValue(normals[1])
	const dot = n1.dot(n2)
	if (dot < -0.999999 || dot > 0.999999) throw new Error("Cannot finish coplanar or degenerate edges.")
	const polygons = modeling.geometries.geom3.toPolygons(solid)
	const scale = Math.max(...modeling.measurements.measureBoundingBox(solid).flat().map(Math.abs), 1) * 8
	const cut = (normal: Vector3, offset: number) => {
		const rotation = new Quaternion().setFromUnitVectors(new Vector3(0, 0, 1), normal)
		const matrix = new Matrix4().compose(normal.clone().multiplyScalar(offset + scale / 2), rotation, new Vector3(1, 1, 1))
		const cutter = modeling.transforms.transform(matrix.elements as Mat4, modeling.primitives.cuboid({ size: [scale * 2, scale * 2, scale] }))
		solid = modeling.booleans.subtract(solid, cutter)
	}
	// A convex source edge has the solid on the inward side of both adjacent planes locally.
	const midpoint = a.clone().add(b).multiplyScalar(0.5)
	const inward = n1.clone().add(n2).normalize()
	const near = midpoint.clone().addScaledVector(inward, -Math.min(radius, a.distanceTo(b)) * 1e-4)
	if (
		!polygons.every((p) => {
			const plane = modeling.geometries.poly3.plane(p)
			const normal = new Vector3(plane[0], plane[1], plane[2])
			return (normal.distanceTo(n1) > 1e-6 && normal.distanceTo(n2) > 1e-6) || normal.dot(near) <= plane[3] + 1e-6
		})
	)
		throw new Error("Only convex source edges can be finished.")
	const sin = Math.sqrt(1 - dot * dot)
	if (finish.kind === "chamfer") {
		const normal = n1.clone().divideScalar(d2).add(n2.clone().divideScalar(radius))
		const length = normal.length()
		normal.divideScalar(length)
		cut(normal, normal.dot(a) - sin / length)
	} else {
		const segments = finish.segments ?? 8
		if (!Number.isInteger(segments) || segments < 2 || segments > 64) throw new Error("Edge fillet segments must be between 2 and 64.")
		const center = a.clone().addScaledVector(n1.clone().add(n2), -radius / (1 + dot))
		const angle = Math.acos(dot)
		for (let i = 1; i < segments; i++) {
			const t = i / segments
			const normal = n1
				.clone()
				.multiplyScalar(Math.sin((1 - t) * angle) / sin)
				.addScaledVector(n2, Math.sin(t * angle) / sin)
				.normalize()
			cut(normal, normal.dot(center) + radius)
		}
	}
	if (modeling.geometries.geom3.toPolygons(solid).length === 0) throw new Error("Edge finish removed the entire solid.")
	return solid
}
