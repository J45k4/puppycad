import { extrudeSolidFeature, resolveSketchTargetFrame } from "./cad/extrude"
import type { PartDocument, SketchTarget } from "./schema"
import type { Point2D } from "./types"
import { requireValue } from "./required"

/** Project the supporting extrusion's edges into the sketch's local frame.
 * These are display references, not sketch entities or projected constraints.
 */
export function sketchSupportEdges(part: Pick<PartDocument, "features">, target: SketchTarget): Point2D[][] {
	if (target.type !== "face") return []
	const feature = part.features.find((feature) => feature.id === target.face.extrudeId)
	if (feature?.type !== "extrude") throw Error("Missing supporting extrusion.")
	const frame = resolveSketchTargetFrame(part, target)
	const solid = extrudeSolidFeature(part, feature).solid
	const projected = new Map(
		solid.vertices.map((vertex) => {
			const p = { x: vertex.position.x - frame.origin.x, y: vertex.position.y - frame.origin.y, z: vertex.position.z - frame.origin.z }
			return [vertex.id, { x: p.x * frame.xAxis.x + p.y * frame.xAxis.y + p.z * frame.xAxis.z, y: p.x * frame.yAxis.x + p.y * frame.yAxis.y + p.z * frame.yAxis.z }]
		})
	)
	const keys = new Set<string>()
	return solid.edges.flatMap((edge) => {
		const points = edge.vertexIds.map((id) => requireValue(projected.get(id)))
		if (points.length < 2 || points.every((point) => Math.hypot(point.x - requireValue(points[0]).x, point.y - requireValue(points[0]).y) < 1e-8)) return []
		const key = points.map((point) => `${point.x.toFixed(7)},${point.y.toFixed(7)}`).join(";")
		const reverse = [...points]
			.reverse()
			.map((point) => `${point.x.toFixed(7)},${point.y.toFixed(7)}`)
			.join(";")
		if (keys.has(key) || keys.has(reverse)) return []
		keys.add(key)
		return [points]
	})
}
