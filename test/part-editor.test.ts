import { beforeEach, describe, expect, it } from "bun:test"
import { Window as HappyDOMWindow } from "happy-dom"
import * as THREE from "three"
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

function getExtrudePreviewPoint(editor: PartEditor, previewCanvas: HTMLCanvasElement, extrudeId: string): { x: number; y: number } {
	const partEditor = editor as unknown as {
		drawPreview: () => void
		previewCamera: THREE.PerspectiveCamera
		previewSolids: Array<{
			extrudeId: string
			mesh: THREE.Mesh
			faces: Array<{
				faceId: string
				label: string
				mesh: THREE.Mesh
			}>
		}>
	}
	partEditor.drawPreview()
	const solid = partEditor.previewSolids.find((entry) => entry.extrudeId === extrudeId)
	expect(solid).toBeDefined()
	if (!solid) {
		throw new Error(`Expected extrude preview for ${extrudeId}`)
	}
	const bounds = new THREE.Box3().setFromObject(solid.mesh)
	const center = bounds.getCenter(new THREE.Vector3())
	const projected = center.project(partEditor.previewCamera)
	const rect = previewCanvas.getBoundingClientRect()
	return {
		x: rect.left + ((projected.x + 1) / 2) * rect.width,
		y: rect.top + ((1 - projected.y) / 2) * rect.height
	}
}

function getExtrudeFacePreviewPoint(editor: PartEditor, previewCanvas: HTMLCanvasElement, extrudeId: string, faceLabel: string): { x: number; y: number } {
	const partEditor = editor as unknown as {
		drawPreview: () => void
		previewCamera: THREE.PerspectiveCamera
		previewSolids: Array<{
			extrudeId: string
			faces: Array<{
				faceId: string
				label: string
				mesh: THREE.Mesh
			}>
		}>
	}
	partEditor.drawPreview()
	const solid = partEditor.previewSolids.find((entry) => entry.extrudeId === extrudeId)
	expect(solid).toBeDefined()
	if (!solid) {
		throw new Error(`Expected extrude preview for ${extrudeId}`)
	}
	const face = solid.faces.find((entry) => entry.label === faceLabel)
	expect(face).toBeDefined()
	if (!face) {
		throw new Error(`Expected face "${faceLabel}" for ${extrudeId}`)
	}
	face.mesh.geometry.computeBoundingBox()
	const bounds = face.mesh.geometry.boundingBox
	expect(bounds).toBeDefined()
	if (!bounds) {
		throw new Error(`Expected face bounds for ${faceLabel}`)
	}
	const localPoint = new THREE.Vector3(
		THREE.MathUtils.lerp(bounds.min.x, bounds.max.x, 0.5),
		THREE.MathUtils.lerp(bounds.min.y, bounds.max.y, 0.5),
		THREE.MathUtils.lerp(bounds.min.z, bounds.max.z, 0.5)
	)
	const projected = face.mesh.localToWorld(localPoint).project(partEditor.previewCamera)
	const rect = previewCanvas.getBoundingClientRect()
	return {
		x: rect.left + ((projected.x + 1) / 2) * rect.width,
		y: rect.top + ((1 - projected.y) / 2) * rect.height
	}
}

