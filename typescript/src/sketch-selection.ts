import { finiteSketchCurveIntersections } from "./sketch-edit"
import { ellipticArcBounds, ellipticArcPoint } from "./sketch-elliptic-arc"
import { splineBezierSegments } from "./sketch-spline"
import { cubicBezierBounds } from "./sketch-bezier"
import { lineEllipseIntersections } from "./sketch-ellipse-intersections"
import type { SketchEntity, Ellipse } from "./schema"
import type { Point2D } from "./types"
import { primitivePoints } from "./sketch-primitives"
import { ellipsePoint } from "./sketch-ellipse"
export type SelectionBox = { minX: number; minY: number; maxX: number; maxY: number }
type Segment = [Point2D, Point2D]
type Round = { center: Point2D; radius: number; startAngle: number; sweep: number }
const TAU = Math.PI * 2
const EPS = 1e-9
const inside = (p: Point2D, b: SelectionBox) => p.x >= b.minX - EPS && p.x <= b.maxX + EPS && p.y >= b.minY - EPS && p.y <= b.maxY + EPS
const containsAngle = (curve: Round, angle: number) => (((Math.sign(curve.sweep) * (angle - curve.startAngle)) % TAU) + TAU) % TAU <= Math.abs(curve.sweep) + EPS
const roundPoint = (curve: Round, angle: number): Point2D => ({ x: curve.center.x + curve.radius * Math.cos(angle), y: curve.center.y + curve.radius * Math.sin(angle) })
function lineTouchesBox([a, b]: Segment, box: SelectionBox): boolean {
	let low = 0
	let high = 1
	for (const [origin, delta, min, max] of [
		[a.x, b.x - a.x, box.minX, box.maxX],
		[a.y, b.y - a.y, box.minY, box.maxY]
	]) {
		if (origin === undefined || delta === undefined || min === undefined || max === undefined) continue
		if (Math.abs(delta) < EPS) {
			if (origin < min - EPS || origin > max + EPS) return false
			continue
		}
		const t0 = (min - origin) / delta
		const t1 = (max - origin) / delta
		low = Math.max(low, Math.min(t0, t1))
		high = Math.min(high, Math.max(t0, t1))
		if (low > high + EPS) return false
	}
	return true
}
function circleRoots(a: Point2D, b: Point2D): number[] {
	const dx = b.x - a.x
	const dy = b.y - a.y
	const aa = dx * dx + dy * dy
	const bb = 2 * (a.x * dx + a.y * dy)
	const cc = a.x * a.x + a.y * a.y - 1
	if (aa < 1e-24) return Math.abs(cc) < EPS ? [0] : []
	const d = bb * bb - 4 * aa * cc
	if (d < -EPS * Math.max(1, bb * bb, Math.abs(4 * aa * cc))) return []
	const root = Math.sqrt(Math.max(0, d))
	return [(-bb - root) / (2 * aa), (-bb + root) / (2 * aa)].filter((t) => t >= -EPS && t <= 1 + EPS)
}
function geometry(entity: SketchEntity) {
	const lines: Segment[] = []
	const rounds: Round[] = []
	let points: Point2D[] = []
	let ellipse: Ellipse | undefined
	if (entity.type === "point") points = [entity.center]
	else if (entity.type === "ellipticArc") points = [ellipticArcPoint(entity, 0), ellipticArcPoint(entity, 1)]
	else if (entity.type === "spline") {
		points = splineBezierSegments(entity).flatMap((segment) => [segment[0], segment[3]])
	} else if (entity.type === "line") {
		points = [entity.p0, entity.p1]
		lines.push([entity.p0, entity.p1])
	} else if (entity.type === "circle" || entity.type === "arc")
		rounds.push({ center: entity.center, radius: entity.radius, startAngle: entity.type === "arc" ? entity.startAngle : 0, sweep: entity.type === "arc" ? entity.sweep : TAU })
	else if (entity.type === "ellipse") ellipse = entity
	else if (entity.type === "capsule") {
		const angle = Math.atan2(entity.to.y - entity.from.y, entity.to.x - entity.from.x)
		const radius = entity.width / 2
		const a: Round = { center: entity.from, radius, startAngle: angle + Math.PI / 2, sweep: Math.PI }
		const b: Round = { center: entity.to, radius, startAngle: angle - Math.PI / 2, sweep: Math.PI }
		rounds.push(a, b)
		lines.push([roundPoint(a, angle + Math.PI / 2), roundPoint(b, angle + Math.PI / 2)], [roundPoint(a, angle - Math.PI / 2), roundPoint(b, angle - Math.PI / 2)])
	} else {
		points = entity.type === "cornerRectangle" ? [entity.p0, { x: entity.p1.x, y: entity.p0.y }, entity.p1, { x: entity.p0.x, y: entity.p1.y }] : primitivePoints(entity)
		points.forEach((p, i) => {
			const next = points[(i + 1) % points.length]
			if (next) lines.push([p, next])
		})
	}
	for (const round of rounds)
		for (const angle of [round.startAngle, round.startAngle + round.sweep, 0, Math.PI / 2, Math.PI, (3 * Math.PI) / 2])
			if (containsAngle(round, angle)) points.push(roundPoint(round, angle))
	let bounds: SelectionBox
	if (entity.type === "ellipticArc") {
		const box = ellipticArcBounds(entity)
		bounds = { minX: box.min.x, minY: box.min.y, maxX: box.max.x, maxY: box.max.y }
	} else if (entity.type === "spline") {
		const boxes = splineBezierSegments(entity).map(cubicBezierBounds)
		bounds = {
			minX: Math.min(...boxes.map((b) => b.min.x)),
			maxX: Math.max(...boxes.map((b) => b.max.x)),
			minY: Math.min(...boxes.map((b) => b.min.y)),
			maxY: Math.max(...boxes.map((b) => b.max.y))
		}
	} else if (ellipse) {
		const angle = (ellipse.rotation * Math.PI) / 180
		const x = Math.hypot((ellipse.width / 2) * Math.cos(angle), (ellipse.height / 2) * Math.sin(angle))
		const y = Math.hypot((ellipse.width / 2) * Math.sin(angle), (ellipse.height / 2) * Math.cos(angle))
		bounds = { minX: ellipse.center.x - x, maxX: ellipse.center.x + x, minY: ellipse.center.y - y, maxY: ellipse.center.y + y }
		points = [ellipsePoint(ellipse, 0)]
	} else bounds = { minX: Math.min(...points.map((p) => p.x)), maxX: Math.max(...points.map((p) => p.x)), minY: Math.min(...points.map((p) => p.y)), maxY: Math.max(...points.map((p) => p.y)) }
	return { lines, rounds, points, ellipse, bounds }
}
/** Containment selects whole entities; crossing selects their actual curves, not filled interiors. */
export function sketchEntityInBox(entity: SketchEntity, box: SelectionBox, crossing: boolean): boolean {
	const g = geometry(entity)
	const b = g.bounds
	if (b.maxX < box.minX - EPS || b.minX > box.maxX + EPS || b.maxY < box.minY - EPS || b.minY > box.maxY + EPS) return false
	const contained = b.minX >= box.minX - EPS && b.maxX <= box.maxX + EPS && b.minY >= box.minY - EPS && b.maxY <= box.maxY + EPS
	if (!crossing || contained) return contained
	if (g.points.some((point) => inside(point, box)) || g.lines.some((line) => lineTouchesBox(line, box))) return true
	const corners = [
		{ x: box.minX, y: box.minY },
		{ x: box.maxX, y: box.minY },
		{ x: box.maxX, y: box.maxY },
		{ x: box.minX, y: box.maxY }
	]
	for (let i = 0; i < 4; i++) {
		const a = corners[i]
		const b = corners[(i + 1) % 4]
		if (!a || !b) continue
		for (const curve of g.rounds) {
			const local = (p: Point2D) => ({ x: (p.x - curve.center.x) / curve.radius, y: (p.y - curve.center.y) / curve.radius })
			for (const t of circleRoots(local(a), local(b)))
				if (containsAngle(curve, Math.atan2(a.y + (b.y - a.y) * t - curve.center.y, a.x + (b.x - a.x) * t - curve.center.x))) return true
		}
		if ((entity.type === "ellipticArc" || entity.type === "spline") && finiteSketchCurveIntersections(entity, { id: "box-edge", type: "line", p0: a, p1: b }).length) return true
		if (g.ellipse && lineEllipseIntersections({ id: "box-edge", type: "line", p0: a, p1: b }, g.ellipse, true).length) return true
	}
	return false
}
