import { PART_PROJECT_DEFAULT_HEIGHT, PART_PROJECT_DEFAULT_ROTATION } from "./project-file"
import type { PartProjectExtrudedModel, PartProjectItemData, PartProjectPreviewRotation } from "./project-file"
import { UiComponent } from "./ui"
import * as THREE from "three"

type Point2D = { x: number; y: number }

type ExtrudedModel = PartProjectExtrudedModel

export type PartEditorState = PartProjectItemData

type PartEditorOptions = {
	initialState?: PartEditorState
	onStateChange?: () => void
}

const SKETCH_CANVAS_SIZE = 360

export class PartEditor extends UiComponent<HTMLDivElement> {
	private readonly sketchCanvas: HTMLCanvasElement
	private readonly sketchCtx: CanvasRenderingContext2D
	private readonly previewCanvas: HTMLCanvasElement
	private readonly previewRenderer: THREE.WebGLRenderer
	private readonly previewScene: THREE.Scene
	private readonly previewCamera: THREE.PerspectiveCamera
	private readonly previewRootGroup: THREE.Group
	private readonly previewContentGroup: THREE.Group
	private readonly previewPlaceholderMesh: THREE.Mesh
	private readonly previewPlaceholderText: HTMLParagraphElement
	private previewMesh: THREE.Mesh | null = null
	private previewEdges: THREE.LineSegments | null = null
	private readonly heightInput: HTMLInputElement
	private readonly statusText: HTMLParagraphElement
	private readonly extrudeSummary: HTMLParagraphElement
	private readonly finishButton: HTMLButtonElement
	private readonly undoButton: HTMLButtonElement
	private readonly resetButton: HTMLButtonElement
	private readonly extrudeButton: HTMLButtonElement
	private readonly previewContainer: HTMLDivElement
	private sketchPoints: Point2D[] = []
	private isSketchClosed = false
	private extrudedModel: ExtrudedModel | null = null
	private readonly previewRotation: PartProjectPreviewRotation = {
		yaw: PART_PROJECT_DEFAULT_ROTATION.yaw,
		pitch: PART_PROJECT_DEFAULT_ROTATION.pitch
	}
	private isRotatingPreview = false
	private lastRotationPointer: { x: number; y: number } | null = null
	private resizeObserver: ResizeObserver | null = null
	private readonly onStateChange?: () => void

