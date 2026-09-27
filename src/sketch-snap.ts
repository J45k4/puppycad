import { ellipticArcBounds } from "./sketch-elliptic-arc"
import { splineBezierSegments } from "./sketch-spline"
import { sketchCurveMidpoint } from "./sketch-point-constraints"
import { finiteSketchCurveIntersections } from "./sketch-edit"
import type { SketchEntity, SketchAnchorName } from "./schema"
import type { Point2D } from "./types"
import { entityAnchorNames, entityAnchorPoint } from "./sketch-curves"
import type { SketchAnchor } from "./sketch-solver"
export type SketchSnap = { position: Point2D; kind: "anchor" | "origin" | "grid" | "intersection" | "midpoint"; anchor?: SketchAnchor; curves?: [string, string]; curve?: string }
/** The same target is used for placement and preview; tolerance is in screen pixels. */
export function sketchSnap(entities: readonly SketchEntity[], raw: Point2D, scale: number, options: { endpointsOnly?: boolean; grid?: boolean; geometry?: boolean } = {}): SketchSnap | null {
	const { endpointsOnly = false, grid = true, geometry = true } = options
	let best: SketchSnap | null = null
	let distance = 8
	for (const entity of geometry || endpointsOnly ? entities : []) {
		if (endpointsOnly && entity.type !== "line" && entity.type !== "arc" && entity.type !== "spline" && entity.type !== "ellipticArc") continue
		if (endpointsOnly && entity.type === "spline" && entity.closed) continue
		if (endpointsOnly && entity.type === "ellipticArc" && Math.abs(entity.sweep) >= 2 * Math.PI) continue
		const endpoints: SketchAnchorName[] = entity.type === "spline" ? ["point0", `point${entity.points.length - 1}`] : ["p0", "p1"]
		for (const point of endpointsOnly ? endpoints : entityAnchorNames(entity)) {
			const position = entityAnchorPoint(entity, point)
			const d = Math.hypot(position.x - raw.x, position.y - raw.y) * scale
			if (d < distance) {
				distance = d
				best = { position: { ...position }, kind: "anchor", anchor: { entityId: entity.id, point } }
			}
		}
	}
	if (endpointsOnly) return best
	if (geometry)
		for (const entity of entities) {
			if (entity.type !== "line" && entity.type !== "arc" && entity.type !== "ellipticArc") continue
			if (entity.type === "ellipticArc" && Math.abs(entity.sweep) >= 2 * Math.PI) continue
			const position = sketchCurveMidpoint(entity)
			const d = Math.hypot(position.x - raw.x, position.y - raw.y) * scale
			if (d < distance) {
				distance = d
				best = { position, kind: "midpoint", curve: entity.id }
			}
		}
	if (geometry) {
		// Cull curves whose conservative bounds cannot enter the pointer's snap radius.
		const curves = entities.filter((entity) => {
			const margin = distance / Math.max(scale, 1e-12)
			if (entity.type === "line")
				return (
					raw.x >= Math.min(entity.p0.x, entity.p1.x) - margin &&
					raw.x <= Math.max(entity.p0.x, entity.p1.x) + margin &&
					raw.y >= Math.min(entity.p0.y, entity.p1.y) - margin &&
					raw.y <= Math.max(entity.p0.y, entity.p1.y) + margin
				)
			if (entity.type === "ellipticArc") {
				const bounds = ellipticArcBounds(entity)
				return raw.x >= bounds.min.x - margin && raw.x <= bounds.max.x + margin && raw.y >= bounds.min.y - margin && raw.y <= bounds.max.y + margin
			}
			if (entity.type === "spline") {
				const points = splineBezierSegments(entity).flat()
				return (
					raw.x >= Math.min(...points.map((p) => p.x)) - margin &&
					raw.x <= Math.max(...points.map((p) => p.x)) + margin &&
					raw.y >= Math.min(...points.map((p) => p.y)) - margin &&
					raw.y <= Math.max(...points.map((p) => p.y)) + margin
				)
			}
			if (entity.type !== "circle" && entity.type !== "arc" && entity.type !== "ellipse") return false
			const radius = entity.type === "ellipse" ? Math.max(entity.width, entity.height) / 2 : entity.radius
			return Math.abs(raw.x - entity.center.x) <= radius + margin && Math.abs(raw.y - entity.center.y) <= radius + margin
		})
		for (let i = 0; i < curves.length; i++)
			for (const other of curves.slice(i + 1)) {
				const curve = curves[i]
				if (!curve) continue
				for (const position of finiteSketchCurveIntersections(curve, other)) {
					const d = Math.hypot(position.x - raw.x, position.y - raw.y) * scale
					if (d < distance || (best?.kind === "midpoint" && Math.hypot(position.x - best.position.x, position.y - best.position.y) * scale < 1e-7)) {
						distance = d
						best = { position, kind: "intersection", curves: [curve.id, other.id] }
					}
				}
			}
	}
	if (geometry && Math.hypot(raw.x, raw.y) * scale < distance) return { position: { x: 0, y: 0 }, kind: "origin" }
	return best ?? (grid ? { position: { x: Math.round(raw.x), y: Math.round(raw.y) }, kind: "grid" } : null)
}
