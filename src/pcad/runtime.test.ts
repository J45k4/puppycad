import { describe, expect, it } from "bun:test"
import type { Sketch, SolidExtrude } from "../schema"
import { extrudeSolidFeature } from "../cad/extrude"
import { materializeSketch } from "../cad/sketch"
import {
	CadEditor,
	type EdgeNode,
	type ExtrudeNode,
	type ExtrudeOperation,
	type FaceNode,
	type ReferencePlaneNode,
	type SketchNode,
	applyGraphRewrite,
	applyGraphRewrites,
	collectDependentNodeIds,
	createEmptyPCadState,
	getNodeDependencies,
	validatePCadState,
	type PCadState
} from "./runtime"

const plane: ReferencePlaneNode = {
	id: "plane-front",
	type: "referencePlane",
	name: "Front",
	plane: "XY"
}

const sketch: SketchNode = {
	id: "sketch-1",
	type: "sketch",
	name: "Sketch 1",
	targetId: plane.id,
	entities: [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 5 } }],
	dimensions: []
}

const extrude: ExtrudeNode = {
	id: "extrude-1",
	type: "extrude",
	name: "Extrude 1",
	sketchId: sketch.id,
	profileId: "profile-1",
	operation: "newBody",
	depth: 4
}

const edge: EdgeNode = {
	id: "edge-1",
	type: "edge",
	sourceId: extrude.id,
	edgeId: "generated-edge-1"
}

const face: FaceNode = {
	id: "face-1",
	type: "face",
	sourceId: extrude.id,
	faceId: "generated-face-1"
}

function createValidState(): PCadState {
	return {
		nodes: new Map([
			[plane.id, plane],
			[sketch.id, sketch]
		]),
		rootNodeIds: [plane.id]
	}
}

function createDependencyState(): PCadState {
	const chamfer = {
		id: "chamfer-1",
		type: "chamfer" as const,
		edgeId: edge.id,
		d1: 1
	}

	return {
		nodes: new Map([
			[plane.id, plane],
			[sketch.id, sketch],
			[extrude.id, extrude],
			[edge.id, edge],
			[face.id, face],
			[chamfer.id, chamfer]
		]),
		rootNodeIds: [extrude.id, chamfer.id]
	}
}

describe("PCad graph rewrites", () => {
	it("creates an empty PCad state", () => {
		const state = createEmptyPCadState()

		expect([...state.nodes]).toEqual([])
		expect(state.rootNodeIds).toEqual([])
	})

	it("adds, replaces, removes, and sets root nodes", () => {
		const added = applyGraphRewrite(createEmptyPCadState(), { type: "addNode", node: plane, root: true })
		expect(added.nodes.get(plane.id)).toEqual(plane)
		expect(added.rootNodeIds).toEqual([plane.id])

		const renamedPlane: ReferencePlaneNode = { ...plane, name: "Renamed Front" }
		const replaced = applyGraphRewrite(added, { type: "replaceNode", node: renamedPlane })
		expect(replaced.nodes.get(plane.id)).toEqual(renamedPlane)

		const withSketch = applyGraphRewrite(replaced, { type: "addNode", node: sketch })
		const rootedAtSketch = applyGraphRewrite(withSketch, { type: "setRootNodes", rootNodeIds: [sketch.id] })
		expect(rootedAtSketch.rootNodeIds).toEqual([sketch.id])

		const removed = applyGraphRewrite(rootedAtSketch, { type: "removeNodes", nodeIds: [sketch.id] })
		expect(removed.nodes.has(sketch.id)).toBe(false)
		expect(removed.rootNodeIds).toEqual([])
	})

	it("returns the same state when replacing a missing node", () => {
		const state = createEmptyPCadState()
		const replaced = applyGraphRewrite(state, { type: "replaceNode", node: plane })

		expect(replaced).toBe(state)
	})

	it("applies multiple rewrites in order", () => {
		const state = applyGraphRewrites(createEmptyPCadState(), [
			{ type: "addNode", node: plane, root: true },
			{ type: "addNode", node: sketch },
			{ type: "setRootNodes", rootNodeIds: [sketch.id] }
		])

		expect([...state.nodes.keys()]).toEqual([plane.id, sketch.id])
		expect(state.rootNodeIds).toEqual([sketch.id])
	})
})

describe("PCad dependencies", () => {
	it("extracts node dependencies", () => {
		expect(getNodeDependencies(plane)).toEqual([])
		expect(getNodeDependencies(sketch)).toEqual([plane.id])
		expect(getNodeDependencies(extrude)).toEqual([sketch.id])
		expect(getNodeDependencies(face)).toEqual([extrude.id])
		expect(getNodeDependencies(edge)).toEqual([extrude.id])
		expect(getNodeDependencies({ id: "chamfer-1", type: "chamfer", edgeId: edge.id, d1: 1 })).toEqual([edge.id])
	})

	it("collects transitive dependent nodes for cascade deletion", () => {
		const deletedIds = collectDependentNodeIds(createDependencyState(), [sketch.id])

		expect([...deletedIds].sort()).toEqual([sketch.id, extrude.id, edge.id, face.id, "chamfer-1"].sort())
	})
})

