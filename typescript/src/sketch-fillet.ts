import type { Arc, Line, Sketch } from "./schema"
import { removeCornerRelations } from "./sketch-corner-relations"
import { requireValue } from "./required"

/** Round one shared line corner without flattening the new arc. */
export function filletSketchCorner(input: Sketch, firstId: string, secondId: string, radius: number): { sketch: Sketch; arcId: string; radiusId: string; removedRelations: string[] } {
	if (!Number.isFinite(radius) || radius <= 0) throw Error("Fillet radius must be positive.")
	const sketch = structuredClone(input)
	const a = sketch.entities.find((e) => e.id === firstId)
	const b = sketch.entities.find((e) => e.id === secondId)
	if (a?.type !== "line" || b?.type !== "line" || a.id === b.id) throw Error("Select two connected lines for a sketch fillet.")
	if (!!a.construction !== !!b.construction) throw Error("Select lines with matching construction status.")
	const names = ["p0", "p1"] as const
	const shared = names.flatMap((pa) => names.filter((pb) => Math.hypot(a[pa].x - b[pb].x, a[pa].y - b[pb].y) < 1e-6).map((pb) => ({ pa, pb })))
	if (shared.length !== 1) throw Error("The two lines must share exactly one endpoint.")
	const { pa, pb } = requireValue(shared[0])
	const corner = { ...a[pa] }
	const direction = (line: Line, point: "p0" | "p1") => {
		const far = line[point === "p0" ? "p1" : "p0"]
		const length = Math.hypot(far.x - corner.x, far.y - corner.y)
		if (length < 1e-9) throw Error("Fillet requires nonzero lines.")
		return { x: (far.x - corner.x) / length, y: (far.y - corner.y) / length, length }
	}
	const u = direction(a, pa)
	const v = direction(b, pb)
	const angle = Math.acos(Math.max(-1, Math.min(1, u.x * v.x + u.y * v.y)))
	if (angle < 1e-6 || Math.PI - angle < 1e-6) throw Error("Fillet requires a non-collinear corner.")
	const setback = radius / Math.tan(angle / 2)
	if (setback >= Math.min(u.length, v.length) - 1e-7) throw Error("Fillet radius is too large for these lines.")
	const bisectorLength = Math.hypot(u.x + v.x, u.y + v.y)
	const centerDistance = radius / Math.sin(angle / 2)
	const center = { x: corner.x + ((u.x + v.x) / bisectorLength) * centerDistance, y: corner.y + ((u.y + v.y) / bisectorLength) * centerDistance }
	a[pa] = { x: corner.x + u.x * setback, y: corner.y + u.y * setback }
	b[pb] = { x: corner.x + v.x * setback, y: corner.y + v.y * setback }
	const startAngle = Math.atan2(a[pa].y - center.y, a[pa].x - center.x)
	const endAngle = Math.atan2(b[pb].y - center.y, b[pb].x - center.x)
	const sweep = Math.atan2(Math.sin(endAngle - startAngle), Math.cos(endAngle - startAngle))
	const ids = new Set([...sketch.entities, ...(sketch.relations ?? []), ...sketch.dimensions].map((e) => e.id))
	const fresh = (prefix: string) => {
		let i = 1
		while (ids.has(`${prefix}-${i}`)) i++
		const id = `${prefix}-${i}`
		ids.add(id)
		return id
	}
	const arc: Arc = { id: fresh("fillet"), type: "arc", center, radius, startAngle, sweep, segments: 128, construction: !!a.construction }
	const removedRelations = removeCornerRelations(sketch, a.id, b.id, pa, pb)
	const orientation = (line: Line, point: "p0" | "p1"): { lineSign: 1 | -1; sweepSign: 1 | -1 } => {
		const radial = { x: line[point].x - center.x, y: line[point].y - center.y }
		const tangent = { x: -radial.y * Math.sign(sweep), y: radial.x * Math.sign(sweep) }
		return { lineSign: tangent.x * (line.p1.x - line.p0.x) + tangent.y * (line.p1.y - line.p0.y) >= 0 ? 1 : -1, sweepSign: sweep > 0 ? 1 : -1 }
	}
	const radiusId = fresh("fillet-radius")
	sketch.entities.push(arc)
	sketch.relations ??= []
	sketch.relations.push(
		{ id: radiusId, type: "radius", entityId: arc.id, value: radius },
		{ id: fresh("fillet-join"), type: "coincident", a: { entityId: a.id, point: pa }, b: { entityId: arc.id, point: "p0" } },
		{ id: fresh("fillet-join"), type: "coincident", a: { entityId: b.id, point: pb }, b: { entityId: arc.id, point: "p1" } },
		{ id: fresh("fillet-tangent"), type: "endpointTangent", a: { entityId: arc.id, point: "p0" }, b: a.id, orientation: orientation(a, pa) },
		{ id: fresh("fillet-tangent"), type: "endpointTangent", a: { entityId: arc.id, point: "p1" }, b: b.id, orientation: orientation(b, pb) }
	)
	sketch.vertices = []
	sketch.loops = []
	sketch.profiles = []
	return { sketch, arcId: arc.id, radiusId, removedRelations }
}
