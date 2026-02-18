import { PART_PROJECT_DEFAULT_HEIGHT, PART_PROJECT_DEFAULT_ROTATION } from "./project-file"
import type { PartProjectExtrudedModel, PartProjectItemData, PartProjectPreviewRotation } from "./project-file"
import { UiComponent } from "./ui"
import * as THREE from "three"

type Point2D = { x: number; y: number }

type ExtrudedModel = PartProjectExtrudedModel
type PartStudioTool = "view" | "sketch"
type SketchTool = "line" | "rectangle"
type ReferencePlaneVisual = {
	name: "Front" | "Top" | "Right"
	mesh: THREE.Mesh
	edge: THREE.LineSegments
	fillMaterial: THREE.MeshBasicMaterial
	edgeMaterial: THREE.LineBasicMaterial
}
type ReferencePlaneHandle = {
	mesh: THREE.Mesh<THREE.RingGeometry, THREE.MeshBasicMaterial>
	xSign: -1 | 1
	ySign: -1 | 1
}

export type PartEditorState = PartProjectItemData

type PartEditorOptions = {
	initialState?: PartEditorState
	onStateChange?: () => void
}

const SKETCH_CANVAS_SIZE = 360
const REFERENCE_PLANE_SIZE = 1.9
const PREVIEW_MIN_CAMERA_DISTANCE = 0.5
const PREVIEW_MAX_CAMERA_DISTANCE = 12
const PREVIEW_ZOOM_SENSITIVITY = 0.0015

export class PartEditor extends UiComponent<HTMLDivElement> {
	private readonly sketchCanvas: HTMLCanvasElement
	private readonly sketchCtx: CanvasRenderingContext2D
	private readonly previewCanvas: HTMLCanvasElement
	private readonly previewRenderer: THREE.WebGLRenderer
	private readonly previewScene: THREE.Scene
	private readonly previewCamera: THREE.PerspectiveCamera
	private readonly previewRootGroup: THREE.Group
	private readonly previewContentGroup: THREE.Group
	private readonly previewReferenceGroup: THREE.Group
	private readonly previewReferencePlanes: ReferencePlaneVisual[] = []
	private readonly previewReferenceHandleGeometry = new THREE.RingGeometry(0.028, 0.04, 24)
	private readonly previewReferenceHandleMaterial = new THREE.MeshBasicMaterial({
		color: 0x94a3b8,
		transparent: true,
		opacity: 0.95,
		side: THREE.DoubleSide,
		depthWrite: false,
		depthTest: false
	})
	private readonly previewReferenceHandles: ReferencePlaneHandle[] = []
	private readonly sketchOverlayGroup = new THREE.Group()
	private previewMesh: THREE.Mesh | null = null
	private previewEdges: THREE.LineSegments | null = null
	private sketchOverlayCommittedLine: THREE.Line | THREE.LineLoop | THREE.LineSegments | null = null
	private sketchOverlayPreviewLine: THREE.Line | THREE.LineLoop | null = null
	private sketchOverlayPoints: THREE.Points | null = null
	private sketchOverlayLabel: THREE.Sprite | null = null
	private sketchOverlayParentPlane: THREE.Mesh | null = null
	private readonly heightInput: HTMLInputElement
	private readonly statusText: HTMLParagraphElement
	private readonly extrudeSummary: HTMLParagraphElement
	private readonly finishButton: HTMLButtonElement
	private readonly undoButton: HTMLButtonElement
	private readonly resetButton: HTMLButtonElement
	private readonly extrudeButton: HTMLButtonElement
	private readonly previewContainer: HTMLDivElement
	private readonly sketchPanel: HTMLDivElement
	private sketchPoints: Point2D[] = []
	private isSketchClosed = false
	private extrudedModel: ExtrudedModel | null = null
	private readonly previewRotation: PartProjectPreviewRotation = {
		yaw: PART_PROJECT_DEFAULT_ROTATION.yaw,
		pitch: PART_PROJECT_DEFAULT_ROTATION.pitch
	}
	private readonly previewPan = { x: 0, y: 0 }
	private isRotatingPreview = false
	private isPanningPreview = false
	private reverseRotatePreview = false
	private lastRotationPointer: { x: number; y: number } | null = null
	private resizeObserver: ResizeObserver | null = null
	private readonly previewRaycaster = new THREE.Raycaster()
	private readonly previewPointer = new THREE.Vector2()
	private hoveredReferencePlane: THREE.Mesh | null = null
	private selectedReferencePlane: THREE.Mesh | null = null
	private pointerOverSelectedPlane = false
	private activeResizeHandle: ReferencePlaneHandle | null = null
	private resizingReferencePlane: THREE.Mesh | null = null
	private readonly onStateChange?: () => void
	private activeTool: PartStudioTool = "view"
	private activeSketchTool: SketchTool | null = "line"
	private sketchName = "Sketch 1"
	private paneToolbar: UiComponent<HTMLElement> | null = null
	private sketchToolButton: HTMLButtonElement | null = null
	private sketchToolsBar: HTMLDivElement | null = null
	private lineSketchToolButton: HTMLButtonElement | null = null
	private rectangleSketchToolButton: HTMLButtonElement | null = null
	private pendingRectangleStart: Point2D | null = null
	private pendingLineStart: Point2D | null = null
	private lineToolNeedsFreshStart = true
	private sketchHoverPoint: Point2D | null = null
	private draggingSketchPointIndex: number | null = null
	private sketchBreakIndices = new Set<number>()

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
		body.style.alignItems = "stretch"
		body.style.gap = "16px"
		body.style.padding = "16px"
		body.style.boxSizing = "border-box"
		body.style.overflow = "hidden"
		this.root.appendChild(body)

		this.sketchPanel = document.createElement("div")
		this.sketchPanel.style.flex = "1 1 auto"
		this.sketchPanel.style.minWidth = "0"
		this.sketchPanel.style.minHeight = "0"
		this.sketchPanel.style.display = "none"
		this.sketchPanel.style.flexDirection = "column"
		this.sketchPanel.style.gap = "12px"
		body.appendChild(this.sketchPanel)

		const sketchHeader = document.createElement("div")
		sketchHeader.innerHTML =
			'<h3 style="margin:0;font-size:16px;">Sketch</h3><p style="margin:4px 0 0;color:#475569;font-size:13px;">Click inside the sketch area to create points. Add at least three points, then finish the sketch to extrude it.</p>'
		this.sketchPanel.appendChild(sketchHeader)

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
		this.sketchCanvas.style.maxWidth = "100%"
		this.sketchCanvas.style.aspectRatio = "1 / 1"
		this.sketchCanvas.style.height = "auto"
		this.sketchPanel.appendChild(this.sketchCanvas)

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
		this.sketchPanel.appendChild(controlsRow)

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
		this.sketchPanel.appendChild(extrudeControls)

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
		this.sketchPanel.appendChild(this.extrudeButton)

		this.statusText = document.createElement("p")
		this.statusText.style.margin = "4px 0 0"
		this.statusText.style.fontSize = "13px"
		this.statusText.style.color = "#475569"
		this.sketchPanel.appendChild(this.statusText)

		this.extrudeSummary = document.createElement("p")
		this.extrudeSummary.style.margin = "0"
		this.extrudeSummary.style.fontSize = "13px"
		this.extrudeSummary.style.color = "#0f172a"
		this.sketchPanel.appendChild(this.extrudeSummary)

