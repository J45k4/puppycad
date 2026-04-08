import { describe, expect, it } from "bun:test"
import type { Profile, SketchFeature } from "../contract"
import { getProfilesFromSketch } from "./sketch"

describe("getProfilesFromSketch", () => {
	it("returns stored profiles when the sketch already has them", () => {
		const storedProfiles: Profile[] = [
			{
				id: "stored-profile",
				vertices: [
					{ x: 0, y: 0 },
					{ x: 10, y: 0 },
					{ x: 10, y: 10 }
				],
				loops: [[0, 1, 2]]
			}
		]

		const sketch: SketchFeature = {
			id: "sketch-1",
			type: "sketch",
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [],
			profiles: storedProfiles
		}

		const profiles = getProfilesFromSketch(sketch)

		expect(profiles).toEqual(storedProfiles)
		expect(profiles).not.toBe(storedProfiles)
		expect(profiles[0]?.vertices).not.toBe(storedProfiles[0]?.vertices)
	})

	it("derives a profile from a closed line loop", () => {
		const sketch: SketchFeature = {
			id: "sketch-2",
			type: "sketch",
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{ type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 0 } },
				{ type: "line", p0: { x: 20, y: 0 }, p1: { x: 20, y: 10 } },
				{ type: "line", p0: { x: 20, y: 10 }, p1: { x: 0, y: 10 } },
				{ type: "line", p0: { x: 0, y: 10 }, p1: { x: 0, y: 0 } }
			],
			profiles: []
		}

		const profiles = getProfilesFromSketch(sketch)

		expect(profiles).toHaveLength(1)
		expect(profiles[0]?.id).toBe("sketch-2-profile-1")
		expect(profiles[0]?.loops).toEqual([[0, 1, 2, 3]])
		expect(profiles[0]?.vertices).toEqual([
			{ x: 0, y: 0 },
			{ x: 20, y: 0 },
			{ x: 20, y: 10 },
			{ x: 0, y: 10 }
		])
	})

	it("groups nested loops into outer boundaries and holes", () => {
		const sketch: SketchFeature = {
			id: "sketch-3",
			type: "sketch",
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{
					type: "cornerRectangle",
					p0: { x: 0, y: 0 },
					p1: { x: 20, y: 20 }
				},
				{
					type: "cornerRectangle",
					p0: { x: 5, y: 5 },
					p1: { x: 15, y: 15 }
				}
			],
			profiles: []
		}

		const profiles = getProfilesFromSketch(sketch)

		expect(profiles).toHaveLength(1)
		expect(profiles[0]?.loops).toHaveLength(2)
		expect(profiles[0]?.vertices).toEqual([
			{ x: 0, y: 0 },
			{ x: 20, y: 0 },
			{ x: 20, y: 20 },
			{ x: 0, y: 20 },
			{ x: 5, y: 15 },
			{ x: 15, y: 15 },
			{ x: 15, y: 5 },
			{ x: 5, y: 5 }
		])
	})

	it("ignores open chains that do not form a profile", () => {
		const sketch: SketchFeature = {
			id: "sketch-4",
			type: "sketch",
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{ type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } },
				{ type: "line", p0: { x: 10, y: 0 }, p1: { x: 10, y: 10 } }
			],
			profiles: []
		}

		expect(getProfilesFromSketch(sketch)).toEqual([])
	})
})
