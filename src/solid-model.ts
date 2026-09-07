import { finishSolidEdge, type EdgeFinish } from "./edge-finish"
import modeling from "@jscad/modeling"
import type { Geom2, Geom3 } from "@jscad/modeling/src/geometries/types"
import type { Mat4 } from "@jscad/modeling/src/maths/types"
import { Matrix4, Vector3 } from "three"
import { extrudeSolidFeature } from "./cad/extrude"
import type { PartDocument } from "./schema"
import type { Point2D } from "./types"
import { requireValue } from "./required"

export type SolidCombine = "join" | "cut" | "intersect"
export type ProfileFinish = { kind: "fillet" | "chamfer"; radius: number; vertices?: number[]; segments?: number }
export type SolidStep = {
	id: string
	type: "extrusion" | "revolve"
	operation: SolidCombine
	/** Existing sketch/extrusion feature, for extrusion steps. */
	featureId?: string
	/** Revolve profile coordinates are radius (x), height (y), in millimetres. */
	outline?: Point2D[]
	angle?: number
	segments?: number
	topScale?: number
	translation?: { x: number; y: number; z: number }
	finishes?: ProfileFinish[]
	edgeFinishes?: EdgeFinish[]
}
const { booleans, extrusions, geometries, transforms } = modeling

/** Circular tangent arcs at selected profile vertices. Reject overlapping or degenerate corners. */
export function finishProfile(input: readonly Point2D[], finish: ProfileFinish): Point2D[] {
	const radius = finish.radius
	if (!Number.isFinite(radius) || radius <= 0) throw new Error("Finish radius must be positive.")
	const segments = finish.segments ?? 8
	if (!Number.isInteger(segments) || segments < 1 || segments > 128) throw new Error("Finish segments must be between 1 and 128.")
	const selected = new Set(finish.vertices ?? input.map((_, i) => i))
	for (const i of selected) if (!Number.isInteger(i) || i < 0 || i >= input.length) throw new Error("Unknown profile vertex.")
	const corners = input.map((p, i) => {
		if (!selected.has(i)) return { points: [{ ...p }], trim: 0 }
		const prev = requireValue(input[(i + input.length - 1) % input.length])
		const next = requireValue(input[(i + 1) % input.length])
		const u = new Vector3(prev.x - p.x, prev.y - p.y, 0).normalize()
		const v = new Vector3(next.x - p.x, next.y - p.y, 0).normalize()
		const angle = Math.acos(Math.max(-1, Math.min(1, u.dot(v))))
		if (angle < 1e-6 || Math.PI - angle < 1e-6) throw new Error("Cannot finish a degenerate or straight profile corner.")
		const trim = finish.kind === "chamfer" ? radius : radius / Math.tan(angle / 2)
		const start = new Vector3(p.x, p.y, 0).addScaledVector(u, trim)
		const end = new Vector3(p.x, p.y, 0).addScaledVector(v, trim)
		if (finish.kind === "chamfer") return { trim, points: [start, end].map(({ x, y }) => ({ x, y })) }
		const center = new Vector3(p.x, p.y, 0).addScaledVector(u.clone().add(v).normalize(), radius / Math.sin(angle / 2))
		const a = Math.atan2(start.y - center.y, start.x - center.x)
		let sweep = Math.atan2(end.y - center.y, end.x - center.x) - a
		if (sweep > Math.PI) sweep -= 2 * Math.PI
		if (sweep < -Math.PI) sweep += 2 * Math.PI
		return {
			trim,
			points: Array.from({ length: segments + 1 }, (_, j) => ({
				x: center.x + radius * Math.cos(a + (sweep * j) / segments),
				y: center.y + radius * Math.sin(a + (sweep * j) / segments)
			}))
		}
	})
	for (let i = 0; i < input.length; i++) {
		const p = requireValue(input[i])
		const q = requireValue(input[(i + 1) % input.length])
		if (requireValue(corners[i]).trim + requireValue(corners[(i + 1) % input.length]).trim >= Math.hypot(q.x - p.x, q.y - p.y) - 1e-7)
			throw new Error("Finish radius is too large for adjacent profile edges.")
	}
	return corners.flatMap((c) => c.points)
}
function profile(outline: readonly Point2D[], holes: readonly Point2D[][] = []): Geom2 {
	if (outline.length < 3 || outline.some((p) => !Number.isFinite(p.x) || !Number.isFinite(p.y))) throw new Error("A profile requires at least three finite points.")
	const sides: [[number, number], [number, number]][] = []
	for (const [index, points] of [outline, ...holes].entries()) {
		const area = points.reduce((sum, p, i) => {
			const q = requireValue(points[(i + 1) % points.length])
			return sum + p.x * q.y - q.x * p.y
		}, 0)
		if (Math.abs(area) < 1e-9) throw new Error("Profile area must be nonzero.")
		const oriented = area > 0 === (index === 0) ? points : [...points].reverse()
		for (let i = 0; i < oriented.length; i++) {
			const p = requireValue(oriented[i])
			const q = requireValue(oriented[(i + 1) % oriented.length])
			sides.push([
				[p.x, p.y],
				[q.x, q.y]
			])
		}
	}
	return geometries.geom2.create(sides)
}
function finished(outline: Point2D[], step: SolidStep): Point2D[] {
	return (step.finishes ?? []).reduce((points, finish) => finishProfile(points, finish), outline)
}
function primitive(document: PartDocument, step: SolidStep): Geom3 {
	let solid: Geom3
	if (step.type === "revolve") {
		const outline = finished(requireValue(step.outline), step)
		if (outline.some((p) => p.x < -1e-8)) throw new Error("Revolve profile radii cannot be negative.")
		const angle = step.angle ?? 360
		const segments = step.segments ?? 96
		if (!Number.isFinite(angle) || angle <= 0 || angle > 360) throw new Error("Revolve angle must be in (0, 360].")
		if (!Number.isInteger(segments) || segments < 8 || segments > 1024) throw new Error("Revolve segments must be between 8 and 1024.")
		solid = extrusions.extrudeRotate({ angle: (angle * Math.PI) / 180, segments }, profile(outline))
	} else {
		const feature = document.features.find((f) => f.id === step.featureId && f.type === "extrude")
		if (!feature || feature.type !== "extrude") throw new Error(`Unknown extrusion: ${step.featureId}`)
		const extrusion = extrudeSolidFeature(document, feature)
		const section = profile(finished(requireValue(extrusion.profileLoops[0]), step), extrusion.profileLoops.slice(1))
		const scale = step.topScale ?? 1
		if (!Number.isFinite(scale) || scale <= 0) throw new Error("Top scale must be positive.")
		if (scale === 1) solid = extrusions.extrudeLinear({ height: extrusion.depth }, section)
		else {
			const base = extrusions.slice.fromSides(geometries.geom2.toSides(section))
			solid = extrusions.extrudeFromSlices(
				{
					numberOfSlices: 2,
					callback: (progress) =>
						extrusions.slice.transform(
							new Matrix4().makeScale(1 + progress * (scale - 1), 1 + progress * (scale - 1), 1).setPosition(0, 0, progress * extrusion.depth)
								.elements as Mat4,
							base
						)
				},
				base
			)
		}
		const { origin, xAxis, yAxis, normal } = extrusion.frame
		const matrix = new Matrix4()
			.makeBasis(new Vector3(xAxis.x, xAxis.y, xAxis.z), new Vector3(yAxis.x, yAxis.y, yAxis.z), new Vector3(normal.x, normal.y, normal.z))
			.setPosition(origin.x, origin.y, origin.z)
		solid = transforms.transform(matrix.elements as Mat4, solid)
		const legacy: EdgeFinish[] = document.features
			.filter((f) => f.type === "chamfer" && f.target.edge.extrudeId === feature.id)
			.map((f) => {
				if (f.type !== "chamfer") throw new Error("Invalid chamfer.")
				return { kind: "chamfer", edgeId: f.target.edge.edgeId, radius: f.d1, distance2: f.d2 }
			})
		for (const finish of [...legacy, ...(step.edgeFinishes ?? [])]) solid = finishSolidEdge(solid, extrusion.solid, finish)
	}
	if (step.translation || step.type === "extrusion") {
		const { x, y, z } = step.type === "extrusion" ? extrusionTranslation(document, requireValue(step.featureId)) : requireValue(step.translation)
		if (![x, y, z].every(Number.isFinite)) throw new Error("Translation must be finite.")
		solid = transforms.translate([x, y, z], solid)
	}
	return solid
}
/** Evaluate ordered solid operations. Geometry is shared by preview and export. */
export function evaluateSolid(document: PartDocument): Geom3 {
	const steps: SolidStep[] = document.solidSteps ?? document.features.filter((f) => f.type === "extrude").map((f) => ({ id: f.id, type: "extrusion", featureId: f.id, operation: "join" }))
	if (!steps.length) throw new Error("Part has no solid features.")
	let result: Geom3 | undefined
	const ids = new Set<string>()
	for (const step of steps) {
		if (!step.id?.trim() || ids.has(step.id)) throw new Error("Solid feature ids must be unique and nonempty.")
		ids.add(step.id)
		if (!["extrusion", "revolve"].includes(step.type) || !["join", "cut", "intersect"].includes(step.operation)) throw new Error("Invalid solid operation.")
		const tool = primitive(document, step)
		if (!result) {
			if (step.operation !== "join") throw new Error("The first solid operation must be a join.")
			result = tool
		} else result = step.operation === "join" ? booleans.union(result, tool) : step.operation === "cut" ? booleans.subtract(result, tool) : booleans.intersect(result, tool)
	}
	if (!result || geometries.geom3.toPolygons(result).length === 0) throw new Error("Solid operations produced an empty part.")
	return result
}

/** A face-attached extrusion inherits the explicit translation of its source chain. */
export function extrusionTranslation(document: PartDocument, featureId: string, visited = new Set<string>()): { x: number; y: number; z: number } {
	if (visited.has(featureId)) throw new Error("Cyclic extrusion attachment.")
	visited.add(featureId)
	const offset = { ...(document.solidSteps?.find((s) => s.featureId === featureId)?.translation ?? { x: 0, y: 0, z: 0 }) }
	const feature = document.features.find((f) => f.type === "extrude" && f.id === featureId)
	if (feature?.type !== "extrude") return offset
	const sketch = document.features.find((f) => f.type === "sketch" && f.id === feature.target.sketchId)
	if (sketch?.type === "sketch" && sketch.target.type === "face") {
		const parent = extrusionTranslation(document, sketch.target.face.extrudeId, visited)
		offset.x += parent.x
		offset.y += parent.y
		offset.z += parent.z
	}
	return offset
}
