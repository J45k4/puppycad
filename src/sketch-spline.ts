import type { Spline } from "./schema"
import type { Point2D } from "./types"
import { cubicBezierDerivative, cubicBezierPoint, interpolateBezierSpline, interpolatePeriodicBezierSpline, sampleCubicBezier, splitCubicBezier, type CubicBezier } from "./sketch-bezier"
import { requireValue } from "./required"

/** Validate serialized native data; never substitute sampled line geometry. */
export function normalizeSpline(value: unknown): Spline {
	if (!value || typeof value !== "object") throw Error("Invalid spline data.")
	const data = value as Record<string, unknown>
	if (data.type !== "spline" || typeof data.id !== "string" || !data.id || (data.mode !== "fit" && data.mode !== "control") || !Array.isArray(data.points)) throw Error("Invalid spline data.")
	if (data.closed !== undefined && typeof data.closed !== "boolean") throw Error("Invalid spline closure flag.")
	if (data.closed === true && data.mode !== "fit") throw Error("Periodic closure requires a fit spline.")
	if (data.construction !== undefined && typeof data.construction !== "boolean") throw Error("Invalid spline construction flag.")
	if (data.points.length < 2 || data.points.length > 4096) throw Error("Spline requires 2–4096 points.")
	const points = data.points.map((point: unknown) => {
		if (!point || typeof point !== "object") throw Error("Invalid spline point.")
		const p = point as Record<string, unknown>
		if (typeof p.x !== "number" || typeof p.y !== "number" || !Number.isFinite(p.x) || !Number.isFinite(p.y)) throw Error("Spline points must be finite.")
		return { x: p.x, y: p.y }
	})
	if (data.mode === "control" && (points.length < 4 || (points.length - 1) % 3 !== 0)) throw Error("Cubic control splines require 3n + 1 points.")
	if (data.mode === "fit") (data.closed ? interpolatePeriodicBezierSpline : interpolateBezierSpline)(points)
	return {
		id: data.id,
		type: "spline",
		mode: data.mode,
		points,
		...(data.closed === undefined ? {} : { closed: data.closed }),
		...(data.construction === undefined ? {} : { construction: data.construction })
	}
}
export function splineBezierSegments(spline: Spline): CubicBezier[] {
	const validated = normalizeSpline(spline)
	if (validated.mode === "fit") return (validated.closed ? interpolatePeriodicBezierSpline : interpolateBezierSpline)(validated.points)
	const segments: CubicBezier[] = []
	for (let i = 0; i + 3 < validated.points.length; i += 3)
		segments.push([
			{ ...requireValue(validated.points[i]) },
			{ ...requireValue(validated.points[i + 1]) },
			{ ...requireValue(validated.points[i + 2]) },
			{ ...requireValue(validated.points[i + 3]) }
		])
	return segments
}

/** Samples an open chain. Shared joins belong to the preceding segment at t=1. */
export function sampleSpline(spline: Spline, tolerance: number): { point: Point2D; segment: number; t: number }[] {
	const result: { point: Point2D; segment: number; t: number }[] = []
	for (const [segment, curve] of splineBezierSegments(spline).entries()) {
		const samples = sampleCubicBezier(curve, tolerance)
		for (const sample of segment === 0 ? samples : samples.slice(1)) {
			if (result.length >= 65536) throw Error("Spline sampling exceeds point limit.")
			result.push({ ...sample, segment })
		}
	}
	return result
}

/** Exact native subdivision. Fit curves become control chains to retain their shape. */
export function splitSpline(spline: Spline, segment: number, t: number, secondId: string): [Spline, Spline] {
	if (spline.closed) throw Error("Splitting periodic splines is not supported yet.")
	const curves = splineBezierSegments(spline)
	if (!Number.isInteger(segment) || segment < 0 || segment >= curves.length || !Number.isFinite(t) || t < 0 || t > 1) throw Error("Invalid spline split parameter.")
	if ((segment === 0 && t === 0) || (segment === curves.length - 1 && t === 1)) throw Error("Choose a split point inside the curve.")
	if (!secondId.trim() || secondId === spline.id) throw Error("Split pieces require distinct non-empty IDs.")
	const [left, right] = splitCubicBezier(requireValue(curves[segment]), t)
	const before = [...curves.slice(0, segment), ...(t > 0 ? [left] : [])]
	const after = [...(t < 1 ? [right] : []), ...curves.slice(segment + 1)]
	const piece = (chain: CubicBezier[], id: string): Spline =>
		normalizeSpline({
			...spline,
			id,
			mode: "control",
			points: chain.flatMap((curve, index) => (index === 0 ? curve : curve.slice(1)))
		})
	return [piece(before, spline.id), piece(after, secondId)]
}

