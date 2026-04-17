import { describe, expect, it } from "bun:test"
import { extrudeSolidFeature, getExtrudedFaceDescriptors } from "../src/cad/extrude"
import { materializeSketch } from "../src/cad/sketch"
import { applyPartAction, type PartAction } from "../src/part-actions"
import type { PartDocument, Sketch, SketchPlane, SolidExtrude } from "../src/schema"

function createSketch(id: string, plane: SketchPlane, entities: Sketch["entities"], options: { dirty?: boolean; name?: string } = {}): Sketch {
	return materializeSketch({
		type: "sketch",
		id,
		name: options.name ?? id,
		dirty: options.dirty ?? false,
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

function createExtrude(id: string, sketch: Sketch, depth = 12): SolidExtrude {
	const profile = sketch.profiles[0]
	if (!profile) {
		throw new Error(`Sketch "${sketch.id}" is missing a profile.`)
	}

	return {
		type: "extrude",
		id,
		name: id,
		target: {
			type: "profileRef",
			sketchId: sketch.id,
			profileId: profile.id
		},
		depth
	}
}

function createBaseFaceChain(): {
	baseSketch: Sketch
	baseExtrude: SolidExtrude
	faceSketch: Sketch
	faceExtrude: SolidExtrude
} {
	const baseSketch = createSketch("sketch-base", "XY", [{ id: "rect-base", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 12, y: 8 } }], {
		name: "Base Sketch"
	})
	const baseExtrude = createExtrude("extrude-base", baseSketch, 10)
	const topFaceId = getExtrudedFaceDescriptors(extrudeSolidFeature({ features: [baseSketch, baseExtrude] }, baseExtrude)).find((face) => face.label === "Top Face")?.faceId
	if (!topFaceId) {
		throw new Error("Expected top face id.")
	}

	const faceSketch = materializeSketch({
		type: "sketch",
		id: "sketch-face",
		name: "Face Sketch",
		dirty: false,
		target: {
			type: "face",
			face: {
				type: "extrudeFace",
				extrudeId: baseExtrude.id,
				faceId: topFaceId
			}
		},
		entities: [{ id: "rect-face", type: "cornerRectangle", p0: { x: 2, y: 2 }, p1: { x: 6, y: 5 } }],
		vertices: [],
		loops: [],
		profiles: []
	})
	const faceExtrude = createExtrude("extrude-face", faceSketch, 6)
	return {
		baseSketch,
		baseExtrude,
		faceSketch,
		faceExtrude
	}
}

describe("applyPartAction", () => {
	it("creates a sketch on a reference plane", () => {
		const result = applyPartAction(
			{ features: [] },
			{
				type: "createSketch",
				sketchId: "sketch-1",
				name: "Sketch 1",
				target: {
					type: "plane",
					plane: "XY"
				}
			}
		)

		expect(result.features).toHaveLength(1)
		expect(result.features[0]).toMatchObject({
			type: "sketch",
			id: "sketch-1",
			name: "Sketch 1",
			dirty: true,
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: []
		})
	})

	it("creates a sketch on an extrude face", () => {
		const baseSketch = createSketch("sketch-1", "XY", [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 8, y: 6 } }])
		const baseExtrude = createExtrude("extrude-1", baseSketch)
		const topFaceId = getExtrudedFaceDescriptors(extrudeSolidFeature({ features: [baseSketch, baseExtrude] }, baseExtrude)).find((face) => face.label === "Top Face")?.faceId
		if (!topFaceId) {
			throw new Error("Expected top face id.")
		}

		const result = applyPartAction(
			{ features: [baseSketch, baseExtrude] },
			{
				type: "createSketch",
				sketchId: "sketch-2",
				name: "Sketch 2",
				target: {
					type: "face",
					face: {
						type: "extrudeFace",
						extrudeId: baseExtrude.id,
						faceId: topFaceId
					}
				}
			}
		)

		expect(result.features[2]).toMatchObject({
			type: "sketch",
			id: "sketch-2",
			target: {
				type: "face",
				face: {
					type: "extrudeFace",
					extrudeId: baseExtrude.id,
					faceId: topFaceId
				}
			},
			dirty: true
		})
	})

	it("adds line and rectangle entities and materializes topology", () => {
		const initial: PartDocument = {
			features: [
				createSketch("sketch-1", "XY", [], {
					dirty: true
				})
			]
		}

		const withLine = applyPartAction(initial, {
			type: "addSketchEntity",
			sketchId: "sketch-1",
			entity: {
				id: "line-1",
				type: "line",
				p0: { x: 0, y: 0 },
				p1: { x: 10, y: 0 }
			}
		})
		const withRectangle = applyPartAction(withLine, {
			type: "addSketchEntity",
			sketchId: "sketch-1",
			entity: {
				id: "rect-1",
				type: "cornerRectangle",
				p0: { x: 2, y: 2 },
				p1: { x: 8, y: 6 }
			}
		})

		const sketch = withRectangle.features[0]
		expect(sketch?.type).toBe("sketch")
		if (!sketch || sketch.type !== "sketch") {
			throw new Error("Expected sketch.")
		}
		expect(sketch.entities).toEqual([
			{
				id: "line-1",
				type: "line",
				p0: { x: 0, y: 0 },
				p1: { x: 10, y: 0 }
			},
			{
				id: "rect-1",
				type: "cornerRectangle",
				p0: { x: 2, y: 2 },
				p1: { x: 8, y: 6 }
			}
		])
		expect(sketch.profiles).toHaveLength(1)
		expect(sketch.loops).toHaveLength(1)
	})

	it("undoes and resets sketch entities", () => {
		const initial: PartDocument = {
			features: [
				createSketch(
					"sketch-1",
					"XY",
					[
						{ id: "line-1", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } },
						{ id: "rect-1", type: "cornerRectangle", p0: { x: 2, y: 2 }, p1: { x: 8, y: 6 } }
					],
					{ dirty: true }
				)
			]
		}

		const undone = applyPartAction(initial, {
			type: "undoSketchEntity",
			sketchId: "sketch-1"
		})
		const reset = applyPartAction(undone, {
			type: "resetSketch",
			sketchId: "sketch-1"
		})

		const undoneSketch = undone.features[0]
		expect(undoneSketch?.type).toBe("sketch")
		if (!undoneSketch || undoneSketch.type !== "sketch") {
			throw new Error("Expected sketch after undo.")
		}
		expect(undoneSketch.entities).toHaveLength(1)

		const resetSketch = reset.features[0]
		expect(resetSketch?.type).toBe("sketch")
		if (!resetSketch || resetSketch.type !== "sketch") {
			throw new Error("Expected sketch after reset.")
		}
		expect(resetSketch.entities).toEqual([])
		expect(resetSketch.profiles).toEqual([])
	})

	it("finishes a sketch with a single profile", () => {
		const initial: PartDocument = {
			features: [
				createSketch("sketch-1", "XY", [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 8, y: 6 } }], {
					dirty: true
				})
			]
		}

		const result = applyPartAction(initial, {
			type: "finishSketch",
			sketchId: "sketch-1"
		})

		expect(result.features[0]).toMatchObject({
			type: "sketch",
			id: "sketch-1",
			dirty: false
		})
	})

	it("creates an extrude from a finished single-profile sketch", () => {
		const sketch = createSketch("sketch-1", "XY", [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 12, y: 8 } }])
		const profile = sketch.profiles[0]
		if (!profile) {
			throw new Error("Expected profile.")
		}

		const result = applyPartAction(
			{ features: [sketch] },
			{
				type: "createExtrude",
				extrudeId: "extrude-1",
				name: "Extrude 1",
				sketchId: sketch.id,
				profileId: profile.id,
				depth: 30
			}
		)

		expect(result.features[1]).toMatchObject({
			type: "extrude",
			id: "extrude-1",
			name: "Extrude 1",
			target: {
				type: "profileRef",
				sketchId: sketch.id,
				profileId: profile.id
			},
			depth: 30
		})
	})

	it("updates the depth of an extrude", () => {
		const sketch = createSketch("sketch-1", "XY", [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 12, y: 8 } }])
		const extrude = createExtrude("extrude-1", sketch, 12)
		const result = applyPartAction(
			{ features: [sketch, extrude] },
			{
				type: "setExtrudeDepth",
				extrudeId: extrude.id,
				depth: 48
			}
		)

		expect(result.features[1]).toMatchObject({
			type: "extrude",
			id: extrude.id,
			depth: 48
		})
	})

	it("renames a sketch", () => {
		const sketch = createSketch("sketch-1", "XY", [], {
			dirty: true,
			name: "Original"
		})
		const result = applyPartAction(
			{ features: [sketch] },
			{
				type: "renameSketch",
				sketchId: sketch.id,
				name: "Renamed Sketch"
			}
		)

		expect(result.features[0]).toMatchObject({
			type: "sketch",
			id: sketch.id,
			name: "Renamed Sketch"
		})
	})

	it("deletes a sketch and all dependent features", () => {
		const chain = createBaseFaceChain()
		const result = applyPartAction(
			{
				features: [chain.baseSketch, chain.baseExtrude, chain.faceSketch, chain.faceExtrude]
			},
			{
				type: "deleteSketch",
				sketchId: chain.baseSketch.id
			}
		)

		expect(result.features).toEqual([])
	})

	it("deletes an extrude and dependent face-based features", () => {
		const chain = createBaseFaceChain()
		const result = applyPartAction(
			{
				features: [chain.baseSketch, chain.baseExtrude, chain.faceSketch, chain.faceExtrude]
			},
			{
				type: "deleteExtrude",
				extrudeId: chain.baseExtrude.id
			}
		)

		expect(result.features).toEqual([chain.baseSketch])
	})

	it("returns the original state for invalid action preconditions", () => {
		const dirtySketch = createSketch("sketch-1", "XY", [], {
			dirty: true,
			name: "Sketch 1"
		})
		const finishedSketch = createSketch("sketch-2", "XY", [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 8, y: 6 } }], {
			name: "Sketch 2"
		})
		const extrude = createExtrude("extrude-1", finishedSketch, 12)
		const invalidActions: PartAction[] = [
			{
				type: "createSketch",
				sketchId: "sketch-3",
				name: "Sketch 3",
				target: {
					type: "plane",
					plane: "XY"
				}
			},
			{
				type: "addSketchEntity",
				sketchId: finishedSketch.id,
				entity: {
					id: "line-2",
					type: "line",
					p0: { x: 0, y: 0 },
					p1: { x: 4, y: 0 }
				}
			},
			{
				type: "finishSketch",
				sketchId: dirtySketch.id
			},
			{
				type: "createExtrude",
				extrudeId: "extrude-2",
				name: "Extrude 2",
				sketchId: dirtySketch.id,
				profileId: "missing-profile",
				depth: 20
			},
			{
				type: "setExtrudeDepth",
				extrudeId: extrude.id,
				depth: -1
			},
			{
				type: "deleteExtrude",
				extrudeId: "missing-extrude"
			}
		]

		for (const action of invalidActions) {
			const state: PartDocument = {
				features: [dirtySketch, finishedSketch, extrude]
			}
			expect(applyPartAction(state, action)).toBe(state)
		}
	})
})
