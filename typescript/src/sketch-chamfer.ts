import type { Line, Sketch } from "./schema"
import { requireValue } from "./required"
import { removeCornerRelations } from "./sketch-corner-relations"

export function chamferSetbacks(a: Line, pa: "p0" | "p1", b: Line, pb: "p0" | "p1"): [number, number] {
	const direction = (line: Line, point: "p0" | "p1") => {
		const near = line[point]
		const far = line[point === "p0" ? "p1" : "p0"]
		const length = Math.hypot(far.x - near.x, far.y - near.y)
		if (length < 1e-8) throw Error("Chamfer requires nonzero remaining lines.")
		return { x: (far.x - near.x) / length, y: (far.y - near.y) / length }
	}
	const u = direction(a, pa)
	const v = direction(b, pb)
	const cross = u.x * v.y - u.y * v.x
	if (Math.abs(cross) < 1e-8) throw Error("Chamfer requires a non-collinear corner.")
	const d = { x: b[pb].x - a[pa].x, y: b[pb].y - a[pa].y }
	return [-(d.x * v.y - d.y * v.x) / cross, -(d.x * u.y - d.y * u.x) / cross]
}

export type ChamferMode = "two-distances" | "distance-angle"
function cornerAngle(a: Line, pa: "p0" | "p1", b: Line, pb: "p0" | "p1"): number {
	const af = a[pa === "p0" ? "p1" : "p0"]
	const bf = b[pb === "p0" ? "p1" : "p0"]
	const u = { x: af.x - a[pa].x, y: af.y - a[pa].y }
	const v = { x: bf.x - b[pb].x, y: bf.y - b[pb].y }
	return Math.atan2(Math.abs(u.x * v.y - u.y * v.x), u.x * v.x + u.y * v.y)
}
export function chamferSecondSetback(a: Line, pa: "p0" | "p1", b: Line, pb: "p0" | "p1", first: number, angle: number): number {
	const alpha = (angle * Math.PI) / 180
	const theta = cornerAngle(a, pa, b, pb)
	if (!Number.isFinite(angle) || alpha <= 0 || alpha + theta >= Math.PI - 1e-8) throw Error("Chamfer angle must fit inside the corner triangle.")
	return (first * Math.sin(alpha)) / Math.sin(alpha + theta)
}
export function chamferAngle(a: Line, pa: "p0" | "p1", b: Line, pb: "p0" | "p1"): number {
	const far = a[pa === "p0" ? "p1" : "p0"]
	const u = { x: a[pa].x - far.x, y: a[pa].y - far.y }
	const v = { x: b[pb].x - a[pa].x, y: b[pb].y - a[pa].y }
	return (Math.atan2(Math.abs(u.x * v.y - u.y * v.x), u.x * v.x + u.y * v.y) * 180) / Math.PI
}
export function setChamferMode(input: Sketch, relationId: string, mode: ChamferMode): Sketch {
	const sketch = structuredClone(input)
	const r = sketch.relations?.find((r) => r.id === relationId)
	if (r?.type !== "chamfer") throw Error("Select a chamfer dimension.")
	const a = sketch.entities.find((e) => e.id === r.a.entityId)
	const b = sketch.entities.find((e) => e.id === r.b.entityId)
	if (a?.type !== "line" || b?.type !== "line" || (r.a.point !== "p0" && r.a.point !== "p1") || (r.b.point !== "p0" && r.b.point !== "p1")) throw Error("Chamfer requires line endpoints.")
	const values = chamferSetbacks(a, r.a.point, b, r.b.point)
	r.value = values[0]
	r.secondValue = mode === "distance-angle" ? chamferAngle(a, r.a.point, b, r.b.point) : values[1]
	r.mode = mode
	return sketch
}

export function chamferSketchCorner(
	input: Sketch,
	firstId: string,
	secondId: string,
	first: number,
	second = first,
	mode: ChamferMode = "two-distances"
): { sketch: Sketch; lineId: string; dimensionId: string; removedRelations: string[] } {
	if (![first, second].every((v) => Number.isFinite(v) && v > 0)) throw Error("Chamfer setbacks must be positive.")
	const sketch = structuredClone(input)
	const a = sketch.entities.find((e) => e.id === firstId)
	const b = sketch.entities.find((e) => e.id === secondId)
	if (a?.type !== "line" || b?.type !== "line" || a.id === b.id) throw Error("Select two connected lines for a sketch chamfer.")
	if (!!a.construction !== !!b.construction) throw Error("Select lines with matching construction status.")
	const names = ["p0", "p1"] as const
	const shared = names.flatMap((pa) => names.filter((pb) => Math.hypot(a[pa].x - b[pb].x, a[pa].y - b[pb].y) < 1e-6).map((pb) => ({ pa, pb })))
	if (shared.length !== 1) throw Error("The two lines must share exactly one endpoint.")
	const { pa, pb } = requireValue(shared[0])
	chamferSetbacks(a, pa, b, pb)
	const secondDistance = mode === "distance-angle" ? chamferSecondSetback(a, pa, b, pb, first, second) : second
	const corner = { ...a[pa] }
	for (const [line, point, setback] of [
		[a, pa, first],
		[b, pb, secondDistance]
	] as const) {
		const far = line[point === "p0" ? "p1" : "p0"]
		const length = Math.hypot(far.x - corner.x, far.y - corner.y)
		if (setback >= length - 1e-7) throw Error("Chamfer setback is too large for these lines.")
		line[point] = { x: corner.x + ((far.x - corner.x) * setback) / length, y: corner.y + ((far.y - corner.y) * setback) / length }
	}
	const ids = new Set([...sketch.entities, ...(sketch.relations ?? []), ...sketch.dimensions].map((e) => e.id))
	const fresh = (prefix: string) => {
		let i = 1
		while (ids.has(`${prefix}-${i}`)) i++
		const id = `${prefix}-${i}`
		ids.add(id)
		return id
	}
	const removedRelations = removeCornerRelations(sketch, a.id, b.id, pa, pb)
	const lineId = fresh("chamfer")
	const dimensionId = fresh("chamfer-setback")
	sketch.entities.push({ id: lineId, type: "line", p0: { ...a[pa] }, p1: { ...b[pb] }, construction: !!a.construction })
	sketch.relations ??= []
	sketch.relations.push(
		{ id: dimensionId, type: "chamfer", chamferId: lineId, a: { entityId: a.id, point: pa }, b: { entityId: b.id, point: pb }, value: first, secondValue: second, mode },
		{ id: fresh("chamfer-join"), type: "coincident", a: { entityId: a.id, point: pa }, b: { entityId: lineId, point: "p0" } },
		{ id: fresh("chamfer-join"), type: "coincident", a: { entityId: b.id, point: pb }, b: { entityId: lineId, point: "p1" } }
	)
	sketch.vertices = []
	sketch.loops = []
	sketch.profiles = []
	return { sketch, lineId, dimensionId, removedRelations }
}
