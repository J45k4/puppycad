import { describe, expect, it } from "bun:test"
import type { Sketch } from "../schema"
import { materializeSketch } from "./sketch"

describe("materializeSketch", () => {
	it("derives a single profile from a closed line loop", () => {
		const sketch: Sketch = {
			type: "sketch",
			id: "sketch-1",
			name: "Sketch 1",
			dirty: true,
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{ id: "line-1", type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 0 } },
				{ id: "line-2", type: "line", p0: { x: 20, y: 0 }, p1: { x: 20, y: 10 } },
				{ id: "line-3", type: "line", p0: { x: 20, y: 10 }, p1: { x: 0, y: 10 } },
				{ id: "line-4", type: "line", p0: { x: 0, y: 10 }, p1: { x: 0, y: 0 } }
			],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}

		const materialized = materializeSketch(sketch)

		expect(materialized.vertices).toEqual([
			{ x: 0, y: 0 },
			{ x: 20, y: 0 },
			{ x: 20, y: 10 },
			{ x: 0, y: 10 }
		])
		expect(materialized.loops).toEqual([
			{
				id: "sketch-1-loop-1",
				vertexIndices: [0, 1, 2, 3]
			}
		])
		expect(materialized.profiles).toEqual([
			{
				id: "sketch-1-profile-1",
				outerLoopId: "sketch-1-loop-1",
				holeLoopIds: []
			}
		])
		expect(materialized.dirty).toBe(true)
	})

	it("groups nested rectangles into an outer loop and a hole", () => {
		const sketch: Sketch = {
			type: "sketch",
			id: "sketch-2",
			name: "Sketch 2",
			dirty: true,
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{
					id: "rect-1",
					type: "cornerRectangle",
					p0: { x: 0, y: 0 },
					p1: { x: 20, y: 20 }
				},
				{
					id: "rect-2",
					type: "cornerRectangle",
					p0: { x: 5, y: 5 },
					p1: { x: 15, y: 15 }
				}
			],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}

		const materialized = materializeSketch(sketch)

		expect(materialized.loops).toHaveLength(2)
		expect(materialized.profiles).toEqual([
			{
				id: "sketch-2-profile-1",
				outerLoopId: "sketch-2-loop-1",
				holeLoopIds: ["sketch-2-loop-2"]
			}
		])
	})

	it("returns no profiles for an open sketch", () => {
		const sketch: Sketch = {
			type: "sketch",
			id: "sketch-3",
			name: "Sketch 3",
			dirty: true,
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [
				{ id: "line-1", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } },
				{ id: "line-2", type: "line", p0: { x: 10, y: 0 }, p1: { x: 10, y: 10 } }
			],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}

		const materialized = materializeSketch(sketch)

		expect(materialized.loops).toEqual([])
		expect(materialized.profiles).toEqual([])
	})
})
