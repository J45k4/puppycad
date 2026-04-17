import type { PartProjectItemData, PartProjectPreviewRotation, PartProjectReferencePlaneVisibility } from "../contract"
import { extrudeSolidFeature, type ExtrudedSolid } from "../cad/extrude"
import { materializeSketch } from "../cad/sketch"
import { PART_PROJECT_DEFAULT_HEIGHT, PART_PROJECT_DEFAULT_PREVIEW_DISTANCE, PART_PROJECT_DEFAULT_ROTATION } from "../project-file"
import { derivePartQuickActionsModel, type PartQuickActionId, type ReferencePlaneName } from "../part-quick-actions"
import { REFERENCE_PLANE_TO_SKETCH_PLANE, SKETCH_PLANE_TO_REFERENCE_PLANE, type PartFeature, type Sketch, type SketchEntity, type SolidExtrude } from "../schema"
import type { Point2D } from "../types"
import { UiComponent } from "./ui"
import * as THREE from "three"

type PartStudioTool = "view" | "sketch"
type SketchTool = "line" | "rectangle"

type ReferencePlaneVisual = {
	name: ReferencePlaneName
	mesh: THREE.Mesh
	edge: THREE.LineSegments
	label: THREE.Sprite
	fillMaterial: THREE.MeshBasicMaterial
	edgeMaterial: THREE.LineBasicMaterial
}

type PreviewFaceVisual = {
	extrudeId: string
	faceId: string
	label: string
	mesh: THREE.Mesh
	material: THREE.MeshBasicMaterial
}

type PreviewSolidVisual = {
	extrudeId: string
	mesh: THREE.Mesh
	edges: THREE.LineSegments
	fillMaterial: THREE.MeshStandardMaterial
	edgeMaterial: THREE.LineBasicMaterial
	faces: PreviewFaceVisual[]
}

type PreviewRendererLike = Pick<THREE.WebGLRenderer, "render" | "setClearColor" | "setPixelRatio" | "setSize">

export type PartEditorState = PartProjectItemData

export type PartEditorViewState = {
	sketchVisible: boolean
	referencePlaneVisibility: PartProjectReferencePlaneVisibility
}

type PartEditorOptions = {
	initialState?: PartEditorState
	onStateChange?: () => void
	createPreviewRenderer?: (canvas: HTMLCanvasElement) => PreviewRendererLike
}

type SketchListEntry = {
	id: string
	name: string
	plane: ReferencePlaneName
	dirty: boolean
}

type ExtrudeListEntry = {
	id: string
	name: string
	depth: number
}

const SKETCH_CANVAS_SIZE = 360
const REFERENCE_PLANE_SIZE = 18
const PREVIEW_FIELD_OF_VIEW = 60
const PREVIEW_MIN_CAMERA_DISTANCE = 0.5
const PREVIEW_MAX_CAMERA_DISTANCE = 50
const PREVIEW_ZOOM_SENSITIVITY = 0.0015
const SKETCH_SNAP_DISTANCE = 0.45

export class PartEditor extends UiComponent<HTMLDivElement> {
	private readonly sketchCanvas: HTMLCanvasElement
	private readonly sketchCtx: CanvasRenderingContext2D
	private readonly previewCanvas: HTMLCanvasElement
	private readonly previewRenderer: PreviewRendererLike
	private readonly previewScene: THREE.Scene
	private readonly previewCamera: THREE.PerspectiveCamera
	private readonly previewRootGroup: THREE.Group
	private readonly previewContentGroup: THREE.Group
	private readonly previewReferenceGroup: THREE.Group
	private readonly previewSolidsGroup = new THREE.Group()
	private readonly previewSketchGroup = new THREE.Group()
	private readonly previewSketchDraftGroup = new THREE.Group()
	private readonly previewReferencePlanes: ReferencePlaneVisual[] = []
	private readonly previewSolids: PreviewSolidVisual[] = []
	private readonly heightInput: HTMLInputElement
	private readonly statusText: HTMLParagraphElement
	private readonly summaryText: HTMLParagraphElement
	private readonly warningText: HTMLParagraphElement
	private readonly previewContainer: HTMLDivElement
	private readonly sketchPanel: HTMLDivElement
	private readonly quickActionsRail: HTMLDivElement
	private readonly quickActionsTitle: HTMLHeadingElement
	private readonly quickActionsDescription: HTMLParagraphElement
	private readonly quickActionsPrimaryActions: HTMLDivElement
	private readonly quickActionsSketchToolsSection: HTMLDivElement
	private readonly quickActionsSketchToolsActions: HTMLDivElement
	private readonly quickActionsCommandSection: HTMLDivElement
	private readonly quickActionsCommandActions: HTMLDivElement
	private readonly quickActionsHeightSection: HTMLDivElement
	private readonly quickActionsStatusSection: HTMLDivElement
	private readonly onStateChange?: () => void
	private readonly previewRotation: PartProjectPreviewRotation = {
		yaw: PART_PROJECT_DEFAULT_ROTATION.yaw,
		pitch: PART_PROJECT_DEFAULT_ROTATION.pitch
	}
	private readonly previewPan = { x: 0, y: 0 }
	private readonly previewPointer = new THREE.Vector2()
	private readonly previewRaycaster = new THREE.Raycaster()
	private readonly migrationWarnings: string[]
	private referencePlaneVisibility: PartProjectReferencePlaneVisibility = {
		Front: true,
		Top: true,
		Right: true
	}
	private sketchVisible = true
	private features: PartFeature[] = []
	private previewBaseDistance = PART_PROJECT_DEFAULT_PREVIEW_DISTANCE
	private selectedReferencePlane: ReferencePlaneName | null = "Front"
	private selectedSketchId: string | null = null
	private selectedExtrudeId: string | null = null
	private selectedFaceId: string | null = null
	private activeTool: PartStudioTool = "view"
	private activeSketchTool: SketchTool | null = "line"
	private pendingRectangleStart: Point2D | null = null
	private pendingLineStart: Point2D | null = null
	private sketchHoverPoint: Point2D | null = null
	private isRotatingPreview = false
	private isPanningPreview = false
	private reverseRotatePreview = false
	private lastRotationPointer: { x: number; y: number } | null = null
	private resizeObserver: ResizeObserver | null = null
	private hoveredReferencePlane: ReferencePlaneName | null = null
	private hoveredExtrudeId: string | null = null
	private hoveredFaceId: string | null = null