		this.previewContainer = document.createElement("div")
		this.previewContainer.style.flex = "1 1 auto"
		this.previewContainer.style.width = "100%"
		this.previewContainer.style.minWidth = "0"
		this.previewContainer.style.maxWidth = "none"
		this.previewContainer.style.aspectRatio = "auto"
		this.previewContainer.style.height = "100%"
		this.previewContainer.style.minHeight = "0"
		this.previewContainer.style.backgroundColor = "#f1f5f9"
		this.previewContainer.style.borderRadius = "12px"
		this.previewContainer.style.position = "relative"
		this.previewContainer.style.display = "flex"
		this.previewContainer.style.alignItems = "center"
		this.previewContainer.style.justifyContent = "center"
		this.previewContainer.style.boxShadow = "inset 0 0 0 1px rgba(148,163,184,0.35)"
		body.appendChild(this.previewContainer)

		this.previewCanvas = document.createElement("canvas")
		this.previewCanvas.style.width = "100%"
		this.previewCanvas.style.height = "100%"
		this.previewCanvas.style.cursor = "grab"
		this.previewCanvas.tabIndex = 0
		this.previewContainer.appendChild(this.previewCanvas)

		this.previewRenderer = new THREE.WebGLRenderer({
			canvas: this.previewCanvas,
			antialias: true,
			alpha: false
		})
		this.previewRenderer.setPixelRatio(Math.min(2, Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1))
		this.previewRenderer.setClearColor(0xf1f5f9, 1)

		this.previewScene = new THREE.Scene()
		this.previewCamera = new THREE.PerspectiveCamera(45, 1, 0.01, 50)
		this.previewCamera.position.set(0, 0.18, 3.2)

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

		this.previewReferenceGroup = this.createReferencePlanes()
		this.previewContentGroup.add(this.previewReferenceGroup)
		this.previewContentGroup.add(this.sketchOverlayGroup)
		this.sketchOverlayGroup.visible = false

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
		this.previewCanvas.addEventListener("dblclick", this.handlePreviewDoubleClick)
		this.previewCanvas.addEventListener("keydown", this.handlePreviewKeyDown)
		this.previewCanvas.addEventListener("wheel", this.handlePreviewWheel, { passive: false })
		this.previewCanvas.addEventListener("contextmenu", (event) => {
			event.preventDefault()
		})
		document.addEventListener("keydown", this.handleDocumentKeyDown, true)

		this.restoreState(options?.initialState)
		this.updateStudioModeLayout()

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
			sketchName: this.sketchPoints.length > 0 ? this.sketchName : undefined,
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

	public createPaneToolbar(): UiComponent<HTMLElement> {
		if (this.paneToolbar) {
			return this.paneToolbar
		}

		const toolbar = document.createElement("div")
		toolbar.style.display = "flex"
		toolbar.style.alignItems = "center"
		toolbar.style.gap = "6px"

		const sketchButton = document.createElement("button")
		sketchButton.type = "button"
		sketchButton.textContent = "Sketch"
		sketchButton.style.padding = "4px 8px"
		sketchButton.style.borderRadius = "6px"
		sketchButton.style.border = "1px solid #93c5fd"
		sketchButton.style.fontSize = "12px"
		sketchButton.style.fontWeight = "600"
		sketchButton.style.cursor = "pointer"
		sketchButton.draggable = false
		sketchButton.addEventListener("click", (event) => {
			event.preventDefault()
			event.stopPropagation()
			this.setActiveTool(this.activeTool === "sketch" ? "view" : "sketch")
		})
		toolbar.appendChild(sketchButton)
		const sketchToolsBar = document.createElement("div")
		sketchToolsBar.style.display = "none"
		sketchToolsBar.style.alignItems = "center"
		sketchToolsBar.style.gap = "4px"
		sketchToolsBar.style.paddingLeft = "4px"
		sketchToolsBar.style.borderLeft = "1px solid #cbd5e1"

		const createSketchToolButton = (label: string, title: string, onClick: () => void): HTMLButtonElement => {
			const button = document.createElement("button")
			button.type = "button"
			button.textContent = label
			button.title = title
			button.style.width = "26px"
			button.style.height = "24px"
			button.style.borderRadius = "4px"
			button.style.border = "1px solid #cbd5e1"
			button.style.backgroundColor = "#ffffff"
			button.style.color = "#334155"
			button.style.fontSize = "14px"
			button.style.fontWeight = "700"
			button.style.cursor = "pointer"
			button.draggable = false
			button.addEventListener("click", (event) => {
				event.preventDefault()
				event.stopPropagation()
				onClick()
			})
			return button
		}

		const lineButton = createSketchToolButton("/", "Line Tool", () => {
			this.setActiveSketchTool("line")
		})
		const rectangleButton = createSketchToolButton("▭", "Rectangle Tool", () => {
			this.setActiveSketchTool("rectangle")
		})
		sketchToolsBar.appendChild(lineButton)
		sketchToolsBar.appendChild(rectangleButton)
		toolbar.appendChild(sketchToolsBar)

		this.sketchToolButton = sketchButton
		this.sketchToolsBar = sketchToolsBar
		this.lineSketchToolButton = lineButton
		this.rectangleSketchToolButton = rectangleButton
		this.paneToolbar = new UiComponent(toolbar)
		this.updatePaneToolbarStyles()
		this.updateSketchToolButtons()
		return this.paneToolbar
	}

	public selectReferencePlane(planeName: "Top" | "Front" | "Right"): void {
		const plane = this.previewReferencePlanes.find((entry) => entry.name === planeName)?.mesh ?? null
		if (!plane) {
			return
		}
		this.setSelectedReferencePlane(plane)
	}

	public getSketchName(): string {
		return this.sketchName
	}

	public setSketchName(name: string): void {
		const trimmed = name.trim()
		if (!trimmed || trimmed === this.sketchName) {
			return
		}
		this.sketchName = trimmed
		this.updateSketchOverlay()
		this.emitStateChange()
	}

	public enterSketchMode(): void {
		if (this.isSketchClosed) {
			this.isSketchClosed = false
			this.updateControls()
			this.updateStatus()
		}
		this.setActiveTool("sketch")
	}

