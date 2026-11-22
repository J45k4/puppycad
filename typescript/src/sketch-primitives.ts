import { ellipsePoints } from "./sketch-ellipse"
import { polygonVertices } from "./sketch-polygon"
import type { SketchEntity, SketchPrimitive } from "./schema"
import type { Point2D } from "./types"
import { circle as polygonCircle, capsule as polygonCapsule, rectangle as polygonRectangle } from "./model-dsl"
export type PrimitiveOutline =
	| Omit<Extract<SketchPrimitive, { type: "ellipse" }>, "id">
	| Omit<Extract<SketchPrimitive, { type: "polygon" }>, "id">
	| Omit<Extract<SketchPrimitive, { type: "point" }>, "id">
	| Omit<Extract<SketchPrimitive, { type: "circle" }>, "id">
	| Omit<Extract<SketchPrimitive, { type: "capsule" }>, "id">
	| Omit<Extract<SketchPrimitive, { type: "rectangle" }>, "id">
export type SketchOutline = readonly Point2D[] | PrimitiveOutline
export function isSketchPrimitive(entity: { type: string }): entity is SketchPrimitive {
	return entity.type === "ellipse" || entity.type === "polygon" || entity.type === "point" || entity.type === "circle" || entity.type === "capsule" || entity.type === "rectangle"
}
export function primitivePoints(primitive: PrimitiveOutline): Point2D[] {
	switch (primitive.type) {
		case "ellipse":
			return ellipsePoints(primitive)
		case "polygon":
			return polygonVertices(primitive)
		case "point":
			return [{ ...primitive.center }]
		case "circle":
			return polygonCircle(primitive.center, primitive.radius, primitive.segments)
		case "capsule":
			return polygonCapsule(primitive.from, primitive.to, primitive.width, primitive.arcSegments)
		case "rectangle":
			return polygonRectangle(primitive.center, primitive.width, primitive.height, primitive.rotation)
	}
}
export function ellipse(center: Point2D, width: number, height: number, rotation = 0, segments = 128): Omit<Extract<SketchPrimitive, { type: "ellipse" }>, "id"> {
	const result = { type: "ellipse" as const, center: { ...center }, width, height, rotation, segments }
	ellipsePoints(result)
	return result
}
export function polygon(center: Point2D, radius: number, sides = 6, rotation = 0): Omit<Extract<SketchPrimitive, { type: "polygon" }>, "id"> {
	const result = { type: "polygon" as const, center: { ...center }, radius, sides, rotation }
	polygonVertices(result)
	return result
}
export function sketchPoint(center: Point2D): Omit<Extract<SketchPrimitive, { type: "point" }>, "id"> {
	if (!Number.isFinite(center.x) || !Number.isFinite(center.y)) throw Error("Sketch point coordinates must be finite.")
	return { type: "point", center: { ...center } }
}
export function circle(center: Point2D, radius: number, segments = 24): Omit<Extract<SketchPrimitive, { type: "circle" }>, "id"> {
	const result = { type: "circle" as const, center: { ...center }, radius, segments }
	primitivePoints(result)
	return result
}
export function capsule(from: Point2D, to: Point2D, width: number, arcSegments = 7): Omit<Extract<SketchPrimitive, { type: "capsule" }>, "id"> {
	const result = { type: "capsule" as const, from: { ...from }, to: { ...to }, width, arcSegments }
	primitivePoints(result)
	return result
}
export function rectangle(center: Point2D, width: number, height: number, rotation = 0): Omit<Extract<SketchPrimitive, { type: "rectangle" }>, "id"> {
	const result = { type: "rectangle" as const, center: { ...center }, width, height, rotation }
	primitivePoints(result)
	return result
}
export function outlineEntities(outline: SketchOutline, prefix: string): SketchEntity[] {
	if (!Array.isArray(outline)) return [{ ...structuredClone(outline as PrimitiveOutline), id: prefix }]
	return outline.map((p, i) => ({ type: "line", id: `${prefix}/${i}`, p0: { ...p }, p1: { ...outline[(i + 1) % outline.length] } }))
}
export function normalizePrimitive(input: unknown, id: string): SketchPrimitive | undefined {
	if (!input || typeof input !== "object") return
	const v = input as Record<string, unknown>
	const point = (p: unknown): p is Point2D => !!p && typeof p === "object" && Number.isFinite((p as Point2D).x) && Number.isFinite((p as Point2D).y)
	const positive = (n: unknown): n is number => typeof n === "number" && Number.isFinite(n) && n > 0
	const segments = (n: unknown, min: number): n is number => positive(n) && Number.isInteger(n) && n >= min && n <= 4096
	if (
		v.type === "polygon" &&
		point(v.center) &&
		positive(v.radius) &&
		typeof v.rotation === "number" &&
		Number.isFinite(v.rotation) &&
		typeof v.sides === "number" &&
		Number.isInteger(v.sides) &&
		v.sides >= 3 &&
		v.sides <= 64
	)
		return { id, type: "polygon", center: { ...v.center }, radius: v.radius, rotation: v.rotation, sides: v.sides, ...(v.construction === true ? { construction: true } : {}) }
	if (v.type === "ellipse" && point(v.center) && positive(v.width) && positive(v.height) && typeof v.rotation === "number" && Number.isFinite(v.rotation) && segments(v.segments, 8))
		return {
			id,
			type: "ellipse",
			center: { ...v.center },
			width: v.width,
			height: v.height,
			rotation: v.rotation,
			segments: v.segments,
			...(v.construction === true ? { construction: true } : {})
		}
	if (v.type === "point" && point(v.center)) return { id, type: "point", center: { ...v.center }, ...(v.construction === true ? { construction: true } : {}) }
	if (v.type === "circle" && point(v.center) && positive(v.radius) && segments(v.segments, 3))
		return { id, ...(v.construction === true ? { construction: true } : {}), type: "circle", center: { ...v.center }, radius: v.radius, segments: v.segments }
	if (v.type === "capsule" && point(v.from) && point(v.to) && positive(v.width) && segments(v.arcSegments, 1) && Math.hypot(v.from.x - v.to.x, v.from.y - v.to.y) > 1e-9)
		return { id, ...(v.construction === true ? { construction: true } : {}), type: "capsule", from: { ...v.from }, to: { ...v.to }, width: v.width, arcSegments: v.arcSegments }
	if (v.type === "rectangle" && point(v.center) && positive(v.width) && positive(v.height) && typeof v.rotation === "number" && Number.isFinite(v.rotation))
		return { id, ...(v.construction === true ? { construction: true } : {}), type: "rectangle", center: { ...v.center }, width: v.width, height: v.height, rotation: v.rotation }
}
