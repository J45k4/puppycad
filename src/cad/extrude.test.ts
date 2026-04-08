import { describe, expect, it } from "bun:test"
import type { Profile } from "../contract"
import { extrudeProfile, resolveExtrudeEdgeId, resolveExtrudeFaceId } from "./extrude"

describe("extrudeProfile", () => {
	it("creates a prism solid with cap and side topology from a profile", () => {
		const profile: Profile = {
			id: "base-profile",
			vertices: [
				{ x: 0, y: 0 },
				{ x: 20, y: 0 },
				{ x: 20, y: 10 },
				{ x: 0, y: 10 }
			],
			loops: [[0, 1, 2, 3]]
		}

		const extrusion = extrudeProfile(profile, {
			featureId: "base-extrude",
			distance: 12
		})

		expect(extrusion.solid.id).toBe("base-extrude-solid")
		expect(extrusion.solid.vertices).toHaveLength(8)
		expect(extrusion.solid.edges).toHaveLength(12)
		expect(extrusion.solid.faces).toHaveLength(6)
		expect(extrusion.faces.sides).toHaveLength(4)
		expect(extrusion.edges.topLoops).toEqual([extrusion.edges.top])
		expect(extrusion.edges.bottomLoops).toEqual([extrusion.edges.bottom])
		expect(extrusion.vertices.topLoops).toEqual([extrusion.vertices.top])
		expect(extrusion.vertices.bottomLoops).toEqual([extrusion.vertices.bottom])
		expect(extrusion.solid.vertices.find((vertex) => vertex.id === extrusion.vertices.bottom[0])?.position).toEqual({
			x: 0,
			y: 0,
			z: 0
		})
		expect(extrusion.solid.vertices.find((vertex) => vertex.id === extrusion.vertices.top[0])?.position).toEqual({
			x: 0,
			y: 0,
			z: 12
		})
		expect(resolveExtrudeFaceId(extrusion, { type: "cap", side: "top" })).toBe(extrusion.faces.top)
		expect(resolveExtrudeFaceId(extrusion, { type: "side", index: 2 })).toBe(extrusion.faces.sides[2] ?? null)
		expect(resolveExtrudeEdgeId(extrusion, { type: "capLoop", side: "bottom", index: 1 })).toBe(extrusion.edges.bottom[1] ?? null)
		expect(resolveExtrudeEdgeId(extrusion, { type: "side", index: 3 })).toBe(extrusion.edges.sides[3] ?? null)
	})

	it("supports profiles with holes and preserves per-loop topology", () => {
		const profile: Profile = {
			id: "frame-profile",
			vertices: [
				{ x: 0, y: 0 },
				{ x: 30, y: 0 },
				{ x: 30, y: 30 },
				{ x: 0, y: 30 },
				{ x: 10, y: 10 },
				{ x: 20, y: 10 },
				{ x: 20, y: 20 },
				{ x: 10, y: 20 }
			],
			loops: [
				[0, 1, 2, 3],
				[4, 5, 6, 7]
			]
		}

		const extrusion = extrudeProfile(profile, {
			featureId: "frame-extrude",
			distance: 5
		})

		expect(extrusion.solid.vertices).toHaveLength(16)
		expect(extrusion.solid.edges).toHaveLength(24)
		expect(extrusion.solid.faces).toHaveLength(10)
		expect(extrusion.edges.topLoops).toHaveLength(2)
		expect(extrusion.edges.bottomLoops).toHaveLength(2)
		expect(extrusion.vertices.topLoops).toHaveLength(2)
		expect(extrusion.vertices.bottomLoops).toHaveLength(2)
		expect(extrusion.faces.sides).toHaveLength(8)
		expect(extrusion.solid.faces.find((face) => face.id === extrusion.faces.bottom)?.edgeIds).toEqual(extrusion.edges.bottom)
		expect(extrusion.solid.faces.find((face) => face.id === extrusion.faces.top)?.edgeIds).toEqual(extrusion.edges.top)
		expect(resolveExtrudeEdgeId(extrusion, { type: "capLoop", side: "top", index: 5 })).toBe(extrusion.edges.top[5] ?? null)
	})

	it("supports different sketch planes and negative extrusion direction", () => {
		const profile: Profile = {
			id: "triangle-profile",
			vertices: [
				{ x: 1, y: 2 },
				{ x: 4, y: 2 },
				{ x: 1, y: 6 }
			],
			loops: [[0, 1, 2]]
		}

		const extrusion = extrudeProfile(profile, {
			featureId: "triangle-extrude",
			distance: 3,
			direction: "negative",
			plane: "YZ",
			startOffset: 7
		})

		expect(extrusion.solid.vertices.find((vertex) => vertex.id === extrusion.vertices.bottom[0])?.position).toEqual({
			x: 7,
			y: 1,
			z: 2
		})
		expect(extrusion.solid.vertices.find((vertex) => vertex.id === extrusion.vertices.top[0])?.position).toEqual({
			x: 4,
			y: 1,
			z: 2
		})
	})
})
