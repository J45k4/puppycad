import { ellipticArcParameter, nearestEllipticArcPoint, normalizeEllipticArc } from "./sketch-elliptic-arc"
import { lineSplineIntersections, circleSplineIntersections, ellipseSplineIntersections, splineSplineIntersections } from "./sketch-spline-intersections"
import { nearestSplinePoint, splinePortionWithAnchors, splineBezierSegments } from "./sketch-spline"
import { pointOnSketchCurveResidual } from "./sketch-point-constraints"
import { lineEllipseIntersections, circleEllipseIntersections, ellipseEllipseIntersections } from "./sketch-ellipse-intersections"
import type { Arc, Circle, Line, Sketch, SketchEntity } from "./schema"
import type { Point2D } from "./types"
import type { SketchAnchor } from "./sketch-solver"
import { arcPoint, entityAnchorNames, entityAnchorPoint } from "./sketch-curves"
import { primitivePoints } from "./sketch-primitives"
import { requireValue } from "./required"

type Curve = Line | Circle | Arc
const EPS = 1e-7
const TAU = 2 * Math.PI
const mod = (n: number) => ((n % TAU) + TAU) % TAU
const sub = (a: Point2D, b: Point2D) => ({ x: a.x - b.x, y: a.y - b.y })
const cross = (a: Point2D, b: Point2D) => a.x * b.y - a.y * b.x
const dot = (a: Point2D, b: Point2D) => a.x * b.x + a.y * b.y
const near = (a: Point2D, b: Point2D) => Math.hypot(a.x - b.x, a.y - b.y) < EPS
function parameter(curve: Curve, p: Point2D): number {
	if (curve.type === "line") {
		const d = sub(curve.p1, curve.p0)
		return dot(sub(p, curve.p0), d) / dot(d, d)
	}
	if (curve.type === "arc") {
		if (near(arcPoint(curve, 0), p)) return 0
		if (near(arcPoint(curve, 1), p)) return 1
	}
	const angle = Math.atan2(p.y - curve.center.y, p.x - curve.center.x)
	if (curve.type === "circle") return mod(angle) / TAU
	return curve.sweep > 0 ? mod(angle - curve.startAngle) / curve.sweep : -mod(curve.startAngle - angle) / curve.sweep
}
function at(curve: Curve, t: number): Point2D {
	if (curve.type === "line") return { x: curve.p0.x + t * (curve.p1.x - curve.p0.x), y: curve.p0.y + t * (curve.p1.y - curve.p0.y) }
	if (curve.type === "arc") return arcPoint(curve, t)
	return { x: curve.center.x + curve.radius * Math.cos(t * TAU), y: curve.center.y + curve.radius * Math.sin(t * TAU) }
}
function contains(curve: Curve, p: Point2D): boolean {
	if (curve.type === "circle") return true
	if (near(at(curve, 0), p) || near(at(curve, 1), p)) return true
	const t = parameter(curve, p)
	return t >= -EPS && t <= 1 + EPS
}
/** Exact intersections of supporting lines/circles, filtered to the boundary's finite extent. */
function intersections(a: Curve, b: Curve): Point2D[] {
	if (a.type === "line" && b.type === "line") {
		const da = sub(a.p1, a.p0)
		const db = sub(b.p1, b.p0)
		const det = cross(da, db)
		if (Math.abs(det) < EPS) return Math.abs(cross(sub(b.p0, a.p0), da)) < EPS ? [b.p0, b.p1] : []
		return [at(a, cross(sub(b.p0, a.p0), db) / det)]
	}
	if (a.type === "line" || b.type === "line") {
		const line = a.type === "line" ? a : (b as Line)
		const circle = a.type !== "line" ? a : (b as Circle | Arc)
		const d = sub(line.p1, line.p0)
		const f = sub(line.p0, circle.center)
		const aa = dot(d, d)
		const bb = 2 * dot(f, d)
		const cc = dot(f, f) - circle.radius ** 2
		const disc = bb * bb - 4 * aa * cc
		if (aa < EPS || disc < -EPS) return []
		const root = Math.sqrt(Math.max(0, disc))
		return [at(line, (-bb - root) / (2 * aa)), at(line, (-bb + root) / (2 * aa))]
	}
	const d = Math.hypot(b.center.x - a.center.x, b.center.y - a.center.y)
	if (d < EPS || d > a.radius + b.radius + EPS || d < Math.abs(a.radius - b.radius) - EPS) return []
	const along = (a.radius * a.radius - b.radius * b.radius + d * d) / (2 * d)
	const height = Math.sqrt(Math.max(0, a.radius * a.radius - along * along))
	const ux = (b.center.x - a.center.x) / d
	const uy = (b.center.y - a.center.y) / d
	const center = { x: a.center.x + ux * along, y: a.center.y + uy * along }
	return [
		{ x: center.x - uy * height, y: center.y + ux * height },
		{ x: center.x + uy * height, y: center.y - ux * height }
	]
}
function ellipticAngle(arc: Extract<SketchEntity, { type: "ellipticArc" }>, point: Point2D): number {
	const rotation = (arc.rotation * Math.PI) / 180
	const dx = point.x - arc.center.x
	const dy = point.y - arc.center.y
	return Math.atan2((-dx * Math.sin(rotation) + dy * Math.cos(rotation)) / arc.height, (dx * Math.cos(rotation) + dy * Math.sin(rotation)) / arc.width)
}
function onEllipticSweep(arc: Extract<SketchEntity, { type: "ellipticArc" }>, point: Point2D): boolean {
	return ellipticArcParameter(arc, ellipticAngle(arc, point)) !== null
}
/** Isolated contacts on finite native curves, shared by intersection snapping. */
export function finiteSketchCurveIntersections(a: SketchEntity, b: SketchEntity): Point2D[] {
	if (a.type === "cornerRectangle" || a.type === "rectangle" || a.type === "polygon" || a.type === "capsule") {
		return boundaries(a)
			.flatMap((edge) => finiteSketchCurveIntersections(edge, b))
			.filter((point, index, all) => !all.slice(0, index).some((other) => near(point, other)))
	}
	if (b.type === "cornerRectangle" || b.type === "rectangle" || b.type === "polygon" || b.type === "capsule") return finiteSketchCurveIntersections(b, a)
	if (a.type === "ellipticArc") return finiteSketchCurveIntersections({ ...a, type: "ellipse" }, b).filter((point) => onEllipticSweep(a, point))
	if (b.type === "ellipticArc") return finiteSketchCurveIntersections(b, a)
	if (a.type === "spline" && b.type === "spline") return splineSplineIntersections(a, b).map((hit) => hit.point)
	if (a.type === "ellipse" && b.type === "spline") return ellipseSplineIntersections(a, b).map((hit) => hit.point)
	if (b.type === "ellipse" && a.type === "spline") return ellipseSplineIntersections(b, a).map((hit) => hit.point)
	if (a.type === "line" && b.type === "spline") return lineSplineIntersections(a, b).map((hit) => hit.point)
	if (b.type === "line" && a.type === "spline") return lineSplineIntersections(b, a).map((hit) => hit.point)
	if (a.type === "spline" && (b.type === "circle" || b.type === "arc"))
		return circleSplineIntersections(b, a)
			.filter((hit) => contains(b, hit.point))
			.map((hit) => hit.point)
	if (b.type === "spline" && (a.type === "circle" || a.type === "arc"))
		return circleSplineIntersections(a, b)
			.filter((hit) => contains(a, hit.point))
			.map((hit) => hit.point)
	if (a.type === "ellipse" && b.type === "ellipse") return ellipseEllipseIntersections(a, b)
	const supported = (entity: SketchEntity): entity is Curve => entity.type === "line" || entity.type === "circle" || entity.type === "arc"
	if (a.type === "ellipse" || b.type === "ellipse") {
		const ellipse = a.type === "ellipse" ? a : b.type === "ellipse" ? b : null
		const other = a.type === "ellipse" ? b : a
		if (!ellipse || !supported(other)) return []
		return (other.type === "line" ? lineEllipseIntersections(other, ellipse, true) : circleEllipseIntersections(other, ellipse)).filter((point) => contains(other, point))
	}
	if (!supported(a) || !supported(b)) return []
	if (a.type === "line" && b.type === "line" && Math.abs(cross(sub(a.p1, a.p0), sub(b.p1, b.p0))) < EPS) return []
	return intersections(a, b).filter((point, index, all) => contains(a, point) && contains(b, point) && !all.slice(0, index).some((other) => near(point, other)))
}
function boundaries(entity: SketchEntity): Curve[] {
	if (entity.type === "point" || entity.type === "spline" || entity.type === "ellipticArc") return []
	if (entity.type === "line" || entity.type === "circle" || entity.type === "arc") return [entity]
	if (entity.type === "capsule") {
		const angle = Math.atan2(entity.to.y - entity.from.y, entity.to.x - entity.from.x)
		const radius = entity.width / 2
		const a: Arc = { id: entity.id, type: "arc", center: entity.from, radius, startAngle: angle + Math.PI / 2, sweep: Math.PI, segments: 128 }
		const b: Arc = { ...a, center: entity.to, startAngle: angle - Math.PI / 2 }
		return [a, b, { id: entity.id, type: "line", p0: arcPoint(a, 0), p1: arcPoint(b, 1) }, { id: entity.id, type: "line", p0: arcPoint(a, 1), p1: arcPoint(b, 0) }]
	}
	const points = entity.type === "cornerRectangle" ? [entity.p0, { x: entity.p1.x, y: entity.p0.y }, entity.p1, { x: entity.p0.x, y: entity.p1.y }] : primitivePoints(entity)
	return points.map((p, i) => ({ id: entity.id, type: "line", p0: p, p1: requireValue(points[(i + 1) % points.length]) }))
}
function fragment(curve: Curve, start: number, end: number, id: string): Line | Arc {
	if (curve.type === "line") return { ...curve, id, p0: at(curve, start), p1: at(curve, end) }
	return {
		id,
		type: "arc",
		center: { ...curve.center },
		radius: curve.radius,
		startAngle: curve.type === "circle" ? start * TAU : curve.startAngle + start * curve.sweep,
		sweep: (end - start) * (curve.type === "circle" ? TAU : curve.sweep),
		segments: Math.max(8, curve.segments),
		construction: curve.construction
	}
}
export type SketchEditOperation = "Trim" | "Split" | "Extend"
/** Return a new sketch. Only relations invalidated by changed endpoints or length are removed. */
export function editSketchCurve(input: Sketch, id: string, click: Point2D, operation: SketchEditOperation, secondClick?: Point2D): { sketch: Sketch; removedRelations: string[] } {
	const sketch = structuredClone(input)
	const curve = sketch.entities.find((e) => e.id === id)
	if (!curve || (curve.type !== "line" && curve.type !== "arc" && curve.type !== "circle" && curve.type !== "spline" && curve.type !== "ellipse" && curve.type !== "ellipticArc"))
		throw Error(`${operation} requires a line, circular arc, circle, spline, ellipse or elliptic arc. Select an individual curve rather than a rectangle, polygon or slot.`)
	if (operation === "Extend" && (curve.type === "ellipse" || (curve.type === "ellipticArc" && Math.abs(curve.sweep) >= TAU))) throw Error("A closed ellipse has no endpoint for extension.")
	const elliptic = curve.type === "ellipse" ? normalizeEllipticArc({ ...curve, type: "ellipticArc", startAngle: 0, sweep: TAU }) : curve.type === "ellipticArc" ? curve : null
	if (curve.type === "spline" && operation === "Extend") throw Error("Spline extension is not supported yet.")
	const splineHit = curve.type === "spline" ? nearestSplinePoint(curve, click) : null
	const splineCount = curve.type === "spline" ? splineBezierSegments(curve).length : 0
	const t =
		curve.type === "spline"
			? (requireValue(splineHit).segment + requireValue(splineHit).t) / splineCount
			: curve.type === "ellipse" || curve.type === "ellipticArc"
				? nearestEllipticArcPoint(requireValue(elliptic), click).t
				: parameter(curve, click)
	if (!Number.isFinite(t)) throw Error("Cannot edit a zero-length curve.")
	const hits =
		curve.type === "ellipse" || curve.type === "ellipticArc"
			? sketch.entities.filter((e) => e.id !== id).flatMap((e) => finiteSketchCurveIntersections(operation === "Extend" ? { ...curve, type: "ellipse" } : curve, e))
			: curve.type === "spline"
				? []
				: sketch.entities
						.filter((e) => e.id !== id)
						.flatMap((entity) =>
							entity.type === "spline"
								? (curve.type === "line"
										? lineSplineIntersections(curve, entity, operation !== "Extend")
										: circleSplineIntersections(curve, entity)
									).map((hit) => hit.point)
								: entity.type === "ellipse" || entity.type === "ellipticArc"
									? (curve.type === "line"
											? lineEllipseIntersections(curve, { ...entity, type: "ellipse" })
											: circleEllipseIntersections(curve, { ...entity, type: "ellipse" })
										).filter((p) => entity.type !== "ellipticArc" || onEllipticSweep(entity, p))
									: boundaries(entity).flatMap((b) => intersections(curve, b).filter((p) => contains(b, p)))
						)
						.filter((p) => operation === "Extend" || contains(curve, p))
	const cuts = (
		curve.type === "spline"
			? sketch.entities
					.filter((e) => e.id !== id)
					.flatMap((e) =>
						e.type === "spline"
							? splineSplineIntersections(curve, e).map((hit) => (hit.segment + hit.t) / splineCount)
							: e.type === "ellipse" || e.type === "ellipticArc"
								? ellipseSplineIntersections({ ...e, type: "ellipse" }, curve)
										.filter((hit) => e.type !== "ellipticArc" || onEllipticSweep(e, hit.point))
										.map((hit) => (hit.segment + hit.t) / splineCount)
								: boundaries(e).flatMap((b) =>
										(b.type === "line"
											? lineSplineIntersections(b, curve)
											: circleSplineIntersections(b, curve).filter((hit) => contains(b, hit.point))
										).map((hit) => (hit.segment + hit.t) / splineCount)
									)
					)
			: hits.map((p) => {
					if (curve.type !== "ellipse" && curve.type !== "ellipticArc") return parameter(curve, p)
					const arc = requireValue(elliptic)
					if (operation !== "Extend") return nearestEllipticArcPoint(arc, p).t
					const full = { ...arc, sweep: Math.sign(arc.sweep) * TAU }
					return (requireValue(ellipticArcParameter(full, ellipticAngle(arc, p))) * TAU) / Math.abs(arc.sweep)
				})
	)
		.sort((a, b) => a - b)
		.filter((v, i, all) => i === 0 || Math.abs(v - requireValue(all[i - 1])) > EPS)
	let intervals: [number, number][] = []
	if (operation === "Split") {
		if (curve.type === "circle" || curve.type === "ellipse") {
			if (!secondClick) throw Error(`Choose a second point to split the ${curve.type}.`)
			const second = curve.type === "ellipse" ? nearestEllipticArcPoint(requireValue(elliptic), secondClick).t : parameter(curve, secondClick)
			const a = Math.min(t, second)
			const b = Math.max(t, second)
			if (b - a < EPS || b - a > 1 - EPS) throw Error("Choose two distinct split points.")
			intervals = [
				[a, b],
				[b, a + 1]
			]
		} else {
			if (!(curve.type === "spline" && curve.closed) && (t <= EPS || t >= 1 - EPS)) throw Error("Choose a split point inside the curve.")
			intervals = [
				[0, t],
				[t, 1]
			]
		}
	} else if (operation === "Extend") {
		if (curve.type === "circle") throw Error("A closed circle has no endpoint to extend.")
		const start = t < 0.5
		const candidates = (curve.type === "arc" || curve.type === "ellipticArc") && start ? cuts.filter((v) => v > 1 + EPS).map((v) => v - TAU / Math.abs(curve.sweep)) : cuts
		const target = start ? candidates.filter((v) => v < -EPS).at(-1) : candidates.find((v) => v > 1 + EPS)
		if (target === undefined) throw Error("No boundary found beyond this endpoint.")
		intervals = [start ? [target, 1] : [0, target]]
	} else if (curve.type === "circle" || curve.type === "ellipse" || (curve.type === "spline" && curve.closed)) {
		if (cuts.length >= 2) {
			const lower = cuts.findLast((v) => v <= t) ?? requireValue(cuts.at(-1)) - 1
			const upper = cuts.find((v) => v > t) ?? requireValue(cuts[0]) + 1
			intervals = [[upper, lower + 1]]
		}
	} else {
		const inside = cuts.filter((v) => v > EPS && v < 1 - EPS)
		const lower = inside.findLast((v) => v < t) ?? 0
		const upper = inside.find((v) => v > t) ?? 1
		if (lower > EPS) intervals.push([0, lower])
		if (upper < 1 - EPS) intervals.push([upper, 1])
	}
	const ids = new Set([...sketch.entities, ...(sketch.relations ?? [])].map((e) => e.id))
	const fresh = (prefix: string) => {
		let n = 1
		while (ids.has(`${prefix}-${n}`)) n++
		const id = `${prefix}-${n}`
		ids.add(id)
		return id
	}
	if (curve.type === "spline" && curve.closed && operation === "Split") intervals = [[t, t + 1]]
	const retainedIntervals = intervals.filter(([a, b]) => b - a > EPS)
	const splinePieces = curve.type === "spline" ? retainedIntervals.map(([a, b], i) => splinePortionWithAnchors(curve, a, b, i === 0 ? id : fresh(`${id}-split`))) : []
	const pieces =
		curve.type === "spline"
			? splinePieces.map((piece) => piece.spline)
			: retainedIntervals.map(([a, b], i) =>
					curve.type === "ellipse" || curve.type === "ellipticArc"
						? normalizeEllipticArc({
								...requireValue(elliptic),
								id: i === 0 ? id : fresh(`${id}-split`),
								startAngle: requireValue(elliptic).startAngle + a * requireValue(elliptic).sweep,
								sweep: (b - a) * requireValue(elliptic).sweep
							})
						: fragment(curve, a, b, i === 0 ? id : fresh(`${id}-split`))
				)
	const mapAnchor = (anchor: SketchAnchor): SketchAnchor | null => {
		if (anchor.entityId !== id) return anchor
		if (!pieces.length) return null
		if (curve.type === "spline") {
			const source = Number(anchor.point.replace(/^point/, ""))
			for (const piece of splinePieces) {
				const target = piece.anchors.get(source)
				if (target !== undefined) return { entityId: piece.spline.id, point: `point${target}` }
			}
			return null
		}
		const point = entityAnchorPoint(curve, anchor.point)
		for (const piece of pieces) for (const name of entityAnchorNames(piece)) if (near(entityAnchorPoint(piece, name), point)) return { entityId: piece.id, point: name }
		return null
	}
	const removedRelations: string[] = []
	sketch.relations = (sketch.relations ?? []).filter((r) => {
		let keep = true
		if ("entityId" in r && r.entityId === id) keep = pieces.length > 0 && (r.reference || (r.type !== "length" && r.type !== "arcSweep"))
		else if (r.type === "fixed") {
			const anchor = mapAnchor(r.anchor)
			if (anchor) {
				r.anchor = anchor
				if (r.fixation) r.fixation = anchor.entityId
			} else keep = false
		} else if (r.type === "linearPattern" || r.type === "circularPattern") {
			keep = !r.sources.includes(id) && !r.instances.flat().includes(id)
			if (r.type === "circularPattern" && r.centerAnchor) {
				const anchor = mapAnchor(r.centerAnchor)
				if (anchor) r.centerAnchor = anchor
				else keep = false
			}
		} else if (r.type === "offsetChain") keep = !r.sources.some((s) => s.entityId === id) && !r.targets.includes(id)
		else if (r.type === "normal" || r.type === "midpoint" || r.type === "pointOnCurve" || r.type === "endpointTangent") {
			const anchor = mapAnchor(r.a)
			keep = r.b !== id && anchor !== null
			if (anchor) {
				r.a = anchor
				if ((r.type === "pointOnCurve" || r.type === "normal" || r.type === "endpointTangent") && r.b === id) {
					const owner = pieces.find((piece) => piece.id === anchor.entityId) ?? sketch.entities.find((entity) => entity.id === anchor.entityId)
					const position = owner ? entityAnchorPoint(owner, anchor.point) : null
					const target = position ? pieces.find((piece) => pointOnSketchCurveResidual(piece, position).every((value) => Math.abs(value) <= 1e-6)) : undefined
					if (target) {
						r.b = target.id
						keep = true
					}
				}
			}
		} else if ("a" in r) {
			if (typeof r.a === "string") keep = (r.a !== id && r.b !== id) || (pieces.length > 0 && r.type !== "offset" && r.type !== "mirror" && r.type !== "internalTangent")
			else {
				const a = mapAnchor(r.a)
				const b = mapAnchor(r.b as SketchAnchor)
				if (a && b) {
					r.a = a
					r.b = b
				} else keep = false
			}
		}
		if ((r.type === "symmetric" || r.type === "mirror") && r.symmetryLine === id) keep = false
		if (r.type === "chamfer" && r.chamferId === id) keep = false
		if (!keep) removedRelations.push(r.id)
		return keep
	})
	sketch.dimensions = sketch.dimensions.filter((d) => {
		if (d.entityId !== id) return true
		removedRelations.push(d.id)
		return false
	})
	sketch.vertices = []
	sketch.loops = []
	sketch.profiles = []
	const index = sketch.entities.indexOf(curve)
	sketch.entities.splice(index, 1, ...pieces)
	if (pieces.length === 2) {
		const a = requireValue(pieces[0])
		const b = requireValue(pieces[1])
		if (a.type === "arc" && b.type === "arc") {
			sketch.relations.push({ id: fresh("concentric"), type: "concentric", a: a.id, b: b.id }, { id: fresh("equal"), type: "equal", a: a.id, b: b.id })
		} else if (a.type === "ellipticArc" && b.type === "ellipticArc") {
			sketch.relations.push({ id: fresh("same-ellipse"), type: "sameEllipse", a: a.id, b: b.id })
		} else if (a.type === "line" && b.type === "line") sketch.relations.push({ id: fresh("collinear"), type: "collinear", a: a.id, b: b.id })
		if (operation === "Split")
			sketch.relations.push({
				id: fresh("coincident"),
				type: "coincident",
				a: { entityId: a.id, point: a.type === "spline" ? `point${a.points.length - 1}` : "p1" },
				b: { entityId: b.id, point: b.type === "spline" ? "point0" : "p0" }
			})
		if (operation === "Split" && (curve.type === "circle" || curve.type === "ellipse"))
			sketch.relations.push({ id: fresh("coincident"), type: "coincident", a: { entityId: a.id, point: "p0" }, b: { entityId: b.id, point: "p1" } })
	}
	return { sketch, removedRelations }
}