	public constructor(options?: PartEditorOptions) {
		super(document.createElement("div"))
		this.onStateChange = options?.onStateChange
		this.root.style.width = "100%"
		this.root.style.height = "100%"
		this.root.style.minWidth = "0"
		this.root.style.minHeight = "0"
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.backgroundColor = "#f8fafc"

		const header = document.createElement("div")
		header.style.padding = "12px 16px"
		header.style.borderBottom = "1px solid #d0d7de"
		header.style.backgroundColor = "#fff"
		header.innerHTML = '<h2 style="margin:0;font-size:18px;">Part Studio</h2>'
		this.root.appendChild(header)

		const body = document.createElement("div")
		body.style.display = "flex"
		body.style.flexWrap = "wrap"
		body.style.flex = "1"
		body.style.minWidth = "0"
		body.style.minHeight = "0"
		body.style.alignItems = "flex-start"
		body.style.gap = "16px"
		body.style.padding = "16px"
		body.style.boxSizing = "border-box"
		body.style.overflow = "auto"
		this.root.appendChild(body)

		const sketchPanel = document.createElement("div")
		sketchPanel.style.flex = "1 1 360px"
		sketchPanel.style.minWidth = "0"
		sketchPanel.style.maxWidth = "420px"
		sketchPanel.style.display = "flex"
		sketchPanel.style.flexDirection = "column"
		sketchPanel.style.gap = "12px"
		body.appendChild(sketchPanel)

		const sketchHeader = document.createElement("div")
		sketchHeader.innerHTML =
			'<h3 style="margin:0;font-size:16px;">Sketch</h3><p style="margin:4px 0 0;color:#475569;font-size:13px;">Click inside the sketch area to create points. Add at least three points, then finish the sketch to extrude it.</p>'
		sketchPanel.appendChild(sketchHeader)

		this.sketchCanvas = document.createElement("canvas")
		this.sketchCanvas.style.border = "1px solid #cbd5f5"
		this.sketchCanvas.style.borderRadius = "8px"
		this.sketchCanvas.style.background =
			"linear-gradient(45deg, #f8fafc 25%, transparent 25%), linear-gradient(-45deg, #f8fafc 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #f8fafc 75%), linear-gradient(-45deg, transparent 75%, #f8fafc 75%)"
		this.sketchCanvas.style.backgroundSize = "24px 24px"
		this.sketchCanvas.style.backgroundPosition = "0 0, 0 12px, 12px -12px, -12px 0"
		this.sketchCanvas.style.cursor = "crosshair"
		this.sketchCanvas.width = SKETCH_CANVAS_SIZE * window.devicePixelRatio
		this.sketchCanvas.height = SKETCH_CANVAS_SIZE * window.devicePixelRatio
		this.sketchCanvas.style.width = "100%"
		this.sketchCanvas.style.maxWidth = `${SKETCH_CANVAS_SIZE}px`
		this.sketchCanvas.style.aspectRatio = "1 / 1"
		this.sketchCanvas.style.height = "auto"
		sketchPanel.appendChild(this.sketchCanvas)

		const sketchCtx = this.sketchCanvas.getContext("2d")
		if (!sketchCtx) {
			throw new Error("Failed to initialize sketch canvas context")
		}
		this.sketchCtx = sketchCtx
		const sketchScale = window.devicePixelRatio
		this.sketchCtx.scale(sketchScale, sketchScale)

		const controlsRow = document.createElement("div")
		controlsRow.style.display = "grid"
		controlsRow.style.gridTemplateColumns = "repeat(2, minmax(0, 1fr))"
		controlsRow.style.gap = "8px"
		sketchPanel.appendChild(controlsRow)

		this.undoButton = this.createButton("Undo", this.handleUndo)
		controlsRow.appendChild(this.undoButton)

		this.resetButton = this.createButton("Reset", this.handleReset)
		controlsRow.appendChild(this.resetButton)

		this.finishButton = this.createButton("Finish Sketch", this.handleFinishSketch)
		controlsRow.appendChild(this.finishButton)

		const extrudeControls = document.createElement("div")
		extrudeControls.style.display = "flex"
		extrudeControls.style.gap = "8px"
		extrudeControls.style.alignItems = "center"
		extrudeControls.style.marginTop = "4px"
		sketchPanel.appendChild(extrudeControls)

		const heightLabel = document.createElement("label")
		heightLabel.textContent = "Extrude height"
		heightLabel.style.fontSize = "13px"
		heightLabel.style.color = "#0f172a"
		extrudeControls.appendChild(heightLabel)

		this.heightInput = document.createElement("input")
		this.heightInput.type = "number"
		this.heightInput.min = "1"
		this.heightInput.value = String(PART_PROJECT_DEFAULT_HEIGHT)
		this.heightInput.step = "1"
		this.heightInput.style.width = "80px"
		this.heightInput.style.padding = "4px 6px"
		this.heightInput.style.border = "1px solid #cbd5f5"
		this.heightInput.style.borderRadius = "4px"
		extrudeControls.appendChild(this.heightInput)
		this.heightInput.addEventListener("input", this.handleHeightInputChange)

		this.extrudeButton = this.createButton("Extrude", this.handleExtrude)
		this.extrudeButton.style.gridColumn = "span 2"
		sketchPanel.appendChild(this.extrudeButton)

		this.statusText = document.createElement("p")
		this.statusText.style.margin = "4px 0 0"
		this.statusText.style.fontSize = "13px"
		this.statusText.style.color = "#475569"
		sketchPanel.appendChild(this.statusText)

		this.extrudeSummary = document.createElement("p")
		this.extrudeSummary.style.margin = "0"
		this.extrudeSummary.style.fontSize = "13px"
		this.extrudeSummary.style.color = "#0f172a"
		sketchPanel.appendChild(this.extrudeSummary)

		this.previewContainer = document.createElement("div")
		this.previewContainer.style.flex = "1 1 360px"
		this.previewContainer.style.minWidth = "0"
		this.previewContainer.style.maxWidth = `${SKETCH_CANVAS_SIZE}px`
		this.previewContainer.style.aspectRatio = "1 / 1"
		this.previewContainer.style.height = "auto"
		this.previewContainer.style.minHeight = "260px"
		this.previewContainer.style.backgroundColor = "#1f2937"
		this.previewContainer.style.borderRadius = "12px"
		this.previewContainer.style.position = "relative"
		this.previewContainer.style.display = "flex"
		this.previewContainer.style.alignItems = "center"
		this.previewContainer.style.justifyContent = "center"
		this.previewContainer.style.boxShadow = "inset 0 0 0 1px rgba(255,255,255,0.05)"
		body.appendChild(this.previewContainer)

		this.previewCanvas = document.createElement("canvas")
		this.previewCanvas.style.width = "100%"
		this.previewCanvas.style.height = "100%"
		this.previewCanvas.style.cursor = "grab"
		this.previewContainer.appendChild(this.previewCanvas)

		this.previewPlaceholderText = document.createElement("p")
		this.previewPlaceholderText.textContent = "Sketch a profile and extrude to see it here"
		this.previewPlaceholderText.style.position = "absolute"
		this.previewPlaceholderText.style.left = "50%"
		this.previewPlaceholderText.style.bottom = "12%"
		this.previewPlaceholderText.style.transform = "translateX(-50%)"
		this.previewPlaceholderText.style.margin = "0"
		this.previewPlaceholderText.style.padding = "0 12px"
		this.previewPlaceholderText.style.pointerEvents = "none"
		this.previewPlaceholderText.style.color = "rgba(255,255,255,0.75)"
		this.previewPlaceholderText.style.font = "14px Inter, system-ui, sans-serif"
		this.previewContainer.appendChild(this.previewPlaceholderText)

		this.previewRenderer = new THREE.WebGLRenderer({
			canvas: this.previewCanvas,
			antialias: true,
			alpha: false
		})
		this.previewRenderer.setPixelRatio(Math.min(2, Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1))
		this.previewRenderer.setClearColor(0x1f2937, 1)

		this.previewScene = new THREE.Scene()
		this.previewCamera = new THREE.PerspectiveCamera(45, 1, 0.01, 50)
		this.previewCamera.position.set(0, 0.15, 3)

		this.previewRootGroup = new THREE.Group()
		this.previewContentGroup = new THREE.Group()
		this.previewRootGroup.add(this.previewContentGroup)
		this.previewScene.add(this.previewRootGroup)

		const ambientLight = new THREE.HemisphereLight(0xf8fafc, 0x0f172a, 0.8)
		this.previewScene.add(ambientLight)
		const keyLight = new THREE.DirectionalLight(0xffffff, 1.2)
		keyLight.position.set(1.2, 2.5, 1.6)
		this.previewScene.add(keyLight)
		const fillLight = new THREE.DirectionalLight(0x93c5fd, 0.45)
		fillLight.position.set(-1.8, -0.4, 1.1)
		this.previewScene.add(fillLight)

		const placeholderGeometry = new THREE.BoxGeometry(1.2, 0.8, 1.2)
		const placeholderMaterial = new THREE.MeshStandardMaterial({
			color: 0xffffff,
			transparent: true,
			opacity: 0.12,
			roughness: 0.65,
			metalness: 0
		})
		this.previewPlaceholderMesh = new THREE.Mesh(placeholderGeometry, placeholderMaterial)
		this.previewContentGroup.add(this.previewPlaceholderMesh)

		this.sketchCanvas.addEventListener("click", this.handleSketchCanvasClick)
		this.sketchCanvas.addEventListener("mousemove", this.handleSketchHover)
		this.sketchCanvas.addEventListener("mouseleave", this.handleSketchHover)
		this.sketchCanvas.addEventListener("dblclick", (event) => {
			event.preventDefault()
			this.handleFinishSketch()
		})

		this.previewCanvas.addEventListener("pointerdown", this.handlePreviewPointerDown)
		this.previewCanvas.addEventListener("pointermove", this.handlePreviewPointerMove)
		this.previewCanvas.addEventListener("pointerup", this.handlePreviewPointerUp)
		this.previewCanvas.addEventListener("pointerleave", this.handlePreviewPointerUp)
		this.previewCanvas.addEventListener("pointercancel", this.handlePreviewPointerUp)
		this.previewCanvas.addEventListener("contextmenu", (event) => {
			event.preventDefault()
		})

		this.restoreState(options?.initialState)

		if (typeof ResizeObserver === "function") {
			this.resizeObserver = new ResizeObserver(() => {
				// Defer to the next frame to avoid ResizeObserver loop limit errors
				requestAnimationFrame(() => this.updatePreviewSize())
			})
			this.resizeObserver.observe(this.previewContainer)
		} else {
			// Fallback for environments missing ResizeObserver
			window.addEventListener("resize", () => {
				requestAnimationFrame(() => this.updatePreviewSize())
			})
		}
		// Trigger size calculation once the element is attached.
		requestAnimationFrame(() => this.updatePreviewSize())
	}