/** Locate the nearest curve point, bounding distance error by tolerance in sketch units. */
export function nearestSplinePoint(spline: Spline, target: Point2D, tolerance = 1e-7): { point: Point2D; segment: number; t: number; distance: number } {
	if (!Number.isFinite(target.x) || !Number.isFinite(target.y) || !Number.isFinite(tolerance) || tolerance <= 0) throw Error("Spline target and positive tolerance must be finite.")
	const curves = splineBezierSegments(spline)
	let best = { point: { ...requireValue(curves[0])[0] }, segment: 0, t: 0, distance: Number.POSITIVE_INFINITY }
	let visits = 0
	const consider = (segment: number, t: number) => {
		const point = cubicBezierPoint(requireValue(curves[segment]), t)
		const distance = Math.hypot(point.x - target.x, point.y - target.y)
		if (distance < best.distance) best = { point, segment, t, distance }
	}
	const lowerBound = (curve: CubicBezier) => {
		const xs = curve.map((p) => p.x)
		const ys = curve.map((p) => p.y)
		return Math.hypot(Math.max(Math.min(...xs) - target.x, 0, target.x - Math.max(...xs)), Math.max(Math.min(...ys) - target.y, 0, target.y - Math.max(...ys)))
	}
	for (let segment = 0; segment < curves.length; segment++) {
		consider(segment, 0)
		consider(segment, 1)
		consider(segment, 0.5)
	}
	const visit = (curve: CubicBezier, segment: number, from: number, to: number, depth: number) => {
		if (lowerBound(curve) >= best.distance - tolerance) return
		if (++visits > 200000 || depth >= 50) throw Error("Spline targeting cannot meet the requested tolerance.")
		const middle = (from + to) / 2
		consider(segment, middle)
		const [left, right] = splitCubicBezier(curve, 0.5)
		if (lowerBound(left) <= lowerBound(right)) {
			visit(left, segment, from, middle, depth + 1)
			visit(right, segment, middle, to, depth + 1)
		} else {
			visit(right, segment, middle, to, depth + 1)
			visit(left, segment, from, middle, depth + 1)
		}
	}
	for (const [segment, curve] of curves.entries()) visit(curve, segment, 0, 1, 0)
	// Refine within the winning segment for stable constraint Jacobians.
	const curve = requireValue(curves[best.segment])
	for (let iteration = 0; iteration < 12; iteration++) {
		const t = best.t
		const d = cubicBezierDerivative(curve, t)
		const dd = {
			x: 6 * ((1 - t) * (curve[2].x - 2 * curve[1].x + curve[0].x) + t * (curve[3].x - 2 * curve[2].x + curve[1].x)),
			y: 6 * ((1 - t) * (curve[2].y - 2 * curve[1].y + curve[0].y) + t * (curve[3].y - 2 * curve[2].y + curve[1].y))
		}
		const dx = best.point.x - target.x
		const dy = best.point.y - target.y
		const denominator = d.x * d.x + d.y * d.y + dx * dd.x + dy * dd.y
		if (denominator <= 1e-20) break
		const next = Math.max(0, Math.min(1, t - (dx * d.x + dy * d.y) / denominator))
		consider(best.segment, next)
		if (best.t === t) break
	}
	return best
}

/** Open a periodic curve at a parameter without changing its geometric locus. */
export function openPeriodicSpline(spline: Spline, segment: number, t: number): Spline {
	if (!spline.closed) throw Error("Opening requires a closed spline.")
	const curves = splineBezierSegments(spline)
	if (!Number.isInteger(segment) || segment < 0 || segment >= curves.length || !Number.isFinite(t) || t < 0 || t > 1) throw Error("Invalid spline split parameter.")
	const [left, right] = splitCubicBezier(requireValue(curves[segment]), t)
	const chain = [...(t < 1 ? [right] : []), ...curves.slice(segment + 1), ...curves.slice(0, segment), ...(t > 0 ? [left] : [])]
	return normalizeSpline({ ...spline, mode: "control", closed: false, points: chain.flatMap((curve, i) => (i === 0 ? curve : curve.slice(1))) })
}

/** Exact portion in normalized segment coordinates; periodic portions may cross the seam. */
export function splinePortion(spline: Spline, from: number, to: number, id: string): Spline {
	return splinePortionWithAnchors(spline, from, to, id).spline
}

/** Retain source anchor indices by segment identity, never by coincident coordinates. */
export function splinePortionWithAnchors(spline: Spline, from: number, to: number, id: string): { spline: Spline; anchors: Map<number, number> } {
	const curves = splineBezierSegments(spline)
	if (!Number.isFinite(from + to) || from < 0 || to <= from || to - from > 1 + 1e-12 || (!spline.closed && to > 1)) throw Error("Invalid spline portion.")
	// Normalizing a join by the segment count and multiplying back can move it
	// by one rounding unit. Preserve exact joins without absorbing nearby cuts.
	const segmentCoordinate = (value: number) => {
		const coordinate = value * curves.length
		const join = Math.round(coordinate)
		return Math.abs(coordinate - join) <= 4 * Number.EPSILON * Math.max(1, Math.abs(coordinate)) ? join : coordinate
	}
	const start = segmentCoordinate(from)
	const end = segmentCoordinate(to)
	const pieces: CubicBezier[] = []
	const anchors = new Map<number, number>()
	for (let i = Math.floor(start); i < Math.ceil(end); i++) {
		const lo = Math.max(0, start - i)
		const hi = Math.min(1, end - i)
		if (hi - lo < 1e-12) continue
		let piece = requireValue(curves[i % curves.length])
		if (hi < 1) piece = splitCubicBezier(piece, hi)[0]
		if (lo > 0) piece = splitCubicBezier(piece, lo / hi)[1]
		const segment = i % curves.length
		const offset = pieces.length * 3
		const retain = (source: number, target: number) => {
			if (!anchors.has(source)) anchors.set(source, target)
		}
		if (lo === 0) retain(spline.mode === "fit" ? segment : segment * 3, offset)
		if (hi === 1) retain(spline.mode === "fit" ? (spline.closed ? (segment + 1) % curves.length : segment + 1) : segment * 3 + 3, offset + 3)
		if (spline.mode === "control" && lo === 0 && hi === 1) {
			retain(segment * 3 + 1, offset + 1)
			retain(segment * 3 + 2, offset + 2)
		}
		pieces.push(piece)
	}
	if (spline.closed && to === from + 1) {
		const last = requireValue(pieces.at(-1))
		pieces[pieces.length - 1] = [last[0], last[1], last[2], { ...requireValue(pieces[0])[0] }]
	}
	return { spline: normalizeSpline({ ...spline, id, mode: "control", closed: false, points: pieces.flatMap((piece, i) => (i === 0 ? piece : piece.slice(1))) }), anchors }
}
