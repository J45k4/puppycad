import type { Ellipse, Line } from "./schema"
import type { Point2D } from "./types"

/** Analytic intersection of a supporting line (or finite segment) with a rotated ellipse. */
export function lineEllipseIntersections(line: Line, ellipse: Ellipse, finite = false): Point2D[] {
	const angle = (ellipse.rotation * Math.PI) / 180
	const c = Math.cos(angle)
	const s = Math.sin(angle)
	const rx = ellipse.width / 2
	const ry = ellipse.height / 2
	if (!(rx > 0 && ry > 0) || ![rx, ry, angle].every(Number.isFinite)) throw Error("Ellipse axes and rotation must be valid.")
	const x = line.p0.x - ellipse.center.x
	const y = line.p0.y - ellipse.center.y
	const f = { x: (x * c + y * s) / rx, y: (-x * s + y * c) / ry }
	const dx = line.p1.x - line.p0.x
	const dy = line.p1.y - line.p0.y
	const d = { x: (dx * c + dy * s) / rx, y: (-dx * s + dy * c) / ry }
	const length = Math.hypot(d.x, d.y)
	if (length === 0) return Math.abs(f.x * f.x + f.y * f.y - 1) < 1e-12 ? [{ ...line.p0 }] : []
	const u = { x: d.x / length, y: d.y / length }
	const along = f.x * u.x + f.y * u.y
	const perpendicular = u.x * f.y - u.y * f.x
	const discriminant = 1 - perpendicular * perpendicular
	if (discriminant < -1e-12) return []
	const root = Math.abs(discriminant) <= 1e-12 ? 0 : Math.sqrt(discriminant)
	const distances = root === 0 ? [0] : [-root, root]
	return distances.flatMap((distance) => {
		const t = (distance - along) / length
		if (finite && (t < -1e-10 || t > 1 + 1e-10)) return []
		// Reconstruct from the ellipse to avoid cancellation on very long lines.
		const px = (-u.y * perpendicular + u.x * distance) * rx
		const py = (u.x * perpendicular + u.y * distance) * ry
		return [{ x: ellipse.center.x + px * c - py * s, y: ellipse.center.y + px * s + py * c }]
	})
}

/** Isolate all real roots in [-1, 1], including repeated roots at derivative extrema. */
export function boundedPolynomialRoots(input: number[]): number[] {
	const scale = Math.max(...input.map(Math.abs))
	if (scale < 1e-14) return []
	const coefficients = input.map((v) => v / scale)
	while (coefficients.length > 1 && Math.abs(coefficients[coefficients.length - 1] ?? 0) < 1e-14) coefficients.pop()
	if (coefficients.length < 2) return []
	const evaluate = (x: number) => coefficients.reduceRight((value, coefficient) => value * x + coefficient, 0)
	const critical = coefficients.length === 2 ? [] : boundedPolynomialRoots(coefficients.slice(1).map((value, i) => value * (i + 1)))
	const divisions = [-1, ...critical.filter((v) => v > -1 && v < 1), 1].sort((a, b) => a - b)
	const roots = divisions.filter((x) => Math.abs(evaluate(x)) < 1e-12)
	for (let i = 1; i < divisions.length; i++) {
		let lo = divisions[i - 1] ?? -1
		let hi = divisions[i] ?? 1
		let lowValue = evaluate(lo)
		const highValue = evaluate(hi)
		if (Math.abs(lowValue) < 1e-12 || Math.abs(highValue) < 1e-12 || lowValue * highValue >= 0) continue
		for (let step = 0; step < 70; step++) {
			const mid = (lo + hi) / 2
			const value = evaluate(mid)
			if (value === 0) {
				lo = mid
				hi = mid
				break
			}
			if (lowValue * value <= 0) hi = mid
			else {
				lo = mid
				lowValue = value
			}
		}
		roots.push((lo + hi) / 2)
	}
	return roots.sort((a, b) => a - b).filter((value, i, all) => i === 0 || Math.abs(value - (all[i - 1] ?? 0)) > 1e-10)
}

/** Supporting-circle intersections with a rotated ellipse, without tessellation.
 * Two bounded half-angle charts avoid roots at infinity and unstable huge root bounds.
 */
