import { describe, expect, it } from "bun:test"
import type { PartDocument, Sketch, SketchPlane, SolidExtrude } from "../schema"
import { extrudeSolidFeature, getExtrudedFaceDescriptors, getPlaneSketchFrame } from "./extrude"
import { materializeSketch } from "./sketch"

function createSketch(id: string, plane: SketchPlane, entities: Sketch["entities"]): Sketch {
	return materializeSketch({
		type: "sketch",
		id,
		name: id,
		dirty: false,
		target: {
			type: "plane",
			plane
		},
		entities,
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	})
}

describe("extrudeSolidFeature", () => {
	it("keeps reference plane frames right-handed", () => {
		for (const plane of ["XY", "XZ", "YZ"] as const) {
			const frame = getPlaneSketchFrame(plane)
			const normal = crossVector(frame.xAxis, frame.yAxis)
			expect(normal.x).toBeCloseTo(frame.normal.x, 6)
			expect(normal.y).toBeCloseTo(frame.normal.y, 6)
			expect(normal.z).toBeCloseTo(frame.normal.z, 6)
		}
	})

	it("extrudes a single-profile plane sketch", () => {
		const sketch = createSketch("sketch-1", "XY", [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 20, y: 10 } }])
		const part: PartDocument = {
			features: [
				sketch,
				{
					type: "extrude",
					id: "extrude-1",
					target: {
						type: "profileRef",
						sketchId: sketch.id,
						profileId: sketch.profiles[0]?.id ?? ""
					},
					depth: 12
				}
			]
		}

		const extrusion = extrudeSolidFeature(part, part.features[1] as Extract<PartDocument["features"][number], { type: "extrude" }>)

		expect(extrusion.solid.id).toBe("extrude-1-solid")
		expect(extrusion.solid.vertices).toHaveLength(8)
		expect(extrusion.solid.faces).toHaveLength(6)
		expect(extrusion.profileLoops).toEqual([
			[
				{ x: 0, y: 0 },
				{ x: 20, y: 0 },
				{ x: 20, y: 10 },
				{ x: 0, y: 10 }
			]
		])
		expect(extrusion.solid.vertices[0]?.position).toEqual({ x: 0, y: 0, z: 0 })
		expect(extrusion.solid.vertices[1]?.position).toEqual({ x: 0, y: 0, z: 12 })
	})

	it("maps reference planes to the correct 3d axes", () => {
		const sketch = createSketch("sketch-2", "YZ", [
			{ id: "line-1", type: "line", p0: { x: 1, y: 2 }, p1: { x: 4, y: 2 } },
			{ id: "line-2", type: "line", p0: { x: 4, y: 2 }, p1: { x: 1, y: 6 } },
			{ id: "line-3", type: "line", p0: { x: 1, y: 6 }, p1: { x: 1, y: 2 } }
		])
		const part: PartDocument = {
			features: [
				sketch,
				{
					type: "extrude",
					id: "extrude-2",
					target: {
						type: "profileRef",
						sketchId: sketch.id,
						profileId: sketch.profiles[0]?.id ?? ""
					},
					depth: 3
				}
			]
		}

		const extrusion = extrudeSolidFeature(part, part.features[1] as Extract<PartDocument["features"][number], { type: "extrude" }>, { startOffset: 7 })

		expect(extrusion.frame.normal).toEqual({ x: 1, y: 0, z: 0 })
		expect(extrusion.frame.xAxis).toEqual({ x: 0, y: 1, z: 0 })
		expect(extrusion.frame.yAxis).toEqual({ x: 0, y: 0, z: 1 })
		expect(extrusion.solid.vertices[0]?.position).toEqual({ x: 7, y: 1, z: 2 })
		expect(extrusion.solid.vertices[1]?.position).toEqual({ x: 10, y: 1, z: 2 })
	})

	it("extrudes Top reference plane sketches along positive Y", () => {
		const sketch = createSketch("sketch-top", "XZ", [{ id: "rect-top", type: "cornerRectangle", p0: { x: 1, y: 2 }, p1: { x: 5, y: 6 } }])
		const part: PartDocument = {
			features: [
				sketch,
				{
					type: "extrude",
					id: "extrude-top",
					target: {
						type: "profileRef",
						sketchId: sketch.id,
						profileId: sketch.profiles[0]?.id ?? ""
					},
					depth: 9
				}
			]
		}

		const extrusion = extrudeSolidFeature(part, part.features[1] as Extract<PartDocument["features"][number], { type: "extrude" }>)

		expect(extrusion.frame.xAxis).toEqual({ x: 1, y: 0, z: 0 })
		expect(extrusion.frame.yAxis).toEqual({ x: 0, y: 0, z: -1 })
		expect(extrusion.frame.normal).toEqual({ x: 0, y: 1, z: 0 })
		expect(extrusion.solid.vertices[0]?.position).toEqual({ x: 1, y: 0, z: -2 })
		expect(extrusion.solid.vertices[1]?.position).toEqual({ x: 1, y: 9, z: -2 })
	})

	it("extrudes a sketch that targets an extrude face", () => {
		const baseSketch = createSketch("sketch-base", "XY", [{ id: "base-rect", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 8, y: 6 } }])
		const baseExtrude: SolidExtrude = {
			type: "extrude",
			id: "extrude-base",
			target: {
				type: "profileRef",
				sketchId: baseSketch.id,
				profileId: baseSketch.profiles[0]?.id ?? ""
			},
			depth: 4
		}
		const topFaceId = getExtrudedFaceDescriptors(extrudeSolidFeature({ features: [baseSketch, baseExtrude] }, baseExtrude)).find((face) => face.label === "Top Face")?.faceId
		expect(topFaceId).toBeDefined()
		if (!topFaceId) {
			throw new Error("Expected top face id")
		}

		const faceSketch = materializeSketch({
			type: "sketch",
			id: "sketch-face",
			name: "Sketch 2",
			dirty: false,
			target: {
				type: "face",
				face: {
					type: "extrudeFace",
					extrudeId: baseExtrude.id,
					faceId: topFaceId
				}
			},
			entities: [{ id: "face-rect", type: "cornerRectangle", p0: { x: 2, y: 1 }, p1: { x: 5, y: 3 } }],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		})
		const faceExtrude: SolidExtrude = {
			type: "extrude",
			id: "extrude-face",
			target: {
				type: "profileRef",
				sketchId: faceSketch.id,
				profileId: faceSketch.profiles[0]?.id ?? ""
			},
			depth: 2
		}
		const part: PartDocument = {
			features: [baseSketch, baseExtrude, faceSketch, faceExtrude]
		}

		const extrusion = extrudeSolidFeature(part, faceExtrude)

		expect(extrusion.frame.origin).toEqual({ x: 0, y: 0, z: 4 })
		expect(extrusion.frame.normal).toEqual({ x: 0, y: 0, z: 1 })
		expect(extrusion.solid.vertices[0]?.position).toEqual({ x: 2, y: 1, z: 4 })
		expect(extrusion.solid.vertices[1]?.position).toEqual({ x: 2, y: 1, z: 6 })
	})

	it("throws when the profile reference is invalid", () => {
		const sketch = createSketch("sketch-3", "XY", [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 10 } }])
		const part: PartDocument = {
			features: [
				sketch,
				{
					type: "extrude",
					id: "extrude-3",
					target: {
						type: "profileRef",
						sketchId: sketch.id,
						profileId: "missing-profile"
					},
					depth: 5
				}
			]
		}

		expect(() => extrudeSolidFeature(part, part.features[1] as Extract<PartDocument["features"][number], { type: "extrude" }>)).toThrow(
			'Profile "missing-profile" does not exist on this sketch.'
		)
	})
})

function crossVector(a: { x: number; y: number; z: number }, b: { x: number; y: number; z: number }): { x: number; y: number; z: number } {
	return {
		x: a.y * b.z - a.z * b.y,
		y: a.z * b.x - a.x * b.z,
		z: a.x * b.y - a.y * b.x
	}
}
