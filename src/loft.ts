import modeling from "@jscad/modeling"
import type { Geom3 } from "@jscad/modeling/src/geometries/types"
import type { Mat4 } from "@jscad/modeling/src/maths/types"
import { Matrix4 } from "three"

export type CircularLoft = {
	bottomRadius: number
	topRadius: number
	height: number
	segments: number
}
export function validateCircularLoft(loft: CircularLoft): void {
	if (![loft.bottomRadius, loft.topRadius, loft.height].every((n) => Number.isFinite(n) && n > 0)) throw Error("Loft radii and plane offset must be positive.")
	if (!Number.isInteger(loft.segments) || loft.segments < 8 || loft.segments > 1024) throw Error("Loft segments must be between 8 and 1024.")
}
/** Ruled loft between coaxial circular sections on parallel XY planes. */
export function circularLoft(loft: CircularLoft): Geom3 {
	validateCircularLoft(loft)
	const { extrusions, geometries, primitives } = modeling
	const base = extrusions.slice.fromSides(geometries.geom2.toSides(primitives.circle({ radius: loft.bottomRadius, segments: loft.segments })))
	return extrusions.extrudeFromSlices(
		{
			numberOfSlices: 2,
			callback: (progress) => {
				const scale = 1 + progress * (loft.topRadius / loft.bottomRadius - 1)
				return extrusions.slice.transform(new Matrix4().makeScale(scale, scale, 1).setPosition(0, 0, progress * loft.height).elements as Mat4, base)
			}
		},
		base
	)
}
/** Interior tool for an inward shell with its top face removed. Thickness is normal to the conical wall. */
export function circularLoftInterior(loft: CircularLoft, thickness: number): Geom3 {
	validateCircularLoft(loft)
	if (!Number.isFinite(thickness) || thickness <= 0 || thickness >= loft.height) throw Error("Shell thickness must be positive and smaller than the loft height.")
	const slope = (loft.topRadius - loft.bottomRadius) / loft.height
	const radial = thickness * Math.sqrt(1 + slope * slope)
	const bottomRadius = loft.bottomRadius + slope * thickness - radial
	const topRadius = loft.topRadius - radial
	if (Math.min(bottomRadius, topRadius) <= 0) throw Error("Shell thickness consumes the loft interior.")
	// Extend the tool beyond the open face to avoid coplanar boolean caps.
	const extension = Math.min(0.01, topRadius / (2 * (Math.abs(slope) + 1)))
	const tool = circularLoft({ bottomRadius, topRadius: topRadius + slope * extension, height: loft.height - thickness + extension, segments: loft.segments })
	return modeling.transforms.translate([0, 0, thickness], tool)
}