export function circleEllipseIntersections(circle: { center: Point2D; radius: number }, ellipse: Ellipse): Point2D[] {
	const rx = ellipse.width / 2
	const ry = ellipse.height / 2
	if (![rx, ry, circle.radius].every((v) => Number.isFinite(v) && v > 0)) throw Error("Circle and ellipse sizes must be positive.")
	const angle = (ellipse.rotation * Math.PI) / 180
	const c = Math.cos(angle)
	const s = Math.sin(angle)
	const dx = circle.center.x - ellipse.center.x
	const dy = circle.center.y - ellipse.center.y
	if (Math.hypot(dx, dy) > Math.max(rx, ry) + circle.radius) return []
	const length = Math.max(rx, ry, circle.radius, Math.abs(dx), Math.abs(dy))
	const x = (dx * c + dy * s) / length
	const y = (-dx * s + dy * c) / length
	const r = circle.radius / length
	const points: Point2D[] = []
	for (const sign of [1, -1]) {
		const a = (sign * rx) / length - x
		const b = (-sign * rx) / length - x
		const d = (2 * sign * ry) / length
		const coefficients = [a * a + y * y - r * r, -2 * y * d, 2 * a * b + d * d + 2 * y * y - 2 * r * r, -2 * y * d, b * b + y * y - r * r]
		for (const t of boundedPolynomialRoots(coefficients)) {
			const px = (sign * rx * (1 - t * t)) / (1 + t * t)
			const py = (sign * ry * 2 * t) / (1 + t * t)
			const point = { x: ellipse.center.x + px * c - py * s, y: ellipse.center.y + px * s + py * c }
			if (!points.some((p) => Math.hypot(p.x - point.x, p.y - point.y) < 1e-8 * Math.max(1, rx, ry))) points.push(point)
		}
	}
	return points
}

/** Isolated contacts of two rotated ellipses. Coincident outlines have no isolated contacts. */
export function ellipseEllipseIntersections(a: Ellipse, b: Ellipse): Point2D[] {
	for (const ellipse of [a, b]) {
		if (![ellipse.width, ellipse.height].every((v) => Number.isFinite(v) && v > 0) || ![ellipse.rotation, ellipse.center.x, ellipse.center.y].every(Number.isFinite))
			throw Error("Ellipse axes, center and rotation must be valid.")
	}
	const aa = (a.rotation * Math.PI) / 180
	const ba = (b.rotation * Math.PI) / 180
	const c = Math.cos(aa)
	const s = Math.sin(aa)
	const bc = Math.cos(ba)
	const bs = Math.sin(ba)
	const dx = a.center.x - b.center.x
	const dy = a.center.y - b.center.y
	const rx = a.width / 2
	const ry = a.height / 2
	if (Math.hypot(dx, dy) > Math.max(rx, ry) + Math.max(b.width, b.height) / 2) return []
	// Express A's center and two semi-axis vectors in B's unit-circle coordinates.
	const center = { x: (2 * (dx * bc + dy * bs)) / b.width, y: (2 * (-dx * bs + dy * bc)) / b.height }
	const u = { x: (2 * rx * (c * bc + s * bs)) / b.width, y: (2 * rx * (-c * bs + s * bc)) / b.height }
	const v = { x: (2 * ry * (-s * bc + c * bs)) / b.width, y: (2 * ry * (s * bs + c * bc)) / b.height }
	const points: Point2D[] = []
	for (const sign of [1, -1]) {
		const x = [center.x + sign * u.x, 2 * sign * v.x, center.x - sign * u.x]
		const y = [center.y + sign * u.y, 2 * sign * v.y, center.y - sign * u.y]
		const coefficients = [-1, 0, -2, 0, -1]
		for (let i = 0; i < 3; i++) for (let j = 0; j < 3; j++) coefficients[i + j] = (coefficients[i + j] ?? 0) + (x[i] ?? 0) * (x[j] ?? 0) + (y[i] ?? 0) * (y[j] ?? 0)
		for (const t of boundedPolynomialRoots(coefficients)) {
			const px = (sign * rx * (1 - t * t)) / (1 + t * t)
			const py = (sign * ry * 2 * t) / (1 + t * t)
			const point = { x: a.center.x + px * c - py * s, y: a.center.y + px * s + py * c }
			if (!points.some((p) => Math.hypot(p.x - point.x, p.y - point.y) < 1e-8 * Math.max(1, rx, ry))) points.push(point)
		}
	}
	return points
}