	public constructor(options?: PartEditorOptions) {
		super(document.createElement("div"))
		this.onStateChange = options?.onStateChange
		this.migrationWarnings = [...(options?.initialState?.migrationWarnings ?? [])]

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

		this.warningText = document.createElement("p")
		this.warningText.style.margin = "8px 0 0"
		this.warningText.style.fontSize = "13px"
		this.warningText.style.color = "#9a3412"
		header.appendChild(this.warningText)

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
		this.sketchPanel.style.flex = "0 0 360px"
		this.sketchPanel.style.minWidth = "320px"
		this.sketchPanel.style.display = "none"
		this.sketchPanel.style.flexDirection = "column"
		this.sketchPanel.style.gap = "12px"
		body.appendChild(this.sketchPanel)

		const sketchHeader = document.createElement("div")
		sketchHeader.innerHTML =
			'<h3 style="margin:0;font-size:16px;">Sketch</h3><p style="margin:4px 0 0;color:#475569;font-size:13px;">Use the active drawing tool to author a plane sketch. Finish the sketch before extruding it.</p>'
		this.sketchPanel.appendChild(sketchHeader)

		this.sketchCanvas = document.createElement("canvas")
		this.sketchCanvas.style.border = "1px solid #cbd5f5"
		this.sketchCanvas.style.borderRadius = "8px"
		this.sketchCanvas.style.background =
			"linear-gradient(45deg, #f8fafc 25%, transparent 25%), linear-gradient(-45deg, #f8fafc 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #f8fafc 75%), linear-gradient(-45deg, transparent 75%, #f8fafc 75%)"
		this.sketchCanvas.style.backgroundSize = "24px 24px"
		this.sketchCanvas.style.backgroundPosition = "0 0, 0 12px, 12px -12px, -12px 0"
		this.sketchCanvas.style.cursor = "crosshair"
		this.sketchCanvas.width = SKETCH_CANVAS_SIZE * getDevicePixelRatio()
		this.sketchCanvas.height = SKETCH_CANVAS_SIZE * getDevicePixelRatio()
		this.sketchCanvas.style.width = "100%"
		this.sketchCanvas.style.maxWidth = "100%"
		this.sketchCanvas.style.aspectRatio = "1 / 1"
		this.sketchCanvas.style.height = "auto"
		this.sketchPanel.appendChild(this.sketchCanvas)

		this.sketchCtx = getSketchCanvasContext(this.sketchCanvas)
		this.sketchCtx.scale(getDevicePixelRatio(), getDevicePixelRatio())

		this.previewContainer = document.createElement("div")
		this.previewContainer.style.flex = "1 1 640px"
		this.previewContainer.style.width = "auto"
		this.previewContainer.style.minWidth = "0"
		this.previewContainer.style.maxWidth = "none"
		this.previewContainer.style.height = "100%"
		this.previewContainer.style.minHeight = "0"
		this.previewContainer.style.backgroundColor = "#f1f5f9"
		this.previewContainer.style.borderRadius = "12px"
		this.previewContainer.style.position = "relative"
		this.previewContainer.style.display = "flex"
		this.previewContainer.style.alignItems = "center"
		this.previewContainer.style.justifyContent = "center"
		this.previewContainer.style.overflow = "hidden"
		this.previewContainer.style.boxShadow = "inset 0 0 0 1px rgba(148,163,184,0.35)"
		body.appendChild(this.previewContainer)

		this.quickActionsRail = document.createElement("div")
		this.quickActionsRail.style.display = "none"
		this.quickActionsRail.style.position = "absolute"
		this.quickActionsRail.style.top = "16px"
		this.quickActionsRail.style.right = "16px"
		this.quickActionsRail.style.bottom = "16px"
		this.quickActionsRail.style.width = "220px"
		this.quickActionsRail.style.maxWidth = "calc(100% - 32px)"
		this.quickActionsRail.style.backgroundColor = "rgba(255,255,255,0.96)"
		this.quickActionsRail.style.backdropFilter = "blur(10px)"
		this.quickActionsRail.style.border = "1px solid rgba(203,226,241,0.95)"
		this.quickActionsRail.style.borderRadius = "12px"
		this.quickActionsRail.style.boxShadow = "0 14px 32px rgba(15,23,42,0.14)"
		this.quickActionsRail.style.padding = "16px"
		this.quickActionsRail.style.flexDirection = "column"
		this.quickActionsRail.style.gap = "16px"
		this.quickActionsRail.style.overflowY = "auto"
		this.quickActionsRail.style.zIndex = "2"
		this.previewContainer.appendChild(this.quickActionsRail)

		const quickActionsHeader = document.createElement("div")
		quickActionsHeader.style.display = "flex"
		quickActionsHeader.style.flexDirection = "column"
		quickActionsHeader.style.gap = "6px"
		this.quickActionsRail.appendChild(quickActionsHeader)

		this.quickActionsTitle = document.createElement("h3")
		this.quickActionsTitle.style.margin = "0"
		this.quickActionsTitle.style.fontSize = "16px"
		quickActionsHeader.appendChild(this.quickActionsTitle)

		this.quickActionsDescription = document.createElement("p")
		this.quickActionsDescription.style.margin = "0"
		this.quickActionsDescription.style.fontSize = "13px"
		this.quickActionsDescription.style.lineHeight = "1.5"
		this.quickActionsDescription.style.color = "#475569"
		quickActionsHeader.appendChild(this.quickActionsDescription)

		this.quickActionsPrimaryActions = document.createElement("div")
		this.quickActionsPrimaryActions.style.display = "grid"
		this.quickActionsPrimaryActions.style.gap = "8px"
		this.quickActionsRail.appendChild(this.quickActionsPrimaryActions)

		this.quickActionsSketchToolsSection = this.createQuickActionsSection("Sketch Tools")
		this.quickActionsSketchToolsActions = document.createElement("div")
		this.quickActionsSketchToolsActions.style.display = "grid"
		this.quickActionsSketchToolsActions.style.gridTemplateColumns = "repeat(2, minmax(0, 1fr))"
		this.quickActionsSketchToolsActions.style.gap = "8px"
		this.quickActionsSketchToolsSection.appendChild(this.quickActionsSketchToolsActions)
		this.quickActionsRail.appendChild(this.quickActionsSketchToolsSection)

		this.quickActionsCommandSection = this.createQuickActionsSection("Sketch Actions")
		this.quickActionsCommandActions = document.createElement("div")
		this.quickActionsCommandActions.style.display = "grid"
		this.quickActionsCommandActions.style.gap = "8px"
		this.quickActionsCommandSection.appendChild(this.quickActionsCommandActions)
		this.quickActionsRail.appendChild(this.quickActionsCommandSection)

		this.quickActionsHeightSection = this.createQuickActionsSection("Extrude")
		const extrudeControls = document.createElement("div")
		extrudeControls.style.display = "flex"
		extrudeControls.style.flexDirection = "column"
		extrudeControls.style.gap = "8px"
		this.quickActionsHeightSection.appendChild(extrudeControls)
		const heightLabel = document.createElement("label")
		heightLabel.textContent = "Extrude height"
		heightLabel.style.fontSize = "13px"
		heightLabel.style.color = "#0f172a"
		extrudeControls.appendChild(heightLabel)
		this.heightInput = document.createElement("input")
		this.heightInput.type = "number"
		this.heightInput.min = "1"
		this.heightInput.step = "1"
		this.heightInput.value = String(PART_PROJECT_DEFAULT_HEIGHT)
		this.heightInput.style.width = "80px"
		this.heightInput.style.padding = "4px 6px"
		this.heightInput.style.border = "1px solid #cbd5f5"
		this.heightInput.style.borderRadius = "4px"
		this.heightInput.addEventListener("input", () => this.updateControls())
		this.heightInput.addEventListener("change", this.handleHeightInputChange)
		extrudeControls.appendChild(this.heightInput)
		this.quickActionsRail.appendChild(this.quickActionsHeightSection)

		this.quickActionsStatusSection = this.createQuickActionsSection("Status")
		this.statusText = document.createElement("p")
		this.statusText.style.margin = "4px 0 0"
		this.statusText.style.fontSize = "13px"
		this.statusText.style.color = "#475569"
		this.summaryText = document.createElement("p")
		this.summaryText.style.margin = "0"
		this.summaryText.style.fontSize = "13px"
		this.summaryText.style.color = "#0f172a"
		this.quickActionsStatusSection.appendChild(this.statusText)
		this.quickActionsStatusSection.appendChild(this.summaryText)
		this.quickActionsRail.appendChild(this.quickActionsStatusSection)

		this.previewCanvas = document.createElement("canvas")
		this.previewCanvas.style.width = "100%"
		this.previewCanvas.style.height = "100%"
		this.previewCanvas.style.cursor = "grab"
		this.previewCanvas.tabIndex = 0
		this.previewContainer.appendChild(this.previewCanvas)

		this.previewRenderer = createPreviewRenderer(this.previewCanvas, options?.createPreviewRenderer)
		this.previewRenderer.setPixelRatio(Math.min(2, getDevicePixelRatio()))
		this.previewRenderer.setClearColor(0xf1f5f9, 1)

		this.previewScene = new THREE.Scene()
		this.previewCamera = new THREE.PerspectiveCamera(PREVIEW_FIELD_OF_VIEW, 1, 0.01, 200)
		this.previewCamera.position.set(0, 0.18, this.previewBaseDistance)

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
		this.previewContentGroup.add(this.previewSketchGroup)
		this.previewContentGroup.add(this.previewSketchDraftGroup)
		this.previewContentGroup.add(this.previewSolidsGroup)

		this.previewCanvas.addEventListener("pointerdown", this.handlePreviewPointerDown)
		this.previewCanvas.addEventListener("pointermove", this.handlePreviewPointerMove)
		this.previewCanvas.addEventListener("pointerup", this.handlePreviewPointerUp)
		this.previewCanvas.addEventListener("pointerleave", this.handlePreviewPointerUp)
		this.previewCanvas.addEventListener("pointercancel", this.handlePreviewPointerUp)
		this.previewCanvas.addEventListener("wheel", this.handlePreviewWheel, { passive: false })
		this.previewCanvas.addEventListener("contextmenu", (event) => event.preventDefault())
		document.addEventListener("keydown", this.handleDocumentKeyDown, true)

		this.restoreState(options?.initialState)

		if (typeof ResizeObserver === "function") {
			this.resizeObserver = new ResizeObserver(() => {
				queueFrame(() => this.updatePreviewSize())
			})
			this.resizeObserver.observe(this.previewContainer)
		} else if (typeof window !== "undefined") {
			window.addEventListener("resize", () => queueFrame(() => this.updatePreviewSize()))
		}

		queueFrame(() => this.updatePreviewSize())
	}

	public getState(): PartEditorState {
		return {
			features: structuredClone(this.features) as PartFeature[]
		}
	}

	public getViewState(): PartEditorViewState {
		return {
			sketchVisible: this.sketchVisible,
			referencePlaneVisibility: {
				Front: this.referencePlaneVisibility.Front,
				Top: this.referencePlaneVisibility.Top,
				Right: this.referencePlaneVisibility.Right
			}
		}
	}

