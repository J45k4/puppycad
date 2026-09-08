import { Vector3 } from "three"
import type { PartDocument } from "../schema"
import { extrusionFor, sketchFor } from "../solid-edit"
import { extrusionTranslation } from "../solid-model"
import { extrudeSolidFeature } from "../cad/extrude"
export type SketchSource = { stepId: string; sketchId: string; loopIndex: number; edgeIndex: number; entityId?: string; distance: number; border: Vector3[] }
/** Match an evaluated surface point to a source extrusion's profile boundary. */
export function pickSolidSketchSource(document: PartDocument, point: Vector3, tolerance: number, stepId?: string): SketchSource | null {
	let best: SketchSource | null = null
	for (const step of document.solidSteps ?? []) {
		if (stepId && step.id !== stepId) continue
		if (step.type !== "extrusion" || (step.topScale && step.topScale !== 1) || step.finishes?.length || step.edgeFinishes?.length) continue
		const feature = extrusionFor(document, step)
		const extrusion = extrudeSolidFeature(document, feature)
		const sketch = sketchFor(document, step)
		const f = extrusion.frame
		const offset = extrusionTranslation(document, feature.id)
		const origin = new Vector3(f.origin.x + offset.x, f.origin.y + offset.y, f.origin.z + offset.z)
		const x = new Vector3(f.xAxis.x, f.xAxis.y, f.xAxis.z)
		const y = new Vector3(f.yAxis.x, f.yAxis.y, f.yAxis.z)
		const normal = new Vector3(f.normal.x, f.normal.y, f.normal.z)
		const delta = point.clone().sub(origin)
		const depth = delta.dot(normal)
		if (depth < -tolerance || depth > extrusion.depth + tolerance) continue
		const px = delta.dot(x)
		const py = delta.dot(y)
		extrusion.profileLoops.forEach((loop, loopIndex) => {
			loop.forEach((a, edgeIndex) => {
				const b = loop[(edgeIndex + 1) % loop.length]
				if (!b) return
				const dx = b.x - a.x
				const dy = b.y - a.y
				const t = Math.max(0, Math.min(1, ((px - a.x) * dx + (py - a.y) * dy) / (dx * dx + dy * dy)))
				const distance = Math.hypot(px - a.x - t * dx, py - a.y - t * dy)
				if (distance > tolerance || (best && distance > best.distance + 1e-7)) return
				const same = (p: { x: number; y: number }, q: { x: number; y: number }) => Math.hypot(p.x - q.x, p.y - q.y) < 1e-6
				const entity = sketch.entities.find((e) => e.type === "line" && ((same(e.p0, a) && same(e.p1, b)) || (same(e.p1, a) && same(e.p0, b))))
				best = {
					stepId: step.id,
					sketchId: sketch.id,
					loopIndex,
					edgeIndex,
					entityId: entity?.id,
					distance,
					border: [...loop, loop[0]]
						.filter((p) => p !== undefined)
						.map((p) => origin.clone().addScaledVector(x, p.x).addScaledVector(y, p.y).addScaledVector(normal, depth))
				}
			})
		})
	}
	return best
}

/** Use evaluated crease segments at the nearest rim, not a section through the clicked wall. */
export function sourceRimSegments(source: SketchSource, edges: readonly Vector3[]): Vector3[] {
	const origin = source.border[0]
	const second = source.border[1]
	const third = source.border[2]
	if (!origin || !second || !third) return []
	const normal = second.clone().sub(origin).cross(third.clone().sub(origin)).normalize()
	const tolerance = 1e-3
	const onProfile = (point: Vector3) => {
		const projected = point.clone().addScaledVector(normal, -point.clone().sub(origin).dot(normal))
		for (let i = 1; i < source.border.length; i++) {
			const a = source.border[i - 1]
			const b = source.border[i]
			if (!a || !b) continue
			const edge = b.clone().sub(a)
			const t = Math.max(0, Math.min(1, projected.clone().sub(a).dot(edge) / edge.lengthSq()))
			if (a.clone().addScaledVector(edge, t).distanceTo(projected) < tolerance) return true
		}
		return false
	}
	const candidates: { a: Vector3; b: Vector3; depth: number }[] = []
	for (let i = 0; i + 1 < edges.length; i += 2) {
		const a = edges[i]
		const b = edges[i + 1]
		if (!a || !b) continue
		const depth = a.clone().sub(origin).dot(normal)
		if (Math.abs(b.clone().sub(a).dot(normal)) > tolerance || !onProfile(a) || !onProfile(b)) continue
		candidates.push({ a, b, depth })
	}
	const nearest = candidates.reduce<number | null>((best, c) => (best === null || Math.abs(c.depth) < Math.abs(best) ? c.depth : best), null)
	return nearest === null ? [] : candidates.filter((c) => Math.abs(c.depth - nearest) < tolerance).flatMap((c) => [c.a.clone(), c.b.clone()])
}
