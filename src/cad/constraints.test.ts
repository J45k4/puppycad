import { describe, expect, it } from "bun:test"
import type { SketchFeature } from "../contract"
import { solveSketchConstraints } from "./constraints"
import { getProfilesFromSketch } from "./sketch"

describe("solveSketchConstraints", () => {
	it("applies horizontal and distance constraints to a line", () => {
		const sketch: SketchFeature = {
			id: "sketch-constraint-1",
			type: "sketch",
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{
					id: "line-1",
					type: "line",
					p0: { x: 0, y: 0 },
					p1: { x: 5, y: 4 }
				}
			],
			profiles: [],
			constraints: [
				{
					type: "horizontal",
					e: "line-1"
				}
			],
			dimensions: [
				{
					id: "dim-1",
					type: "distance",
					between: ["line-1:p0", "line-1:p1"],
					value: 10
				}
			]
		}

		const { sketch: solvedSketch, unsupportedConstraints, unsupportedDimensions } = solveSketchConstraints(sketch)
		const line = solvedSketch.entities[0]

		expect(line).toEqual({
			id: "line-1",
			type: "line",
			p0: { x: 0, y: 0 },
			p1: { x: 10, y: 0 }
		})
		expect(unsupportedConstraints).toEqual([])
		expect(unsupportedDimensions).toEqual([])
	})

	it("applies circle equality, concentricity, and diameter dimensions", () => {
		const sketch: SketchFeature = {
			id: "sketch-constraint-2",
			type: "sketch",
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{
					id: "circle-1",
					type: "centerPointCircle",
					center: { x: 4, y: 8 },
					radius: 2
				},
				{
					id: "circle-2",
					type: "centerPointCircle",
					center: { x: 0, y: 0 },
					radius: 1
				}
			],
			profiles: [],
			constraints: [
				{
					type: "concentric",
					a: "circle-1",
					b: "circle-2"
				},
				{
					type: "equal",
					eA: "circle-1",
					eB: "circle-2"
				}
			],
			dimensions: [
				{
					id: "dim-2",
					type: "diameter",
					of: "circle-1",
					value: 12
				}
			]
		}

		const { sketch: solvedSketch } = solveSketchConstraints(sketch)
		const circle1 = solvedSketch.entities[0]
		const circle2 = solvedSketch.entities[1]

		expect(circle1).toEqual({
			id: "circle-1",
			type: "centerPointCircle",
			center: { x: 4, y: 8 },
			radius: 6
		})
		expect(circle2).toEqual({
			id: "circle-2",
			type: "centerPointCircle",
			center: { x: 4, y: 8 },
			radius: 6
		})
	})

	it("lets profile extraction use solved line constraints", () => {
		const sketch: SketchFeature = {
			id: "sketch-constraint-3",
			type: "sketch",
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{
					id: "l1",
					type: "line",
					p0: { x: 0, y: 0 },
					p1: { x: 20, y: 0 }
				},
				{
					id: "l2",
					type: "line",
					p0: { x: 21, y: 1 },
					p1: { x: 21, y: 10 }
				},
				{
					id: "l3",
					type: "line",
					p0: { x: 20, y: 11 },
					p1: { x: 0, y: 11 }
				},
				{
					id: "l4",
					type: "line",
					p0: { x: 0, y: 10 },
					p1: { x: 0, y: 0 }
				}
			],
			profiles: [],
			constraints: [
				{ type: "coincident", a: "l1:p1", b: "l2:p0" },
				{ type: "vertical", e: "l2" },
				{ type: "coincident", a: "l2:p1", b: "l3:p0" },
				{ type: "horizontal", e: "l3" },
				{ type: "coincident", a: "l3:p1", b: "l4:p0" },
				{ type: "coincident", a: "l4:p1", b: "l1:p0" }
			]
		}

		expect(getProfilesFromSketch(sketch)).toEqual([
			{
				id: "sketch-constraint-3-profile-1",
				vertices: [
					{ x: 0, y: 0 },
					{ x: 20, y: 0 },
					{ x: 20, y: 10 },
					{ x: 0, y: 10 }
				],
				loops: [[0, 1, 2, 3]]
			}
		])
	})
})