function getExtrudeFacePreviewPointAt(editor: PartEditor, previewCanvas: HTMLCanvasElement, extrudeId: string, faceLabel: string, u: number, v: number): { x: number; y: number } {
	const partEditor = editor as unknown as {
		drawPreview: () => void
		previewCamera: THREE.PerspectiveCamera
		previewSolids: Array<{
			extrudeId: string
			faces: Array<{
				faceId: string
				label: string
				mesh: THREE.Mesh
			}>
		}>
	}
	partEditor.drawPreview()
	const solid = partEditor.previewSolids.find((entry) => entry.extrudeId === extrudeId)
	expect(solid).toBeDefined()
	if (!solid) {
		throw new Error(`Expected extrude preview for ${extrudeId}`)
	}
	const face = solid.faces.find((entry) => entry.label === faceLabel)
	expect(face).toBeDefined()
	if (!face) {
		throw new Error(`Expected face "${faceLabel}" for ${extrudeId}`)
	}
	face.mesh.geometry.computeBoundingBox()
	const bounds = face.mesh.geometry.boundingBox
	expect(bounds).toBeDefined()
	if (!bounds) {
		throw new Error(`Expected face bounds for ${faceLabel}`)
	}
	const localPoint = new THREE.Vector3(
		THREE.MathUtils.lerp(bounds.min.x, bounds.max.x, u),
		THREE.MathUtils.lerp(bounds.min.y, bounds.max.y, v),
		THREE.MathUtils.lerp(bounds.min.z, bounds.max.z, 0.5)
	)
	const projected = face.mesh.localToWorld(localPoint).project(partEditor.previewCamera)
	const rect = previewCanvas.getBoundingClientRect()
	return {
		x: rect.left + ((projected.x + 1) / 2) * rect.width,
		y: rect.top + ((1 - projected.y) / 2) * rect.height
	}
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
		expect(state.solids).toHaveLength(1)
		expect(state.features[0]).toMatchObject({
			type: "sketch",
			dirty: false,
			profiles: [{ id: expect.any(String) }]
		})
		expect(state.features[1]).toMatchObject({
			type: "extrude",
			depth: 30
		})
		expect(state.solids?.[0]).toMatchObject({
			featureId: state.features[1]?.id,
			faces: expect.any(Array)
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
				targetLabel: "Top",
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

	it("allows selecting an extrude directly from the preview", () => {
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
		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 145, 145)
		clickPreview(domWindow, previewCanvas, 215, 215)
		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		const extrude = editor.listExtrudes()[0]
		expect(extrude).toBeDefined()
		if (!extrude) {
			throw new Error("Expected extrude")
		}

		editor.selectReferencePlane("Front")
		expect(editor.root.textContent).not.toContain("Delete Extrude")

		const previewPoint = getExtrudePreviewPoint(editor, previewCanvas, extrude.id)
		clickPreview(domWindow, previewCanvas, previewPoint.x, previewPoint.y)

		expect(editor.root.textContent).toContain("Delete Extrude")
		const heightInput = editor.root.querySelector('input[type="number"]') as HTMLInputElement | null
		expect(heightInput?.value).toBe("30")
	})

	it("allows selecting an extrude face directly from the preview", () => {
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
		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 145, 145)
		clickPreview(domWindow, previewCanvas, 215, 215)
		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		const extrude = editor.listExtrudes()[0]
		expect(extrude).toBeDefined()
		if (!extrude) {
			throw new Error("Expected extrude")
		}

		editor.selectReferencePlane("Front")
		const previewPoint = getExtrudeFacePreviewPoint(editor, previewCanvas, extrude.id, "Bottom Face")
		clickPreview(domWindow, previewCanvas, previewPoint.x, previewPoint.y)

		expect(editor.root.textContent).toContain("Bottom Face selected.")
		expect(editor.root.textContent).toContain("Delete Extrude")
		const partEditor = editor as unknown as {
			previewSolids: Array<{
				extrudeId: string
				fillMaterial: THREE.MeshStandardMaterial
				faces: Array<{
					label: string
					material: THREE.MeshBasicMaterial
				}>
			}>
		}
		const previewSolid = partEditor.previewSolids.find((entry) => entry.extrudeId === extrude.id)
		expect(previewSolid?.fillMaterial.color.getHex()).toBe(0x3b82f6)
		expect(previewSolid?.faces.find((face) => face.label === "Bottom Face")?.material.color.getHex()).toBe(0xf59e0b)
	})

	it("supports sketching on a selected extrude face", () => {
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
		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 145, 145)
		clickPreview(domWindow, previewCanvas, 215, 215)
		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		const baseExtrude = editor.listExtrudes()[0]
		expect(baseExtrude).toBeDefined()
		if (!baseExtrude) {
			throw new Error("Expected base extrude")
		}

		const faceCenter = getExtrudeFacePreviewPoint(editor, previewCanvas, baseExtrude.id, "Top Face")
		clickPreview(domWindow, previewCanvas, faceCenter.x, faceCenter.y)
		const selectedFaceLabel = editor.root.textContent?.includes("Top Face selected.") ? "Top Face" : "Bottom Face"
		expect(editor.root.textContent).toContain(`${selectedFaceLabel} selected.`)
		clickButton(domWindow, editor.root, "Sketch")
		expect(editor.root.textContent).toContain(`Sketch: ${selectedFaceLabel}`)

		clickButton(domWindow, editor.root, "Rectangle")
		const start = getExtrudeFacePreviewPointAt(editor, previewCanvas, baseExtrude.id, selectedFaceLabel, 0.3, 0.3)
		const end = getExtrudeFacePreviewPointAt(editor, previewCanvas, baseExtrude.id, selectedFaceLabel, 0.7, 0.7)
		clickPreview(domWindow, previewCanvas, start.x, start.y)
		clickPreview(domWindow, previewCanvas, end.x, end.y)
		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		expect(editor.listSketches()).toHaveLength(2)
		expect(editor.listExtrudes()).toHaveLength(2)
		const state = editor.getState()
		expect(state.features[2]).toMatchObject({
			type: "sketch",
			target: {
				type: "face",
				face: {
					type: "extrudeFace",
					extrudeId: baseExtrude.id
				}
			}
		})
		expect(state.features[3]).toMatchObject({
			type: "extrude"
		})
	})

	it("allows extruding a finished face sketch after reselecting it", () => {
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
		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 145, 145)
		clickPreview(domWindow, previewCanvas, 215, 215)
		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		const baseExtrude = editor.listExtrudes()[0]
		expect(baseExtrude).toBeDefined()
		if (!baseExtrude) {
			throw new Error("Expected base extrude")
		}

		const faceCenter = getExtrudeFacePreviewPoint(editor, previewCanvas, baseExtrude.id, "Top Face")
		clickPreview(domWindow, previewCanvas, faceCenter.x, faceCenter.y)
		const selectedFaceLabel = editor.root.textContent?.includes("Top Face selected.") ? "Top Face" : "Bottom Face"
		expect(editor.root.textContent).toContain(`${selectedFaceLabel} selected.`)
		clickButton(domWindow, editor.root, "Sketch")
		clickButton(domWindow, editor.root, "Rectangle")
		const start = getExtrudeFacePreviewPointAt(editor, previewCanvas, baseExtrude.id, selectedFaceLabel, 0.3, 0.3)
		const end = getExtrudeFacePreviewPointAt(editor, previewCanvas, baseExtrude.id, selectedFaceLabel, 0.7, 0.7)
		clickPreview(domWindow, previewCanvas, start.x, start.y)
		clickPreview(domWindow, previewCanvas, end.x, end.y)
		clickButton(domWindow, editor.root, "Finish Sketch")

		const faceSketch = editor.listSketches()[1]
		expect(faceSketch).toBeDefined()
		if (!faceSketch) {
			throw new Error("Expected face sketch")
		}

		editor.selectReferencePlane("Front")
		editor.selectSketch(faceSketch.id)
		expect(editor.root.textContent).toContain("Sketch:")
		clickButton(domWindow, editor.root, "Extrude")

		expect(editor.listExtrudes()).toHaveLength(2)
	})
})
