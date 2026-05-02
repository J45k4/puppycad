import { beforeEach, describe, expect, it } from "bun:test"
import { Window } from "happy-dom"
import type { PCadGraphNode, PCadState } from "../src/schema"
import { PCadNodeEditor } from "../src/ui/pcad-node-editor"

const plane: PCadGraphNode = {
	id: "plane-front",
	type: "referencePlane",
	name: "Front",
	plane: "XY"
}

const sketch: PCadGraphNode = {
	id: "sketch-1",
	type: "sketch",
	name: "Sketch 1",
	targetId: plane.id,
	entities: [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 8 } }],
	dimensions: []
}

const extrude: PCadGraphNode = {
	id: "extrude-1",
	type: "extrude",
	name: "Extrude 1",
	sketchId: sketch.id,
	profileId: "profile-1",
	operation: "newBody",
	depth: 12
}

const edge: PCadGraphNode = {
	id: "edge-extrude-1-edge-1",
	type: "edge",
	sourceId: extrude.id,
	edgeId: "extrude-1-solid-edge-1"
}

const face: PCadGraphNode = {
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
			[extrude.id, extrude],
			[edge.id, edge],
			[face.id, face],
			[chamfer.id, chamfer]
		]),
		rootNodeIds: [plane.id, extrude.id, chamfer.id]
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

		expect(components.map((component) => component.data?.nodeId).sort()).toEqual([plane.id, sketch.id, extrude.id, edge.id, face.id, chamfer.id].sort())
		expect(connections).toHaveLength(5)

		const idByNode = new Map(components.map((component) => [component.data?.nodeId, component.id]))
		const planeComponentId = idByNode.get(plane.id)
		const sketchComponentId = idByNode.get(sketch.id)
		const edgeComponentId = idByNode.get(edge.id)
		const chamferComponentId = idByNode.get(chamfer.id)
		expect(planeComponentId).toBeNumber()
		expect(sketchComponentId).toBeNumber()
		expect(edgeComponentId).toBeNumber()
		expect(chamferComponentId).toBeNumber()
		if (!planeComponentId || !sketchComponentId || !edgeComponentId || !chamferComponentId) {
			throw new Error("Expected graph component ids")
		}
		expect(connections).toContainEqual({
			from: { componentId: planeComponentId, edge: "right", ratio: 0.5 },
			to: { componentId: sketchComponentId, edge: "left", ratio: 0.5 }
		})
		expect(connections).toContainEqual({
			from: { componentId: edgeComponentId, edge: "right", ratio: 0.5 },
			to: { componentId: chamferComponentId, edge: "left", ratio: 0.5 }
		})
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
