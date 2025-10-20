import { beforeEach, describe, expect, it } from "bun:test"
import { Window } from "happy-dom"
import type { CanvasComponent, Connection, EditorCanvasOptions } from "../src/canvas"
import { SchemanticEditor } from "../src/schemantic"
import type { SchemanticEditorState } from "../src/schemantic"

type SchematicComponentData = { type?: string }

type FakeEditorCanvasOptions = EditorCanvasOptions<SchematicComponentData>

type EditorCanvasMinimal = {
	root: HTMLDivElement
	getComponents: () => CanvasComponent<SchematicComponentData>[]
	getConnections: () => Connection[]
	setGridSpacing: (spacing: number) => void
	getGridSpacing: () => number
}

class FakeEditorCanvas implements EditorCanvasMinimal {
	public root: HTMLDivElement
	public options: FakeEditorCanvasOptions
	public components: CanvasComponent<SchematicComponentData>[]
	public connections: Connection[]
	public gridSpacing: number

	public constructor(options: FakeEditorCanvasOptions = {}) {
		this.root = document.createElement("div") as HTMLDivElement
		this.options = options
		this.components = (options.initialComponents as CanvasComponent<SchematicComponentData>[] | undefined)?.map((component) => ({ ...component })) ?? []
		this.connections = (options.initialConnections ?? []).map((connection) => ({
			from: { ...connection.from },
			to: { ...connection.to }
		}))
		this.gridSpacing = options.gridSpacing ?? 100
	}

	public getComponents(): CanvasComponent<SchematicComponentData>[] {
		return this.components.map((component) => ({ ...component }))
	}

	public getConnections(): Connection[] {
		return this.connections.map((connection) => ({
			from: { ...connection.from },
			to: { ...connection.to }
		}))
	}

	public setGridSpacing(spacing: number): void {
		this.gridSpacing = spacing
	}

	public getGridSpacing(): number {
		return this.gridSpacing
	}
}

describe("SchemanticEditor", () => {
	beforeEach(() => {
		const window = new Window()
		globalThis.window = window as unknown as typeof globalThis.window
		globalThis.document = window.document as unknown as Document
	})

	it("passes initial state and component helpers to the editor canvas", () => {
		const initialState: SchemanticEditorState = {
			components: [{ id: 5, x: 10, y: 20, width: 30, height: 40, data: { type: "ic" } }],
			connections: [
				{
					from: { componentId: 5, edge: "left" as const, ratio: 0.2 },
					to: { componentId: 5, edge: "right" as const, ratio: 0.8 }
				}
			]
		}
		let capturedOptions: FakeEditorCanvasOptions | undefined

		new SchemanticEditor({
			initialState,
			createEditorCanvas: (options) => {
				capturedOptions = options
				return new FakeEditorCanvas(options)
			}
		})

		expect(capturedOptions?.initialComponents).toEqual(initialState.components)
		expect(capturedOptions?.initialConnections).toEqual(initialState.connections)
		expect(capturedOptions?.gridSpacing).toBe(80)

		const label = capturedOptions?.getComponentLabel?.({
			id: 1,
			x: 0,
			y: 0,
			width: 10,
			height: 10,
			data: { type: "ic" }
		} as CanvasComponent<SchematicComponentData>)
		expect(label).toBe("IC")

		const fallbackLabel = capturedOptions?.getComponentLabel?.({
			id: 3,
			x: 0,
			y: 0,
			width: 10,
			height: 10
		} as CanvasComponent<SchematicComponentData>)
		expect(fallbackLabel).toBe("C3")

		const created = capturedOptions?.createComponent?.("resistor", { x: 160, y: 140 }, { createId: () => 42 })
		expect(created).toEqual({
			id: 42,
			x: 110,
			y: 120,
			width: 100,
			height: 40,
			data: { type: "resistor" }
		})

		const originalWarn = console.warn
		const warnings: string[] = []
		console.warn = (...args: unknown[]) => {
			warnings.push(String(args[0]))
		}

		const unknown = capturedOptions?.createComponent?.("unknown", { x: 160, y: 140 }, { createId: () => 7 })

		console.warn = originalWarn

		expect(unknown).toBeNull()
		expect(warnings[0]).toContain("Unknown schemantic component: unknown")
	})

	it("returns current canvas state when requested", () => {
		const fakeCanvas = new FakeEditorCanvas()
		fakeCanvas.components = [{ id: 1, x: 10, y: 20, width: 30, height: 40, data: { type: "capacitor" } }]
		fakeCanvas.connections = [
			{
				from: { componentId: 1, edge: "left" as const, ratio: 0.1 },
				to: { componentId: 1, edge: "right" as const, ratio: 0.9 }
			}
		]

		const editor = new SchemanticEditor({
			createEditorCanvas: () => fakeCanvas
		})

		expect(editor.getState()).toEqual({
			components: fakeCanvas.components,
			connections: fakeCanvas.connections
		})
	})

	it("notifies listeners when the canvas updates components or connections", () => {
		const notifications: number[] = []
		let capturedOptions: FakeEditorCanvasOptions | undefined

		new SchemanticEditor({
			onStateChange: () => notifications.push(notifications.length),
			createEditorCanvas: (options) => {
				capturedOptions = options
				return new FakeEditorCanvas(options)
			}
		})

		expect(capturedOptions).toBeDefined()
		capturedOptions?.onComponentsChange?.([])
		capturedOptions?.onConnectionsChange?.([])

		expect(notifications).toHaveLength(2)
	})

	it("provides a toolbar to adjust grid spacing", () => {
		let fakeCanvas: FakeEditorCanvas | null = null
		const editor = new SchemanticEditor({
			createEditorCanvas: (options) => {
				fakeCanvas = new FakeEditorCanvas(options)
				return fakeCanvas
			}
		})

		if (!fakeCanvas) {
			throw new Error("Expected canvas to be created")
		}
		const canvas = fakeCanvas as FakeEditorCanvas

		const toolbar = editor.createToolbar()
		const select = toolbar.root.querySelector("select") as HTMLSelectElement | null
		expect(select).not.toBeNull()
		if (!select) {
			throw new Error("Expected toolbar to provide a select element")
		}

		expect(select.value).toBe("80")

		select.value = "120"
		select.dispatchEvent(new Event("change"))

		expect(editor.getGridSpacing()).toBe(120)
		expect(canvas.gridSpacing).toBe(120)
	})
})
