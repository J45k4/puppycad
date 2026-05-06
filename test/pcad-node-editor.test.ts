import { beforeEach, describe, expect, it } from "bun:test"
import { Window } from "happy-dom"
import type { PartFeature, PCadGraphNode, PCadState, Solid } from "../src/schema"
import { sketchEntityNodeToEntity, sketchEntityToNode } from "../src/pcad/sketch-entities"
import { PCadNodeEditor } from "../src/ui/pcad-node-editor"

const plane: PCadGraphNode = {
	id: "plane-front",
	type: "referencePlane",
	name: "Front",
	plane: "XY"
}

const sketch: Extract<PCadGraphNode, { type: "sketch" }> = {
	id: "sketch-1",
	type: "sketch",
	name: "Sketch 1",
	targetId: plane.id,
	dimensions: []
}

const rectangle = sketchEntityToNode(sketch.id, { id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 8 } })

const extrude: Extract<PCadGraphNode, { type: "extrude" }> = {
	id: "extrude-1",
	type: "extrude",
	name: "Extrude 1",
	sketchId: sketch.id,
	profileId: "profile-1",
	operation: "newBody",
	depth: 12
}

const edge: Extract<PCadGraphNode, { type: "edge" }> = {
	id: "edge-extrude-1-edge-1",
	type: "edge",
	sourceId: extrude.id,
	edgeId: "extrude-1-solid-edge-1"
}

const face: Extract<PCadGraphNode, { type: "face" }> = {
	id: "face-extrude-1-face-1",
	type: "face",
	sourceId: extrude.id,
	faceId: "extrude-1-solid-face-1"
}

const chamfer: PCadGraphNode = {
	id: "chamfer-1",
	type: "chamfer",
	name: "Chamfer 1",
	edgeId: edge.id,
	d1: 2
}

function createState(): PCadState {
	return {
		nodes: new Map<string, PCadGraphNode>([
			[plane.id, plane],
			[sketch.id, sketch],
			[rectangle.id, rectangle],
			[extrude.id, extrude],
			[edge.id, edge],
			[face.id, face],
			[chamfer.id, chamfer]
		]),
		rootNodeIds: [plane.id, extrude.id, chamfer.id]
	}
}

function createStateWithoutAuthoredTopologyRefs(): PCadState {
	return {
		nodes: new Map<string, PCadGraphNode>([
			[plane.id, plane],
			[sketch.id, sketch],
			[rectangle.id, rectangle],
			[extrude.id, extrude]
		]),
		rootNodeIds: [plane.id, extrude.id]
	}
}

function createGeneratedState(): { features: PartFeature[]; solids: Solid[] } {
	return {
		features: [
			{
				type: "sketch",
				id: sketch.id,
				name: sketch.name,
				dirty: false,
				target: { type: "plane", plane: "XY" },
				entities: [sketchEntityNodeToEntity(rectangle)],
				dimensions: [],
				vertices: [
					{ x: 0, y: 0 },
					{ x: 10, y: 0 },
					{ x: 10, y: 8 },
					{ x: 0, y: 8 }
				],
				loops: [{ id: "loop-1", vertexIndices: [0, 1, 2, 3] }],
				profiles: [{ id: extrude.profileId, outerLoopId: "loop-1", holeLoopIds: [] }]
			},
			{
				type: "extrude",
				id: extrude.id,
				name: extrude.name,
				target: {
					type: "profileRef",
					sketchId: sketch.id,
					profileId: extrude.profileId
				},
				depth: extrude.depth
			}
		],
		solids: [
			{
				id: `${extrude.id}-solid`,
				featureId: extrude.id,
				vertices: [
					{ id: "v1", position: { x: 0, y: 0, z: 0 } },
					{ id: "v2", position: { x: 1, y: 0, z: 0 } }
				],
				edges: [{ id: edge.edgeId, vertexIds: ["v1", "v2"] }],
				faces: [{ id: face.faceId, edgeIds: [edge.edgeId] }]
			}
		]
	}
}

