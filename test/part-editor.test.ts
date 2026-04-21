import { beforeEach, describe, expect, it } from "bun:test"
import { Window as HappyDOMWindow } from "happy-dom"
import * as THREE from "three"
import { PartEditor } from "../src/ui/part"
import type { SketchFrame3D } from "../src/cad/extrude"

class FakePreviewRenderer {
	public render(): void {}

	public setClearColor(): void {}

	public setPixelRatio(): void {}

	public setSize(): void {}
}

let domWindow: HappyDOMWindow

function dispatchPreviewPointer(window: HappyDOMWindow, canvas: HTMLCanvasElement, type: string, x: number, y: number, button = 0): void {
	const event = new window.Event(type, { bubbles: true, cancelable: true }) as unknown as Event & {
		button: number
		clientX: number
		clientY: number
		isPrimary: boolean
		pointerId: number
		pointerType: string
	}
	event.button = button
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

function rotatePreview(window: HappyDOMWindow, canvas: HTMLCanvasElement, startX: number, startY: number, endX: number, endY: number): void {
	dispatchPreviewPointer(window, canvas, "pointerdown", startX, startY, 2)
	dispatchPreviewPointer(window, canvas, "pointermove", endX, endY)
	dispatchPreviewPointer(window, canvas, "pointerup", endX, endY, 2)
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

function setCanvasRect(canvas: HTMLCanvasElement): void {
	canvas.getBoundingClientRect = () => ({
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
}

async function flushAsyncUi(): Promise<void> {
	await Promise.resolve()
	await Promise.resolve()
}

async function submitTextPrompt(window: HappyDOMWindow, value: string, confirmLabel = "Save"): Promise<void> {
	const input = document.body.querySelector("input.modal-input") as HTMLInputElement | null
	expect(input).not.toBeNull()
	if (!input) {
		throw new Error("Expected modal input")
	}
	input.value = value
	const confirmButton = Array.from(document.body.querySelectorAll("button")).find((entry) => entry.textContent?.trim() === confirmLabel)
	expect(confirmButton).toBeDefined()
	if (!confirmButton) {
		throw new Error(`Expected modal button "${confirmLabel}"`)
	}
	confirmButton.dispatchEvent(new window.MouseEvent("click", { bubbles: true }) as unknown as Event)
	await flushAsyncUi()
}

function findEmptyPreviewPoint(editor: PartEditor, previewCanvas: HTMLCanvasElement): { x: number; y: number } {
	const partEditor = editor as unknown as {
		getExtrudeAt: (clientX: number, clientY: number) => string | null
		getExtrudeFaceAt: (clientX: number, clientY: number) => { extrudeId: string; faceId: string } | null
		getExtrudeEdgeAt: (clientX: number, clientY: number) => { extrudeId: string; edgeId: string } | null
		getReferencePlaneAt: (clientX: number, clientY: number) => string | null
	}
	const rect = previewCanvas.getBoundingClientRect()
	for (const v of [0.05, 0.15, 0.25, 0.75, 0.85, 0.95]) {
		for (const u of [0.05, 0.15, 0.25, 0.75, 0.85, 0.95]) {
			const x = rect.left + rect.width * u
			const y = rect.top + rect.height * v
			if (!partEditor.getExtrudeEdgeAt(x, y) && !partEditor.getExtrudeFaceAt(x, y) && !partEditor.getExtrudeAt(x, y) && !partEditor.getReferencePlaneAt(x, y)) {
				return { x, y }
			}
		}
	}
	throw new Error("Expected a preview point that misses solids, faces, and planes")
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

function getExtrudeEdgePreviewPoint(editor: PartEditor, previewCanvas: HTMLCanvasElement, extrudeId: string, edgeIndex = 0): { x: number; y: number; edgeId: string; label: string } {
	const partEditor = editor as unknown as {
		drawPreview: () => void
		previewCamera: THREE.PerspectiveCamera
		previewSolids: Array<{
			extrudeId: string
			edges: Array<{
				edgeId: string
				label: string
				line: THREE.Line
			}>
		}>
	}
	partEditor.drawPreview()
	const solid = partEditor.previewSolids.find((entry) => entry.extrudeId === extrudeId)
	expect(solid).toBeDefined()
	if (!solid) {
		throw new Error(`Expected extrude preview for ${extrudeId}`)
	}
	const edge = solid.edges[edgeIndex]
	expect(edge).toBeDefined()
	if (!edge) {
		throw new Error(`Expected edge ${edgeIndex} for ${extrudeId}`)
	}
	const position = edge.line.geometry.getAttribute("position")
	expect(position?.count).toBeGreaterThanOrEqual(2)
	if (!position || position.count < 2) {
		throw new Error(`Expected edge geometry for ${edge.label}`)
	}
	const start = edge.line.localToWorld(new THREE.Vector3().fromBufferAttribute(position, 0)).project(partEditor.previewCamera)
	const end = edge.line.localToWorld(new THREE.Vector3().fromBufferAttribute(position, 1)).project(partEditor.previewCamera)
	const rect = previewCanvas.getBoundingClientRect()
	return {
		x: rect.left + (((start.x + end.x) / 2 + 1) / 2) * rect.width,
		y: rect.top + ((1 - (start.y + end.y) / 2) / 2) * rect.height,
		edgeId: edge.edgeId,
		label: edge.label
	}
}

function getOccludedExtrudeEdgePreviewPoint(editor: PartEditor, previewCanvas: HTMLCanvasElement, extrudeId: string): { x: number; y: number; edgeId: string; label: string } {
	const partEditor = editor as unknown as {
		drawPreview: () => void
		previewCamera: THREE.PerspectiveCamera
		previewScene: THREE.Scene
		previewSolids: Array<{
			extrudeId: string
			mesh: THREE.Mesh
			edges: Array<{
				edgeId: string
				label: string
				line: THREE.Line
			}>
		}>
	}
	partEditor.drawPreview()
	partEditor.previewScene.updateMatrixWorld(true)
	partEditor.previewCamera.updateMatrixWorld(true)
	const solid = partEditor.previewSolids.find((entry) => entry.extrudeId === extrudeId)
	expect(solid).toBeDefined()
	if (!solid) {
		throw new Error(`Expected extrude preview for ${extrudeId}`)
	}

	const rect = previewCanvas.getBoundingClientRect()
	const raycaster = new THREE.Raycaster()
	for (const edge of solid.edges) {
		const position = edge.line.geometry.getAttribute("position")
		if (!position || position.count < 2) {
			continue
		}
		const start = edge.line.localToWorld(new THREE.Vector3().fromBufferAttribute(position, 0))
		const end = edge.line.localToWorld(new THREE.Vector3().fromBufferAttribute(position, 1))
		const midpoint = start.clone().add(end).multiplyScalar(0.5)
		const projectedMidpoint = midpoint.clone().project(partEditor.previewCamera)
		if (projectedMidpoint.z < -1 || projectedMidpoint.z > 1) {
			continue
		}
		const x = rect.left + ((projectedMidpoint.x + 1) / 2) * rect.width
		const y = rect.top + ((1 - projectedMidpoint.y) / 2) * rect.height
		raycaster.setFromCamera(new THREE.Vector2(((x - rect.left) / rect.width) * 2 - 1, -((y - rect.top) / rect.height) * 2 + 1), partEditor.previewCamera)
		const surfaceHit = raycaster.intersectObject(solid.mesh, false)[0]
		if (!surfaceHit) {
			continue
		}
		const surfaceDepth = surfaceHit.point.clone().project(partEditor.previewCamera).z
		if (projectedMidpoint.z > surfaceDepth + 0.0002) {
			return {
				x,
				y,
				edgeId: edge.edgeId,
				label: edge.label
			}
		}
	}
	throw new Error("Expected an occluded edge candidate")
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

function getExtrudeFacePreviewLocalPointAt(editor: PartEditor, extrudeId: string, faceLabel: string, u: number, v: number): THREE.Vector3 {
	const partEditor = editor as unknown as {
		drawPreview: () => void
		previewContentGroup: THREE.Group
		previewSolids: Array<{
			extrudeId: string
			faces: Array<{
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
	return partEditor.previewContentGroup.worldToLocal(face.mesh.localToWorld(localPoint))
}

function projectPreviewLocalPoint(editor: PartEditor, previewCanvas: HTMLCanvasElement, localPoint: THREE.Vector3): { x: number; y: number } {
	const partEditor = editor as unknown as {
		drawPreview: () => void
		previewCamera: THREE.PerspectiveCamera
		previewContentGroup: THREE.Group
	}
	partEditor.drawPreview()
	const projected = partEditor.previewContentGroup.localToWorld(localPoint.clone()).project(partEditor.previewCamera)
	const rect = previewCanvas.getBoundingClientRect()
	return {
		x: rect.left + ((projected.x + 1) / 2) * rect.width,
		y: rect.top + ((1 - projected.y) / 2) * rect.height
	}
}

function sketchPointToPreviewLocalPoint(point: { x: number; y: number }, frame: SketchFrame3D): THREE.Vector3 {
	const origin = new THREE.Vector3(frame.origin.x, frame.origin.y, frame.origin.z)
	const xAxis = new THREE.Vector3(frame.xAxis.x, frame.xAxis.y, frame.xAxis.z)
	const yAxis = new THREE.Vector3(frame.yAxis.x, frame.yAxis.y, frame.yAxis.z)
	return origin.add(xAxis.multiplyScalar(point.x)).add(yAxis.multiplyScalar(point.y))
}

function getSelectedSketchPreviewPoint(editor: PartEditor, previewCanvas: HTMLCanvasElement, point: { x: number; y: number }): { x: number; y: number } {
	const partEditor = editor as unknown as {
		getSelectedSketch: () => { target: unknown } | null
		resolveSketchFrame: (target: unknown) => SketchFrame3D | null
	}
	const sketch = partEditor.getSelectedSketch()
	expect(sketch).toBeDefined()
	if (!sketch) {
		throw new Error("Expected selected sketch")
	}
	const frame = partEditor.resolveSketchFrame(sketch.target)
	expect(frame).toBeDefined()
	if (!frame) {
		throw new Error("Expected sketch frame")
	}
	return projectPreviewLocalPoint(editor, previewCanvas, sketchPointToPreviewLocalPoint(point, frame))
}

function toSketchPoint(localPoint: THREE.Vector3, frame: SketchFrame3D): { x: number; y: number } {
	const origin = new THREE.Vector3(frame.origin.x, frame.origin.y, frame.origin.z)
	const xAxis = new THREE.Vector3(frame.xAxis.x, frame.xAxis.y, frame.xAxis.z).normalize()
	const yAxis = new THREE.Vector3(frame.yAxis.x, frame.yAxis.y, frame.yAxis.z).normalize()
	const offset = localPoint.clone().sub(origin)
	return {
		x: offset.dot(xAxis),
		y: offset.dot(yAxis)
	}
}

describe("PartEditor", () => {
	beforeEach(() => {
		domWindow = new HappyDOMWindow()
		globalThis.window = domWindow as unknown as typeof globalThis.window
		globalThis.document = domWindow.document as unknown as Document
		globalThis.HTMLElement = domWindow.HTMLElement as unknown as typeof globalThis.HTMLElement
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

	it("allows selecting a line in an open sketch and applying a length dimension", async () => {
		const editor = new PartEditor({
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const previewCanvas = getPreviewCanvas(editor.root)
		setCanvasRect(previewCanvas)

		editor.selectReferencePlane("Front")
		editor.enterSketchMode()
		clickPreview(domWindow, previewCanvas, 150, 180)
		clickPreview(domWindow, previewCanvas, 210, 180)

		const initialSketch = editor.getState().features[0]
		expect(initialSketch?.type).toBe("sketch")
		if (!initialSketch || initialSketch.type !== "sketch") {
			throw new Error("Expected sketch")
		}
		const initialLine = initialSketch.entities[0]
		expect(initialLine?.type).toBe("line")
		if (!initialLine || initialLine.type !== "line") {
			throw new Error("Expected line entity")
		}

		expect(editor.root.textContent).not.toContain("Dimension")

		const midpoint = getSelectedSketchPreviewPoint(editor, previewCanvas, {
			x: (initialLine.p0.x + initialLine.p1.x) / 2,
			y: (initialLine.p0.y + initialLine.p1.y) / 2
		})
		clickPreview(domWindow, previewCanvas, midpoint.x, midpoint.y)
		expect(editor.root.textContent).toContain("Dimension")

		clickButton(domWindow, editor.root, "Dimension")
		await flushAsyncUi()
		await submitTextPrompt(domWindow, "6")

		const state = editor.getState()
		const sketch = state.features[0]
		expect(sketch?.type).toBe("sketch")
		if (!sketch || sketch.type !== "sketch") {
			throw new Error("Expected sketch")
		}
		const line = sketch.entities[0]
		expect(line?.type).toBe("line")
		if (!line || line.type !== "line") {
			throw new Error("Expected line entity")
		}
		expect(sketch.dimensions).toEqual([
			{
				id: expect.any(String),
				type: "lineLength",
				entityId: line.id,
				value: 6
			}
		])
		expect(line.p0).toEqual(initialLine.p0)
		expect(Math.hypot(line.p1.x - line.p0.x, line.p1.y - line.p0.y)).toBeCloseTo(6, 6)

		const reloaded = new PartEditor({
			initialState: state,
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const reloadedSketch = reloaded.getState().features[0]
		expect(reloadedSketch?.type).toBe("sketch")
		if (!reloadedSketch || reloadedSketch.type !== "sketch") {
			throw new Error("Expected reloaded sketch")
		}
		expect(reloadedSketch.dimensions).toEqual(sketch.dimensions)
	})

	it("allows selecting rectangle sides and applying width and height dimensions", async () => {
		const editor = new PartEditor({
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const previewCanvas = getPreviewCanvas(editor.root)
		setCanvasRect(previewCanvas)

		editor.selectReferencePlane("Front")
		editor.enterSketchMode()
		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 140, 140)
		clickPreview(domWindow, previewCanvas, 220, 220)
		const initialSketch = editor.getState().features[0]
		expect(initialSketch?.type).toBe("sketch")
		if (!initialSketch || initialSketch.type !== "sketch") {
			throw new Error("Expected sketch")
		}
		const initialRectangle = initialSketch.entities[0]
		expect(initialRectangle?.type).toBe("cornerRectangle")
		if (!initialRectangle || initialRectangle.type !== "cornerRectangle") {
			throw new Error("Expected rectangle entity")
		}

		const topSidePoint = getSelectedSketchPreviewPoint(editor, previewCanvas, {
			x: (initialRectangle.p0.x + initialRectangle.p1.x) / 2,
			y: Math.max(initialRectangle.p0.y, initialRectangle.p1.y)
		})
		clickPreview(domWindow, previewCanvas, topSidePoint.x, topSidePoint.y)
		clickButton(domWindow, editor.root, "Dimension")
		await flushAsyncUi()
		await submitTextPrompt(domWindow, "8")

		let sketch = editor.getState().features[0]
		expect(sketch?.type).toBe("sketch")
		if (!sketch || sketch.type !== "sketch") {
			throw new Error("Expected sketch")
		}
		let rectangle = sketch.entities[0]
		expect(rectangle?.type).toBe("cornerRectangle")
		if (!rectangle || rectangle.type !== "cornerRectangle") {
			throw new Error("Expected rectangle entity")
		}
		expect(Math.abs(rectangle.p1.x - rectangle.p0.x)).toBeCloseTo(8, 6)

		const rightSidePoint = getSelectedSketchPreviewPoint(editor, previewCanvas, {
			x: Math.max(rectangle.p0.x, rectangle.p1.x),
			y: (rectangle.p0.y + rectangle.p1.y) / 2
		})
		clickPreview(domWindow, previewCanvas, rightSidePoint.x, rightSidePoint.y)
		clickButton(domWindow, editor.root, "Dimension")
		await flushAsyncUi()
		await submitTextPrompt(domWindow, "4")

		sketch = editor.getState().features[0]
		expect(sketch?.type).toBe("sketch")
		if (!sketch || sketch.type !== "sketch") {
			throw new Error("Expected sketch")
		}
		rectangle = sketch.entities[0]
		expect(rectangle?.type).toBe("cornerRectangle")
		if (!rectangle || rectangle.type !== "cornerRectangle") {
			throw new Error("Expected rectangle entity")
		}
		expect(Math.abs(rectangle.p1.x - rectangle.p0.x)).toBeCloseTo(8, 6)
		expect(Math.abs(rectangle.p1.y - rectangle.p0.y)).toBeCloseTo(4, 6)
		expect(sketch.dimensions).toEqual(
			expect.arrayContaining([
				{
					id: expect.any(String),
					type: "rectangleWidth",
					entityId: rectangle.id,
					value: 8
				},
				{
					id: expect.any(String),
					type: "rectangleHeight",
					entityId: rectangle.id,
					value: 4
				}
			])
		)
	})

	it("reopens dirty face sketches and allows side selection directly from the preview", () => {
		const editor = new PartEditor({
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const previewCanvas = getPreviewCanvas(editor.root)
		setCanvasRect(previewCanvas)

		editor.selectReferencePlane("Front")
		editor.enterSketchMode()
		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 140, 140)
		clickPreview(domWindow, previewCanvas, 220, 220)
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
		const sketch = editor.getState().features.find((feature) => feature.type === "sketch" && feature.dirty)
		expect(sketch?.type).toBe("sketch")
		if (!sketch || sketch.type !== "sketch") {
			throw new Error("Expected dirty sketch")
		}
		const rectangle = sketch.entities[0]
		expect(rectangle?.type).toBe("cornerRectangle")
		if (!rectangle || rectangle.type !== "cornerRectangle") {
			throw new Error("Expected rectangle entity")
		}

		clickButton(domWindow, editor.root, "Exit Sketch")
		editor.selectSketch(sketch.id)

		const rightSidePoint = getSelectedSketchPreviewPoint(editor, previewCanvas, {
			x: Math.max(rectangle.p0.x, rectangle.p1.x),
			y: (rectangle.p0.y + rectangle.p1.y) / 2
		})
		clickPreview(domWindow, previewCanvas, rightSidePoint.x, rightSidePoint.y)
		expect(editor.root.textContent).toContain("Dimension")
	})

	it("allows selecting and dimensioning a finished face sketch from the preview", async () => {
		const editor = new PartEditor({
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const previewCanvas = getPreviewCanvas(editor.root)
		setCanvasRect(previewCanvas)

		editor.selectReferencePlane("Front")
		editor.enterSketchMode()
		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 140, 140)
		clickPreview(domWindow, previewCanvas, 220, 220)
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

		let sketches = editor.getState().features.filter((feature) => feature.type === "sketch")
		const faceSketch = sketches[1]
		expect(faceSketch?.type).toBe("sketch")
		if (!faceSketch || faceSketch.type !== "sketch") {
			throw new Error("Expected face sketch")
		}

		clickButton(domWindow, editor.root, "Finish Sketch")
		editor.selectSketch(faceSketch.id)

		sketches = editor.getState().features.filter((feature) => feature.type === "sketch")
		const finishedFaceSketch = sketches[1]
		expect(finishedFaceSketch?.type).toBe("sketch")
		if (!finishedFaceSketch || finishedFaceSketch.type !== "sketch") {
			throw new Error("Expected finished face sketch")
		}
		const rectangle = finishedFaceSketch.entities[0]
		expect(rectangle?.type).toBe("cornerRectangle")
		if (!rectangle || rectangle.type !== "cornerRectangle") {
			throw new Error("Expected rectangle entity")
		}

		const rightSidePoint = getSelectedSketchPreviewPoint(editor, previewCanvas, {
			x: Math.max(rectangle.p0.x, rectangle.p1.x),
			y: (rectangle.p0.y + rectangle.p1.y) / 2
		})
		clickPreview(domWindow, previewCanvas, rightSidePoint.x, rightSidePoint.y)
		expect(editor.root.textContent).toContain("Dimension")

		clickButton(domWindow, editor.root, "Dimension")
		await flushAsyncUi()
		await submitTextPrompt(domWindow, "5")

		sketches = editor.getState().features.filter((feature) => feature.type === "sketch")
		const dimensionedFaceSketch = sketches[1]
		expect(dimensionedFaceSketch?.type).toBe("sketch")
		if (!dimensionedFaceSketch || dimensionedFaceSketch.type !== "sketch") {
			throw new Error("Expected dimensioned face sketch")
		}
		expect(dimensionedFaceSketch.dirty).toBe(false)
		expect(dimensionedFaceSketch.dimensions).toEqual([
			{
				id: expect.any(String),
				type: "rectangleHeight",
				entityId: rectangle.id,
				value: 5
			}
		])
		const dimensionedRectangle = dimensionedFaceSketch.entities[0]
		expect(dimensionedRectangle?.type).toBe("cornerRectangle")
		if (!dimensionedRectangle || dimensionedRectangle.type !== "cornerRectangle") {
			throw new Error("Expected dimensioned rectangle entity")
		}
		expect(Math.abs(dimensionedRectangle.p1.y - dimensionedRectangle.p0.y)).toBeCloseTo(5, 6)
	})

	it("supports dimensioning a line on a face sketch", async () => {
		const editor = new PartEditor({
			createPreviewRenderer: () => new FakePreviewRenderer()
		})
		const previewCanvas = getPreviewCanvas(editor.root)
		setCanvasRect(previewCanvas)

		editor.selectReferencePlane("Front")
		editor.enterSketchMode()
		clickButton(domWindow, editor.root, "Rectangle")
		clickPreview(domWindow, previewCanvas, 140, 140)
		clickPreview(domWindow, previewCanvas, 220, 220)
		clickButton(domWindow, editor.root, "Finish Sketch")
		clickButton(domWindow, editor.root, "Extrude")

		const baseExtrude = editor.listExtrudes()[0]
		expect(baseExtrude).toBeDefined()
		if (!baseExtrude) {
			throw new Error("Expected base extrude")
		}

		const faceCenter = getExtrudeFacePreviewPoint(editor, previewCanvas, baseExtrude.id, "Top Face")
		clickPreview(domWindow, previewCanvas, faceCenter.x, faceCenter.y)
		clickButton(domWindow, editor.root, "Sketch")

		clickPreview(domWindow, previewCanvas, 150, 180)
		clickPreview(domWindow, previewCanvas, 210, 180)

		let sketches = editor.getState().features.filter((feature) => feature.type === "sketch")
		const initialFaceSketch = sketches[1]
		expect(initialFaceSketch?.type).toBe("sketch")
		if (!initialFaceSketch || initialFaceSketch.type !== "sketch") {
			throw new Error("Expected face sketch")
		}
		const initialLine = initialFaceSketch.entities[0]
		expect(initialLine?.type).toBe("line")
		if (!initialLine || initialLine.type !== "line") {
			throw new Error("Expected face sketch line")
		}

		const midpoint = getSelectedSketchPreviewPoint(editor, previewCanvas, {
			x: (initialLine.p0.x + initialLine.p1.x) / 2,
			y: (initialLine.p0.y + initialLine.p1.y) / 2
		})
		clickPreview(domWindow, previewCanvas, midpoint.x, midpoint.y)
		clickButton(domWindow, editor.root, "Dimension")
		await flushAsyncUi()
		await submitTextPrompt(domWindow, "5")

		sketches = editor.getState().features.filter((feature) => feature.type === "sketch")
		const faceSketch = sketches[1]
		expect(faceSketch?.type).toBe("sketch")
		if (!faceSketch || faceSketch.type !== "sketch") {
			throw new Error("Expected face sketch")
		}
		const line = faceSketch.entities[0]
		expect(line?.type).toBe("line")
		if (!line || line.type !== "line") {
			throw new Error("Expected face sketch line")
		}
		expect(faceSketch.target).toMatchObject({
			type: "face",
			face: {
				type: "extrudeFace",
				extrudeId: baseExtrude.id
			}
		})
		expect(faceSketch.dimensions).toEqual([
			{
				id: expect.any(String),
				type: "lineLength",
				entityId: line.id,
				value: 5
			}
		])
		expect(Math.hypot(line.p1.x - line.p0.x, line.p1.y - line.p0.y)).toBeCloseTo(5, 6)
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

	it("rotates around the point under the cursor instead of the world origin", () => {
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

		const startPoint = getExtrudeFacePreviewPointAt(editor, previewCanvas, extrude.id, "Bottom Face", 0.75, 0.25)
		const anchoredPoint = getExtrudeFacePreviewLocalPointAt(editor, extrude.id, "Bottom Face", 0.75, 0.25)

		rotatePreview(domWindow, previewCanvas, startPoint.x, startPoint.y, startPoint.x + 56, startPoint.y + 32)

		const rotatedPoint = projectPreviewLocalPoint(editor, previewCanvas, anchoredPoint)
		expect(Math.hypot(rotatedPoint.x - startPoint.x, rotatedPoint.y - startPoint.y)).toBeLessThan(2)
	})

	it("falls back to the view-center target when rotating over empty space", () => {
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
		editor.setReferencePlaneVisible("Front", false)
		editor.setReferencePlaneVisible("Top", false)
		editor.setReferencePlaneVisible("Right", false)

		const partEditor = editor as unknown as {
			drawPreview: () => void
			getOrbitAnchorPoint: (clientX: number, clientY: number) => THREE.Vector3 | null
			previewBaseDistance: number
			previewOrbitPivot: THREE.Vector3
		}
		partEditor.previewBaseDistance = 48
		partEditor.drawPreview()
		const expectedPivot = partEditor.getOrbitAnchorPoint(180, 180)
		expect(expectedPivot).toBeDefined()
		if (!expectedPivot) {
			throw new Error("Expected center fallback pivot")
		}
		expect(partEditor.previewOrbitPivot.length()).toBe(0)
		const emptyPoint = findEmptyPreviewPoint(editor, previewCanvas)

		rotatePreview(domWindow, previewCanvas, emptyPoint.x, emptyPoint.y, emptyPoint.x + 50, emptyPoint.y + 35)

		expect(partEditor.previewOrbitPivot.distanceTo(expectedPivot)).toBeLessThan(1e-6)
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

	it("allows selecting an extrude edge directly from the preview", () => {
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
		const previewPoint = getExtrudeEdgePreviewPoint(editor, previewCanvas, extrude.id)
		const partEditor = editor as unknown as {
			getExtrudeEdgeAt: (clientX: number, clientY: number) => { extrudeId: string; edgeId: string } | null
			previewSolids: Array<{
				extrudeId: string
				fillMaterial: THREE.MeshStandardMaterial
				edges: Array<{
					edgeId: string
					material: THREE.LineBasicMaterial
					highlight: THREE.Mesh
					highlightMaterial: THREE.MeshBasicMaterial
				}>
			}>
		}
		expect(partEditor.getExtrudeEdgeAt(previewPoint.x, previewPoint.y)).toEqual({
			extrudeId: extrude.id,
			edgeId: previewPoint.edgeId
		})

		clickPreview(domWindow, previewCanvas, previewPoint.x, previewPoint.y)

		expect(editor.root.textContent).toContain(`${previewPoint.label} selected.`)
		expect(editor.root.textContent).toContain(`Edge: ${previewPoint.label}`)
		expect(editor.root.textContent).toContain("Delete Extrude")
		const previewSolid = partEditor.previewSolids.find((entry) => entry.extrudeId === extrude.id)
		expect(previewSolid?.fillMaterial.color.getHex()).toBe(0x3b82f6)
		const selectedEdge = previewSolid?.edges.find((edge) => edge.edgeId === previewPoint.edgeId)
		expect(selectedEdge?.material.color.getHex()).toBe(0xf59e0b)
		expect(selectedEdge?.highlight.visible).toBe(true)
		expect(selectedEdge?.highlight.scale.x).toBeCloseTo(0.04, 6)
		expect(selectedEdge?.highlightMaterial.color.getHex()).toBe(0xf59e0b)
	})

	it("clears the current selection when clicking empty preview space", () => {
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

		const previewPoint = getExtrudeEdgePreviewPoint(editor, previewCanvas, extrude.id)
		clickPreview(domWindow, previewCanvas, previewPoint.x, previewPoint.y)
		expect(editor.root.textContent).toContain(`Edge: ${previewPoint.label}`)

		editor.setReferencePlaneVisible("Front", false)
		editor.setReferencePlaneVisible("Top", false)
		editor.setReferencePlaneVisible("Right", false)
		const partEditor = editor as unknown as {
			drawPreview: () => void
			previewBaseDistance: number
			previewSolids: Array<{
				extrudeId: string
				fillMaterial: THREE.MeshStandardMaterial
				edges: Array<{
					edgeId: string
					material: THREE.LineBasicMaterial
					highlight: THREE.Mesh
				}>
			}>
		}
		partEditor.previewBaseDistance = 80
		partEditor.drawPreview()
		const emptyPoint = findEmptyPreviewPoint(editor, previewCanvas)
		clickPreview(domWindow, previewCanvas, emptyPoint.x, emptyPoint.y)

		expect(editor.root.textContent).not.toContain(`Edge: ${previewPoint.label}`)
		expect(editor.root.textContent).not.toContain("Delete Extrude")
		const previewSolid = partEditor.previewSolids.find((entry) => entry.extrudeId === extrude.id)
		expect(previewSolid?.fillMaterial.color.getHex()).toBe(0x3b82f6)
		const clearedEdge = previewSolid?.edges.find((edge) => edge.edgeId === previewPoint.edgeId)
		expect(clearedEdge?.material.color.getHex()).toBe(0xe2e8f0)
		expect(clearedEdge?.highlight.visible).toBe(false)
	})

	it("does not select extrude edges through an occluding body face", () => {
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

		const partEditor = editor as unknown as {
			drawPreview: () => void
			getExtrudeEdgeAt: (clientX: number, clientY: number) => { extrudeId: string; edgeId: string } | null
			previewRotation: { yaw: number; pitch: number }
		}
		partEditor.previewRotation.yaw = -0.7
		partEditor.previewRotation.pitch = 0.35
		partEditor.drawPreview()

		const occludedEdge = getOccludedExtrudeEdgePreviewPoint(editor, previewCanvas, extrude.id)

		expect(partEditor.getExtrudeEdgeAt(occludedEdge.x, occludedEdge.y)?.edgeId).not.toBe(occludedEdge.edgeId)
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

	it("keeps side-face sketch input on the selected face after orbiting", () => {
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

		const partEditor = editor as unknown as {
			drawPreview: () => void
			previewSolids: Array<{
				extrudeId: string
				faces: Array<{
					faceId: string
					label: string
					frame: SketchFrame3D
				}>
			}>
			selectExtrudeFace: (extrudeId: string, faceId?: string) => void
			setPreviewOrbitPivot: (nextPivot: THREE.Vector3 | null) => void
		}
		partEditor.drawPreview()
		const previewSolid = partEditor.previewSolids.find((entry) => entry.extrudeId === baseExtrude.id)
		expect(previewSolid).toBeDefined()
		if (!previewSolid) {
			throw new Error("Expected preview solid")
		}
		const sideFace = previewSolid.faces.find((face) => face.label.startsWith("Side Face"))
		expect(sideFace).toBeDefined()
		if (!sideFace) {
			throw new Error("Expected side face")
		}

		partEditor.selectExtrudeFace(baseExtrude.id, sideFace.faceId)
		clickButton(domWindow, editor.root, "Sketch")
		clickButton(domWindow, editor.root, "Rectangle")
		partEditor.setPreviewOrbitPivot(new THREE.Vector3(4, -3, 1.5))
		partEditor.drawPreview()

		const startLocal = getExtrudeFacePreviewLocalPointAt(editor, baseExtrude.id, sideFace.label, 0.25, 0.3)
		const endLocal = getExtrudeFacePreviewLocalPointAt(editor, baseExtrude.id, sideFace.label, 0.7, 0.75)
		const startPoint = projectPreviewLocalPoint(editor, previewCanvas, startLocal)
		const endPoint = projectPreviewLocalPoint(editor, previewCanvas, endLocal)
		clickPreview(domWindow, previewCanvas, startPoint.x, startPoint.y)
		clickPreview(domWindow, previewCanvas, endPoint.x, endPoint.y)

		const faceSketch = editor.getState().features[2]
		expect(faceSketch).toBeDefined()
		expect(faceSketch?.type).toBe("sketch")
		if (!faceSketch || faceSketch.type !== "sketch") {
			throw new Error("Expected face sketch")
		}
		const rectangle = faceSketch.entities[0]
		expect(rectangle?.type).toBe("cornerRectangle")
		if (!rectangle || rectangle.type !== "cornerRectangle") {
			throw new Error("Expected rectangle entity")
		}

		const expectedStart = toSketchPoint(startLocal, sideFace.frame)
		const expectedEnd = toSketchPoint(endLocal, sideFace.frame)
		expect(rectangle.p0.x).toBeCloseTo(expectedStart.x, 6)
		expect(rectangle.p0.y).toBeCloseTo(expectedStart.y, 6)
		expect(rectangle.p1.x).toBeCloseTo(expectedEnd.x, 6)
		expect(rectangle.p1.y).toBeCloseTo(expectedEnd.y, 6)
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
