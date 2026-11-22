import { boundedPolynomialRoots } from "./sketch-ellipse-intersections"
import type { Ellipse } from "./schema"
import type { Point2D } from "./types"
import { ellipsePoint } from "./sketch-ellipse"

/** Exact elliptical geometry. Start and sweep are radians; ellipse rotation is degrees. */
export type EllipticArc = Omit<Ellipse, "type"> & { type: "ellipticArc"; startAngle: number; sweep: number }
const TAU = 2 * Math.PI

export function normalizeEllipticArc(input: unknown): EllipticArc {
	if (!input || typeof input !== "object") throw Error("Invalid elliptic arc.")
	const arc = input as Record<string, unknown>
	const center = arc.center as Point2D | undefined
	if (arc.type !== "ellipticArc" || typeof arc.id !== "string" || !arc.id.trim() || !center || !Number.isFinite(center.x) || !Number.isFinite(center.y)) throw Error("Invalid elliptic arc.")
	for (const name of ["width", "height", "rotation", "startAngle", "sweep", "segments"])
		if (typeof arc[name] !== "number" || !Number.isFinite(arc[name])) throw Error("Elliptic arc parameters must be finite.")
	const value = arc as unknown as EllipticArc
	if (value.width <= 0 || value.height <= 0 || Math.abs(value.sweep) > TAU || value.sweep === 0)
		throw Error("Elliptic arc axes must be positive and its sweep must be nonzero and at most one turn.")
	if (!Number.isInteger(value.segments) || value.segments < 8 || value.segments > 4096) throw Error("Elliptic arc segments must be between 8 and 4096.")
	if (value.construction !== undefined && typeof value.construction !== "boolean") throw Error("Invalid elliptic arc construction flag.")
	return {
		id: value.id,
		type: "ellipticArc",
		center: { x: center.x, y: center.y },
		width: value.width,
		height: value.height,
		rotation: value.rotation,
		startAngle: value.startAngle >= 0 && value.startAngle < TAU ? value.startAngle : ((value.startAngle % TAU) + TAU) % TAU,
		sweep: value.sweep,
		segments: value.segments,
		...(value.construction === undefined ? {} : { construction: value.construction })
	}
}

export function ellipticArcPoint(arc: EllipticArc, t: number): Point2D {
	if (!Number.isFinite(t) || t < 0 || t > 1) throw Error("Elliptic arc parameter must be between zero and one.")
	return ellipsePoint(arc, arc.startAngle + (t === 1 && Math.abs(arc.sweep) === TAU ? 0 : t * arc.sweep))
}

export function ellipticArcPortion(arc: EllipticArc, from: number, to: number, id: string): EllipticArc {
	if (!Number.isFinite(from + to) || from < 0 || to > 1 || from >= to) throw Error("Invalid elliptic arc portion.")
	const source = normalizeEllipticArc(arc)
	return normalizeEllipticArc({ ...source, id, startAngle: source.startAngle + from * source.sweep, sweep: (to - from) * source.sweep })
}

/** Chord interpolation error is bounded by max(axis radius) * angleStep² / 8. */
export function sampleEllipticArc(input: EllipticArc, tolerance = 0.01): Point2D[] {
	const arc = normalizeEllipticArc(input)
	if (!Number.isFinite(tolerance) || tolerance <= 0) throw Error("Elliptic arc tolerance must be positive and finite.")
	const radius = Math.max(arc.width, arc.height) / 2
	const count = Math.max(1, Math.ceil(Math.abs(arc.sweep) * Math.sqrt(radius / (8 * tolerance))))
	if (!Number.isFinite(count) || count > 65535) throw Error("Elliptic arc sampling exceeds point limit.")
	return Array.from({ length: count + 1 }, (_, i) => ellipticArcPoint(arc, i / count))
}

/** Convert an ellipse angle to a parameter only when it lies on the retained arc. */
export function ellipticArcParameter(arc: EllipticArc, angle: number): number | null {
	const directed = arc.sweep > 0 ? angle - arc.startAngle : arc.startAngle - angle
	if (!Number.isFinite(directed)) return null
	const remainder = directed % TAU
	const distance = remainder < 0 ? remainder + TAU : remainder
	const sweep = Math.abs(arc.sweep)
	// Inverse trigonometry can land on either side of an exact endpoint.
	// Apply the same tolerance there without wrapping a tiny positive angle away.
	const tolerance = Math.min(sweep * 1e-6, Math.max(8 * Number.EPSILON * TAU, sweep * 1e-12))
	const startError = Math.min(distance, TAU - distance)
	const endError = Math.abs(distance - sweep)
	if (startError <= tolerance && startError <= endError) return 0
	if (endError <= tolerance) return 1
	return distance <= sweep ? distance / sweep : null
}

export function ellipticArcBounds(input: EllipticArc): { min: Point2D; max: Point2D } {
	const arc = normalizeEllipticArc(input)
	const rotation = (arc.rotation * Math.PI) / 180
	const a = arc.width / 2
	const b = arc.height / 2
	const points = [ellipticArcPoint(arc, 0), ellipticArcPoint(arc, 1)]
	for (const angle of [Math.atan2(-b * Math.sin(rotation), a * Math.cos(rotation)), Math.atan2(b * Math.cos(rotation), a * Math.sin(rotation))]) {
		for (const value of [angle, angle + Math.PI]) {
			const t = ellipticArcParameter(arc, value)
			if (t !== null) points.push(ellipticArcPoint(arc, t))
		}
	}
	return { min: { x: Math.min(...points.map((p) => p.x)), y: Math.min(...points.map((p) => p.y)) }, max: { x: Math.max(...points.map((p) => p.x)), y: Math.max(...points.map((p) => p.y)) } }
}

/** Find all stationary distance candidates using two bounded half-angle quartics. */
export function nearestEllipticArcPoint(input: EllipticArc, target: Point2D): { point: Point2D; t: number; distance: number } {
	const arc = normalizeEllipticArc(input)
	if (!Number.isFinite(target.x) || !Number.isFinite(target.y)) throw Error("Elliptic arc target must be finite.")
	const rotation = (arc.rotation * Math.PI) / 180
	const dx = target.x - arc.center.x
	const dy = target.y - arc.center.y
	const scale = Math.max(arc.width / 2, arc.height / 2, Math.abs(dx), Math.abs(dy))
	const a = arc.width / 2 / scale
	const b = arc.height / 2 / scale
	const x = (dx / scale) * Math.cos(rotation) + (dy / scale) * Math.sin(rotation)
	const y = (-dx / scale) * Math.sin(rotation) + (dy / scale) * Math.cos(rotation)
	const difference = b * b - a * a
	let best = { point: ellipticArcPoint(arc, 0), t: 0, distance: Number.POSITIVE_INFINITY }
	const consider = (t: number) => {
		const point = ellipticArcPoint(arc, t)
		const distance = Math.hypot(point.x - target.x, point.y - target.y)
		if (distance < best.distance) best = { point, t, distance }
	}
	consider(0)
	consider(1)
	for (const sign of [1, -1]) {
		const ax = sign * a * x
		const by = sign * b * y
		for (const root of boundedPolynomialRoots([-by, 2 * (difference + ax), 0, 2 * (ax - difference), by])) {
			const angle = 2 * Math.atan(root) + (sign === -1 ? Math.PI : 0)
			const t = ellipticArcParameter(arc, angle)
			if (t !== null) consider(t)
		}
	}
	return best
}