describe("PCadNodeEditor", () => {
	beforeEach(() => {
		const window = new Window()
		globalThis.window = window as unknown as typeof globalThis.window
		globalThis.document = window.document as unknown as Document
		globalThis.HTMLElement = window.HTMLElement as unknown as typeof globalThis.HTMLElement
	})

	it("adapts PCad graph nodes and dependencies to canvas components", () => {
		const editor = new PCadNodeEditor({ state: createState() })
		const canvas = editor.getCanvasForTesting()
		const components = canvas.getComponents()
		const connections = canvas.getConnections()

		expect(
			components
				.map((component) => component.data?.nodeId)
				.filter(Boolean)
				.sort()
		).toEqual([plane.id, sketch.id, rectangle.id, extrude.id, edge.id, face.id, chamfer.id].sort())
		expect(connections).toHaveLength(6)

		const idByNode = new Map(components.map((component) => [component.data?.nodeId, component.id]))
		const planeComponentId = idByNode.get(plane.id)
		const sketchComponentId = idByNode.get(sketch.id)
		const entityComponentId = idByNode.get(rectangle.id)
		const edgeComponentId = idByNode.get(edge.id)
		const chamferComponentId = idByNode.get(chamfer.id)
		expect(planeComponentId).toBeNumber()
		expect(sketchComponentId).toBeNumber()
		expect(entityComponentId).toBeNumber()
		expect(edgeComponentId).toBeNumber()
		expect(chamferComponentId).toBeNumber()
		if (!planeComponentId || !sketchComponentId || !entityComponentId || !edgeComponentId || !chamferComponentId) {
			throw new Error("Expected graph component ids")
		}
		expect(connections).toContainEqual({
			from: { componentId: planeComponentId, edge: "right", ratio: 0.5 },
			to: { componentId: sketchComponentId, edge: "left", ratio: 0.5 }
		})
		expect(connections).toContainEqual({
			from: { componentId: sketchComponentId, edge: "right", ratio: 0.5 },
			to: { componentId: entityComponentId, edge: "left", ratio: 0.5 }
		})
		expect(connections).toContainEqual({
			from: { componentId: edgeComponentId, edge: "right", ratio: 0.5 },
			to: { componentId: chamferComponentId, edge: "left", ratio: 0.5 }
		})
	})

	it("hides unreferenced generated solid geometry until its parent is selected or show all is enabled", () => {
		const editor = new PCadNodeEditor({ state: createStateWithoutAuthoredTopologyRefs(), generatedState: createGeneratedState() })
		let components = editor.getCanvasForTesting().getComponents()
		expect(components.some((component) => component.data?.nodeType === "generatedSolid")).toBe(false)
		expect(components.some((component) => component.data?.nodeType === "generatedSolidFace")).toBe(false)

		const showAll = editor.root.querySelector<HTMLInputElement>('input[type="checkbox"]')
		expect(showAll).not.toBeNull()
		if (!showAll) {
			throw new Error("Expected show all generated toggle")
		}
		showAll.checked = true
		showAll.dispatchEvent(new window.Event("change", { bubbles: true }))
		components = editor.getCanvasForTesting().getComponents()

		expect(components.some((component) => component.data?.nodeType === "generatedSolid")).toBe(true)
		expect(components.some((component) => component.data?.nodeType === "generatedSolidFace")).toBe(true)
	})

	it("shows generated geometry when its parent node is selected", () => {
		const editor = new PCadNodeEditor({
			state: createStateWithoutAuthoredTopologyRefs(),
			generatedState: createGeneratedState(),
			selectedNodeId: extrude.id
		})
		const components = editor.getCanvasForTesting().getComponents()

		expect(components.some((component) => component.data?.nodeType === "generatedSolid")).toBe(true)
		expect(components.some((component) => component.data?.nodeType === "generatedSolidFace")).toBe(true)
	})

	it("displays generated sketch topology and solid state as read-only graph nodes", () => {
		const editor = new PCadNodeEditor({ state: createState(), generatedState: createGeneratedState() })
		const canvas = editor.getCanvasForTesting()
		const components = canvas.getComponents()
		const connections = canvas.getConnections()

		const generatedSketch = components.find((component) => component.data?.nodeType === "generatedSketch")
		const generatedSketchVertex = components.find((component) => component.data?.nodeType === "generatedSketchVertex")
		const generatedSketchLoop = components.find((component) => component.data?.nodeType === "generatedSketchLoop")
		const generatedSketchProfile = components.find((component) => component.data?.nodeType === "generatedSketchProfile")
		const generatedSolid = components.find((component) => component.data?.nodeType === "generatedSolid")
		const generatedSolidVertex = components.find((component) => component.data?.nodeType === "generatedSolidVertex")
		const generatedSolidEdge = components.find((component) => component.data?.nodeType === "generatedSolidEdge")
		const generatedSolidFace = components.find((component) => component.data?.nodeType === "generatedSolidFace")
		expect(generatedSketch?.data?.detail).toBe("4 vertices · 1 loops · 1 profiles")
		expect(generatedSolid?.data?.detail).toBe("2 vertices · 1 edges · 1 faces")
		expect(generatedSketch?.data?.generated).toBe(true)
		expect(generatedSketchVertex?.data?.detail).toBe("(0, 0)")
		expect(generatedSketchLoop?.data?.detail).toBe("4 vertices")
		expect(generatedSketchProfile?.data?.detail).toBe("0 holes")
		expect(generatedSolid?.data?.generated).toBe(true)
		expect(generatedSolidVertex?.data?.detail).toBe("(0, 0, 0)")
		expect(generatedSolidEdge?.data?.detail).toBe("2 vertices")
		expect(generatedSolidFace?.data?.detail).toBe("1 edges")

		const idByGraph = new Map(components.map((component) => [component.data?.graphId, component.id]))
		const sketchComponentId = idByGraph.get(sketch.id)
		const generatedSketchComponentId = idByGraph.get("generated:sketch-1:sketch")
		const extrudeComponentId = idByGraph.get(extrude.id)
		const generatedSolidComponentId = idByGraph.get("generated:extrude-1-solid:solid")
		expect(sketchComponentId).toBeNumber()
		expect(generatedSketchComponentId).toBeNumber()
		expect(extrudeComponentId).toBeNumber()
		expect(generatedSolidComponentId).toBeNumber()
		if (!sketchComponentId || !generatedSketchComponentId || !extrudeComponentId || !generatedSolidComponentId) {
			throw new Error("Expected generated graph component ids")
		}
		expect(connections).toContainEqual({
			from: { componentId: sketchComponentId, edge: "right", ratio: 0.5 },
			to: { componentId: generatedSketchComponentId, edge: "left", ratio: 0.5 }
		})
		expect(connections).toContainEqual({
			from: { componentId: extrudeComponentId, edge: "right", ratio: 0.5 },
			to: { componentId: generatedSolidComponentId, edge: "left", ratio: 0.5 }
		})
	})

	it("inspects generated state without mapping it to a PCad selection callback", () => {
		const selected: Array<string | null> = []
		const generatedSelections: string[] = []
		const editor = new PCadNodeEditor({
			state: createState(),
			generatedState: createGeneratedState(),
			onSelectNode: (nodeId) => selected.push(nodeId),
			onSelectGenerated: (selection) => generatedSelections.push(selection.graphId)
		})
		const canvas = editor.getCanvasForTesting()
		const generatedSolid = canvas.getComponents().find((component) => component.data?.nodeType === "generatedSolid")
		expect(generatedSolid).toBeDefined()
		if (!generatedSolid) {
			throw new Error("Expected generated solid component")
		}

		canvas.setSelection([generatedSolid.id])

		expect(selected).toEqual([])
		expect(generatedSelections).toEqual([])
		expect(editor.root.textContent).toContain("Generated solid")
		expect(editor.root.textContent).toContain("Faces")
	})

	it("maps generated solid topology selection to generated selection callbacks", () => {
		const generatedSelections: string[] = []
		const editor = new PCadNodeEditor({
			state: createState(),
			generatedState: createGeneratedState(),
			onSelectGenerated: (selection) => generatedSelections.push(`${selection.type}:${selection.graphId}`)
		})
		const canvas = editor.getCanvasForTesting()
		const generatedEdge = canvas.getComponents().find((component) => component.data?.nodeType === "generatedSolidEdge")
		expect(generatedEdge).toBeDefined()
		if (!generatedEdge) {
			throw new Error("Expected generated edge component")
		}

		canvas.setSelection([generatedEdge.id])

		expect(generatedSelections).toEqual(["solidEdge:generated:extrude-1-solid:solid-edge:extrude-1-solid-edge-1"])
	})

	it("maps sketch entity node selection to PCad node callbacks", () => {
		const selected: Array<string | null> = []
		const editor = new PCadNodeEditor({
			state: createState(),
			onSelectNode: (nodeId) => selected.push(nodeId)
		})
		const canvas = editor.getCanvasForTesting()
		const sketchEntity = canvas.getComponents().find((component) => component.data?.nodeId === rectangle.id)
		expect(sketchEntity).toBeDefined()
		if (!sketchEntity) {
			throw new Error("Expected sketch entity component")
		}

		canvas.setSelection([sketchEntity.id])

		expect(selected).toEqual([rectangle.id])
	})

	it("maps canvas selection back to PCad node ids", () => {
		const selected: Array<string | null> = []
		const editor = new PCadNodeEditor({
			state: createState(),
			onSelectNode: (nodeId) => selected.push(nodeId)
		})
		const canvas = editor.getCanvasForTesting()
		const extrudeComponent = canvas.getComponents().find((component) => component.data?.nodeId === extrude.id)
		expect(extrudeComponent).toBeDefined()
		if (!extrudeComponent) {
			throw new Error("Expected extrude component")
		}

		canvas.setSelection([extrudeComponent.id])

		expect(selected).toEqual([extrude.id])
	})

	it("does not delete graph components through the canvas Delete key", () => {
		const editor = new PCadNodeEditor({ state: createState(), selectedNodeId: extrude.id })
		const canvas = editor.getCanvasForTesting()
		const before = canvas.getComponents()

		canvas.canvasElement.dispatchEvent(new window.KeyboardEvent("keydown", { key: "Delete", bubbles: true }))

		expect(canvas.getComponents()).toEqual(before)
	})
})
