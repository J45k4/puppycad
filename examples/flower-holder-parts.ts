import { capsule, circle, v2, type PartBuilder } from "../src/sdk"
import type { Vector3D } from "../src/contract"

/** Dimensions recovered from the Onshape glTF, in mm. All exported parts start at z=0. */
export const FLOWER_HOLDER = {
	ringRadius: 105,
	armPinOffset: 105 / Math.sqrt(2),
	hubOffset: 45 / Math.sqrt(2),
	upperOpening: { x: -6.22381, y: -5.98907 },
	lowerOpening: { x: -11.7784, y: -11.60518 },
	lowerPosition: { x: -16.31757616996765, y: 2.760103438049555, z: 246.58972024917603 },
	bowlPosition: { x: -44.163644313812256, y: -9.914782829582691, z: 48.33023250102997 }
} as const
const segments = 144
const origin = v2(0, 0)
export const ringLegLocations = [v2(105, 0), v2(0, 105), v2(-105, 0), v2(0, -105)]
export const holderPinLocations = [
	v2(FLOWER_HOLDER.armPinOffset, FLOWER_HOLDER.armPinOffset),
	v2(-FLOWER_HOLDER.armPinOffset, FLOWER_HOLDER.armPinOffset),
	v2(FLOWER_HOLDER.armPinOffset, -FLOWER_HOLDER.armPinOffset)
]
export const holderHub = v2(FLOWER_HOLDER.hubOffset, FLOWER_HOLDER.hubOffset)
export type FlowerHolderPart = { id: string; name: string; referenceMesh: number; localZOffset: number; build: (part: PartBuilder) => void }
function cutOpening(part: PartBuilder, edge: { x: number; y: number }, depth: number) {
	part.extrude("access-opening", { outline: [v2(-150, -150), v2(edge.x, -150), v2(edge.x, edge.y), v2(-150, edge.y)], depth, operation: "cut" })
}
function ring(part: PartBuilder, id: string, outer: number, inner: number, depth: number, z = 0) {
	part.extrude(id, { outline: circle(origin, outer, segments), holes: [circle(origin, inner, segments)], depth, translation: { x: 0, y: 0, z } })
}
export const flowerHolderParts: readonly FlowerHolderPart[] = [
	{
		id: "bowl",
		name: "Bowl",
		referenceMesh: 3,
		localZOffset: 0,
		build(part) {
			// 2.5 mm normal wall thickness on a cone with dr/dz = 0.275.
			const wallRadial = 2.5 * Math.sqrt(1 + 0.275 ** 2)
			part.revolve("shell", { outline: [v2(0, 0), v2(85, 0), v2(112.5, 100), v2(112.5 - wallRadial, 100), v2(85 + 0.275 * 2.5 - wallRadial, 2.5), v2(0, 2.5)], segments })
		}
	},
	{
		id: "lower-ring",
		name: "Lower channel ring",
		referenceMesh: 2,
		localZOffset: 0,
		build(part) {
			ring(part, "inner-wall", 100, 97, 25)
			ring(part, "outer-wall", 113, 110, 25)
			// Four bridge plates use the measured, slightly asymmetric sketch edge angles.
			const low = (x: number) => -9.34279 + ((x - 99.56261) * (-9.89206 + 9.34279)) / (109.55431 - 99.56261)
			const high = (x: number) => 8.83798 + ((x - 99.60868) * (9.79986 - 8.83798)) / (109.5626 - 99.60868)
			const bridge = [v2(96, low(96)), v2(114, low(114)), v2(114, high(114)), v2(96, high(96))]
			ringLegLocations.forEach((point, index) => {
				const angle = (index * Math.PI) / 2
				const rotated = bridge.map((p) => v2(p.x * Math.cos(angle) - p.y * Math.sin(angle), p.x * Math.sin(angle) + p.y * Math.cos(angle)))
				part.extrude(`bridge-${index + 1}`, { outline: rotated, depth: 5, translation: { x: 0, y: 0, z: 20 } })
				part.extrude(`leg-socket-${index + 1}`, { outline: circle(point, 5, 64), depth: 5.2, translation: { x: 0, y: 0, z: 19.9 }, operation: "cut" })
			})
			part.extrude("trim-bridges", { outline: circle(origin, 113, segments), holes: [circle(origin, 97, segments)], depth: 25, operation: "intersect" })
			cutOpening(part, FLOWER_HOLDER.lowerOpening, 25)
		}
	},
	{
		id: "upper-ring",
		name: "Upper ring with four legs",
		referenceMesh: 1,
		localZOffset: -60,
		build(part) {
			ring(part, "top-ring", 113, 97, 5, 60)
			cutOpening(part, FLOWER_HOLDER.upperOpening, 65)
			holderPinLocations.forEach((point, index) =>
				part.extrude(`holder-hole-${index + 1}`, { outline: circle(point, 7, 64), depth: 5, translation: { x: 0, y: 0, z: 60 }, operation: "cut" })
			)
			ringLegLocations.forEach((point, index) => part.extrude(`leg-${index + 1}`, { outline: circle(point, 4.95, 64), depth: 60 }))
		}
	},
	{
		id: "centre-holder",
		name: "Three-arm centre holder",
		referenceMesh: 0,
		localZOffset: -5,
		build(part) {
			part.extrude("hub", { outline: circle(holderHub, 16, 96), depth: 8, translation: { x: 0, y: 0, z: 10 } })
			holderPinLocations.forEach((point, index) => {
				part.extrude(`arm-${index + 1}`, { outline: capsule(holderHub, point, 20, 32), depth: 8, translation: { x: 0, y: 0, z: 10 } })
				part.extrude(`pin-${index + 1}`, { outline: circle(point, 6.475, 64), depth: 10 })
			})
			part.extrude("stem-hole", { outline: circle(holderHub, 6.65, 64), depth: 8, translation: { x: 0, y: 0, z: 10 }, operation: "cut" })
		}
	},
	{
		id: "stem",
		name: "250 mm perforated stem",
		referenceMesh: 4,
		localZOffset: -14,
		build(part) {
			part.revolve("stem-profile", { outline: [v2(0, 0), v2(5.675, 0), v2(6.475, 0.8), v2(6.475, 14), v2(11, 14), v2(11, 17), v2(8, 22), v2(8, 264), v2(0, 264)], segments: 96 })
			part.fillet("stem-profile", 3, [7], 15)
			// XZ sketch's second coordinate points toward -Z; extrude in +Y through the stem.
			for (const [index, z] of [5, 59, 104, 149, 194, 244].entries())
				part.extrude(`cross-hole-${index + 1}`, {
					outline: circle(v2(0, -z), index === 0 ? 1.75 : 2.5, 48),
					depth: 24,
					on: { type: "plane", plane: "XZ" },
					translation: { x: 0, y: -12, z: 0 },
					operation: "cut"
				})
		}
	}
]
export function referencePartPosition(id: string): Vector3D {
	const lower = FLOWER_HOLDER.lowerPosition
	if (id === "bowl") return { ...FLOWER_HOLDER.bowlPosition }
	if (id === "lower-ring") return { ...lower }
	if (id === "upper-ring") return { ...lower, z: lower.z + 20 }
	if (id === "centre-holder") return { ...lower, z: lower.z + 75 }
	if (id === "stem") return { x: lower.x + holderHub.x, y: lower.y + holderHub.y, z: lower.z + 79 }
	throw new Error(`Unknown flower holder part: ${id}`)
}