describe("CadEditor", () => {
	it("creates extrudes with explicit operations", () => {
		const editor = new CadEditor(createValidState())
		const node = editor.extrudeSketchProfile({
			id: "extrude-cut",
			sketchId: sketch.id,
			profileId: "profile-1",
			operation: "cut",
			depth: 6
		})

		expect(node.operation).toBe("cut")
		expect(editor.getState().nodes.get(node.id)).toEqual(node)
		expect(editor.getState().rootNodeIds).toEqual([plane.id, node.id])
	})

	it("supports generic undo and redo", () => {
		const editor = new CadEditor(createValidState())

		editor.extrudeSketchProfile({
			id: extrude.id,
			sketchId: sketch.id,
			profileId: extrude.profileId,
			operation: extrude.operation,
			depth: extrude.depth
		})
		expect(editor.getState().nodes.has(extrude.id)).toBe(true)

		editor.undo()
		expect(editor.getState().nodes.has(extrude.id)).toBe(false)

		editor.redo()
		expect(editor.getState().nodes.has(extrude.id)).toBe(true)
	})

	it("deletes a node and its dependents as a cascade", () => {
		const editor = new CadEditor(createDependencyState())

		editor.deleteNodeCascade(sketch.id)

		expect([...editor.getState().nodes.keys()]).toEqual([plane.id])
		expect(editor.getState().rootNodeIds).toEqual([])
	})

	it("updates sketch dimensions and validates entity ids", () => {
		const editor = new CadEditor(createValidState())

		const updatedSketch = editor.setSketchDimension(sketch.id, {
			id: "dim-1",
			type: "rectangleWidth",
			entityId: "rect-1",
			value: 10
		})

		expect(updatedSketch.dimensions).toEqual([{ id: "dim-1", type: "rectangleWidth", entityId: "rect-1", value: 10 }])
		expect(() => editor.setSketchDimension(sketch.id, { id: "dim-2", type: "rectangleHeight", entityId: "missing", value: 5 })).toThrow(
			'Sketch entity "missing" does not exist.'
		)
	})

	it("rejects invalid ids, depths, and extrude operations", () => {
		const editor = new CadEditor(createValidState())

		expect(() =>
			editor.extrudeSketchProfile({
				id: "bad-extrude",
				sketchId: "missing-sketch",
				profileId: "profile-1",
				operation: "newBody",
				depth: 1
			})
		).toThrow('Sketch "missing-sketch" does not exist.')
		expect(() =>
			editor.extrudeSketchProfile({
				id: "bad-extrude",
				sketchId: sketch.id,
				profileId: "profile-1",
				operation: "newBody",
				depth: 0
			})
		).toThrow("Extrude depth must be greater than zero.")
		expect(() =>
			editor.extrudeSketchProfile({
				id: "bad-extrude",
				sketchId: sketch.id,
				profileId: "profile-1",
				operation: "shell" as ExtrudeOperation,
				depth: 1
			})
		).toThrow("Extrude operation must be newBody, join, or cut.")
		expect(() => editor.deleteNodeCascade("missing-node")).toThrow('Node "missing-node" does not exist.')
	})

	it("validates graph references and numeric operation fields", () => {
		expect(() =>
			validatePCadState({
				nodes: new Map([[sketch.id, sketch]]),
				rootNodeIds: [sketch.id]
			})
		).toThrow('Sketch "sketch-1" target "plane-front" does not exist.')

		expect(() =>
			validatePCadState({
				nodes: new Map([
					[plane.id, plane],
					[sketch.id, sketch],
					[extrude.id, { ...extrude, depth: Number.NaN }]
				]),
				rootNodeIds: [extrude.id]
			})
		).toThrow('Extrude "extrude-1" depth must be greater than zero.')
	})

	it("creates a sketch and extrude that materialize to expected geometry", () => {
		const editableSketch: SketchNode = {
			id: "sketch-geometry",
			type: "sketch",
			name: "Geometry Sketch",
			targetId: plane.id,
			entities: [],
			dimensions: []
		}
		const editor = new CadEditor({
			nodes: new Map([
				[plane.id, plane],
				[editableSketch.id, editableSketch]
			]),
			rootNodeIds: [plane.id]
		})

		const sketchedRectangle = editor.addSketchEntity(editableSketch.id, {
			id: "rect-geometry",
			type: "cornerRectangle",
			p0: { x: 0, y: 0 },
			p1: { x: 20, y: 10 }
		})
		const legacySketch = materializeSketch(toLegacySketch(sketchedRectangle, plane))
		const profileId = legacySketch.profiles[0]?.id
		expect(profileId).toBeString()

		const extrudeNode = editor.extrudeSketchProfile({
			id: "extrude-geometry",
			sketchId: editableSketch.id,
			profileId: profileId ?? "",
			operation: "newBody",
			depth: 12
		})
		const legacyExtrude: SolidExtrude = {
			type: "extrude",
			id: extrudeNode.id,
			name: extrudeNode.name,
			target: {
				type: "profileRef",
				sketchId: extrudeNode.sketchId,
				profileId: extrudeNode.profileId
			},
			depth: extrudeNode.depth
		}

		const extrusion = extrudeSolidFeature({ features: [legacySketch, legacyExtrude] }, legacyExtrude)

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
})

function toLegacySketch(node: SketchNode, target: ReferencePlaneNode): Sketch {
	return {
		type: "sketch",
		id: node.id,
		name: node.name,
		dirty: false,
		target: {
			type: "plane",
			plane: target.plane
		},
		entities: node.entities.map((entity) => ({ ...entity })),
		dimensions: node.dimensions.map((dimension) => ({ ...dimension })),
		vertices: [],
		loops: [],
		profiles: []
	}
}