	public getState(): PartEditorState {
		const heightValue = this.getHeightFromInput()
		return {
			sketchPoints: this.sketchPoints.map((point) => ({ x: point.x, y: point.y })),
			isSketchClosed: this.isSketchClosed,
			extrudedModel:
				this.extrudedModel === null
					? undefined
					: {
							base: this.extrudedModel.base.map((point) => ({ x: point.x, y: point.y })),
							height: this.extrudedModel.height,
							scale: this.extrudedModel.scale,
							rawHeight: this.extrudedModel.rawHeight
						},
			height: heightValue,
			previewRotation: {
				yaw: this.previewRotation.yaw,
				pitch: this.previewRotation.pitch
			}
		}
	}

	private restoreState(state?: PartEditorState) {
		const height = state && Number.isFinite(state.height) ? state.height : PART_PROJECT_DEFAULT_HEIGHT
		const rotation = state?.previewRotation ?? PART_PROJECT_DEFAULT_ROTATION
		this.sketchPoints = state?.sketchPoints?.map((point) => ({ x: point.x, y: point.y })) ?? []
		this.isSketchClosed = state?.isSketchClosed ?? false
		this.extrudedModel = state?.extrudedModel
			? {
					base: state.extrudedModel.base.map((point) => ({ x: point.x, y: point.y })),
					height: state.extrudedModel.height,
					scale: state.extrudedModel.scale,
					rawHeight: state.extrudedModel.rawHeight
				}
			: null
		this.heightInput.value = String(height)
		this.previewRotation.yaw = rotation.yaw
		this.previewRotation.pitch = rotation.pitch
		this.drawSketch()
		this.syncPreviewGeometry()
		this.drawPreview()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private getHeightFromInput(): number {
		const parsed = Number.parseFloat(this.heightInput.value)
		return Number.isFinite(parsed) ? parsed : PART_PROJECT_DEFAULT_HEIGHT
	}

	private emitStateChange() {
		this.onStateChange?.()
	}

	private createButton(label: string, onClick: () => void): HTMLButtonElement {
		const button = document.createElement("button")
		button.textContent = label
		button.type = "button"
		button.style.padding = "6px 10px"
		button.style.borderRadius = "6px"
		button.style.border = "1px solid #94a3b8"
		button.style.backgroundColor = "#fff"
		button.style.color = "#0f172a"
		button.style.cursor = "pointer"
		button.style.fontSize = "13px"
		button.onmouseenter = () => {
			if (!button.disabled) {
				button.style.backgroundColor = "#f1f5f9"
			}
		}
		button.onmouseleave = () => {
			button.style.backgroundColor = button.disabled ? "#e2e8f0" : "#fff"
		}
		button.onclick = (event) => {
			event.preventDefault()
			onClick()
		}
		return button
	}

	private handleHeightInputChange = () => {
		this.emitStateChange()
	}

	private handleSketchCanvasClick = (event: MouseEvent) => {
		if (this.isSketchClosed) {
			return
		}
		const rect = this.sketchCanvas.getBoundingClientRect()
		const x = event.clientX - rect.left
		const y = event.clientY - rect.top
		this.sketchPoints = [...this.sketchPoints, { x, y }]
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleSketchHover = (event: MouseEvent) => {
		const rect = this.sketchCanvas.getBoundingClientRect()
		const x = event.clientX - rect.left
		const y = event.clientY - rect.top
		this.drawSketch({ x, y, active: event.type === "mousemove" })
	}

	private handleUndo = () => {
		if (!this.sketchPoints.length || this.isSketchClosed) {
			return
		}
		this.sketchPoints = this.sketchPoints.slice(0, -1)
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleReset = () => {
		this.sketchPoints = []
		this.isSketchClosed = false
		this.extrudedModel = null
		this.drawSketch()
		this.syncPreviewGeometry()
		this.drawPreview()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleFinishSketch = () => {
		if (this.sketchPoints.length < 3) {
			return
		}
		this.isSketchClosed = true
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleExtrude = () => {
		if (!this.isSketchClosed || this.sketchPoints.length < 3) {
			return
		}
		const height = Number.parseFloat(this.heightInput.value)
		if (!Number.isFinite(height) || height <= 0) {
			this.heightInput.focus()
			return
		}
		const normalized = this.normalizeSketch(height)
		if (!normalized) {
			return
		}
		this.extrudedModel = normalized
		this.syncPreviewGeometry()
		this.drawPreview()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handlePreviewPointerDown = (event: PointerEvent) => {
		const isRightMouseClick = event.pointerType === "mouse" ? event.button === 2 : event.isPrimary && event.button === 0
		if (!isRightMouseClick) {
			return
		}
		event.preventDefault()
		this.isRotatingPreview = true
		this.lastRotationPointer = { x: event.clientX, y: event.clientY }
		this.previewCanvas.setPointerCapture(event.pointerId)
		this.previewCanvas.style.cursor = "grabbing"
	}

	private handlePreviewPointerMove = (event: PointerEvent) => {
		if (!this.isRotatingPreview || !this.lastRotationPointer) {
			return
		}
		const dx = event.clientX - this.lastRotationPointer.x
		const dy = event.clientY - this.lastRotationPointer.y
		this.lastRotationPointer = { x: event.clientX, y: event.clientY }
		this.previewRotation.yaw -= dx * 0.01
		this.previewRotation.pitch += dy * 0.01
		const limit = Math.PI / 2 - 0.1
		this.previewRotation.pitch = Math.min(limit, Math.max(-limit, this.previewRotation.pitch))
		this.drawPreview()
	}

	private handlePreviewPointerUp = (event: PointerEvent) => {
		if (!this.isRotatingPreview) {
			return
		}
		if (this.previewCanvas.hasPointerCapture(event.pointerId)) {
			this.previewCanvas.releasePointerCapture(event.pointerId)
		}
		this.isRotatingPreview = false
		this.lastRotationPointer = null
		this.previewCanvas.style.cursor = "grab"
		this.emitStateChange()
	}

	private updateStatus() {
		const pointCount = this.sketchPoints.length
		const pointText = pointCount === 1 ? "point" : "points"
		const sketchState = this.isSketchClosed ? "Sketch closed" : "Sketch open"
		this.statusText.textContent = `${sketchState}. ${pointCount} ${pointText}.`
		if (this.extrudedModel) {
			this.extrudeSummary.textContent = `Extruded height: ${this.extrudedModel.rawHeight.toFixed(1)} units`
		} else {
			this.extrudeSummary.textContent = ""
		}
	}

	private updateControls() {
		this.finishButton.disabled = this.isSketchClosed || this.sketchPoints.length < 3
		this.undoButton.disabled = this.isSketchClosed || this.sketchPoints.length === 0
		this.resetButton.disabled = this.sketchPoints.length === 0 && !this.extrudedModel
		this.extrudeButton.disabled = !this.isSketchClosed
		this.heightInput.disabled = !this.isSketchClosed
		this.finishButton.style.backgroundColor = this.finishButton.disabled ? "#e2e8f0" : "#fff"
		this.undoButton.style.backgroundColor = this.undoButton.disabled ? "#e2e8f0" : "#fff"
		this.resetButton.style.backgroundColor = this.resetButton.disabled ? "#e2e8f0" : "#fff"
		this.extrudeButton.style.backgroundColor = this.extrudeButton.disabled ? "#cbd5f5" : "#3b82f6"
		this.extrudeButton.style.color = this.extrudeButton.disabled ? "#64748b" : "#ffffff"
		this.extrudeButton.style.border = this.extrudeButton.disabled ? "1px solid #cbd5f5" : "1px solid #1d4ed8"
	}

	private drawSketch(hover?: { x: number; y: number; active: boolean }) {
		this.sketchCtx.clearRect(0, 0, SKETCH_CANVAS_SIZE, SKETCH_CANVAS_SIZE)
		this.sketchCtx.fillStyle = "rgba(59,130,246,0.05)"
		this.sketchCtx.fillRect(0, 0, SKETCH_CANVAS_SIZE, SKETCH_CANVAS_SIZE)

		// Axes
		this.sketchCtx.strokeStyle = "rgba(15,23,42,0.2)"
		this.sketchCtx.lineWidth = 1
		this.sketchCtx.beginPath()
		this.sketchCtx.moveTo(SKETCH_CANVAS_SIZE / 2, 0)
		this.sketchCtx.lineTo(SKETCH_CANVAS_SIZE / 2, SKETCH_CANVAS_SIZE)
		this.sketchCtx.moveTo(0, SKETCH_CANVAS_SIZE / 2)
		this.sketchCtx.lineTo(SKETCH_CANVAS_SIZE, SKETCH_CANVAS_SIZE / 2)
		this.sketchCtx.stroke()

		if (this.sketchPoints.length > 0) {
			const first = this.sketchPoints[0]
			if (!first) {
				return
			}
			this.sketchCtx.lineWidth = 2
			this.sketchCtx.strokeStyle = "#2563eb"
			this.sketchCtx.beginPath()
			this.sketchCtx.moveTo(first.x, first.y)
			for (const point of this.sketchPoints.slice(1)) {
				this.sketchCtx.lineTo(point.x, point.y)
			}
			if (this.isSketchClosed) {
				this.sketchCtx.closePath()
			}
			this.sketchCtx.stroke()

			if (this.isSketchClosed && this.sketchPoints.length >= 3) {
				this.sketchCtx.fillStyle = "rgba(59,130,246,0.2)"
				this.sketchCtx.fill()
			}

			for (const point of this.sketchPoints) {
				this.drawSketchPoint(point, "#1d4ed8")
			}
		}

		if (hover?.active && !this.isSketchClosed && this.sketchPoints.length > 0) {
			const last = this.sketchPoints[this.sketchPoints.length - 1]
			if (last) {
				this.sketchCtx.setLineDash([4, 4])
				this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
				this.sketchCtx.beginPath()
				this.sketchCtx.moveTo(last.x, last.y)
				this.sketchCtx.lineTo(hover.x, hover.y)
				this.sketchCtx.stroke()
				this.sketchCtx.setLineDash([])
			}
			this.drawSketchPoint({ x: hover.x, y: hover.y }, "#0f172a", true)
		}
	}

	private drawSketchPoint(point: Point2D, color: string, isPreview = false) {
		this.sketchCtx.fillStyle = color
		this.sketchCtx.beginPath()
		this.sketchCtx.arc(point.x, point.y, isPreview ? 4 : 5, 0, Math.PI * 2)
		this.sketchCtx.fill()
		if (!isPreview) {
			this.sketchCtx.strokeStyle = "#ffffff"
			this.sketchCtx.lineWidth = 1.5
			this.sketchCtx.beginPath()
			this.sketchCtx.arc(point.x, point.y, 5, 0, Math.PI * 2)
			this.sketchCtx.stroke()
		}
	}

	private normalizeSketch(height: number): ExtrudedModel | null {
		if (this.sketchPoints.length < 3) {
			return null
		}
		const xs = this.sketchPoints.map((p) => p.x)
		const ys = this.sketchPoints.map((p) => p.y)
		const minX = Math.min(...xs)
		const maxX = Math.max(...xs)
		const minY = Math.min(...ys)
		const maxY = Math.max(...ys)
		const width = maxX - minX
		const depth = maxY - minY
		const scale = Math.max(width, depth, height)
		if (scale === 0) {
			return null
		}
		const centerX = (minX + maxX) / 2
		const centerY = (minY + maxY) / 2
		const base = this.sketchPoints.map((point) => ({
			x: (point.x - centerX) / scale,
			y: (centerY - point.y) / scale
		}))
		return {
			base,
			height: height / scale,
			scale,
			rawHeight: height
		}
	}

	private updatePreviewSize() {
		const rect = this.previewCanvas.getBoundingClientRect()
		const width = Math.max(0, Math.floor(rect.width))
		const height = Math.max(0, Math.floor(rect.height))
		if (width === 0 || height === 0) {
			return
		}
		this.previewRenderer.setPixelRatio(Math.min(2, Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1))
		this.previewRenderer.setSize(width, height, false)
		this.previewCamera.aspect = width / height
		this.previewCamera.updateProjectionMatrix()
		this.drawPreview()
	}

	private drawPreview() {
		this.previewRootGroup.rotation.set(this.previewRotation.pitch, this.previewRotation.yaw, 0)
		this.previewRenderer.render(this.previewScene, this.previewCamera)
	}

	private syncPreviewGeometry() {
		if (this.previewMesh) {
			this.previewContentGroup.remove(this.previewMesh)
			this.previewMesh.geometry.dispose()
			const meshMaterial = this.previewMesh.material
			if (Array.isArray(meshMaterial)) {
				for (const material of meshMaterial) {
					material.dispose()
				}
			} else {
				meshMaterial.dispose()
			}
			this.previewMesh = null
		}

		if (this.previewEdges) {
			this.previewContentGroup.remove(this.previewEdges)
			this.previewEdges.geometry.dispose()
			const edgeMaterial = this.previewEdges.material
			if (Array.isArray(edgeMaterial)) {
				for (const material of edgeMaterial) {
					material.dispose()
				}
			} else {
				edgeMaterial.dispose()
			}
			this.previewEdges = null
		}

		if (!this.extrudedModel || this.extrudedModel.base.length < 3) {
			this.previewPlaceholderMesh.visible = true
			this.previewPlaceholderText.style.display = "block"
			return
		}

		const { base, height } = this.extrudedModel
		const shapePoints = base.map((point) => new THREE.Vector2(point.x, point.y))
		const shape = new THREE.Shape(shapePoints)
		const geometry = new THREE.ExtrudeGeometry(shape, {
			depth: height,
			bevelEnabled: false,
			steps: 1
		})
		geometry.translate(0, 0, -height / 2)
		geometry.computeVertexNormals()

		const material = new THREE.MeshStandardMaterial({
			color: 0x3b82f6,
			roughness: 0.35,
			metalness: 0.05
		})
		this.previewMesh = new THREE.Mesh(geometry, material)
		this.previewContentGroup.add(this.previewMesh)

		const edgeGeometry = new THREE.EdgesGeometry(geometry)
		const edgeMaterial = new THREE.LineBasicMaterial({
			color: 0xe2e8f0,
			transparent: true,
			opacity: 0.8
		})
		this.previewEdges = new THREE.LineSegments(edgeGeometry, edgeMaterial)
		this.previewContentGroup.add(this.previewEdges)

		this.previewPlaceholderMesh.visible = false
		this.previewPlaceholderText.style.display = "none"
	}
}
