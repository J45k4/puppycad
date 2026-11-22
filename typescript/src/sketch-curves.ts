import { ellipticArcPoint } from "./sketch-elliptic-arc"
import { ellipsePoint } from "./sketch-ellipse"
import { polygonVertices } from "./sketch-polygon"
import type { Arc, Circle, SketchAnchorName } from "./schema"
import type { Point2D } from "./types"
const TAU = Math.PI * 2
export function arcPoint(arc: Arc, fraction: number): Point2D {
	const angle = arc.startAngle + arc.sweep * fraction
	return { x: arc.center.x + arc.radius * Math.cos(angle), y: arc.center.y + arc.radius * Math.sin(angle) }
}
export function arcPoints(arc: Arc): Point2D[] {
	const count = Math.max(2, Math.ceil((Math.abs(arc.sweep) / TAU) * arc.segments))
	return Array.from({ length: count + 1 }, (_, i) => arcPoint(arc, i / count))
}
export function normalizeArc(input: unknown, id: string): Arc | undefined {
	if (!input || typeof input !== "object") return
	const a = input as Arc
	if (
		a.type !== "arc" ||
		!a.center ||
		![a.center.x, a.center.y, a.radius, a.startAngle, a.sweep, a.segments].every(Number.isFinite) ||
		a.radius <= 0 ||
		Math.abs(a.sweep) < 1e-9 ||
		Math.abs(a.sweep) >= TAU ||
		!Number.isInteger(a.segments) ||
		a.segments < 8 ||
		a.segments > 4096
	)
		return
	return { id, type: "arc", center: { ...a.center }, radius: a.radius, startAngle: a.startAngle, sweep: a.sweep, segments: a.segments, ...(a.construction ? { construction: true } : {}) }
}
/** Native circle through three distinct, non-collinear points. */
export function threePointCircle(id: string, start: Point2D, end: Point2D, through: Point2D): Circle {
	if (![start.x, start.y, end.x, end.y, through.x, through.y].every(Number.isFinite)) throw Error("Three points must be finite.")
	const bx = end.x - start.x
	const by = end.y - start.y
	const cx = through.x - start.x
	const cy = through.y - start.y
	const det = 2 * (bx * cy - by * cx)
	if (Math.abs(det) <= 1e-12 * Math.max(bx * bx + by * by, cx * cx + cy * cy)) throw Error("Three points must be distinct and not collinear.")
	const b2 = bx * bx + by * by
	const c2 = cx * cx + cy * cy
	const center = { x: start.x + (cy * b2 - by * c2) / det, y: start.y + (bx * c2 - cx * b2) / det }
	const radius = Math.hypot(start.x - center.x, start.y - center.y)
	if (![center.x, center.y, radius].every(Number.isFinite) || radius <= 0) throw Error("Cannot create a circle through those points.")
	return { id, type: "circle", center, radius, segments: 128 }
}
/** Start, end, and a point on the desired arc; signed sweep retains major arcs. */
export function threePointArc(id: string, start: Point2D, end: Point2D, through: Point2D): Arc {
	const { center, radius } = threePointCircle(id, start, end, through)
	const startAngle = Math.atan2(start.y - center.y, start.x - center.x)
	const normalized = (a: number) => ((a % TAU) + TAU) % TAU
	const endAngle = normalized(Math.atan2(end.y - center.y, end.x - center.x) - startAngle)
	const midAngle = normalized(Math.atan2(through.y - center.y, through.x - center.x) - startAngle)
	const sweep = midAngle < endAngle ? endAngle : endAngle - TAU
	const result = normalizeArc({ type: "arc", center, radius, startAngle, sweep, segments: 128 }, id)
	if (!result) throw Error("Choose three distinct arc points.")
	return result
}
export function entityAnchorNames(entity: import("./schema").SketchEntity): SketchAnchorName[] {
	if (entity.type === "spline") return entity.points.map((_, i) => `point${i}` as SketchAnchorName)
	if (entity.type === "polygon") return ["center", ...Array.from({ length: entity.sides }, (_, i) => `vertex${i}` as SketchAnchorName)]
	if (entity.type === "ellipse" || entity.type === "rectangle") return ["p0", "p1", "p2", "p3", "center"]
	if (entity.type === "arc" || entity.type === "ellipticArc") return ["p0", "p1", "center"]
	if (entity.type === "line" || entity.type === "cornerRectangle") return ["p0", "p1"]
	if (entity.type === "capsule") return ["from", "to"]
	return ["center"]
}
export function entityAnchorPoint(entity: import("./schema").SketchEntity, point: SketchAnchorName): Point2D {
	if (entity.type === "spline" && /^point\d+$/.test(point)) {
		const p = entity.points[Number(point.slice(5))]
		if (!p) throw Error(`Entity ${entity.id} has no ${point} point.`)
		return p
	}
	if (entity.type === "ellipse" && ["p0", "p1", "p2", "p3"].includes(point)) return ellipsePoint(entity, (Number(point.slice(1)) * Math.PI) / 2)
	if (entity.type === "polygon" && /^vertex\d+$/.test(point)) {
		const vertex = polygonVertices(entity)[Number(point.slice(6))]
		if (!vertex) throw Error(`Entity ${entity.id} has no ${point} point.`)
		return vertex
	}
	if (entity.type === "ellipticArc" && (point === "p0" || point === "p1")) return ellipticArcPoint(entity, point === "p0" ? 0 : 1)
	if (entity.type === "arc" && (point === "p0" || point === "p1")) return arcPoint(entity, point === "p0" ? 0 : 1)
	if (entity.type === "rectangle" && ["p0", "p1", "p2", "p3"].includes(point)) {
		const x = ((point === "p0" || point === "p3" ? 1 : -1) * entity.width) / 2
		const y = ((point === "p0" || point === "p1" ? 1 : -1) * entity.height) / 2
		const angle = (entity.rotation * Math.PI) / 180
		return { x: entity.center.x + x * Math.cos(angle) - y * Math.sin(angle), y: entity.center.y + x * Math.sin(angle) + y * Math.cos(angle) }
	}
	const value = (entity as unknown as Record<string, unknown>)[point] as Point2D | undefined
	if (!value || !Number.isFinite(value.x) || !Number.isFinite(value.y)) throw Error(`Entity ${entity.id} has no ${point} point.`)
	return value
}

/** Center, radial start, then end direction; the start fixes the radius. */
export function centerPointArc(id: string, center: Point2D, start: Point2D, end: Point2D, clockwise = false): Arc {
	const radius = Math.hypot(start.x - center.x, start.y - center.y)
	if (radius < 1e-9 || Math.hypot(end.x - center.x, end.y - center.y) < 1e-9) throw Error("Arc start and end directions must differ from the center.")
	const startAngle = Math.atan2(start.y - center.y, start.x - center.x)
	const endAngle = Math.atan2(end.y - center.y, end.x - center.x)
	const positive = (((endAngle - startAngle) % TAU) + TAU) % TAU
	const sweep = clockwise ? positive - TAU : positive
	const result = normalizeArc({ type: "arc", center, radius, startAngle, sweep, segments: 128 }, id)
	if (!result) throw Error("Choose different start and end directions; use Circle for a full revolution.")
	return result
}