	public deleteSketch(): void {
		this.sketchPoints = []
		this.sketchBreakIndices.clear()
		this.isSketchClosed = false
		this.pendingRectangleStart = null
		this.pendingLineStart = null
		this.lineToolNeedsFreshStart = true
		this.sketchHoverPoint = null
		this.extrudedModel = null
		this.drawSketch()
		this.syncPreviewGeometry()
		this.updateSketchOverlay()
		this.drawPreview()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private setActiveTool(tool: PartStudioTool) {
		if (this.activeTool === tool) {
			return
		}
		this.activeTool = tool
		this.pointerOverSelectedPlane = false
		this.pendingRectangleStart = null
		this.pendingLineStart = null
		this.sketchHoverPoint = null
		if (tool === "sketch" && !this.selectedReferencePlane) {
			const defaultPlane = this.previewReferencePlanes[0]?.mesh ?? null
			if (defaultPlane) {
				this.setSelectedReferencePlane(defaultPlane)
			}
		}
		this.updatePaneToolbarStyles()
		this.updateStudioModeLayout()
		this.updateSketchOverlay()
	}

	private setActiveSketchTool(tool: SketchTool | null) {
		if (this.activeSketchTool === tool && !this.pendingRectangleStart) {
			if (tool === "line") {
				this.pendingLineStart = null
				this.lineToolNeedsFreshStart = true
				this.sketchHoverPoint = null
				this.updateSketchToolButtons()
				this.drawSketch()
				this.updateSketchOverlay()
				this.updatePreviewCursor()
			}
			return
		}
		this.activeSketchTool = tool
		this.pendingRectangleStart = null
		if (tool === "line") {
			this.pendingLineStart = null
			this.lineToolNeedsFreshStart = true
		} else {
			this.pendingLineStart = null
		}
		this.sketchHoverPoint = null
		this.updateSketchToolButtons()
		this.drawSketch()
		this.updateSketchOverlay()
		this.updatePreviewCursor()
	}

	private updatePaneToolbarStyles() {
		if (!this.sketchToolButton) {
			return
		}
		const active = this.activeTool === "sketch"
		this.sketchToolButton.style.backgroundColor = active ? "#2563eb" : "#ffffff"
		this.sketchToolButton.style.color = active ? "#ffffff" : "#1e293b"
		this.sketchToolButton.style.borderColor = active ? "#1d4ed8" : "#93c5fd"
		if (this.sketchToolsBar) {
			this.sketchToolsBar.style.display = active ? "flex" : "none"
		}
	}

	private updateSketchToolButtons() {
		if (this.lineSketchToolButton) {
			const active = this.activeSketchTool === "line"
			this.lineSketchToolButton.style.backgroundColor = active ? "#e2e8f0" : "#ffffff"
			this.lineSketchToolButton.style.borderColor = active ? "#94a3b8" : "#cbd5e1"
		}
		if (this.rectangleSketchToolButton) {
			const active = this.activeSketchTool === "rectangle"
			this.rectangleSketchToolButton.style.backgroundColor = active ? "#e2e8f0" : "#ffffff"
			this.rectangleSketchToolButton.style.borderColor = active ? "#94a3b8" : "#cbd5e1"
		}
	}

	private updateStudioModeLayout() {
		const sketchMode = this.activeTool === "sketch"
		this.sketchPanel.style.display = "none"
		this.previewContainer.style.display = "flex"
		if (sketchMode) {
			this.drawSketch()
		}
		this.drawPreview()
	}

	private restoreState(state?: PartEditorState) {
		const height = state && Number.isFinite(state.height) ? state.height : PART_PROJECT_DEFAULT_HEIGHT
		const rotation = state?.previewRotation ?? PART_PROJECT_DEFAULT_ROTATION
		this.sketchPoints = state?.sketchPoints?.map((point) => ({ x: point.x, y: point.y })) ?? []
		this.sketchName = state?.sketchName?.trim() ? state.sketchName.trim() : "Sketch 1"
		this.sketchBreakIndices.clear()
		this.pendingLineStart = null
		this.lineToolNeedsFreshStart = true
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
		this.updateSketchOverlay()
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
		if (this.activeTool !== "sketch") {
			return
		}
		if (!this.activeSketchTool) {
			return
		}
		if (this.isSketchClosed) {
			return
		}
		const rect = this.sketchCanvas.getBoundingClientRect()
		const x = event.clientX - rect.left
		const y = event.clientY - rect.top
		if (this.activeSketchTool === "rectangle") {
			if (!this.pendingRectangleStart) {
				this.pendingRectangleStart = { x, y }
				this.drawSketch({ x, y, active: true })
				return
			}
			const start = this.pendingRectangleStart
			this.pendingRectangleStart = null
			this.appendRectangleToSketch(start, { x, y })
		} else {
			const committed = this.handleLinePointInput({ x, y })
			if (!committed) {
				this.drawSketch({ x, y, active: true })
				this.updateSketchOverlay({ x, y })
				this.updateControls()
				this.emitStateChange()
				return
			}
		}
		this.drawSketch()
		this.updateSketchOverlay()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleSketchHover = (event: MouseEvent) => {
		if (this.activeTool !== "sketch") {
			return
		}
		const rect = this.sketchCanvas.getBoundingClientRect()
		const x = event.clientX - rect.left
		const y = event.clientY - rect.top
		this.drawSketch({ x, y, active: event.type === "mousemove" })
		this.updateSketchOverlay(event.type === "mousemove" ? { x, y } : null)
	}

	private handleUndo = () => {
		if (this.isSketchClosed) {
			return
		}
		if (this.pendingLineStart) {
			this.pendingLineStart = null
			this.sketchHoverPoint = null
			this.drawSketch()
			this.updateSketchOverlay()
			this.updateStatus()
			this.updateControls()
			this.emitStateChange()
			return
		}
		if (!this.sketchPoints.length) {
			return
		}
		const removedIndex = this.sketchPoints.length - 1
		this.sketchPoints = this.sketchPoints.slice(0, removedIndex)
		this.sketchBreakIndices.delete(removedIndex)
		this.drawSketch()
		this.updateSketchOverlay()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleReset = () => {
		this.sketchPoints = []
		this.sketchBreakIndices.clear()
		this.isSketchClosed = false
		this.pendingRectangleStart = null
		this.pendingLineStart = null
		this.lineToolNeedsFreshStart = true
		this.sketchHoverPoint = null
		this.extrudedModel = null
		this.drawSketch()
		this.syncPreviewGeometry()
		this.updateSketchOverlay()
		this.drawPreview()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleFinishSketch = () => {
		if (this.sketchPoints.length < 3 || this.sketchBreakIndices.size > 0) {
			return
		}
		this.isSketchClosed = true
		this.sketchHoverPoint = null
		this.drawSketch()
		this.updateSketchOverlay()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleExtrude = () => {
		if (!this.isSketchClosed || this.sketchPoints.length < 3 || this.sketchBreakIndices.size > 0) {
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
		this.previewCanvas.focus({ preventScroll: true })
		const isLeftMouseClick = event.pointerType === "mouse" ? event.button === 0 : false
		const isRightMouseClick = event.pointerType === "mouse" ? event.button === 2 : event.isPrimary && event.button === 0
		const isMiddleMouseClick = event.pointerType === "mouse" && event.button === 1
		if (isLeftMouseClick) {
			const clickedHandle = this.getReferenceHandleAt(event.clientX, event.clientY)
			if (clickedHandle && this.selectedReferencePlane) {
				event.preventDefault()
				this.activeResizeHandle = clickedHandle
				this.resizingReferencePlane = this.selectedReferencePlane
				this.previewCanvas.setPointerCapture(event.pointerId)
				this.previewCanvas.style.cursor = "nwse-resize"
				return
			}
			if (this.activeTool === "sketch" && this.selectedReferencePlane) {
				const pointIndex = this.getSketchPointIndexAtClient(event.clientX, event.clientY, this.selectedReferencePlane)
				if (pointIndex !== null) {
					event.preventDefault()
					this.draggingSketchPointIndex = pointIndex
					this.sketchHoverPoint = null
					this.previewCanvas.setPointerCapture(event.pointerId)
					this.previewCanvas.style.cursor = "move"
					return
				}
			}
			if (this.activeTool === "sketch" && this.selectedReferencePlane && this.isPointInsideReferencePlane(event.clientX, event.clientY, this.selectedReferencePlane)) {
				event.preventDefault()
				this.handleSketchPlanePointInput(event.clientX, event.clientY)
				return
			}
			const clickedPlane = this.getReferencePlaneAt(event.clientX, event.clientY)
			if (this.activeTool === "sketch" && clickedPlane) {
				event.preventDefault()
				if (this.selectedReferencePlane !== clickedPlane) {
					// While a sketch command is active, clicks on other reference planes
					// should not retarget drawing to another plane.
					if (this.activeSketchTool === null) {
						this.setSelectedReferencePlane(clickedPlane)
						this.pendingRectangleStart = null
						this.sketchHoverPoint = null
						this.updateSketchOverlay()
					}
					return
				}
				this.handleSketchPlanePointInput(event.clientX, event.clientY)
				return
			}
			if (clickedPlane) {
				event.preventDefault()
				this.setSelectedReferencePlane(clickedPlane)
				return
			}
			if (this.selectedReferencePlane) {
				this.setSelectedReferencePlane(null)
			}
		}
		const isRotatePointer = isRightMouseClick || (event.pointerType !== "mouse" && event.isPrimary && event.button === 0)
		if (!isRotatePointer && !isMiddleMouseClick) {
			return
		}
		event.preventDefault()
		this.isRotatingPreview = isRotatePointer
		this.isPanningPreview = isMiddleMouseClick
		this.reverseRotatePreview = isRightMouseClick
		this.lastRotationPointer = { x: event.clientX, y: event.clientY }
		this.previewCanvas.setPointerCapture(event.pointerId)
		this.previewCanvas.style.cursor = "grabbing"
	}

	private handlePreviewPointerMove = (event: PointerEvent) => {
		if (this.draggingSketchPointIndex !== null && this.selectedReferencePlane) {
			event.preventDefault()
			const localPoint = this.getPointOnReferencePlane(event.clientX, event.clientY, this.selectedReferencePlane)
			if (!localPoint) {
				return
			}
			const clamped = this.clampPointToPlane(localPoint.x, localPoint.y)
			const point = this.planeLocalToSketchPoint(clamped)
			const nextPoints = [...this.sketchPoints]
			if (!nextPoints[this.draggingSketchPointIndex]) {
				return
			}
			nextPoints[this.draggingSketchPointIndex] = point
			this.sketchPoints = nextPoints
			this.drawSketch()
			this.updateSketchOverlay()
			this.updateStatus()
			this.updateControls()
			this.emitStateChange()
			return
		}
		if (this.activeResizeHandle && this.resizingReferencePlane) {
			event.preventDefault()
			this.resizeReferencePlaneFromPointer(event.clientX, event.clientY, this.resizingReferencePlane)
			return
		}
		if ((!this.isRotatingPreview && !this.isPanningPreview) || !this.lastRotationPointer) {
			if (this.activeTool === "sketch" && this.selectedReferencePlane) {
				const localPoint = this.getPointOnReferencePlane(event.clientX, event.clientY, this.selectedReferencePlane)
				const clamped = localPoint ? this.clampPointToPlane(localPoint.x, localPoint.y) : null
				this.sketchHoverPoint = clamped ? this.planeLocalToSketchPoint(clamped) : null
				this.updateSketchOverlay()
			}
			this.updateReferencePlaneHover(event.clientX, event.clientY)
			return
		}
		this.setHoveredReferencePlane(null)
		this.sketchHoverPoint = null
		this.updateSketchOverlay()
		const dx = event.clientX - this.lastRotationPointer.x
		const dy = event.clientY - this.lastRotationPointer.y
		this.lastRotationPointer = { x: event.clientX, y: event.clientY }
		if (this.isRotatingPreview) {
			const direction = this.reverseRotatePreview ? -1 : 1
			this.previewRotation.yaw -= dx * 0.01 * direction
			this.previewRotation.pitch -= dy * 0.01 * direction
			const limit = Math.PI / 2 - 0.1
			this.previewRotation.pitch = Math.min(limit, Math.max(-limit, this.previewRotation.pitch))
		} else if (this.isPanningPreview) {
			const panScale = this.getPreviewPanUnitsPerPixel()
			this.previewPan.x += dx * panScale.x
			this.previewPan.y -= dy * panScale.y
		}
		this.drawPreview()
	}

	private handlePreviewPointerUp = (event: PointerEvent) => {
		if (this.draggingSketchPointIndex !== null) {
			if (this.previewCanvas.hasPointerCapture(event.pointerId)) {
				this.previewCanvas.releasePointerCapture(event.pointerId)
			}
			this.draggingSketchPointIndex = null
			this.updatePreviewCursor()
			this.emitStateChange()
			return
		}
		if (this.activeResizeHandle) {
			if (this.previewCanvas.hasPointerCapture(event.pointerId)) {
				this.previewCanvas.releasePointerCapture(event.pointerId)
			}
			this.activeResizeHandle = null
			this.resizingReferencePlane = null
			this.updatePreviewCursor()
			this.emitStateChange()
			return
		}
		if (!this.isRotatingPreview && !this.isPanningPreview) {
			if (event.type === "pointerleave" || event.type === "pointercancel") {
				this.pointerOverSelectedPlane = false
				this.setHoveredReferencePlane(null)
				this.updatePreviewCursor()
			}
			return
		}
		if (this.previewCanvas.hasPointerCapture(event.pointerId)) {
			this.previewCanvas.releasePointerCapture(event.pointerId)
		}
		this.isRotatingPreview = false
		this.isPanningPreview = false
		this.reverseRotatePreview = false
		this.lastRotationPointer = null
		this.pointerOverSelectedPlane = false
		if (this.activeTool !== "sketch") {
			this.sketchHoverPoint = null
			this.updateSketchOverlay()
		}
		this.updatePreviewCursor()
		this.emitStateChange()
	}

	private handlePreviewDoubleClick = (event: MouseEvent) => {
		if (this.activeTool !== "sketch") {
			return
		}
		if (this.activeSketchTool !== "line" || this.isSketchClosed || this.sketchPoints.length < 3) {
			return
		}
		event.preventDefault()
		this.handleFinishSketch()
	}

	private handlePreviewKeyDown = (event: KeyboardEvent) => {
		this.tryCancelSketchToolWithEscape(event)
	}

	private handleDocumentKeyDown = (event: KeyboardEvent) => {
		if (!this.root.isConnected) {
			document.removeEventListener("keydown", this.handleDocumentKeyDown, true)
			return
		}
		this.tryCancelSketchToolWithEscape(event)
	}

	private tryCancelSketchToolWithEscape(event: KeyboardEvent) {
		if (event.key !== "Escape" && event.key !== "Esc") {
			return
		}
		if (this.activeTool !== "sketch") {
			return
		}
		if (!this.activeSketchTool && !this.pendingRectangleStart && !this.pendingLineStart) {
			return
		}
		event.preventDefault()
		event.stopPropagation()
		this.setActiveSketchTool(null)
	}

	private handlePreviewWheel = (event: WheelEvent) => {
		if (event.deltaY === 0) {
			return
		}
		event.preventDefault()
		const anchorBefore = this.getPreviewPlaneIntersection(event.clientX, event.clientY)
		const zoomFactor = Math.exp(event.deltaY * PREVIEW_ZOOM_SENSITIVITY)
		const nextDistance = THREE.MathUtils.clamp(this.previewCamera.position.z * zoomFactor, PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
		if (Math.abs(nextDistance - this.previewCamera.position.z) < 0.0001) {
			return
		}
		this.previewCamera.position.z = nextDistance
		this.previewCamera.updateProjectionMatrix()

		if (anchorBefore) {
			const anchorAfter = this.getPreviewPlaneIntersection(event.clientX, event.clientY)
			if (anchorAfter) {
				this.previewPan.x += anchorBefore.x - anchorAfter.x
				this.previewPan.y += anchorBefore.y - anchorAfter.y
			}
		}
		this.drawPreview()
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
		this.finishButton.disabled = this.isSketchClosed || this.sketchPoints.length < 3 || this.sketchBreakIndices.size > 0
		this.undoButton.disabled = this.isSketchClosed || (this.sketchPoints.length === 0 && !this.pendingLineStart)
		this.resetButton.disabled = this.sketchPoints.length === 0 && !this.extrudedModel && !this.pendingLineStart
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
			this.sketchCtx.lineWidth = 2
			this.sketchCtx.strokeStyle = "#2563eb"
			const ranges = this.getSketchRanges()
			for (const range of ranges) {
				const first = this.sketchPoints[range.start]
				if (!first) {
					continue
				}
				this.sketchCtx.beginPath()
				this.sketchCtx.moveTo(first.x, first.y)
				for (let index = range.start + 1; index < range.end; index += 1) {
					const point = this.sketchPoints[index]
					if (!point) {
						continue
					}
					this.sketchCtx.lineTo(point.x, point.y)
				}
				if (this.isSketchClosed && this.sketchBreakIndices.size === 0 && range.end - range.start >= 3) {
					this.sketchCtx.closePath()
				}
				this.sketchCtx.stroke()
			}

			if (this.isSketchClosed && this.sketchPoints.length >= 3 && this.sketchBreakIndices.size === 0) {
				this.sketchCtx.fillStyle = "rgba(59,130,246,0.2)"
				const first = this.sketchPoints[0]
				if (first) {
					this.sketchCtx.beginPath()
					this.sketchCtx.moveTo(first.x, first.y)
					for (const point of this.sketchPoints.slice(1)) {
						this.sketchCtx.lineTo(point.x, point.y)
					}
					this.sketchCtx.closePath()
				}
				this.sketchCtx.fill()
			}

			for (const point of this.sketchPoints) {
				this.drawSketchPoint(point, "#1d4ed8")
			}
		}

		if (hover?.active && !this.isSketchClosed && this.sketchPoints.length > 0) {
			if (this.activeSketchTool === "rectangle" && this.pendingRectangleStart) {
				const start = this.pendingRectangleStart
				this.sketchCtx.setLineDash([4, 4])
				this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
				this.sketchCtx.strokeRect(Math.min(start.x, hover.x), Math.min(start.y, hover.y), Math.abs(hover.x - start.x), Math.abs(hover.y - start.y))
				this.sketchCtx.setLineDash([])
				this.drawSketchPoint({ x: hover.x, y: hover.y }, "#0f172a", true)
			} else if (this.activeSketchTool === "line") {
				const anchor = this.pendingLineStart ?? (!this.lineToolNeedsFreshStart ? this.sketchPoints[this.sketchPoints.length - 1] : null)
				if (anchor) {
					this.sketchCtx.setLineDash([4, 4])
					this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
					this.sketchCtx.beginPath()
					this.sketchCtx.moveTo(anchor.x, anchor.y)
					this.sketchCtx.lineTo(hover.x, hover.y)
					this.sketchCtx.stroke()
					this.sketchCtx.setLineDash([])
				}
				this.drawSketchPoint({ x: hover.x, y: hover.y }, "#0f172a", true)
			}
		}

		if (hover?.active && !this.isSketchClosed && this.activeSketchTool === "rectangle" && this.pendingRectangleStart && this.sketchPoints.length === 0) {
			const start = this.pendingRectangleStart
			this.sketchCtx.setLineDash([4, 4])
			this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
			this.sketchCtx.strokeRect(Math.min(start.x, hover.x), Math.min(start.y, hover.y), Math.abs(hover.x - start.x), Math.abs(hover.y - start.y))
			this.sketchCtx.setLineDash([])
			this.drawSketchPoint({ x: hover.x, y: hover.y }, "#0f172a", true)
		}

		if (hover?.active && !this.isSketchClosed && this.activeSketchTool === "line" && this.pendingLineStart && this.sketchPoints.length === 0) {
			this.sketchCtx.setLineDash([4, 4])
			this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
			this.sketchCtx.beginPath()
			this.sketchCtx.moveTo(this.pendingLineStart.x, this.pendingLineStart.y)
			this.sketchCtx.lineTo(hover.x, hover.y)
			this.sketchCtx.stroke()
			this.sketchCtx.setLineDash([])
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
		if (this.sketchPoints.length < 3 || this.sketchBreakIndices.size > 0) {
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

	private getPreviewPlaneIntersection(clientX: number, clientY: number): Point2D | null {
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		const ndcX = ((clientX - rect.left) / rect.width) * 2 - 1
		const ndcY = -((clientY - rect.top) / rect.height) * 2 + 1
		const near = new THREE.Vector3(ndcX, ndcY, -1).unproject(this.previewCamera)
		const far = new THREE.Vector3(ndcX, ndcY, 1).unproject(this.previewCamera)
		const direction = far.sub(near)
		if (Math.abs(direction.z) < 1e-6) {
			return null
		}
		const t = -near.z / direction.z
		if (!Number.isFinite(t)) {
			return null
		}
		return {
			x: near.x + direction.x * t,
			y: near.y + direction.y * t
		}
	}

	private getPreviewPanUnitsPerPixel(): Point2D {
		const rect = this.previewCanvas.getBoundingClientRect()
		const width = Math.max(1, rect.width)
		const height = Math.max(1, rect.height)
		const distance = Math.max(PREVIEW_MIN_CAMERA_DISTANCE, Math.abs(this.previewCamera.position.z))
		const verticalFovRadians = THREE.MathUtils.degToRad(this.previewCamera.fov)
		const visibleHeight = 2 * distance * Math.tan(verticalFovRadians / 2)
		const unitsPerPixelY = visibleHeight / height
		const unitsPerPixelX = (visibleHeight * this.previewCamera.aspect) / width
		return { x: unitsPerPixelX, y: unitsPerPixelY }
	}

	private drawPreview() {
		this.previewRootGroup.position.set(this.previewPan.x, this.previewPan.y, 0)
		this.previewRootGroup.rotation.set(this.previewRotation.pitch, this.previewRotation.yaw, 0)
		this.previewRenderer.render(this.previewScene, this.previewCamera)
	}

	private updateReferencePlaneHover(clientX: number, clientY: number) {
		if (this.activeResizeHandle) {
			this.previewCanvas.style.cursor = "nwse-resize"
			return
		}
		if (this.activeTool === "sketch") {
			const overSelected = this.selectedReferencePlane ? this.getPointOnReferencePlane(clientX, clientY, this.selectedReferencePlane) !== null : false
			this.pointerOverSelectedPlane = overSelected
			this.setHoveredReferencePlane(null)
			this.updatePreviewCursor()
			return
		}
		this.pointerOverSelectedPlane = false
		if (this.getReferenceHandleAt(clientX, clientY)) {
			this.setHoveredReferencePlane(this.selectedReferencePlane)
			this.previewCanvas.style.cursor = "nwse-resize"
			return
		}
		this.setHoveredReferencePlane(this.getReferencePlaneAt(clientX, clientY))
	}

	private getReferencePlaneAt(clientX: number, clientY: number): THREE.Mesh | null {
		if (!this.previewReferenceGroup.visible || this.previewReferencePlanes.length === 0) {
			return null
		}
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		this.previewPointer.x = ((clientX - rect.left) / rect.width) * 2 - 1
		this.previewPointer.y = -((clientY - rect.top) / rect.height) * 2 + 1
		this.previewRaycaster.setFromCamera(this.previewPointer, this.previewCamera)
		const intersections = this.previewRaycaster.intersectObjects(
			this.previewReferencePlanes.map((plane) => plane.mesh),
			false
		)
		const mesh = intersections[0]?.object
		return mesh instanceof THREE.Mesh ? mesh : null
	}

	private setHoveredReferencePlane(mesh: THREE.Mesh | null) {
		if (this.hoveredReferencePlane === mesh) {
			return
		}
		this.hoveredReferencePlane = mesh
		this.refreshReferencePlaneStyles()
		this.updatePreviewCursor()
		this.drawPreview()
	}

	private setSelectedReferencePlane(mesh: THREE.Mesh | null) {
		if (this.selectedReferencePlane === mesh) {
			return
		}
		this.selectedReferencePlane = mesh
		this.pointerOverSelectedPlane = false
		this.sketchHoverPoint = null
		this.refreshReferencePlaneStyles()
		this.updateReferencePlaneHandles()
		this.updateSketchOverlay()
		this.updatePreviewCursor()
		this.drawPreview()
	}

	private getReferenceHandleAt(clientX: number, clientY: number): ReferencePlaneHandle | null {
		if (!this.selectedReferencePlane || this.previewReferenceHandles.length === 0) {
			return null
		}
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		this.previewPointer.x = ((clientX - rect.left) / rect.width) * 2 - 1
		this.previewPointer.y = -((clientY - rect.top) / rect.height) * 2 + 1
		this.previewRaycaster.setFromCamera(this.previewPointer, this.previewCamera)
		const intersections = this.previewRaycaster.intersectObjects(
			this.previewReferenceHandles.map((handle) => handle.mesh),
			false
		)
		const mesh = intersections[0]?.object
		if (!(mesh instanceof THREE.Mesh)) {
			return null
		}
		return this.previewReferenceHandles.find((handle) => handle.mesh === mesh) ?? null
	}

	private resizeReferencePlaneFromPointer(clientX: number, clientY: number, plane: THREE.Mesh) {
		const localPoint = this.getPointOnReferencePlane(clientX, clientY, plane)
		if (!localPoint) {
			return
		}
		const minHalfSize = 0.2
		const maxHalfSize = 3.5
		const halfX = THREE.MathUtils.clamp(Math.abs(localPoint.x), minHalfSize, maxHalfSize)
		const halfY = THREE.MathUtils.clamp(Math.abs(localPoint.y), minHalfSize, maxHalfSize)
		plane.scale.set((halfX * 2) / REFERENCE_PLANE_SIZE, (halfY * 2) / REFERENCE_PLANE_SIZE, 1)
		this.updateReferencePlaneHandles()
		this.drawPreview()
	}

	private getPointOnReferencePlane(clientX: number, clientY: number, plane: THREE.Mesh): THREE.Vector3 | null {
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		this.previewPointer.x = ((clientX - rect.left) / rect.width) * 2 - 1
		this.previewPointer.y = -((clientY - rect.top) / rect.height) * 2 + 1
		this.previewRaycaster.setFromCamera(this.previewPointer, this.previewCamera)

		const worldNormal = new THREE.Vector3(0, 0, 1).applyQuaternion(plane.getWorldQuaternion(new THREE.Quaternion())).normalize()
		const worldOrigin = plane.getWorldPosition(new THREE.Vector3())
		const planeEquation = new THREE.Plane().setFromNormalAndCoplanarPoint(worldNormal, worldOrigin)
		const worldPoint = this.previewRaycaster.ray.intersectPlane(planeEquation, new THREE.Vector3())
		if (!worldPoint) {
			return null
		}
		return plane.worldToLocal(worldPoint.clone())
	}

	private isPointInsideReferencePlane(clientX: number, clientY: number, plane: THREE.Mesh): boolean {
		const localPoint = this.getPointOnReferencePlane(clientX, clientY, plane)
		if (!localPoint) {
			return false
		}
		const half = REFERENCE_PLANE_SIZE / 2
		return Math.abs(localPoint.x) <= half && Math.abs(localPoint.y) <= half
	}

	private getSketchPointIndexAtClient(clientX: number, clientY: number, plane: THREE.Mesh): number | null {
		if (this.sketchPoints.length === 0) {
			return null
		}
		const localPoint = this.getPointOnReferencePlane(clientX, clientY, plane)
		if (!localPoint) {
			return null
		}
		const clamped = this.clampPointToPlane(localPoint.x, localPoint.y)
		const pointer = this.planeLocalToSketchPoint(clamped)
		const maxDistancePx = 12
		const maxDistanceSquared = maxDistancePx * maxDistancePx
		let bestIndex: number | null = null
		let bestDistanceSquared = maxDistanceSquared
		for (let index = 0; index < this.sketchPoints.length; index += 1) {
			const point = this.sketchPoints[index]
			if (!point) {
				continue
			}
			const dx = point.x - pointer.x
			const dy = point.y - pointer.y
			const distanceSquared = dx * dx + dy * dy
			if (distanceSquared <= bestDistanceSquared) {
				bestIndex = index
				bestDistanceSquared = distanceSquared
			}
		}
		return bestIndex
	}

	private planeLocalToSketchPoint(point: Point2D): Point2D {
		return {
			x: (point.x / REFERENCE_PLANE_SIZE + 0.5) * SKETCH_CANVAS_SIZE,
			y: (0.5 - point.y / REFERENCE_PLANE_SIZE) * SKETCH_CANVAS_SIZE
		}
	}

	private sketchPointToPlaneLocal(point: Point2D): Point2D {
		return {
			x: (point.x / SKETCH_CANVAS_SIZE - 0.5) * REFERENCE_PLANE_SIZE,
			y: (0.5 - point.y / SKETCH_CANVAS_SIZE) * REFERENCE_PLANE_SIZE
		}
	}

	private clampPointToPlane(x: number, y: number): Point2D {
		const half = REFERENCE_PLANE_SIZE / 2
		return {
			x: THREE.MathUtils.clamp(x, -half, half),
			y: THREE.MathUtils.clamp(y, -half, half)
		}
	}

	private handleSketchPlanePointInput(clientX: number, clientY: number) {
		if (!this.selectedReferencePlane || this.isSketchClosed) {
			return
		}
		if (!this.activeSketchTool) {
			return
		}
		const localPoint = this.getPointOnReferencePlane(clientX, clientY, this.selectedReferencePlane)
		if (!localPoint) {
			return
		}
		const clampedLocal = this.clampPointToPlane(localPoint.x, localPoint.y)
		const point = this.planeLocalToSketchPoint(clampedLocal)
		if (this.activeSketchTool === "rectangle") {
			if (!this.pendingRectangleStart) {
				this.pendingRectangleStart = point
				this.sketchHoverPoint = point
				this.updateSketchOverlay()
				return
			}
			const start = this.pendingRectangleStart
			this.pendingRectangleStart = null
			this.appendRectangleToSketch(start, point)
		} else {
			const committed = this.handleLinePointInput(point)
			if (!committed) {
				this.sketchHoverPoint = point
				this.drawSketch(this.sketchHoverPoint ? { ...this.sketchHoverPoint, active: true } : undefined)
				this.updateSketchOverlay()
				this.updateControls()
				this.emitStateChange()
				return
			}
		}
		this.drawSketch(this.sketchHoverPoint ? { ...this.sketchHoverPoint, active: true } : undefined)
		this.updateSketchOverlay()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private appendRectangleToSketch(start: Point2D, end: Point2D) {
		const rectanglePoints: Point2D[] = [
			{ x: start.x, y: start.y },
			{ x: end.x, y: start.y },
			{ x: end.x, y: end.y },
			{ x: start.x, y: end.y }
		]
		if (this.sketchPoints.length === 0) {
			this.sketchPoints = rectanglePoints
			this.isSketchClosed = true
			return
		}
		const closingPoint: Point2D = { x: start.x, y: start.y }
		this.sketchPoints = [...this.sketchPoints, ...rectanglePoints, closingPoint]
		this.isSketchClosed = false
	}

	private handleLinePointInput(point: Point2D): boolean {
		if (this.lineToolNeedsFreshStart) {
			this.pendingLineStart = point
			this.lineToolNeedsFreshStart = false
			return false
		}
		if (this.pendingLineStart) {
			const start = this.pendingLineStart
			this.pendingLineStart = null
			if (this.sketchPoints.length > 0) {
				this.sketchBreakIndices.add(this.sketchPoints.length)
			}
			this.sketchPoints = [...this.sketchPoints, start, point]
			return true
		}
		this.sketchPoints = [...this.sketchPoints, point]
		return true
	}

	private createSketchLineObject(points: Point2D[], color: number, zOffset: number, closed: boolean): THREE.Line | THREE.LineLoop | null {
		if (points.length < 2) {
			return null
		}
		const vertices = points.flatMap((point) => {
			const local = this.sketchPointToPlaneLocal(point)
			return [local.x, local.y, zOffset]
		})
		const geometry = new THREE.BufferGeometry()
		geometry.setAttribute("position", new THREE.Float32BufferAttribute(vertices, 3))
		const material = new THREE.LineBasicMaterial({
			color,
			transparent: true,
			opacity: 1,
			depthTest: false
		})
		return closed ? new THREE.LineLoop(geometry, material) : new THREE.Line(geometry, material)
	}

	private createSketchSegmentObject(points: Point2D[], color: number, zOffset: number): THREE.LineSegments | null {
		const ranges = this.getSketchRanges()
		const vertices: number[] = []
		for (const range of ranges) {
			for (let index = range.start + 1; index < range.end; index += 1) {
				const previous = points[index - 1]
				const current = points[index]
				if (!previous || !current) {
					continue
				}
				const start = this.sketchPointToPlaneLocal(previous)
				const end = this.sketchPointToPlaneLocal(current)
				vertices.push(start.x, start.y, zOffset, end.x, end.y, zOffset)
			}
		}
		if (vertices.length === 0) {
			return null
		}
		const geometry = new THREE.BufferGeometry()
		geometry.setAttribute("position", new THREE.Float32BufferAttribute(vertices, 3))
		const material = new THREE.LineBasicMaterial({
			color,
			transparent: true,
			opacity: 1,
			depthTest: false
		})
		return new THREE.LineSegments(geometry, material)
	}

	private getSketchRanges(): Array<{ start: number; end: number }> {
		if (this.sketchPoints.length === 0) {
			return []
		}
		const ranges: Array<{ start: number; end: number }> = []
		let start = 0
		for (let index = 1; index < this.sketchPoints.length; index += 1) {
			if (this.sketchBreakIndices.has(index)) {
				ranges.push({ start, end: index })
				start = index
			}
		}
		ranges.push({ start, end: this.sketchPoints.length })
		return ranges
	}

	private disposeSketchOverlayObject(object: THREE.Object3D | null) {
		if (!object) {
			return
		}
		object.removeFromParent()
		if (object instanceof THREE.Line || object instanceof THREE.LineLoop || object instanceof THREE.LineSegments || object instanceof THREE.Points) {
			object.geometry.dispose()
			const material = object.material
			if (Array.isArray(material)) {
				for (const entry of material) {
					entry.dispose()
				}
			} else {
				material.dispose()
			}
		}
	}

	private updateSketchOverlay(hoverOverride?: Point2D | null) {
		if (typeof hoverOverride !== "undefined") {
			this.sketchHoverPoint = hoverOverride
		}
		const shouldShow = this.activeTool === "sketch" && this.previewReferenceGroup.visible && !!this.selectedReferencePlane
		if (!shouldShow || !this.selectedReferencePlane) {
			this.sketchOverlayGroup.visible = false
			this.disposeSketchOverlayObject(this.sketchOverlayCommittedLine)
			this.disposeSketchOverlayObject(this.sketchOverlayPreviewLine)
			this.disposeSketchOverlayObject(this.sketchOverlayPoints)
			this.disposeSketchOverlayObject(this.sketchOverlayLabel)
			this.sketchOverlayCommittedLine = null
			this.sketchOverlayPreviewLine = null
			this.sketchOverlayPoints = null
			this.sketchOverlayLabel = null
			return
		}

		if (this.sketchOverlayParentPlane !== this.selectedReferencePlane) {
			this.sketchOverlayGroup.removeFromParent()
			this.selectedReferencePlane.add(this.sketchOverlayGroup)
			this.sketchOverlayParentPlane = this.selectedReferencePlane
		}
		this.sketchOverlayGroup.visible = true
		this.sketchOverlayGroup.position.set(0, 0, 0.004)

		this.disposeSketchOverlayObject(this.sketchOverlayCommittedLine)
		if (this.sketchBreakIndices.size === 0) {
			this.sketchOverlayCommittedLine = this.createSketchLineObject(this.sketchPoints, 0x2563eb, 0, this.isSketchClosed)
		} else {
			this.sketchOverlayCommittedLine = this.createSketchSegmentObject(this.sketchPoints, 0x2563eb, 0)
		}
		if (this.sketchOverlayCommittedLine) {
			this.sketchOverlayCommittedLine.renderOrder = 6
			this.sketchOverlayGroup.add(this.sketchOverlayCommittedLine)
		}

		this.disposeSketchOverlayObject(this.sketchOverlayPreviewLine)
		let previewPoints: Point2D[] | null = null
		if (!this.isSketchClosed && this.sketchHoverPoint) {
			if (this.activeSketchTool === "rectangle" && this.pendingRectangleStart) {
				const start = this.pendingRectangleStart
				const hover = this.sketchHoverPoint
				previewPoints = [
					{ x: start.x, y: start.y },
					{ x: hover.x, y: start.y },
					{ x: hover.x, y: hover.y },
					{ x: start.x, y: hover.y }
				]
			} else if (this.activeSketchTool === "line") {
				const anchor = this.pendingLineStart ?? (!this.lineToolNeedsFreshStart && this.sketchPoints.length > 0 ? this.sketchPoints[this.sketchPoints.length - 1] : null)
				if (anchor) {
					previewPoints = [anchor, this.sketchHoverPoint]
				}
			}
		}
		this.sketchOverlayPreviewLine = previewPoints ? this.createSketchLineObject(previewPoints, 0x0f172a, 0.001, this.activeSketchTool === "rectangle") : null
		if (this.sketchOverlayPreviewLine) {
			const material = this.sketchOverlayPreviewLine.material as THREE.LineBasicMaterial
			material.opacity = 0.7
			this.sketchOverlayPreviewLine.renderOrder = 7
			this.sketchOverlayGroup.add(this.sketchOverlayPreviewLine)
		}

		this.disposeSketchOverlayObject(this.sketchOverlayPoints)
		if (this.sketchPoints.length > 0) {
			const vertices = this.sketchPoints.flatMap((point) => {
				const local = this.sketchPointToPlaneLocal(point)
				return [local.x, local.y, 0.002]
			})
			const geometry = new THREE.BufferGeometry()
			geometry.setAttribute("position", new THREE.Float32BufferAttribute(vertices, 3))
			const material = new THREE.PointsMaterial({
				color: 0x1d4ed8,
				size: 0.03,
				sizeAttenuation: true,
				depthTest: false
			})
			this.sketchOverlayPoints = new THREE.Points(geometry, material)
			this.sketchOverlayPoints.renderOrder = 8
			this.sketchOverlayGroup.add(this.sketchOverlayPoints)
		} else {
			this.sketchOverlayPoints = null
		}

		this.disposeSketchOverlayObject(this.sketchOverlayLabel)
		this.sketchOverlayLabel = this.createReferenceLabelSprite(this.sketchName)
		this.sketchOverlayLabel.position.set(-REFERENCE_PLANE_SIZE / 2 + 0.16, REFERENCE_PLANE_SIZE / 2 - 0.08, 0.003)
		this.sketchOverlayLabel.renderOrder = 9
		this.sketchOverlayGroup.add(this.sketchOverlayLabel)

		this.drawPreview()
	}

	private refreshReferencePlaneStyles() {
		for (const plane of this.previewReferencePlanes) {
			const isSelected = plane.mesh === this.selectedReferencePlane
			const isHovered = plane.mesh === this.hoveredReferencePlane
			if (isSelected) {
				const inSketchMode = this.activeTool === "sketch"
				plane.fillMaterial.color.setHex(inSketchMode ? 0x7dd3fc : 0xf59e0b)
				plane.fillMaterial.opacity = inSketchMode ? 0.16 : 0.35
				plane.edgeMaterial.color.setHex(inSketchMode ? 0x7dd3fc : 0xf59e0b)
				plane.edgeMaterial.opacity = 1
				plane.mesh.renderOrder = 3
				plane.edge.renderOrder = 4
			} else if (isHovered) {
				const hoverColor = this.activeTool === "sketch" ? 0x7dd3fc : 0xf59e0b
				plane.fillMaterial.color.setHex(hoverColor)
				plane.fillMaterial.opacity = this.activeTool === "sketch" ? 0.12 : 0.2
				plane.edgeMaterial.color.setHex(hoverColor)
				plane.edgeMaterial.opacity = 0.95
				plane.mesh.renderOrder = 2
				plane.edge.renderOrder = 3
			} else {
				plane.fillMaterial.color.setHex(0x93b4dc)
				plane.fillMaterial.opacity = 0.14
				plane.edgeMaterial.color.setHex(0x8fb3dd)
				plane.edgeMaterial.opacity = 0.7
				plane.mesh.renderOrder = 1
				plane.edge.renderOrder = 2
			}
		}
	}

	private updateReferencePlaneHandles() {
		if (!this.selectedReferencePlane || !this.previewReferenceGroup.visible) {
			for (const handle of this.previewReferenceHandles) {
				handle.mesh.visible = false
			}
			return
		}
		const half = REFERENCE_PLANE_SIZE / 2
		const corners: Array<{ x: number; y: number; xSign: -1 | 1; ySign: -1 | 1 }> = [
			{ x: -half, y: half, xSign: -1, ySign: 1 },
			{ x: half, y: half, xSign: 1, ySign: 1 },
			{ x: half, y: -half, xSign: 1, ySign: -1 },
			{ x: -half, y: -half, xSign: -1, ySign: -1 }
		]
		for (let index = 0; index < corners.length; index += 1) {
			let handle = this.previewReferenceHandles[index]
			if (!handle) {
				const mesh = new THREE.Mesh(this.previewReferenceHandleGeometry, this.previewReferenceHandleMaterial.clone())
				mesh.renderOrder = 5
				handle = {
					mesh,
					xSign: corners[index]?.xSign ?? 1,
					ySign: corners[index]?.ySign ?? 1
				}
				this.previewReferenceHandles[index] = handle
			}
			const corner = corners[index]
			if (!corner) {
				continue
			}
			handle.xSign = corner.xSign
			handle.ySign = corner.ySign
			handle.mesh.position.set(corner.x, corner.y, 0.001)
			handle.mesh.scale.set(1 / Math.max(0.2, this.selectedReferencePlane.scale.x), 1 / Math.max(0.2, this.selectedReferencePlane.scale.y), 1)
			handle.mesh.visible = true
			if (handle.mesh.parent !== this.selectedReferencePlane) {
				handle.mesh.removeFromParent()
				this.selectedReferencePlane.add(handle.mesh)
			}
		}
	}

	private updatePreviewCursor() {
		if (this.activeResizeHandle) {
			this.previewCanvas.style.cursor = "nwse-resize"
			return
		}
		if (this.isRotatingPreview || this.isPanningPreview) {
			this.previewCanvas.style.cursor = "grabbing"
			return
		}
		if (this.activeTool === "sketch" && this.pointerOverSelectedPlane && this.activeSketchTool !== null) {
			this.previewCanvas.style.cursor = "crosshair"
			return
		}
		this.previewCanvas.style.cursor = this.hoveredReferencePlane ? "pointer" : "grab"
	}

	private createReferencePlanes(): THREE.Group {
		const group = new THREE.Group()
		const addPlane = (name: "Front" | "Top" | "Right", rotation: THREE.Euler, labelPosition: THREE.Vector3) => {
			const fillMaterial = new THREE.MeshBasicMaterial({
				color: 0x93b4dc,
				transparent: true,
				opacity: 0.14,
				side: THREE.DoubleSide,
				depthWrite: false,
				depthTest: false
			})
			const plane = new THREE.Mesh(new THREE.PlaneGeometry(REFERENCE_PLANE_SIZE, REFERENCE_PLANE_SIZE), fillMaterial)
			plane.rotation.copy(rotation)
			group.add(plane)
			const edgeMaterial = new THREE.LineBasicMaterial({
				color: 0x8fb3dd,
				transparent: true,
				opacity: 0.7,
				depthTest: false
			})
			const edge = new THREE.LineSegments(new THREE.EdgesGeometry(plane.geometry), edgeMaterial)
			edge.rotation.copy(rotation)
			group.add(edge)
			this.previewReferencePlanes.push({
				name,
				mesh: plane,
				edge,
				fillMaterial,
				edgeMaterial
			})
			const label = this.createReferenceLabelSprite(name)
			label.position.copy(labelPosition)
			group.add(label)
		}
		addPlane("Front", new THREE.Euler(0, 0, 0), new THREE.Vector3(-REFERENCE_PLANE_SIZE / 2 + 0.16, REFERENCE_PLANE_SIZE / 2 - 0.1, 0))
		addPlane("Top", new THREE.Euler(-Math.PI / 2, 0, 0), new THREE.Vector3(-0.08, 0, -REFERENCE_PLANE_SIZE / 2 + 0.18))
		addPlane("Right", new THREE.Euler(0, Math.PI / 2, 0), new THREE.Vector3(0, REFERENCE_PLANE_SIZE / 2 - 0.1, -REFERENCE_PLANE_SIZE / 2 + 0.16))
		const origin = new THREE.Mesh(new THREE.SphereGeometry(0.02, 16, 12), new THREE.MeshBasicMaterial({ color: 0x111827 }))
		group.add(origin)
		return group
	}

	private createReferenceLabelSprite(text: string): THREE.Sprite {
		const canvas = document.createElement("canvas")
		canvas.width = 128
		canvas.height = 48
		const ctx = canvas.getContext("2d")
		if (ctx) {
			ctx.clearRect(0, 0, canvas.width, canvas.height)
			ctx.fillStyle = "#2d6bcf"
			ctx.font = "700 24px sans-serif"
			ctx.textBaseline = "middle"
			ctx.fillText(text, 8, canvas.height / 2)
		}
		const texture = new THREE.CanvasTexture(canvas)
		texture.minFilter = THREE.LinearFilter
		texture.magFilter = THREE.LinearFilter
		texture.generateMipmaps = false
		const material = new THREE.SpriteMaterial({
			map: texture,
			transparent: true,
			depthWrite: false
		})
		const sprite = new THREE.Sprite(material)
		sprite.scale.set(0.48, 0.18, 1)
		return sprite
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
			this.previewReferenceGroup.visible = true
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

		this.previewReferenceGroup.visible = false
		this.setHoveredReferencePlane(null)
		this.setSelectedReferencePlane(null)
	}
}
