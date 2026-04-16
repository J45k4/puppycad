import { describe, expect, it } from "bun:test"
import type { PartDocument, Sketch } from "../schema"
import { extrudeSolidFeature } from "./extrude"
import { materializeSketch } from "./sketch"

function createSketch(id: string, plane: Sketch["target"]["plane"], entities: Sketch["entities"]): Sketch {
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
		vertices: [],
		loops: [],
		profiles: []
	})
}

describe("extrudeSolidFeature", () => {
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

		expect(extrusion.plane).toBe("YZ")
		expect(extrusion.solid.vertices[0]?.position).toEqual({ x: 7, y: 1, z: 2 })
		expect(extrusion.solid.vertices[1]?.position).toEqual({ x: 10, y: 1, z: 2 })
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
