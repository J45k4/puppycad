import { beforeEach, describe, expect, it } from "bun:test"
import { Window as HappyDOMWindow } from "happy-dom"
import { PartEditor } from "../src/ui/part"

class FakePreviewRenderer {
	public render(): void {}

	public setClearColor(): void {}

	public setPixelRatio(): void {}

	public setSize(): void {}
}

let domWindow: HappyDOMWindow

function dispatchPreviewPointer(window: HappyDOMWindow, canvas: HTMLCanvasElement, type: string, x: number, y: number): void {
	const event = new window.Event(type, { bubbles: true, cancelable: true }) as unknown as Event & {
		button: number
		clientX: number
		clientY: number
		isPrimary: boolean
		pointerId: number
		pointerType: string
	}
	event.button = 0
	event.clientX = x
	event.clientY = y
	event.isPrimary = true
	event.pointerId = 1
	event.pointerType = "mouse"
	canvas.dispatchEvent(event)
}

function clickPreview(window: HappyDOMWindow, canvas: HTMLCanvasElement, x: number, y: number): void {
	dispatchPreviewPointer(window, canvas, "pointermove", x, y)
	dispatchPreviewPointer(window, canvas, "pointerdown", x, y)
	dispatchPreviewPointer(window, canvas, "pointerup", x, y)
}

function clickButton(window: HappyDOMWindow, root: HTMLElement, label: string): void {
	const button = Array.from(root.querySelectorAll("button")).find((entry) => entry.textContent?.trim() === label)
	expect(button).toBeDefined()
	if (!button) {
		throw new Error(`Expected button "${label}"`)
	}
	button.dispatchEvent(new window.MouseEvent("click", { bubbles: true }) as unknown as Event)
}

function getPreviewCanvas(root: HTMLElement): HTMLCanvasElement {
	const previewCanvas = Array.from(root.querySelectorAll("canvas")).find((entry) => entry.tabIndex === 0)
	expect(previewCanvas).toBeDefined()
	if (!previewCanvas) {
		throw new Error("Expected preview canvas")
	}
	return previewCanvas
}

describe("PartEditor", () => {
	beforeEach(() => {
		domWindow = new HappyDOMWindow()
		globalThis.window = domWindow as unknown as typeof globalThis.window
		globalThis.document = domWindow.document as unknown as Document
		globalThis.requestAnimationFrame = ((callback: FrameRequestCallback) => {
			callback(0)
			return 1
		}) as typeof globalThis.requestAnimationFrame
	})

	it("supports a line-based sketch, finish, extrude, and reload flow", () => {
		const editor = new PartEditor({
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const previewCanvas = getPreviewCanvas(editor.root)
		previewCanvas.getBoundingClientRect = () => ({
			left: 0,
			top: 0,
			width: 360,
			height: 360,
			right: 360,
			bottom: 360,
			x: 0,
			y: 0,
			toJSON: () => ({})
		})
		editor.selectReferencePlane("Front")
		editor.enterSketchMode()

		clickPreview(domWindow, previewCanvas, 145, 145)
		clickPreview(domWindow, previewCanvas, 215, 145)
		clickPreview(domWindow, previewCanvas, 215, 145)
		clickPreview(domWindow, previewCanvas, 215, 215)
		clickPreview(domWindow, previewCanvas, 215, 215)
		clickPreview(domWindow, previewCanvas, 145, 215)
		clickPreview(domWindow, previewCanvas, 145, 215)
		clickPreview(domWindow, previewCanvas, 145, 145)

		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		const state = editor.getState()
		expect(state.features).toHaveLength(2)
		expect(state.features[0]).toMatchObject({
			type: "sketch",
			dirty: false,
			profiles: [{ id: expect.any(String) }]
		})
		expect(state.features[1]).toMatchObject({
			type: "extrude",
			depth: 30
		})

		const reloaded = new PartEditor({
			initialState: state,
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		expect(reloaded.listSketches()).toHaveLength(1)
		expect(reloaded.listExtrudes()).toHaveLength(1)
	})

	it("supports rectangle sketches on other reference planes", () => {
		const editor = new PartEditor({
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const previewCanvas = getPreviewCanvas(editor.root)
		previewCanvas.getBoundingClientRect = () => ({
			left: 0,
			top: 0,
			width: 360,
			height: 360,
			right: 360,
			bottom: 360,
			x: 0,
			y: 0,
			toJSON: () => ({})
		})
		editor.selectReferencePlane("Top")
		editor.enterSketchMode()

		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 145, 140)
		clickPreview(domWindow, previewCanvas, 225, 225)
		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		expect(editor.listSketches()).toEqual([
			{
				id: expect.any(String),
				name: "Sketch 1",
				plane: "Top",
				dirty: false
			}
		])
		expect(editor.listExtrudes()).toEqual([
			{
				id: expect.any(String),
				name: "Extrude 1",
				depth: 30
			}
		])
	})

	it("allows editing and deleting a selected extrude", () => {
		const editor = new PartEditor({
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const previewCanvas = getPreviewCanvas(editor.root)
		previewCanvas.getBoundingClientRect = () => ({
			left: 0,
			top: 0,
			width: 360,
			height: 360,
			right: 360,
			bottom: 360,
			x: 0,
			y: 0,
			toJSON: () => ({})
		})

		editor.selectReferencePlane("Front")
		editor.enterSketchMode()
		clickPreview(domWindow, previewCanvas, 145, 145)
		clickPreview(domWindow, previewCanvas, 215, 145)
		clickPreview(domWindow, previewCanvas, 215, 145)
		clickPreview(domWindow, previewCanvas, 215, 215)
		clickPreview(domWindow, previewCanvas, 215, 215)
		clickPreview(domWindow, previewCanvas, 145, 215)
		clickPreview(domWindow, previewCanvas, 145, 215)
		clickPreview(domWindow, previewCanvas, 145, 145)
		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		const extrude = editor.listExtrudes()[0]
		expect(extrude).toBeDefined()
		if (!extrude) {
			throw new Error("Expected extrude")
		}

		editor.selectExtrude(extrude.id)
		const heightInput = editor.root.querySelector('input[type="number"]') as HTMLInputElement | null
		expect(heightInput).not.toBeNull()
		if (!heightInput) {
			throw new Error("Expected height input")
		}
		heightInput.value = "45"
		heightInput.dispatchEvent(new domWindow.Event("change", { bubbles: true }) as unknown as Event)

		expect(editor.listExtrudes()).toEqual([
			{
				id: extrude.id,
				name: "Extrude 1",
				depth: 45
			}
		])
		;(domWindow as unknown as { confirm: () => boolean }).confirm = () => true
		clickButton(domWindow, editor.root, "Delete Extrude")
		expect(editor.listExtrudes()).toEqual([])
	})
})