	public enterSketchMode(): void {
		const existingDirtySketch = this.features.find((feature) => feature.type === "sketch" && feature.dirty)
		if (existingDirtySketch && existingDirtySketch.type === "sketch") {
			this.selectedSketchId = existingDirtySketch.id
			this.selectedExtrudeId = null
			this.selectedReferencePlane = SKETCH_PLANE_TO_REFERENCE_PLANE[existingDirtySketch.target.plane]
			this.activeTool = "sketch"
			this.activeSketchTool = this.activeSketchTool ?? "line"
			this.pendingLineStart = null
			this.pendingRectangleStart = null
			this.sketchHoverPoint = null
			this.focusReferencePlaneForSketch(this.selectedReferencePlane)
			this.drawSketch()
			this.updateStatus()
			this.updateControls()
			return
		}

		const selectedPlane = this.selectedReferencePlane ?? this.getFirstVisibleReferencePlane()
		if (!selectedPlane) {
			return
		}

		const sketchId = `sketch-${this.features.filter((feature) => feature.type === "sketch").length + 1}-${createId()}`
		const nextSketch = materializeSketch({
			type: "sketch",
			id: sketchId,
			name: this.getNextSketchName(),
			dirty: true,
			target: {
				type: "plane",
				plane: REFERENCE_PLANE_TO_SKETCH_PLANE[selectedPlane]
			},
			entities: [],
			vertices: [],
			loops: [],
			profiles: []
		})

		this.features = [...this.features, nextSketch]
		this.selectedReferencePlane = selectedPlane
		this.selectedSketchId = nextSketch.id
		this.selectedExtrudeId = null
		this.selectedFaceId = null
		this.activeTool = "sketch"
		this.activeSketchTool = "line"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.focusReferencePlaneForSketch(selectedPlane)
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	public selectReferencePlane(planeName: ReferencePlaneName): void {
		if (!this.referencePlaneVisibility[planeName]) {
			return
		}
		if (this.getEditableSketch()) {
			return
		}
		this.selectedReferencePlane = planeName
		this.selectedSketchId = null
		this.selectedExtrudeId = null
		this.selectedFaceId = null
		this.activeTool = "view"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.refreshReferencePlaneStyles()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.drawPreview()
	}

	public selectSketch(sketchId: string): void {
		const sketch = this.features.find((feature) => feature.type === "sketch" && feature.id === sketchId)
		if (!sketch || sketch.type !== "sketch") {
			return
		}

		this.selectedSketchId = sketch.id
		this.selectedExtrudeId = null
		this.selectedFaceId = null
		this.selectedReferencePlane = SKETCH_PLANE_TO_REFERENCE_PLANE[sketch.target.plane]
		this.activeTool = sketch.dirty ? "sketch" : "view"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		if (sketch.dirty) {
			this.focusReferencePlaneForSketch(this.selectedReferencePlane)
		}
		this.refreshReferencePlaneStyles()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.drawPreview()
	}

	public setReferencePlaneVisible(planeName: ReferencePlaneName, visible: boolean): void {
		if (this.referencePlaneVisibility[planeName] === visible) {
			return
		}
		this.referencePlaneVisibility[planeName] = visible
		if (!visible && this.selectedReferencePlane === planeName) {
			this.selectedReferencePlane = this.getFirstVisibleReferencePlane()
		}
		this.refreshReferencePlaneStyles()
		this.updateControls()
		this.drawPreview()
		this.emitStateChange()
	}

	public setSketchVisible(visible: boolean): void {
		if (this.sketchVisible === visible) {
			return
		}
		this.sketchVisible = visible
		this.syncPreviewGeometry()
		this.updateControls()
		this.drawPreview()
		this.emitStateChange()
	}

	public listSketches(): SketchListEntry[] {
		return this.features
			.filter((feature): feature is Sketch => feature.type === "sketch")
			.map((sketch) => ({
				id: sketch.id,
				name: sketch.name?.trim() || "Sketch",
				plane: SKETCH_PLANE_TO_REFERENCE_PLANE[sketch.target.plane],
				dirty: sketch.dirty
			}))
	}

	public listExtrudes(): ExtrudeListEntry[] {
		return this.features
			.filter((feature): feature is SolidExtrude => feature.type === "extrude")
			.map((extrude, index) => ({
				id: extrude.id,
				name: extrude.name?.trim() || `Extrude ${index + 1}`,
				depth: extrude.depth
			}))
	}

	public selectExtrude(extrudeId: string): void {
		const extrude = this.features.find((feature) => feature.type === "extrude" && feature.id === extrudeId)
		if (!extrude || extrude.type !== "extrude") {
			return
		}
		this.selectExtrudeFace(extrude.id)
	}

	private selectExtrudeFace(extrudeId: string, faceId?: string): void {
		const extrude = this.features.find((feature) => feature.type === "extrude" && feature.id === extrudeId)
		if (!extrude || extrude.type !== "extrude") {
			return
		}
		const targetSketch = this.features.find((feature) => feature.type === "sketch" && feature.id === extrude.target.sketchId)
		this.selectedExtrudeId = extrude.id
		this.selectedFaceId = faceId ?? null
		this.selectedSketchId = null
		this.selectedReferencePlane = targetSketch?.type === "sketch" ? SKETCH_PLANE_TO_REFERENCE_PLANE[targetSketch.target.plane] : this.selectedReferencePlane
		this.activeTool = "view"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.heightInput.value = String(extrude.depth)
		this.refreshReferencePlaneStyles()
		this.updateStatus()
		this.updateControls()
		this.drawPreview()
	}

	public getSketchName(sketchId?: string): string {
		const sketch = this.resolveSketch(sketchId)
		return sketch?.name?.trim() || "Sketch"
	}

	public getExtrudeName(extrudeId?: string): string {
		const extrude = this.resolveExtrude(extrudeId)
		return extrude?.name?.trim() || "Extrude"
	}

	public setSketchName(name: string, sketchId?: string): void {
		const trimmed = name.trim()
		if (!trimmed) {
			return
		}
		const sketch = this.resolveSketch(sketchId)
		if (!sketch || sketch.name === trimmed) {
			return
		}
		this.features = this.features.map((feature) => {
			if (feature.type !== "sketch" || feature.id !== sketch.id) {
				return feature
			}
			return {
				...feature,
				name: trimmed
			}
		})
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.emitStateChange()
	}

	public deleteSketch(sketchId?: string): void {
		const sketch = this.resolveSketch(sketchId)
		if (!sketch) {
			return
		}
		const deletedExtrudeIds = new Set(
			this.features.filter((feature): feature is SolidExtrude => feature.type === "extrude" && feature.target.sketchId === sketch.id).map((feature) => feature.id)
		)
		this.features = this.features.filter((feature) => {
			if (feature.type === "sketch") {
				return feature.id !== sketch.id
			}
			return feature.target.sketchId !== sketch.id
		})
		if (this.selectedSketchId === sketch.id) {
			this.selectedSketchId = null
			this.activeTool = "view"
		}
		if (this.selectedExtrudeId && deletedExtrudeIds.has(this.selectedExtrudeId)) {
			this.selectedExtrudeId = null
			this.selectedFaceId = null
		}
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	public deleteExtrude(extrudeId?: string): void {
		const extrude = this.resolveExtrude(extrudeId)
		if (!extrude) {
			return
		}
		this.features = this.features.filter((feature) => feature.type !== "extrude" || feature.id !== extrude.id)
		if (this.selectedExtrudeId === extrude.id) {
			this.selectedExtrudeId = null
			this.selectedFaceId = null
		}
		const targetSketch = this.features.find((feature) => feature.type === "sketch" && feature.id === extrude.target.sketchId)
		if (targetSketch?.type === "sketch") {
			this.selectedReferencePlane = SKETCH_PLANE_TO_REFERENCE_PLANE[targetSketch.target.plane]
		}
		this.activeTool = "view"
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private restoreState(state?: PartEditorState): void {
		this.features = (state?.features ?? []).map((feature) => {
			if (feature.type === "sketch") {
				return materializeSketch(feature)
			}
			return { ...feature }
		})
		this.selectedSketchId = this.features.find((feature) => feature.type === "sketch" && feature.dirty)?.id ?? this.getLastSketchId()
		this.selectedExtrudeId = null
		this.selectedFaceId = null
		const selectedSketch = this.getSelectedSketch()
		this.selectedReferencePlane = selectedSketch ? SKETCH_PLANE_TO_REFERENCE_PLANE[selectedSketch.target.plane] : "Front"
		this.activeTool = selectedSketch?.dirty ? "sketch" : "view"
		this.activeSketchTool = "line"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		if (this.activeTool === "sketch" && this.selectedReferencePlane) {
			this.focusReferencePlaneForSketch(this.selectedReferencePlane)
		}
		this.warningText.textContent = this.migrationWarnings.join(" ")
		this.refreshReferencePlaneStyles()
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
	}

	private createQuickActionsSection(title: string): HTMLDivElement {
		const section = document.createElement("div")
		section.style.display = "none"
		section.style.flexDirection = "column"
		section.style.gap = "8px"

		const label = document.createElement("p")
		label.textContent = title
		label.style.margin = "0"
		label.style.fontSize = "11px"
		label.style.fontWeight = "700"
		label.style.letterSpacing = "0.08em"
		label.style.textTransform = "uppercase"
		label.style.color = "#64748b"
		section.appendChild(label)
		return section
	}

	private createQuickActionButton(actionId: PartQuickActionId, label: string, options?: { active?: boolean; disabled?: boolean }): HTMLButtonElement {
		const button = document.createElement("button")
		button.type = "button"
		button.textContent = label
		button.disabled = options?.disabled ?? false
		button.style.width = "100%"
		button.style.minHeight = "34px"
		button.style.padding = "8px 10px"
		button.style.borderRadius = "8px"
		button.style.border = "1px solid #cbd5e1"
		button.style.backgroundColor = "#ffffff"
		button.style.color = "#0f172a"
		button.style.fontSize = "13px"
		button.style.fontWeight = "600"
		button.style.cursor = button.disabled ? "not-allowed" : "pointer"

		if (options?.active) {
			button.style.backgroundColor = "#dbeafe"
			button.style.borderColor = "#60a5fa"
			button.style.color = "#1d4ed8"
		}
		if (actionId === "start-sketch" || actionId === "extrude") {
			button.style.backgroundColor = button.disabled ? "#cbd5f5" : "#2563eb"
			button.style.borderColor = button.disabled ? "#cbd5f5" : "#1d4ed8"
			button.style.color = button.disabled ? "#475569" : "#ffffff"
		}
		if (actionId === "exit-sketch") {
			button.style.backgroundColor = "#f8fafc"
			button.style.borderColor = "#cbd5e1"
		}
		if (actionId === "delete-extrude") {
			button.style.backgroundColor = "#fef2f2"
			button.style.borderColor = "#fca5a5"
			button.style.color = "#b91c1c"
		}

		button.addEventListener("click", (event) => {
			event.preventDefault()
			event.stopPropagation()
			this.handleQuickAction(actionId)
		})
		return button
	}

	private handleQuickAction(actionId: PartQuickActionId): void {
		switch (actionId) {
			case "start-sketch":
				this.enterSketchMode()
				return
			case "exit-sketch":
				this.activeTool = "view"
				this.pendingLineStart = null
				this.pendingRectangleStart = null
				this.sketchHoverPoint = null
				this.drawSketch()
				this.updateStatus()
				this.updateControls()
				return
			case "tool-line":
				this.activeSketchTool = "line"
				this.pendingRectangleStart = null
				this.pendingLineStart = null
				this.sketchHoverPoint = null
				this.drawSketch()
				this.updateControls()
				return
			case "tool-rectangle":
				this.activeSketchTool = "rectangle"
				this.pendingRectangleStart = null
				this.pendingLineStart = null
				this.sketchHoverPoint = null
				this.drawSketch()
				this.updateControls()
				return
			case "undo":
				this.handleUndo()
				return
			case "reset":
				this.handleReset()
				return
			case "finish-sketch":
				this.handleFinishSketch()
				return
			case "extrude":
				this.handleExtrude()
				return
			case "delete-extrude":
				this.handleDeleteSelectedExtrude()
				return
		}
	}

	private handleUndo(): void {
		if (this.pendingLineStart || this.pendingRectangleStart) {
			this.pendingLineStart = null
			this.pendingRectangleStart = null
			this.sketchHoverPoint = null
			this.drawSketch()
			this.updateStatus()
			this.updateControls()
			return
		}

		const sketch = this.getEditableSketch()
		if (!sketch || sketch.entities.length === 0) {
			return
		}

		this.updateSketch(sketch.id, (current) => ({
			...current,
			entities: current.entities.slice(0, -1)
		}))
	}

	private handleReset(): void {
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null

		const sketch = this.getEditableSketch()
		if (!sketch) {
			this.drawSketch()
			this.updateStatus()
			this.updateControls()
			return
		}

		this.updateSketch(sketch.id, (current) => ({
			...current,
			entities: []
		}))
	}

	private handleFinishSketch = (): void => {
		const sketch = this.getEditableSketch()
		if (!sketch || !this.canFinishSketch()) {
			return
		}

		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.features = this.features.map((feature) => {
			if (feature.type !== "sketch" || feature.id !== sketch.id) {
				return feature
			}
			return materializeSketch({
				...feature,
				dirty: false
			})
		})
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleExtrude = (): void => {
		const sketch = this.getSelectedSketch()
		if (!sketch || sketch.dirty || sketch.profiles.length !== 1) {
			return
		}
		const depth = Number.parseFloat(this.heightInput.value)
		if (!Number.isFinite(depth) || depth <= 0) {
			this.heightInput.focus()
			return
		}

		const profile = sketch.profiles[0]
		if (!profile) {
			return
		}

		const nextExtrude: SolidExtrude = {
			type: "extrude",
			id: `extrude-${this.features.filter((feature) => feature.type === "extrude").length + 1}-${createId()}`,
			name: `Extrude ${this.features.filter((feature) => feature.type === "extrude").length + 1}`,
			target: {
				type: "profileRef",
				sketchId: sketch.id,
				profileId: profile.id
			},
			depth
		}

		this.features = [...this.features, nextExtrude]
		this.selectedExtrudeId = nextExtrude.id
		this.selectedFaceId = null
		this.selectedSketchId = null
		this.activeTool = "view"
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleDeleteSelectedExtrude(): void {
		const extrude = this.getSelectedExtrude()
		if (!extrude) {
			return
		}
		const confirmed = typeof window === "undefined" || typeof window.confirm !== "function" ? true : window.confirm(`Delete extrude "${extrude.name?.trim() || "Extrude"}"?`)
		if (!confirmed) {
			return
		}
		this.deleteExtrude(extrude.id)
	}

	private handleHeightInputChange = (): void => {
		const extrude = this.getSelectedExtrude()
		if (!extrude) {
			this.updateControls()
			return
		}
		const depth = Number.parseFloat(this.heightInput.value)
		if (!Number.isFinite(depth) || depth <= 0 || depth === extrude.depth) {
			this.heightInput.value = String(extrude.depth)
			this.updateControls()
			return
		}
		this.features = this.features.map((feature) => {
			if (feature.type !== "extrude" || feature.id !== extrude.id) {
				return feature
			}
			return {
				...feature,
				depth
			}
		})
		this.heightInput.value = String(depth)
		this.syncPreviewGeometry()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private handleSketchCanvasClick = (event: MouseEvent): void => {
		const point = this.getCanvasPoint(event)
		if (!point) {
			return
		}
		this.handleSketchPoint(point)
	}

	private handleSketchCanvasHover = (event: MouseEvent): void => {
		if (this.activeTool !== "sketch") {
			return
		}
		const point = this.getCanvasPoint(event)
		this.sketchHoverPoint = point ? this.snapPoint(point) : null
		this.drawSketch()
	}

	private handlePreviewPointerDown = (event: PointerEvent): void => {
		this.previewCanvas.focus({ preventScroll: true })
		const isLeftMouseClick = event.pointerType === "mouse" ? event.button === 0 : false
		const isRightMouseClick = event.pointerType === "mouse" ? event.button === 2 : event.isPrimary && event.button === 0
		const isMiddleMouseClick = event.pointerType === "mouse" && event.button === 1

		if (isLeftMouseClick && this.activeTool === "sketch") {
			const point = this.getSelectedPlanePoint(event.clientX, event.clientY)
			if (point) {
				event.preventDefault()
				this.handleSketchPoint(point)
				return
			}
		}

		if (isLeftMouseClick && this.activeTool === "view") {
			const faceSelection = this.getExtrudeFaceAt(event.clientX, event.clientY)
			if (faceSelection) {
				event.preventDefault()
				this.selectExtrudeFace(faceSelection.extrudeId, faceSelection.faceId)
				return
			}
			const extrudeId = this.getExtrudeAt(event.clientX, event.clientY)
			if (extrudeId) {
				event.preventDefault()
				this.selectExtrude(extrudeId)
				return
			}
			const plane = this.getReferencePlaneAt(event.clientX, event.clientY)
			if (plane) {
				event.preventDefault()
				this.selectedReferencePlane = plane
				this.selectedSketchId = null
				this.selectedExtrudeId = null
				this.selectedFaceId = null
				this.refreshReferencePlaneStyles()
				this.updateControls()
				this.drawPreview()
				return
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

	private handlePreviewPointerMove = (event: PointerEvent): void => {
		if ((!this.isRotatingPreview && !this.isPanningPreview) || !this.lastRotationPointer) {
			if (this.activeTool === "sketch") {
				this.hoveredReferencePlane = null
				this.hoveredExtrudeId = null
				this.hoveredFaceId = null
				const point = this.getSelectedPlanePoint(event.clientX, event.clientY)
				this.sketchHoverPoint = point ? this.snapPoint(point) : null
				this.updatePreviewCursor()
				this.drawSketch()
				return
			}
			const hoveredFace = this.activeTool === "view" ? this.getExtrudeFaceAt(event.clientX, event.clientY) : null
			const hoveredExtrudeId = hoveredFace?.extrudeId ?? (this.activeTool === "view" ? this.getExtrudeAt(event.clientX, event.clientY) : null)
			const hoveredPlane = hoveredExtrudeId ? null : this.activeTool === "view" ? this.getReferencePlaneAt(event.clientX, event.clientY) : null
			if (hoveredPlane !== this.hoveredReferencePlane || hoveredExtrudeId !== this.hoveredExtrudeId || hoveredFace?.faceId !== this.hoveredFaceId) {
				this.hoveredReferencePlane = hoveredPlane
				this.hoveredExtrudeId = hoveredExtrudeId
				this.hoveredFaceId = hoveredFace?.faceId ?? null
				this.refreshFaceStyles()
				this.refreshSolidStyles()
				this.refreshReferencePlaneStyles()
				this.updatePreviewCursor()
				this.drawPreview()
			}
			return
		}

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

	private handlePreviewPointerUp = (event: PointerEvent): void => {
		if (this.previewCanvas.hasPointerCapture(event.pointerId)) {
			this.previewCanvas.releasePointerCapture(event.pointerId)
		}
		this.isRotatingPreview = false
		this.isPanningPreview = false
		this.reverseRotatePreview = false
		this.lastRotationPointer = null
		if (event.type === "pointerleave" || event.type === "pointercancel") {
			this.hoveredReferencePlane = null
			this.hoveredExtrudeId = null
			this.hoveredFaceId = null
			this.sketchHoverPoint = null
			this.refreshFaceStyles()
			this.refreshSolidStyles()
			this.refreshReferencePlaneStyles()
			this.drawSketch()
		}
		this.updatePreviewCursor()
	}

	private handlePreviewWheel = (event: WheelEvent): void => {
		if (event.deltaY === 0) {
			return
		}
		event.preventDefault()
		const zoomFactor = Math.exp(event.deltaY * PREVIEW_ZOOM_SENSITIVITY)
		this.previewBaseDistance = THREE.MathUtils.clamp(this.previewBaseDistance * zoomFactor, PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
		this.drawPreview()
	}

	private handleDocumentKeyDown = (event: KeyboardEvent): void => {
		if (!this.root.isConnected) {
			document.removeEventListener("keydown", this.handleDocumentKeyDown, true)
			return
		}
		if (event.key !== "Escape" && event.key !== "Esc") {
			return
		}
		if (this.activeTool !== "sketch") {
			return
		}
		if (!this.pendingLineStart && !this.pendingRectangleStart) {
			return
		}
		event.preventDefault()
		event.stopPropagation()
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.drawSketch()
		this.updateControls()
	}

	private appendEntityToSelectedSketch(entity: SketchEntity): void {
		const sketch = this.getEditableSketch()
		if (!sketch) {
			return
		}
		this.updateSketch(sketch.id, (current) => ({
			...current,
			entities: [...current.entities, entity]
		}))
	}

	private handleSketchPoint(point: Point2D): void {
		if (this.activeTool !== "sketch") {
			return
		}
		const sketch = this.getEditableSketch()
		if (!sketch || !this.activeSketchTool) {
			return
		}

		const snapped = this.snapPoint(point)
		if (this.activeSketchTool === "rectangle") {
			if (!this.pendingRectangleStart) {
				this.pendingRectangleStart = snapped
				this.drawSketch()
				this.updateControls()
				return
			}
			const nextRectangleStart = this.pendingRectangleStart
			this.pendingRectangleStart = null
			this.sketchHoverPoint = null
			const nextEntity: SketchEntity = {
				id: `entity-${createId()}`,
				type: "cornerRectangle",
				p0: clonePoint(nextRectangleStart),
				p1: clonePoint(snapped)
			}
			this.appendEntityToSelectedSketch(nextEntity)
			return
		}

		if (!this.pendingLineStart) {
			this.pendingLineStart = snapped
			this.drawSketch()
			this.updateControls()
			return
		}

		const nextEntity: SketchEntity = {
			id: `entity-${createId()}`,
			type: "line",
			p0: clonePoint(this.pendingLineStart),
			p1: clonePoint(snapped)
		}
		this.pendingLineStart = null
		this.sketchHoverPoint = null
		this.appendEntityToSelectedSketch(nextEntity)
	}

	private updateSketch(sketchId: string, updater: (sketch: Sketch) => Sketch): void {
		this.features = this.features.map((feature) => {
			if (feature.type !== "sketch" || feature.id !== sketchId) {
				return feature
			}
			return materializeSketch(updater(feature))
		})
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private getSelectedSketch(): Sketch | null {
		if (!this.selectedSketchId) {
			return null
		}
		const sketch = this.features.find((feature) => feature.type === "sketch" && feature.id === this.selectedSketchId)
		return sketch?.type === "sketch" ? sketch : null
	}

	private getSelectedExtrude(): SolidExtrude | null {
		if (!this.selectedExtrudeId) {
			return null
		}
		const extrude = this.features.find((feature) => feature.type === "extrude" && feature.id === this.selectedExtrudeId)
		return extrude?.type === "extrude" ? extrude : null
	}

	private getEditableSketch(): Sketch | null {
		const selected = this.getSelectedSketch()
		return selected?.dirty ? selected : null
	}

	private resolveSketch(sketchId?: string): Sketch | null {
		if (sketchId) {
			const sketch = this.features.find((feature) => feature.type === "sketch" && feature.id === sketchId)
			return sketch?.type === "sketch" ? sketch : null
		}
		return this.getSelectedSketch()
	}

	private resolveExtrude(extrudeId?: string): SolidExtrude | null {
		if (extrudeId) {
			const extrude = this.features.find((feature) => feature.type === "extrude" && feature.id === extrudeId)
			return extrude?.type === "extrude" ? extrude : null
		}
		return this.getSelectedExtrude()
	}

	private canFinishSketch(): boolean {
		const sketch = this.getEditableSketch()
		return !!sketch && !this.pendingLineStart && !this.pendingRectangleStart && sketch.profiles.length === 1
	}

	private canExtrude(): boolean {
		const sketch = this.getSelectedSketch()
		return !!sketch && !sketch.dirty && sketch.profiles.length === 1
	}

	private getCanvasPoint(event: MouseEvent): Point2D | null {
		const rect = this.sketchCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		return {
			x: event.clientX - rect.left,
			y: event.clientY - rect.top
		}
	}

	private getActiveAnchorPoints(): Point2D[] {
		const sketch = this.getSelectedSketch()
		if (!sketch) {
			return []
		}
		const points: Point2D[] = []
		for (const entity of sketch.entities) {
			if (entity.type === "line") {
				points.push(entity.p0, entity.p1)
				continue
			}
			points.push(entity.p0, { x: entity.p1.x, y: entity.p0.y }, entity.p1, { x: entity.p0.x, y: entity.p1.y })
		}
		return points.map(clonePoint)
	}

	private snapPoint(point: Point2D): Point2D {
		let bestPoint: Point2D | null = null
		let bestDistanceSquared = SKETCH_SNAP_DISTANCE * SKETCH_SNAP_DISTANCE
		for (const candidate of this.getActiveAnchorPoints()) {
			if (this.pendingLineStart && candidate.x === this.pendingLineStart.x && candidate.y === this.pendingLineStart.y) {
				continue
			}
			const dx = candidate.x - point.x
			const dy = candidate.y - point.y
			const distanceSquared = dx * dx + dy * dy
			if (distanceSquared <= bestDistanceSquared) {
				bestDistanceSquared = distanceSquared
				bestPoint = candidate
			}
		}
		return bestPoint ?? point
	}

	private drawSketch(): void {
		this.syncSketchDraftPreview()
		this.sketchCtx.clearRect(0, 0, SKETCH_CANVAS_SIZE, SKETCH_CANVAS_SIZE)
		this.sketchCtx.fillStyle = "rgba(59,130,246,0.05)"
		this.sketchCtx.fillRect(0, 0, SKETCH_CANVAS_SIZE, SKETCH_CANVAS_SIZE)

		this.sketchCtx.strokeStyle = "rgba(15,23,42,0.2)"
		this.sketchCtx.lineWidth = 1
		this.sketchCtx.beginPath()
		this.sketchCtx.moveTo(SKETCH_CANVAS_SIZE / 2, 0)
		this.sketchCtx.lineTo(SKETCH_CANVAS_SIZE / 2, SKETCH_CANVAS_SIZE)
		this.sketchCtx.moveTo(0, SKETCH_CANVAS_SIZE / 2)
		this.sketchCtx.lineTo(SKETCH_CANVAS_SIZE, SKETCH_CANVAS_SIZE / 2)
		this.sketchCtx.stroke()

		const sketch = this.getSelectedSketch()
		if (!sketch) {
			return
		}

		for (const entity of sketch.entities) {
			this.drawSketchEntity(entity, "#2563eb")
		}

		for (const point of this.getActiveAnchorPoints()) {
			this.drawSketchPoint(point, "#1d4ed8")
		}

		if (sketch.profiles.length === 1 && !sketch.dirty) {
			const loop = sketch.loops.find((entry) => entry.id === sketch.profiles[0]?.outerLoopId)
			if (loop) {
				const points = loop.vertexIndices.map((vertexIndex) => sketch.vertices[vertexIndex]).filter((point): point is Point2D => point !== undefined)
				if (points.length >= 3) {
					this.sketchCtx.fillStyle = "rgba(59,130,246,0.2)"
					this.sketchCtx.beginPath()
					this.sketchCtx.moveTo(points[0]?.x ?? 0, points[0]?.y ?? 0)
					for (const point of points.slice(1)) {
						this.sketchCtx.lineTo(point.x, point.y)
					}
					this.sketchCtx.closePath()
					this.sketchCtx.fill()
				}
			}
		}

		if (this.pendingRectangleStart && this.sketchHoverPoint) {
			this.sketchCtx.setLineDash([4, 4])
			this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
			this.sketchCtx.strokeRect(
				Math.min(this.pendingRectangleStart.x, this.sketchHoverPoint.x),
				Math.min(this.pendingRectangleStart.y, this.sketchHoverPoint.y),
				Math.abs(this.sketchHoverPoint.x - this.pendingRectangleStart.x),
				Math.abs(this.sketchHoverPoint.y - this.pendingRectangleStart.y)
			)
			this.sketchCtx.setLineDash([])
		}

		if (this.pendingLineStart) {
			this.drawSketchPoint(this.pendingLineStart, "#f59e0b")
			if (this.sketchHoverPoint) {
				this.sketchCtx.setLineDash([4, 4])
				this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
				this.sketchCtx.beginPath()
				this.sketchCtx.moveTo(this.pendingLineStart.x, this.pendingLineStart.y)
				this.sketchCtx.lineTo(this.sketchHoverPoint.x, this.sketchHoverPoint.y)
				this.sketchCtx.stroke()
				this.sketchCtx.setLineDash([])
			}
		}

		this.drawPreview()
	}

	private drawSketchEntity(entity: SketchEntity, color: string): void {
		this.sketchCtx.strokeStyle = color
		this.sketchCtx.lineWidth = 2
		if (entity.type === "line") {
			this.sketchCtx.beginPath()
			this.sketchCtx.moveTo(entity.p0.x, entity.p0.y)
			this.sketchCtx.lineTo(entity.p1.x, entity.p1.y)
			this.sketchCtx.stroke()
			return
		}
		this.sketchCtx.strokeRect(entity.p0.x, entity.p0.y, entity.p1.x - entity.p0.x, entity.p1.y - entity.p0.y)
	}

	private drawSketchPoint(point: Point2D, color: string): void {
		this.sketchCtx.fillStyle = color
		this.sketchCtx.beginPath()
		this.sketchCtx.arc(point.x, point.y, 5, 0, Math.PI * 2)
		this.sketchCtx.fill()
		this.sketchCtx.strokeStyle = "#ffffff"
		this.sketchCtx.lineWidth = 1.5
		this.sketchCtx.beginPath()
		this.sketchCtx.arc(point.x, point.y, 5, 0, Math.PI * 2)
		this.sketchCtx.stroke()
	}

	private updateStatus(): void {
		const extrude = this.getSelectedExtrude()
		if (extrude) {
			const extrudeLabel = extrude.name?.trim() || "Extrude"
			const selectedFaceLabel = this.selectedFaceId ? this.getPreviewFaceLabel(extrude.id, this.selectedFaceId) : null
			if (selectedFaceLabel) {
				this.statusText.textContent = `${selectedFaceLabel} selected.`
				this.summaryText.textContent = `On ${extrudeLabel}. Depth ${extrude.depth.toFixed(1)}.`
				return
			}
			this.statusText.textContent = `Extrude selected. Depth ${extrude.depth.toFixed(1)}.`
			this.summaryText.textContent = `Editing ${extrudeLabel}.`
			return
		}

		const sketch = this.getSelectedSketch()
		const extrudeCount = this.features.filter((feature) => feature.type === "extrude").length
		if (!sketch) {
			this.statusText.textContent = this.selectedReferencePlane ? `Plane selected: ${this.selectedReferencePlane}.` : "Select a reference plane to start."
			this.summaryText.textContent = extrudeCount > 0 ? `${extrudeCount} extrude${extrudeCount === 1 ? "" : "s"} in the part.` : ""
			return
		}

		const entityCount = sketch.entities.length
		const profileCount = sketch.profiles.length
		const sketchState = sketch.dirty ? "Sketch open" : "Sketch finished"
		this.statusText.textContent = `${sketchState}. ${entityCount} entit${entityCount === 1 ? "y" : "ies"}. ${profileCount} profile${profileCount === 1 ? "" : "s"}.`
		this.summaryText.textContent = extrudeCount > 0 ? `${extrudeCount} extrude${extrudeCount === 1 ? "" : "s"} in the part.` : ""
	}

	private updateControls(): void {
		const selectedExtrude = this.getSelectedExtrude()
		const selectedPlaneVisible = !!this.selectedReferencePlane && this.referencePlaneVisibility[this.selectedReferencePlane]
		const sketch = this.getSelectedSketch()
		const model = derivePartQuickActionsModel({
			activeTool: this.activeTool,
			selectedExtrudeLabel: selectedExtrude?.name?.trim() || (selectedExtrude ? "Extrude" : null),
			selectedPlaneLabel: this.selectedReferencePlane,
			selectedPlaneVisible,
			activeSketchTool: this.activeSketchTool,
			canUndo: !!this.getEditableSketch() && (!!this.pendingLineStart || !!this.pendingRectangleStart || (sketch?.entities.length ?? 0) > 0),
			canReset: !!this.getEditableSketch() && (!!this.pendingLineStart || !!this.pendingRectangleStart || (sketch?.entities.length ?? 0) > 0),
			canFinishSketch: this.canFinishSketch(),
			canExtrude: this.canExtrude()
		})

		this.quickActionsRail.style.display = model.visible ? "flex" : "none"
		this.quickActionsTitle.textContent = model.title
		this.quickActionsDescription.textContent = model.description
		this.quickActionsPrimaryActions.replaceChildren(
			...model.primaryActions.map((action) => this.createQuickActionButton(action.id, action.label, { active: action.active, disabled: action.disabled }))
		)
		this.quickActionsSketchToolsSection.style.display = model.sketchToolActions.length > 0 ? "flex" : "none"
		this.quickActionsSketchToolsActions.replaceChildren(
			...model.sketchToolActions.map((action) => this.createQuickActionButton(action.id, action.label, { active: action.active, disabled: action.disabled }))
		)
		this.quickActionsCommandSection.style.display = model.commandActions.length > 0 ? "flex" : "none"
		this.quickActionsCommandActions.replaceChildren(
			...model.commandActions.map((action) => this.createQuickActionButton(action.id, action.label, { active: action.active, disabled: action.disabled }))
		)
		this.quickActionsHeightSection.style.display = model.showHeightInput ? "flex" : "none"
		this.quickActionsStatusSection.style.display = model.showStatus ? "flex" : "none"
		this.heightInput.disabled = !selectedExtrude && !this.canExtrude()
		this.sketchPanel.style.display = "none"
		this.refreshSolidStyles()
		this.refreshReferencePlaneStyles()
		this.updatePreviewCursor()
		queueFrame(() => this.drawPreview())
	}

	private updatePreviewSize(): void {
		const rect = this.previewCanvas.getBoundingClientRect()
		const width = Math.max(0, Math.floor(rect.width || this.previewContainer.clientWidth || 960))
		const height = Math.max(0, Math.floor(rect.height || this.previewContainer.clientHeight || 640))
		if (width === 0 || height === 0) {
			return
		}
		this.previewRenderer.setPixelRatio(Math.min(2, getDevicePixelRatio()))
		this.previewRenderer.setSize(width, height, false)
		this.previewCamera.aspect = width / height
		this.previewCamera.updateProjectionMatrix()
		this.drawPreview()
	}

	private drawPreview(): void {
		this.syncPreviewView()
		this.previewRenderer.render(this.previewScene, this.previewCamera)
	}

	private getPreviewPanUnitsPerPixel(): Point2D {
		const rect = this.previewCanvas.getBoundingClientRect()
		const width = Math.max(1, rect.width || this.previewContainer.clientWidth || 960)
		const height = Math.max(1, rect.height || this.previewContainer.clientHeight || 640)
		const distance = Math.max(PREVIEW_MIN_CAMERA_DISTANCE, Math.abs(this.previewBaseDistance))
		const verticalFovRadians = THREE.MathUtils.degToRad(this.previewCamera.fov)
		const visibleHeight = 2 * distance * Math.tan(verticalFovRadians / 2)
		return {
			x: (visibleHeight * this.previewCamera.aspect) / width,
			y: visibleHeight / height
		}
	}

	private getReferencePlaneAt(clientX: number, clientY: number): ReferencePlaneName | null {
		return this.getReferencePlaneIntersection(clientX, clientY)?.name ?? null
	}

	private getExtrudeFaceAt(clientX: number, clientY: number): { extrudeId: string; faceId: string } | null {
		if (!this.setPreviewRaycaster(clientX, clientY)) {
			return null
		}
		const intersections = this.previewRaycaster.intersectObjects(
			this.previewSolids.flatMap((solid) => solid.faces.map((face) => face.mesh)),
			false
		)
		const mesh = intersections[0]?.object
		if (!(mesh instanceof THREE.Mesh)) {
			return null
		}
		for (const solid of this.previewSolids) {
			const face = solid.faces.find((entry) => entry.mesh === mesh)
			if (face) {
				return {
					extrudeId: face.extrudeId,
					faceId: face.faceId
				}
			}
		}
		return null
	}

	private getExtrudeAt(clientX: number, clientY: number): string | null {
		if (!this.setPreviewRaycaster(clientX, clientY)) {
			return null
		}
		const intersections = this.previewRaycaster.intersectObjects(
			this.previewSolids.filter((solid) => solid.mesh.visible).map((solid) => solid.mesh),
			false
		)
		const mesh = intersections[0]?.object
		if (!(mesh instanceof THREE.Mesh)) {
			return null
		}
		return this.previewSolids.find((solid) => solid.mesh === mesh)?.extrudeId ?? null
	}

	private getSelectedPlanePoint(clientX: number, clientY: number): Point2D | null {
		const selectedPlane = this.selectedReferencePlane
		if (!selectedPlane || !this.referencePlaneVisibility[selectedPlane]) {
			return null
		}
		return this.getReferencePlaneIntersection(clientX, clientY, selectedPlane)?.point ?? null
	}

	private getReferencePlaneIntersection(clientX: number, clientY: number, onlyPlane?: ReferencePlaneName): { name: ReferencePlaneName; point: Point2D } | null {
		if (!this.setPreviewRaycaster(clientX, clientY)) {
			return null
		}
		const planeVisuals = this.previewReferencePlanes.filter((plane) => plane.mesh.visible && (!onlyPlane || plane.name === onlyPlane))
		const intersections = this.previewRaycaster.intersectObjects(
			planeVisuals.map((plane) => plane.mesh),
			false
		)
		const intersection = intersections[0]
		const mesh = intersection?.object
		if (!(mesh instanceof THREE.Mesh) || !intersection) {
			return null
		}
		const plane = planeVisuals.find((entry) => entry.mesh === mesh)
		if (!plane) {
			return null
		}
		const localPoint = plane.mesh.worldToLocal(intersection.point.clone())
		return {
			name: plane.name,
			point: {
				x: localPoint.x,
				y: localPoint.y
			}
		}
	}

	private setPreviewRaycaster(clientX: number, clientY: number): boolean {
		const rect = this.previewCanvas.getBoundingClientRect()
		const width = rect.width || this.previewContainer.clientWidth || 960
		const height = rect.height || this.previewContainer.clientHeight || 640
		if (width <= 0 || height <= 0) {
			return false
		}
		this.previewPointer.x = ((clientX - rect.left) / width) * 2 - 1
		this.previewPointer.y = -((clientY - rect.top) / height) * 2 + 1
		this.syncPreviewView()
		this.previewCamera.updateMatrixWorld()
		this.previewScene.updateMatrixWorld(true)
		this.previewRaycaster.setFromCamera(this.previewPointer, this.previewCamera)
		return true
	}

	private focusReferencePlaneForSketch(planeName: ReferencePlaneName | null): void {
		if (!planeName) {
			return
		}
		switch (planeName) {
			case "Top":
				this.previewRotation.yaw = 0
				this.previewRotation.pitch = Math.PI / 2 - 0.01
				break
			case "Right":
				this.previewRotation.yaw = -Math.PI / 2
				this.previewRotation.pitch = 0
				break
			default:
				this.previewRotation.yaw = 0
				this.previewRotation.pitch = 0
				break
		}
		this.previewPan.x = 0
		this.previewPan.y = 0
		this.previewBaseDistance = Math.min(this.previewBaseDistance, 28)
	}

	private syncPreviewGeometry(): void {
		for (const solid of this.previewSolids) {
			this.previewSolidsGroup.remove(solid.mesh)
			this.previewSolidsGroup.remove(solid.edges)
			for (const face of solid.faces) {
				this.previewSolidsGroup.remove(face.mesh)
				face.mesh.geometry.dispose()
				disposeMaterial(face.mesh.material)
			}
			solid.mesh.geometry.dispose()
			solid.edges.geometry.dispose()
			disposeMaterial(solid.mesh.material)
			disposeMaterial(solid.edges.material)
		}
		this.previewSolids.length = 0

		while (this.previewSketchGroup.children.length > 0) {
			const child = this.previewSketchGroup.children[0]
			if (!child) {
				break
			}
			this.previewSketchGroup.remove(child)
			disposeObject3D(child)
		}

		if (this.sketchVisible) {
			for (const sketch of this.features.filter((feature): feature is Sketch => feature.type === "sketch")) {
				const sketchVisual = this.createSketchVisual(sketch)
				if (sketchVisual) {
					this.previewSketchGroup.add(sketchVisual)
				}
			}
		}

		const partState: PartProjectItemData = {
			features: structuredClone(this.features) as PartFeature[]
		}
		for (const extrude of this.features.filter((feature): feature is SolidExtrude => feature.type === "extrude")) {
			try {
				const extrusion = extrudeSolidFeature(partState, extrude)
				const visual = this.createExtrudedVisual(extrusion, extrude.id)
				this.previewSolids.push(visual)
				this.previewSolidsGroup.add(visual.mesh)
				this.previewSolidsGroup.add(visual.edges)
				for (const face of visual.faces) {
					this.previewSolidsGroup.add(face.mesh)
				}
			} catch (_error) {
				// Skip invalid extrusions during preview replay.
			}
		}

		if (this.selectedFaceId && !this.previewSolids.some((solid) => solid.faces.some((face) => face.faceId === this.selectedFaceId))) {
			this.selectedFaceId = null
		}
		if (this.hoveredFaceId && !this.previewSolids.some((solid) => solid.faces.some((face) => face.faceId === this.hoveredFaceId))) {
			this.hoveredFaceId = null
		}

		this.refreshFaceStyles()
		this.refreshSolidStyles()
		this.refreshReferencePlaneStyles()
		this.syncSketchDraftPreview()
		this.drawPreview()
	}

	private syncSketchDraftPreview(): void {
		while (this.previewSketchDraftGroup.children.length > 0) {
			const child = this.previewSketchDraftGroup.children[0]
			if (!child) {
				break
			}
			this.previewSketchDraftGroup.remove(child)
			disposeObject3D(child)
		}

		const sketch = this.getEditableSketch()
		if (!sketch) {
			return
		}

		if (this.pendingLineStart) {
			const draft = this.createDraftLineVisual(sketch.target.plane, this.pendingLineStart, this.sketchHoverPoint)
			if (draft) {
				this.previewSketchDraftGroup.add(draft)
			}
		}

		if (this.pendingRectangleStart) {
			const draft = this.createDraftRectangleVisual(sketch.target.plane, this.pendingRectangleStart, this.sketchHoverPoint)
			if (draft) {
				this.previewSketchDraftGroup.add(draft)
			}
		}
	}

	private createSketchVisual(sketch: Sketch): THREE.Object3D | null {
		if (sketch.entities.length === 0) {
			return null
		}

		const segments: number[] = []
		for (const entity of sketch.entities) {
			if (entity.type === "line") {
				const start = sketchPointToPlaneLocal(entity.p0, sketch.target.plane)
				const end = sketchPointToPlaneLocal(entity.p1, sketch.target.plane)
				segments.push(start.x, start.y, start.z, end.x, end.y, end.z)
				continue
			}
			const corners = rectangleCorners(entity)
			for (let index = 0; index < corners.length; index += 1) {
				const start = sketchPointToPlaneLocal(corners[index] ?? corners[0] ?? { x: 0, y: 0 }, sketch.target.plane)
				const end = sketchPointToPlaneLocal(corners[(index + 1) % corners.length] ?? corners[0] ?? { x: 0, y: 0 }, sketch.target.plane)
				segments.push(start.x, start.y, start.z, end.x, end.y, end.z)
			}
		}
		if (segments.length === 0) {
			return null
		}

		const geometry = new THREE.BufferGeometry()
		geometry.setAttribute("position", new THREE.Float32BufferAttribute(segments, 3))
		const material = new THREE.LineBasicMaterial({
			color: sketch.id === this.selectedSketchId ? 0xf59e0b : 0x2563eb,
			transparent: true,
			opacity: sketch.dirty ? 1 : 0.8
		})
		const lines = new THREE.LineSegments(geometry, material)
		lines.renderOrder = 5

		const group = new THREE.Group()
		group.add(lines)
		if (sketch.name) {
			const label = this.createReferenceLabelSprite(sketch.name)
			const labelPoint = sketch.vertices[0] ?? extractSketchLabelPoint(sketch)
			const localPoint = sketchPointToPlaneLocal(labelPoint, sketch.target.plane)
			label.position.set(localPoint.x, localPoint.y, localPoint.z)
			group.add(label)
		}
		return group
	}

	private createDraftLineVisual(plane: Sketch["target"]["plane"], start: Point2D, end: Point2D | null): THREE.Object3D | null {
		const group = new THREE.Group()
		const startMarker = this.createSketchPointMarker(start, plane, 0xf59e0b)
		group.add(startMarker)
		if (!end) {
			return group
		}

		const geometry = new THREE.BufferGeometry().setFromPoints([sketchPointToPlaneLocal(start, plane), sketchPointToPlaneLocal(end, plane)])
		const material = new THREE.LineBasicMaterial({
			color: 0xf59e0b,
			transparent: true,
			opacity: 0.9
		})
		group.add(new THREE.Line(geometry, material))
		group.add(this.createSketchPointMarker(end, plane, 0xf59e0b))
		return group
	}

	private createDraftRectangleVisual(plane: Sketch["target"]["plane"], start: Point2D, end: Point2D | null): THREE.Object3D | null {
		const group = new THREE.Group()
		group.add(this.createSketchPointMarker(start, plane, 0xf59e0b))
		if (!end) {
			return group
		}

		const corners = rectangleCorners({
			id: "draft-rectangle",
			type: "cornerRectangle",
			p0: start,
			p1: end
		})
		const segments: number[] = []
		for (let index = 0; index < corners.length; index += 1) {
			const segmentStart = sketchPointToPlaneLocal(corners[index] ?? corners[0] ?? { x: 0, y: 0 }, plane)
			const segmentEnd = sketchPointToPlaneLocal(corners[(index + 1) % corners.length] ?? corners[0] ?? { x: 0, y: 0 }, plane)
			segments.push(segmentStart.x, segmentStart.y, segmentStart.z, segmentEnd.x, segmentEnd.y, segmentEnd.z)
		}

		const geometry = new THREE.BufferGeometry()
		geometry.setAttribute("position", new THREE.Float32BufferAttribute(segments, 3))
		const material = new THREE.LineBasicMaterial({
			color: 0xf59e0b,
			transparent: true,
			opacity: 0.9
		})
		group.add(new THREE.LineSegments(geometry, material))
		for (const point of corners) {
			group.add(this.createSketchPointMarker(point, plane, 0xf59e0b))
		}
		return group
	}

	private createSketchPointMarker(point: Point2D, plane: Sketch["target"]["plane"], color: number): THREE.Mesh {
		const marker = new THREE.Mesh(new THREE.SphereGeometry(0.12, 16, 12), new THREE.MeshBasicMaterial({ color }))
		const localPoint = sketchPointToPlaneLocal(point, plane)
		marker.position.set(localPoint.x, localPoint.y, localPoint.z)
		return marker
	}

	private createExtrudedVisual(extrusion: ExtrudedSolid, extrudeId: string): PreviewSolidVisual {
		const outerLoop = extrusion.profileLoops[0] ?? []
		const shape = new THREE.Shape(outerLoop.map((point) => new THREE.Vector2(point.x, point.y)))
		for (const hole of extrusion.profileLoops.slice(1)) {
			shape.holes.push(new THREE.Path(hole.map((point) => new THREE.Vector2(point.x, point.y))))
		}
		const geometry = new THREE.ExtrudeGeometry(shape, {
			depth: extrusion.depth,
			bevelEnabled: false,
			steps: 1
		})
		const material = new THREE.MeshStandardMaterial({
			color: 0x3b82f6,
			roughness: 0.35,
			metalness: 0.05,
			side: THREE.DoubleSide
		})
		const mesh = new THREE.Mesh(geometry, material)
		mesh.quaternion.copy(getPlaneQuaternion(extrusion.plane))

		const edgeGeometry = new THREE.EdgesGeometry(geometry)
		const edgeMaterial = new THREE.LineBasicMaterial({
			color: 0xe2e8f0,
			transparent: true,
			opacity: 0.8
		})
		const edges = new THREE.LineSegments(edgeGeometry, edgeMaterial)
		edges.quaternion.copy(mesh.quaternion)

		const faces = createPreviewFaceVisuals(extrusion, extrudeId, mesh.quaternion)
		return {
			extrudeId,
			mesh,
			edges,
			fillMaterial: material,
			edgeMaterial,
			faces
		}
	}

	private createReferencePlanes(): THREE.Group {
		const group = new THREE.Group()
		const addPlane = (name: ReferencePlaneName, quaternion: THREE.Quaternion, labelPosition: THREE.Vector3) => {
			const fillMaterial = new THREE.MeshBasicMaterial({
				color: 0x93b4dc,
				transparent: true,
				opacity: 0.14,
				side: THREE.DoubleSide,
				depthWrite: false
			})
			const plane = new THREE.Mesh(new THREE.PlaneGeometry(REFERENCE_PLANE_SIZE, REFERENCE_PLANE_SIZE), fillMaterial)
			plane.quaternion.copy(quaternion)
			group.add(plane)

			const edgeMaterial = new THREE.LineBasicMaterial({
				color: 0x8fb3dd,
				transparent: true,
				opacity: 0.7
			})
			const edge = new THREE.LineSegments(new THREE.EdgesGeometry(plane.geometry), edgeMaterial)
			edge.quaternion.copy(quaternion)
			group.add(edge)

			const label = this.createReferenceLabelSprite(name)
			label.position.copy(labelPosition)
			group.add(label)

			this.previewReferencePlanes.push({
				name,
				mesh: plane,
				edge,
				label,
				fillMaterial,
				edgeMaterial
			})
		}

		addPlane("Front", getPlaneQuaternion("XY"), new THREE.Vector3(-REFERENCE_PLANE_SIZE / 2 + 0.16, REFERENCE_PLANE_SIZE / 2 - 0.1, 0))
		addPlane("Top", getPlaneQuaternion("XZ"), new THREE.Vector3(-0.08, 0, -REFERENCE_PLANE_SIZE / 2 + 0.18))
		addPlane("Right", getPlaneQuaternion("YZ"), new THREE.Vector3(0, REFERENCE_PLANE_SIZE / 2 - 0.1, -REFERENCE_PLANE_SIZE / 2 + 0.16))

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

	private refreshReferencePlaneStyles(): void {
		for (const plane of this.previewReferencePlanes) {
			const isVisible = this.referencePlaneVisibility[plane.name]
			plane.mesh.visible = isVisible
			plane.edge.visible = isVisible
			plane.label.visible = isVisible
			if (!isVisible) {
				continue
			}
			const isSelected = plane.name === this.selectedReferencePlane
			const isHovered = plane.name === this.hoveredReferencePlane
			if (isSelected) {
				plane.fillMaterial.color.setHex(this.activeTool === "sketch" ? 0x7dd3fc : 0xf59e0b)
				plane.fillMaterial.opacity = this.activeTool === "sketch" ? 0.16 : 0.3
				plane.edgeMaterial.color.setHex(this.activeTool === "sketch" ? 0x7dd3fc : 0xf59e0b)
				plane.edgeMaterial.opacity = 1
			} else if (isHovered) {
				plane.fillMaterial.color.setHex(0x7dd3fc)
				plane.fillMaterial.opacity = 0.12
				plane.edgeMaterial.color.setHex(0x7dd3fc)
				plane.edgeMaterial.opacity = 0.95
			} else {
				plane.fillMaterial.color.setHex(0x93b4dc)
				plane.fillMaterial.opacity = 0.14
				plane.edgeMaterial.color.setHex(0x8fb3dd)
				plane.edgeMaterial.opacity = 0.7
			}
		}
	}

	private refreshSolidStyles(): void {
		for (const solid of this.previewSolids) {
			const isSelected = solid.extrudeId === this.selectedExtrudeId
			const isHovered = solid.extrudeId === this.hoveredExtrudeId
			if (isSelected) {
				solid.fillMaterial.color.setHex(0xf59e0b)
				solid.edgeMaterial.color.setHex(0x7c2d12)
				solid.edgeMaterial.opacity = 1
				continue
			}
			if (isHovered) {
				solid.fillMaterial.color.setHex(0x60a5fa)
				solid.edgeMaterial.color.setHex(0xffffff)
				solid.edgeMaterial.opacity = 1
				continue
			}
			solid.fillMaterial.color.setHex(0x3b82f6)
			solid.edgeMaterial.color.setHex(0xe2e8f0)
			solid.edgeMaterial.opacity = 0.8
		}
	}

	private refreshFaceStyles(): void {
		for (const solid of this.previewSolids) {
			for (const face of solid.faces) {
				const isSelected = face.faceId === this.selectedFaceId
				const isHovered = face.faceId === this.hoveredFaceId
				if (isSelected) {
					face.material.color.setHex(0xf59e0b)
					face.material.opacity = 0.28
					continue
				}
				if (isHovered) {
					face.material.color.setHex(0x7dd3fc)
					face.material.opacity = 0.2
					continue
				}
				face.material.color.setHex(0xffffff)
				face.material.opacity = 0
			}
		}
	}

	private updatePreviewCursor(): void {
		if (this.isRotatingPreview || this.isPanningPreview) {
			this.previewCanvas.style.cursor = "grabbing"
			return
		}
		if (this.activeTool === "sketch") {
			this.previewCanvas.style.cursor = this.sketchHoverPoint ? "crosshair" : "grab"
			return
		}
		this.previewCanvas.style.cursor = this.hoveredFaceId || this.hoveredExtrudeId || this.hoveredReferencePlane ? "pointer" : "grab"
	}

	private getPreviewFaceLabel(extrudeId: string, faceId: string): string | null {
		return this.previewSolids.find((solid) => solid.extrudeId === extrudeId)?.faces.find((face) => face.faceId === faceId)?.label ?? null
	}

	private syncPreviewView(): void {
		this.previewCamera.position.z = THREE.MathUtils.clamp(this.previewBaseDistance, PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
		this.previewRootGroup.position.set(this.previewPan.x, this.previewPan.y, 0)
		this.previewRootGroup.rotation.set(this.previewRotation.pitch, this.previewRotation.yaw, 0)
	}

	private getFirstVisibleReferencePlane(): ReferencePlaneName | null {
		if (this.referencePlaneVisibility.Front) {
			return "Front"
		}
		if (this.referencePlaneVisibility.Top) {
			return "Top"
		}
		if (this.referencePlaneVisibility.Right) {
			return "Right"
		}
		return null
	}

	private getLastSketchId(): string | null {
		const sketches = this.features.filter((feature): feature is Sketch => feature.type === "sketch")
		return sketches[sketches.length - 1]?.id ?? null
	}

	private getNextSketchName(): string {
		return `Sketch ${this.features.filter((feature) => feature.type === "sketch").length + 1}`
	}

	private emitStateChange(): void {
		this.onStateChange?.()
	}
}

function createPreviewRenderer(canvas: HTMLCanvasElement, createRenderer?: (canvas: HTMLCanvasElement) => PreviewRendererLike): PreviewRendererLike {
	if (createRenderer) {
		return createRenderer(canvas)
	}
	try {
		return new THREE.WebGLRenderer({
			canvas,
			antialias: true,
			alpha: false
		})
	} catch (_error) {
		return createNoopPreviewRenderer()
	}
}

function createNoopPreviewRenderer(): PreviewRendererLike {
	return {
		render: () => {},
		setClearColor: () => {},
		setPixelRatio: () => {},
		setSize: () => {}
	}
}

function getPlaneQuaternion(plane: Sketch["target"]["plane"]): THREE.Quaternion {
	switch (plane) {
		case "XZ":
			return new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(new THREE.Vector3(1, 0, 0), new THREE.Vector3(0, 0, 1), new THREE.Vector3(0, -1, 0)))
		case "YZ":
			return new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, 0, 1), new THREE.Vector3(1, 0, 0)))
		default:
			return new THREE.Quaternion()
	}
}

function createPreviewFaceVisuals(extrusion: ExtrudedSolid, extrudeId: string, quaternion: THREE.Quaternion): PreviewFaceVisual[] {
	const faceVisuals: PreviewFaceVisual[] = []
	const totalSideFaces = extrusion.profileLoops.reduce((count, loop) => count + loop.length, 0)
	const bottomFace = extrusion.solid.faces[totalSideFaces]
	const topFace = extrusion.solid.faces[totalSideFaces + 1]
	let sideFaceIndex = 0
	let faceIndex = 0

	for (const loop of extrusion.profileLoops) {
		for (let pointIndex = 0; pointIndex < loop.length; pointIndex += 1) {
			const face = extrusion.solid.faces[faceIndex]
			const start = loop[pointIndex]
			const end = loop[(pointIndex + 1) % loop.length]
			if (face && start && end) {
				const geometry = createQuadFaceGeometry(
					new THREE.Vector3(start.x, start.y, 0),
					new THREE.Vector3(end.x, end.y, 0),
					new THREE.Vector3(end.x, end.y, extrusion.depth),
					new THREE.Vector3(start.x, start.y, extrusion.depth)
				)
				faceVisuals.push(createPreviewFaceVisual(extrudeId, face.id, `Side Face ${sideFaceIndex + 1}`, geometry, quaternion))
				sideFaceIndex += 1
			}
			faceIndex += 1
		}
	}

	if (bottomFace) {
		faceVisuals.push(createPreviewFaceVisual(extrudeId, bottomFace.id, "Bottom Face", createPlanarFaceGeometry(extrusion.profileLoops, 0), quaternion))
	}
	if (topFace) {
		faceVisuals.push(createPreviewFaceVisual(extrudeId, topFace.id, "Top Face", createPlanarFaceGeometry(extrusion.profileLoops, extrusion.depth), quaternion))
	}

	return faceVisuals
}

function createPreviewFaceVisual(extrudeId: string, faceId: string, label: string, geometry: THREE.BufferGeometry, quaternion: THREE.Quaternion): PreviewFaceVisual {
	const material = new THREE.MeshBasicMaterial({
		color: 0xffffff,
		transparent: true,
		opacity: 0,
		side: THREE.DoubleSide,
		depthWrite: false,
		polygonOffset: true,
		polygonOffsetFactor: -2,
		polygonOffsetUnits: -2
	})
	const mesh = new THREE.Mesh(geometry, material)
	mesh.quaternion.copy(quaternion)
	mesh.renderOrder = 8
	return {
		extrudeId,
		faceId,
		label,
		mesh,
		material
	}
}

function createPlanarFaceGeometry(profileLoops: Point2D[][], depth: number): THREE.BufferGeometry {
	const outerLoop = profileLoops[0] ?? []
	const shape = new THREE.Shape(outerLoop.map((point) => new THREE.Vector2(point.x, point.y)))
	for (const hole of profileLoops.slice(1)) {
		shape.holes.push(new THREE.Path(hole.map((point) => new THREE.Vector2(point.x, point.y))))
	}
	const geometry = new THREE.ShapeGeometry(shape)
	geometry.translate(0, 0, depth)
	return geometry
}

function createQuadFaceGeometry(a: THREE.Vector3, b: THREE.Vector3, c: THREE.Vector3, d: THREE.Vector3): THREE.BufferGeometry {
	const geometry = new THREE.BufferGeometry()
	geometry.setAttribute("position", new THREE.Float32BufferAttribute([a.x, a.y, a.z, b.x, b.y, b.z, c.x, c.y, c.z, a.x, a.y, a.z, c.x, c.y, c.z, d.x, d.y, d.z], 3))
	geometry.computeVertexNormals()
	return geometry
}

function sketchPointToPlaneLocal(point: Point2D, plane: Sketch["target"]["plane"]): THREE.Vector3 {
	switch (plane) {
		case "YZ":
			return new THREE.Vector3(0, point.x, point.y)
		case "XZ":
			return new THREE.Vector3(point.x, 0, point.y)
		default:
			return new THREE.Vector3(point.x, point.y, 0)
	}
}

function rectangleCorners(entity: Extract<SketchEntity, { type: "cornerRectangle" }>): Point2D[] {
	const minX = Math.min(entity.p0.x, entity.p1.x)
	const maxX = Math.max(entity.p0.x, entity.p1.x)
	const minY = Math.min(entity.p0.y, entity.p1.y)
	const maxY = Math.max(entity.p0.y, entity.p1.y)
	return [
		{ x: minX, y: minY },
		{ x: maxX, y: minY },
		{ x: maxX, y: maxY },
		{ x: minX, y: maxY }
	]
}

function extractSketchLabelPoint(sketch: Sketch): Point2D {
	if (sketch.entities[0]?.type === "line") {
		return sketch.entities[0].p0
	}
	if (sketch.entities[0]?.type === "cornerRectangle") {
		return sketch.entities[0].p0
	}
	return { x: 0, y: 0 }
}

function disposeMaterial(material: THREE.Material | THREE.Material[]): void {
	if (Array.isArray(material)) {
		for (const entry of material) {
			entry.dispose()
		}
		return
	}
	material.dispose()
}

function disposeObject3D(object: THREE.Object3D): void {
	if (object instanceof THREE.LineSegments || object instanceof THREE.Mesh || object instanceof THREE.Sprite) {
		if ("geometry" in object && object.geometry) {
			object.geometry.dispose()
		}
		if ("material" in object && object.material) {
			disposeMaterial(object.material as THREE.Material | THREE.Material[])
		}
	}
	for (const child of object.children) {
		disposeObject3D(child)
	}
}

function getSketchCanvasContext(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
	const context = canvas.getContext("2d")
	if (context) {
		return context
	}
	return createNoopCanvasContext2D()
}

function createNoopCanvasContext2D(): CanvasRenderingContext2D {
	const noop = () => {}
	return {
		canvas: document.createElement("canvas"),
		globalAlpha: 1,
		globalCompositeOperation: "source-over",
		drawImage: noop,
		fill: noop,
		fillRect: noop,
		clearRect: noop,
		stroke: noop,
		strokeRect: noop,
		beginPath: noop,
		moveTo: noop,
		lineTo: noop,
		closePath: noop,
		arc: noop,
		scale: noop,
		setLineDash: noop,
		save: noop,
		restore: noop,
		translate: noop,
		rotate: noop,
		resetTransform: noop,
		fillText: noop,
		measureText: () => ({ width: 0 }) as TextMetrics,
		fillStyle: "#000",
		strokeStyle: "#000",
		lineWidth: 1
	} as unknown as CanvasRenderingContext2D
}

function createId(): string {
	if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
		return crypto.randomUUID()
	}
	return Math.random().toString(36).slice(2, 10)
}

function queueFrame(callback: () => void): void {
	if (typeof requestAnimationFrame === "function") {
		requestAnimationFrame(callback)
		return
	}
	setTimeout(callback, 0)
}

function getDevicePixelRatio(): number {
	return typeof window !== "undefined" && Number.isFinite(window.devicePixelRatio) ? window.devicePixelRatio : 1
}

function clonePoint(point: Point2D): Point2D {
	return { x: point.x, y: point.y }
}
