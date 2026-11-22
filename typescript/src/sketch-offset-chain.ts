import type { Line, SketchEntity } from "./schema"
import type { Point2D } from "./types"
import { requireValue } from "./required"
import { offsetSketchEntity } from "./sketch-offset"

export type OffsetChainSource = { entityId: string; reversed: boolean }
export type OffsetChainDefinition = { sources: OffsetChainSource[]; closed: boolean; side: 1 | -1 }
const sub = (a: Point2D, b: Point2D): Point2D => ({ x: a.x - b.x, y: a.y - b.y })
const cross = (a: Point2D, b: Point2D) => a.x * b.y - a.y * b.x
const near = (a: Point2D, b: Point2D) => Math.hypot(a.x - b.x, a.y - b.y) < 1e-6

/** Order a single unbranched chain independently of selection order or stored line direction. */
export function orderOffsetChain(entities: readonly SketchEntity[]): OffsetChainDefinition {
	if (entities.length < 2 || entities.some((e) => e.type !== "line")) throw Error("Select a connected chain of at least two lines.")
	const lines = entities as Line[]
	const vertices: { point: Point2D; edges: number[] }[] = []
	const ends = lines.map((line, i) =>
		[line.p0, line.p1].map((point) => {
			let index = vertices.findIndex((v) => near(v.point, point))
			if (index < 0) {
				index = vertices.length
				vertices.push({ point, edges: [] })
			}
			requireValue(vertices[index]).edges.push(i)
			return index
		})
	)
	if (vertices.some((v) => v.edges.length > 2) || lines.some((l) => near(l.p0, l.p1))) throw Error("Offset chains cannot branch or contain zero-length lines.")
	const loose = vertices.map((v, i) => (v.edges.length === 1 ? i : -1)).filter((i) => i >= 0)
	if (loose.length !== 0 && loose.length !== 2) throw Error("Select one connected chain.")
	let vertex = requireValue(loose[0] ?? ends[0]?.[0])
	const used = new Set<number>()
	const sources: OffsetChainSource[] = []
	while (sources.length < lines.length) {
		const next = requireValue(vertices[vertex]).edges.find((i) => !used.has(i))
		if (next === undefined) throw Error("Select one connected chain.")
		const pair = requireValue(ends[next])
		const reversed = pair[1] === vertex
		sources.push({ entityId: requireValue(lines[next]).id, reversed })
		used.add(next)
		vertex = requireValue(pair[reversed ? 0 : 1])
	}
	const closed = loose.length === 0
	const directed = directedOffsetLines(lines, sources)
	const area = directed.reduce((sum, l) => sum + cross(l.p0, l.p1), 0)
	if (closed && Math.abs(area) < 1e-8) throw Error("Closed offset chain must enclose an area.")
	return { sources, closed, side: closed && area > 0 ? -1 : 1 }
}

export function directedOffsetLines(entities: readonly SketchEntity[], sources: readonly OffsetChainSource[]): Line[] {
	return sources.map((source) => {
		const line = entities.find((e) => e.id === source.entityId)
		if (line?.type !== "line") throw Error(`Offset source ${source.entityId} must be a line.`)
		return { ...line, p0: source.reversed ? line.p1 : line.p0, p1: source.reversed ? line.p0 : line.p1 }
	})
}

/** Intersect neighboring supporting lines to obtain exact miter joins. */
export function offsetLineChain(entities: readonly SketchEntity[], definition: OffsetChainDefinition, distance: number, targetIds: readonly string[], validate = true): Line[] {
	const source = directedOffsetLines(entities, definition.sources)
	if (targetIds.length !== source.length) throw Error("Offset chain target count must match its sources.")
	const lines = source.map((line, i) => offsetSketchEntity(line, distance * definition.side, requireValue(targetIds[i])) as Line)
	for (let i = 0; i < lines.length - (definition.closed ? 0 : 1); i++) {
		const a = requireValue(lines[i])
		const b = requireValue(lines[(i + 1) % lines.length])
		const da = sub(requireValue(source[i]).p1, requireValue(source[i]).p0)
		const db = sub(requireValue(source[(i + 1) % source.length]).p1, requireValue(source[(i + 1) % source.length]).p0)
		const determinant = cross(da, db)
		let join: Point2D
		if (Math.abs(determinant) < 1e-10 * Math.hypot(da.x, da.y) * Math.hypot(db.x, db.y)) {
			if (da.x * db.x + da.y * db.y <= 0) throw Error("Offset cannot join a reversing corner.")
			join = { x: (a.p1.x + b.p0.x) / 2, y: (a.p1.y + b.p0.y) / 2 }
		} else {
			const t = cross(sub(b.p0, a.p0), db) / determinant
			join = { x: a.p0.x + t * da.x, y: a.p0.y + t * da.y }
		}
		a.p1 = { ...join }
		b.p0 = { ...join }
	}
	if (validate) {
		lines.forEach((line, i) => {
			const original = requireValue(source[i])
			const a = sub(line.p1, line.p0)
			const b = sub(original.p1, original.p0)
			if (a.x * b.x + a.y * b.y <= 1e-8) throw Error("Offset collapses or reverses an edge. Reduce the distance.")
		})
		for (let i = 0; i < lines.length; i++)
			for (let j = i + 2; j < lines.length; j++) {
				if (definition.closed && i === 0 && j === lines.length - 1) continue
				const a = requireValue(lines[i])
				const b = requireValue(lines[j])
				const da = sub(a.p1, a.p0)
				const db = sub(b.p1, b.p0)
				const d = cross(da, db)
				if (Math.abs(d) < 1e-12) continue
				const delta = sub(b.p0, a.p0)
				const t = cross(delta, db) / d
				const u = cross(delta, da) / d
				if (t >= 0 && t <= 1 && u >= 0 && u <= 1) throw Error("Offset would intersect itself. Reduce the distance.")
			}
	}
	return lines
}
