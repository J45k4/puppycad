import { PART_PROJECT_DEFAULT_HEIGHT, PART_PROJECT_DEFAULT_PREVIEW_DISTANCE, PART_PROJECT_DEFAULT_ROTATION } from "./project-file"
import type { PartProjectExtrudedModel, PartProjectItemData, PartProjectPreviewRotation, PartProjectReferencePlaneVisibility } from "./project-file"
import { derivePartQuickActionsModel, type PartQuickActionId, type ReferencePlaneName, type SketchSurfaceKind } from "./part-quick-actions"
import { UiComponent } from "./ui"
import * as THREE from "three"

type Point2D = { x: number; y: number }

type ExtrudedModel = PartProjectExtrudedModel
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
type ReferencePlaneHandle = {
	mesh: THREE.Mesh<THREE.RingGeometry, THREE.MeshBasicMaterial>
	xSign: -1 | 1
	ySign: -1 | 1
}
type ExtrudedFaceVisual = {
	name: string
	solidMesh: THREE.Mesh
	mesh: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>
	fillMaterial: THREE.MeshBasicMaterial
	interactionPlane: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>
	interactionMaterial: THREE.MeshBasicMaterial
	width: number
	height: number
	normal: THREE.Vector3
}
type SketchSurfaceSelection = {
	kind: SketchSurfaceKind
	name: string
	mesh: THREE.Mesh
	interactionMesh: THREE.Mesh
	normal: THREE.Vector3
}
type PreviewSolidVisual = {
	mesh: THREE.Mesh
	edges: THREE.LineSegments
}

export type PartEditorState = PartProjectItemData

type PartEditorOptions = {
	initialState?: PartEditorState
	onStateChange?: () => void
}

const SKETCH_CANVAS_SIZE = 360
const REFERENCE_PLANE_SIZE = 1.9
const PREVIEW_FIELD_OF_VIEW = 60
const PREVIEW_MIN_CAMERA_DISTANCE = 0.5
const PREVIEW_MAX_CAMERA_DISTANCE = 50
const PREVIEW_ZOOM_SENSITIVITY = 0.0015
const SKETCH_SNAP_DISTANCE = 16
const SKETCH_SNAP_MARKER_SIZE = 12
const SKETCH_SELECTED_MARKER_SIZE = 14

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
	private readonly previewExtrudedFaceGroup = new THREE.Group()
	private readonly previewReferencePlanes: ReferencePlaneVisual[] = []
	private readonly previewExtrudedFaces: ExtrudedFaceVisual[] = []
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
	private referencePlaneVisibility: PartProjectReferencePlaneVisibility = {
		Front: true,
		Top: true,
		Right: true
	}
	private sketchVisible = true
	private readonly previewSolids: PreviewSolidVisual[] = []
	private sketchOverlayCommittedLine: THREE.Line | THREE.LineLoop | THREE.LineSegments | null = null
	private sketchOverlayPreviewLine: THREE.Line | THREE.LineLoop | null = null
	private sketchOverlayPoints: THREE.Points | null = null
	private sketchOverlaySnapIndicator: THREE.LineLoop | null = null
	private sketchOverlaySelectedIndicator: THREE.LineLoop | null = null
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
	private sketchPoints: Point2D[] = []
	private isSketchClosed = false
	private extrudedModels: ExtrudedModel[] = []
	private readonly previewRotation: PartProjectPreviewRotation = {
		yaw: PART_PROJECT_DEFAULT_ROTATION.yaw,
		pitch: PART_PROJECT_DEFAULT_ROTATION.pitch
	}
	private previewBaseDistance = PART_PROJECT_DEFAULT_PREVIEW_DISTANCE
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
	private hoveredExtrudedFace: THREE.Mesh | null = null
	private selectedExtrudedFace: THREE.Mesh | null = null
	private pointerOverSelectedPlane = false
	private activeResizeHandle: ReferencePlaneHandle | null = null
	private resizingReferencePlane: THREE.Mesh | null = null
	private readonly onStateChange?: () => void
	private activeTool: PartStudioTool = "view"
	private activeSketchTool: SketchTool | null = "line"
	private sketchName = "Sketch 1"
	private pendingRectangleStart: Point2D | null = null
	private pendingLineStart: Point2D | null = null
	private pendingLineStartSourceIndex: number | null = null
	private lineToolNeedsFreshStart = true
	private sketchHoverPoint: Point2D | null = null
	private sketchHoverSnapIndex: number | null = null
	private selectedSketchPointIndex: number | null = null
	private draggingSketchPointIndex: number | null = null
	private sketchBreakIndices = new Set<number>()
	private currentSketchSurfaceKey: string | null = null

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

		this.undoButton = this.createButton("Undo", this.handleUndo)
		this.resetButton = this.createButton("Reset", this.handleReset)
		this.finishButton = this.createButton("Finish Sketch", this.handleFinishSketch)

		this.heightInput = document.createElement("input")
		this.heightInput.type = "number"
		this.heightInput.min = "1"
		this.heightInput.value = String(PART_PROJECT_DEFAULT_HEIGHT)
		this.heightInput.step = "1"
		this.heightInput.style.width = "80px"
		this.heightInput.style.padding = "4px 6px"
		this.heightInput.style.border = "1px solid #cbd5f5"
		this.heightInput.style.borderRadius = "4px"
		this.heightInput.addEventListener("input", this.handleHeightInputChange)

		this.extrudeButton = this.createButton("Extrude", this.handleExtrude)

		this.statusText = document.createElement("p")
		this.statusText.style.margin = "4px 0 0"
		this.statusText.style.fontSize = "13px"
		this.statusText.style.color = "#475569"

		this.extrudeSummary = document.createElement("p")
		this.extrudeSummary.style.margin = "0"
		this.extrudeSummary.style.fontSize = "13px"
		this.extrudeSummary.style.color = "#0f172a"

		this.previewContainer = document.createElement("div")
		this.previewContainer.style.flex = "1 1 640px"
		this.previewContainer.style.width = "auto"
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
		heightLabel.style.marginBottom = "0"
		heightLabel.style.fontSize = "13px"
		heightLabel.style.color = "#0f172a"
		extrudeControls.appendChild(heightLabel)
		extrudeControls.appendChild(this.heightInput)
		this.quickActionsRail.appendChild(this.quickActionsHeightSection)

		this.quickActionsStatusSection = this.createQuickActionsSection("Status")
		this.quickActionsStatusSection.appendChild(this.statusText)
		this.quickActionsStatusSection.appendChild(this.extrudeSummary)
		this.quickActionsRail.appendChild(this.quickActionsStatusSection)

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
		this.previewCamera = new THREE.PerspectiveCamera(PREVIEW_FIELD_OF_VIEW, 1, 0.01, 50)
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
		this.previewContentGroup.add(this.previewExtrudedFaceGroup)
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
			extrudedModels: this.extrudedModels.map((model) => ({
				base: model.base.map((point) => ({ x: point.x, y: point.y })),
				height: model.height,
				scale: model.scale,
				rawHeight: model.rawHeight,
				origin: model.origin ? { x: model.origin.x, y: model.origin.y, z: model.origin.z } : undefined,
				rotation: model.rotation ? { x: model.rotation.x, y: model.rotation.y, z: model.rotation.z, w: model.rotation.w } : undefined,
				startOffset: model.startOffset
			})),
			height: heightValue,
			previewDistance: this.previewBaseDistance,
			previewRotation: {
				yaw: this.previewRotation.yaw,
				pitch: this.previewRotation.pitch
			},
			sketchVisible: this.sketchVisible,
			referencePlaneVisibility: {
				Front: this.referencePlaneVisibility.Front,
				Top: this.referencePlaneVisibility.Top,
				Right: this.referencePlaneVisibility.Right
			}
		}
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

		button.addEventListener("click", (event) => {
			event.preventDefault()
			event.stopPropagation()
			this.handleQuickAction(actionId)
		})
		return button
	}

	private handleQuickAction(actionId: PartQuickActionId) {
		switch (actionId) {
			case "start-sketch":
				this.enterSketchMode()
				return
			case "exit-sketch":
				this.setActiveTool("view")
				return
			case "tool-line":
				this.setActiveSketchTool("line")
				return
			case "tool-rectangle":
				this.setActiveSketchTool("rectangle")
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
		}
	}

	private getSelectedReferencePlaneName(): ReferencePlaneName | null {
		if (!this.selectedReferencePlane) {
			return null
		}
		return this.previewReferencePlanes.find((entry) => entry.mesh === this.selectedReferencePlane)?.name ?? null
	}

	private getSelectedExtrudedFace(): ExtrudedFaceVisual | null {
		if (!this.selectedExtrudedFace) {
			return null
		}
		return this.previewExtrudedFaces.find((entry) => entry.mesh === this.selectedExtrudedFace) ?? null
	}

	private getFacingNormalForExtrudedFace(face: ExtrudedFaceVisual): THREE.Vector3 {
		const localNormal = face.normal.clone().normalize()
		const worldNormal = localNormal.clone().applyQuaternion(face.mesh.getWorldQuaternion(new THREE.Quaternion())).normalize()
		const faceCenter = face.mesh.getWorldPosition(new THREE.Vector3())
		const toCamera = this.previewCamera.position.clone().sub(faceCenter).normalize()
		if (worldNormal.dot(toCamera) < 0) {
			localNormal.multiplyScalar(-1)
		}
		return localNormal
	}

	private getHoveredExtrudedFace(): ExtrudedFaceVisual | null {
		if (!this.hoveredExtrudedFace) {
			return null
		}
		return this.previewExtrudedFaces.find((entry) => entry.mesh === this.hoveredExtrudedFace) ?? null
	}

	private getSelectedSketchSurface(): SketchSurfaceSelection | null {
		const selectedPlaneName = this.getSelectedReferencePlaneName()
		if (selectedPlaneName && this.selectedReferencePlane) {
			return {
				kind: "reference-plane",
				name: selectedPlaneName,
				mesh: this.selectedReferencePlane,
				interactionMesh: this.selectedReferencePlane,
				normal: new THREE.Vector3(0, 0, 1).applyEuler(this.selectedReferencePlane.rotation).normalize()
			}
		}
		const selectedFace = this.getSelectedExtrudedFace()
		if (!selectedFace) {
			return null
		}
		return {
			kind: "solid-face",
			name: selectedFace.name,
			mesh: selectedFace.mesh,
			interactionMesh: selectedFace.mesh,
			normal: this.getFacingNormalForExtrudedFace(selectedFace)
		}
	}

	private getSelectedSketchSurfaceKey(): string | null {
		const surface = this.getSelectedSketchSurface()
		return surface ? `${surface.kind}:${surface.name}` : null
	}

	private getSelectedSketchSurfaceSize(): { width: number; height: number } {
		const selectedFace = this.getSelectedExtrudedFace()
		if (selectedFace) {
			return {
				width: selectedFace.width,
				height: selectedFace.height
			}
		}
		return {
			width: REFERENCE_PLANE_SIZE,
			height: REFERENCE_PLANE_SIZE
		}
	}

	private updateQuickActionsRail() {
		const selectedSurface = this.getSelectedSketchSurface()
		const model = derivePartQuickActionsModel({
			activeTool: this.activeTool,
			selectedSurfaceLabel: selectedSurface?.name ?? null,
			selectedSurfaceKind: selectedSurface?.kind ?? null,
			selectedSurfaceVisible: !!selectedSurface,
			activeSketchTool: this.activeSketchTool,
			sketchPointCount: this.sketchPoints.length,
			isSketchClosed: this.isSketchClosed,
			hasSketchBreaks: this.sketchBreakIndices.size > 0,
			hasExtrudedModel: this.extrudedModels.length > 0,
			hasPendingLineStart: this.pendingLineStart !== null
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
		requestAnimationFrame(() => this.drawPreview())
	}

	private getQuickActionsInsetPx(): number {
		if (this.quickActionsRail.style.display === "none") {
			return 0
		}
		const railRect = this.quickActionsRail.getBoundingClientRect()
		if (railRect.width <= 0) {
			return 0
		}
		const rightInset = Number.parseFloat(this.quickActionsRail.style.right)
		return railRect.width + (Number.isFinite(rightInset) ? rightInset : 0)
	}

	private getQuickActionsPreviewOffset(): Point2D {
		const insetPx = this.getQuickActionsInsetPx()
		if (insetPx <= 0) {
			return { x: 0, y: 0 }
		}
		const panUnits = this.getPreviewPanUnitsPerPixel()
		return {
			x: -(insetPx * 0.5) * panUnits.x,
			y: 0
		}
	}

	private getQuickActionsZoomFactor(): number {
		const insetPx = this.getQuickActionsInsetPx()
		if (insetPx <= 0) {
			return 1
		}
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0) {
			return 1
		}
		const visibleFraction = THREE.MathUtils.clamp((rect.width - insetPx) / rect.width, 0.45, 1)
		return (1 / visibleFraction) * 1.25
	}

	private getEffectivePreviewCameraDistance(): number {
		return THREE.MathUtils.clamp(this.previewBaseDistance * this.getQuickActionsZoomFactor(), PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
	}

	public selectReferencePlane(planeName: ReferencePlaneName): void {
		const plane = this.previewReferencePlanes.find((entry) => entry.name === planeName)?.mesh ?? null
		if (!plane || !plane.visible) {
			return
		}
		this.setSelectedReferencePlane(plane)
	}

	public setReferencePlaneVisible(planeName: ReferencePlaneName, visible: boolean): void {
		if (this.referencePlaneVisibility[planeName] === visible) {
			return
		}
		this.referencePlaneVisibility[planeName] = visible
		this.refreshReferencePlaneStyles()
		const plane = this.previewReferencePlanes.find((entry) => entry.name === planeName)
		if (plane && !visible && this.selectedReferencePlane === plane.mesh) {
			this.setSelectedReferencePlane(null)
		}
		this.updateReferencePlaneHandles()
		this.updateSketchOverlay()
		this.drawPreview()
		this.emitStateChange()
	}

	public setSketchVisible(visible: boolean): void {
		if (this.sketchVisible === visible) {
			return
		}
		this.sketchVisible = visible
		this.updateSketchOverlay()
		this.drawPreview()
		this.emitStateChange()
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
		const selectedSurface = this.getSelectedSketchSurface()
		const selectedSurfaceKey = this.getSelectedSketchSurfaceKey()
		if (selectedSurface?.kind === "solid-face" && selectedSurfaceKey) {
			this.clearSketchDraftOnly()
			this.currentSketchSurfaceKey = selectedSurfaceKey
		} else if (selectedSurfaceKey && selectedSurfaceKey !== this.currentSketchSurfaceKey) {
			this.clearSketchDraftOnly()
			this.currentSketchSurfaceKey = selectedSurfaceKey
		} else if (this.isSketchClosed) {
			this.isSketchClosed = false
		}
		this.updateStatus()
		this.updateControls()
		this.setActiveTool("sketch")
	}

	public deleteSketch(): void {
		this.sketchPoints = []
		this.sketchBreakIndices.clear()
		this.isSketchClosed = false
		this.pendingRectangleStart = null
		this.pendingLineStart = null
		this.pendingLineStartSourceIndex = null
		this.lineToolNeedsFreshStart = true
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		this.selectedSketchPointIndex = null
		this.extrudedModels = []
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
		this.pendingLineStartSourceIndex = null
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		this.selectedSketchPointIndex = null
		if (tool === "sketch" && !this.getSelectedSketchSurface()) {
			const defaultPlane = this.previewReferencePlanes[0]?.mesh ?? null
			if (defaultPlane) {
				this.setSelectedReferencePlane(defaultPlane)
			}
		}
		this.updateStudioModeLayout()
		this.updateSketchOverlay()
		this.updateQuickActionsRail()
	}

	private clearSketchDraftOnly() {
		this.sketchPoints = []
		this.sketchBreakIndices.clear()
		this.isSketchClosed = false
		this.pendingRectangleStart = null
		this.pendingLineStart = null
		this.pendingLineStartSourceIndex = null
		this.lineToolNeedsFreshStart = true
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		this.selectedSketchPointIndex = null
		this.draggingSketchPointIndex = null
		this.activeSketchTool = "line"
	}

	private handleSelectedSketchSurfaceChanged() {
		const selectedSurfaceKey = this.getSelectedSketchSurfaceKey()
		if (!selectedSurfaceKey || selectedSurfaceKey === this.currentSketchSurfaceKey) {
			return
		}
		this.clearSketchDraftOnly()
		this.currentSketchSurfaceKey = selectedSurfaceKey
		this.updateStatus()
		this.updateControls()
	}

	private setActiveSketchTool(tool: SketchTool | null) {
		if (this.activeSketchTool === tool && !this.pendingRectangleStart) {
			if (tool === "line") {
				this.pendingLineStart = null
				this.pendingLineStartSourceIndex = null
				this.lineToolNeedsFreshStart = true
				this.sketchHoverPoint = null
				this.sketchHoverSnapIndex = null
				this.selectedSketchPointIndex = null
				this.drawSketch()
				this.updateSketchOverlay()
				this.updatePreviewCursor()
				this.updateQuickActionsRail()
			}
			return
		}
		this.activeSketchTool = tool
		this.pendingRectangleStart = null
		if (tool === "line") {
			this.pendingLineStart = null
			this.pendingLineStartSourceIndex = null
			this.lineToolNeedsFreshStart = true
		} else {
			this.pendingLineStart = null
			this.pendingLineStartSourceIndex = null
		}
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		this.selectedSketchPointIndex = null
		this.drawSketch()
		this.updateSketchOverlay()
		this.updatePreviewCursor()
		this.updateQuickActionsRail()
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
		const previewDistance = state && Number.isFinite(state.previewDistance) ? state.previewDistance : PART_PROJECT_DEFAULT_PREVIEW_DISTANCE
		const rotation = state?.previewRotation ?? PART_PROJECT_DEFAULT_ROTATION
		this.sketchPoints = state?.sketchPoints?.map((point) => ({ x: point.x, y: point.y })) ?? []
		this.sketchName = state?.sketchName?.trim() ? state.sketchName.trim() : "Sketch 1"
		this.sketchBreakIndices.clear()
		this.pendingLineStart = null
		this.pendingLineStartSourceIndex = null
		this.lineToolNeedsFreshStart = true
		this.selectedSketchPointIndex = null
		this.isSketchClosed = state?.isSketchClosed ?? false
		this.extrudedModels = (state?.extrudedModels ?? []).map((model) => ({
			base: model.base.map((point) => ({ x: point.x, y: point.y })),
			height: model.height,
			scale: model.scale,
			rawHeight: model.rawHeight,
			origin: model.origin ? { x: model.origin.x, y: model.origin.y, z: model.origin.z } : undefined,
			rotation: model.rotation ? { x: model.rotation.x, y: model.rotation.y, z: model.rotation.z, w: model.rotation.w } : undefined,
			startOffset: model.startOffset
		}))
		this.sketchVisible = state?.sketchVisible ?? true
		this.referencePlaneVisibility = {
			Front: state?.referencePlaneVisibility?.Front ?? true,
			Top: state?.referencePlaneVisibility?.Top ?? true,
			Right: state?.referencePlaneVisibility?.Right ?? true
		}
		this.refreshReferencePlaneStyles()
		this.updateReferencePlaneHandles()
		this.heightInput.value = String(height)
		this.previewBaseDistance = THREE.MathUtils.clamp(previewDistance, PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
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
		this.selectedSketchPointIndex = null
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
		if (event.type === "mousemove") {
			const hoverPoint = this.getLineSnapPointIfNeeded({ x, y })
			this.sketchHoverPoint = hoverPoint
			this.drawSketch(hoverPoint ? { ...hoverPoint, active: true } : undefined)
			this.updateSketchOverlay(hoverPoint)
			return
		}
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		this.drawSketch({ x, y, active: false })
		this.updateSketchOverlay(null)
	}

	private handleUndo = () => {
		if (this.isSketchClosed) {
			return
		}
		if (this.pendingLineStart) {
			this.pendingLineStart = null
			this.pendingLineStartSourceIndex = null
			this.sketchHoverPoint = null
			this.sketchHoverSnapIndex = null
			this.selectedSketchPointIndex = null
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
		this.pendingLineStartSourceIndex = null
		this.lineToolNeedsFreshStart = true
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		this.selectedSketchPointIndex = null
		this.extrudedModels = []
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
		this.sketchHoverSnapIndex = null
		this.selectedSketchPointIndex = null
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
		this.extrudedModels.push(normalized)
		this.syncPreviewGeometry()
		this.setActiveTool("view")
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
			const selectedInteractionMesh = this.getSelectedSketchSurface()?.interactionMesh ?? null
			const clickedHandle = this.getReferenceHandleAt(event.clientX, event.clientY)
			if (clickedHandle && this.selectedReferencePlane) {
				event.preventDefault()
				this.activeResizeHandle = clickedHandle
				this.resizingReferencePlane = this.selectedReferencePlane
				this.previewCanvas.setPointerCapture(event.pointerId)
				this.previewCanvas.style.cursor = "nwse-resize"
				return
			}
			if (this.activeTool === "sketch" && selectedInteractionMesh && this.activeSketchTool === null) {
				const pointIndex = this.getSketchPointIndexAtClient(event.clientX, event.clientY)
				if (pointIndex !== null) {
					event.preventDefault()
					this.draggingSketchPointIndex = pointIndex
					this.selectedSketchPointIndex = pointIndex
					this.sketchHoverPoint = null
					this.sketchHoverSnapIndex = null
					this.previewCanvas.setPointerCapture(event.pointerId)
					this.previewCanvas.style.cursor = "move"
					return
				}
			}
			if (this.activeTool === "sketch" && selectedInteractionMesh && this.isPointInsideSelectedSketchSurface(event.clientX, event.clientY)) {
				event.preventDefault()
				if (this.activeSketchTool === null) {
					this.selectedSketchPointIndex = null
					this.updateSketchOverlay()
					this.drawSketch()
					return
				}
				this.handleSketchPlanePointInput(event.clientX, event.clientY)
				return
			}
			const clickedFace = this.getExtrudedFaceAt(event.clientX, event.clientY)
			if (this.activeTool === "view" && clickedFace) {
				event.preventDefault()
				this.setSelectedExtrudedFace(clickedFace)
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
						this.sketchHoverSnapIndex = null
						this.selectedSketchPointIndex = null
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
			if (this.selectedExtrudedFace) {
				this.setSelectedExtrudedFace(null)
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
		const selectedInteractionMesh = this.getSelectedSketchSurface()?.interactionMesh ?? null
		if (this.draggingSketchPointIndex !== null && selectedInteractionMesh) {
			event.preventDefault()
			this.sketchHoverSnapIndex = null
			const localPoint = this.getPointOnSelectedSketchSurface(event.clientX, event.clientY)
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
			if (this.activeTool === "sketch" && selectedInteractionMesh) {
				const localPoint = this.getPointOnSelectedSketchSurface(event.clientX, event.clientY)
				const clamped = localPoint ? this.clampPointToPlane(localPoint.x, localPoint.y) : null
				if (clamped) {
					this.sketchHoverPoint = this.getLineSnapPointIfNeeded(this.planeLocalToSketchPoint(clamped))
				} else {
					this.sketchHoverPoint = null
					this.sketchHoverSnapIndex = null
				}
				this.updateSketchOverlay()
			}
			this.updateReferencePlaneHover(event.clientX, event.clientY)
			return
		}
		this.setHoveredReferencePlane(null)
		this.setHoveredExtrudedFace(null)
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
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
				this.sketchHoverPoint = null
				this.sketchHoverSnapIndex = null
				this.setHoveredReferencePlane(null)
				this.updateSketchOverlay()
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
			this.sketchHoverSnapIndex = null
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
		if (this.tryDeleteSelectedSketchPointWithKey(event)) {
			return
		}
		this.tryCancelSketchToolWithEscape(event)
	}

	private handleDocumentKeyDown = (event: KeyboardEvent) => {
		if (!this.root.isConnected) {
			document.removeEventListener("keydown", this.handleDocumentKeyDown, true)
			return
		}
		if (this.tryDeleteSelectedSketchPointWithKey(event)) {
			return
		}
		this.tryCancelSketchToolWithEscape(event)
	}

	private tryDeleteSelectedSketchPointWithKey(event: KeyboardEvent): boolean {
		if (event.key !== "Delete" && event.key !== "Backspace") {
			return false
		}
		if (this.activeTool !== "sketch") {
			return false
		}
		if (this.activeSketchTool !== null) {
			return false
		}
		if (this.selectedSketchPointIndex === null) {
			return false
		}
		event.preventDefault()
		event.stopPropagation()
		this.removeSketchPointAt(this.selectedSketchPointIndex)
		return true
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
		const nextDistance = THREE.MathUtils.clamp(this.previewBaseDistance * zoomFactor, PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
		if (Math.abs(nextDistance - this.previewBaseDistance) < 0.0001) {
			return
		}
		this.previewBaseDistance = nextDistance
		this.previewCamera.position.z = this.getEffectivePreviewCameraDistance()

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
		const latestExtrudedModel = this.extrudedModels[this.extrudedModels.length - 1] ?? null
		if (latestExtrudedModel) {
			this.extrudeSummary.textContent = `Extruded height: ${latestExtrudedModel.rawHeight.toFixed(1)} units`
		} else {
			this.extrudeSummary.textContent = ""
		}
	}

	private updateControls() {
		this.finishButton.disabled = this.isSketchClosed || this.sketchPoints.length < 3 || this.sketchBreakIndices.size > 0
		this.undoButton.disabled = this.isSketchClosed || (this.sketchPoints.length === 0 && !this.pendingLineStart)
		this.resetButton.disabled = this.sketchPoints.length === 0 && this.extrudedModels.length === 0 && !this.pendingLineStart
		this.extrudeButton.disabled = !this.isSketchClosed
		this.heightInput.disabled = !this.isSketchClosed
		this.finishButton.style.backgroundColor = this.finishButton.disabled ? "#e2e8f0" : "#fff"
		this.undoButton.style.backgroundColor = this.undoButton.disabled ? "#e2e8f0" : "#fff"
		this.resetButton.style.backgroundColor = this.resetButton.disabled ? "#e2e8f0" : "#fff"
		this.extrudeButton.style.backgroundColor = this.extrudeButton.disabled ? "#cbd5f5" : "#3b82f6"
		this.extrudeButton.style.color = this.extrudeButton.disabled ? "#64748b" : "#ffffff"
		this.extrudeButton.style.border = this.extrudeButton.disabled ? "1px solid #cbd5f5" : "1px solid #1d4ed8"
		this.updateQuickActionsRail()
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

		const visibleSketchPoints = this.getVisibleSketchPoints()
		if (visibleSketchPoints.length > 0) {
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

			for (const point of visibleSketchPoints) {
				this.drawSketchPoint(point, "#1d4ed8")
			}
			if (this.selectedSketchPointIndex !== null) {
				const selectedPoint = this.sketchPoints[this.selectedSketchPointIndex]
				if (selectedPoint) {
					const size = SKETCH_SELECTED_MARKER_SIZE
					const half = size / 2
					this.sketchCtx.strokeStyle = "#f59e0b"
					this.sketchCtx.lineWidth = 2
					this.sketchCtx.strokeRect(selectedPoint.x - half, selectedPoint.y - half, size, size)
				}
			}
			if (this.sketchHoverSnapIndex !== null) {
				const snapPoint = this.sketchPoints[this.sketchHoverSnapIndex]
				if (snapPoint) {
					const size = SKETCH_SNAP_MARKER_SIZE
					const half = size / 2
					this.sketchCtx.strokeStyle = "#f59e0b"
					this.sketchCtx.lineWidth = 2
					this.sketchCtx.strokeRect(snapPoint.x - half, snapPoint.y - half, size, size)
				}
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

	private getVisibleSketchPoints(): Point2D[] {
		if (!this.pendingLineStart) {
			return [...this.sketchPoints]
		}
		const alreadyVisible = this.sketchPoints.some((point) => point.x === this.pendingLineStart?.x && point.y === this.pendingLineStart?.y)
		return alreadyVisible ? [...this.sketchPoints] : [...this.sketchPoints, this.pendingLineStart]
	}

	private getSketchOverlayPalette() {
		if (this.getSelectedSketchSurface()?.kind === "solid-face") {
			return {
				committedLine: 0xf59e0b,
				previewLine: 0x0f172a,
				points: 0xf59e0b,
				pointSize: 0.05
			}
		}
		return {
			committedLine: 0x2563eb,
			previewLine: 0x0f172a,
			points: 0x1d4ed8,
			pointSize: 0.03
		}
	}

	private normalizeSketch(height: number): ExtrudedModel | null {
		if (this.sketchPoints.length < 3 || this.sketchBreakIndices.size > 0) {
			return null
		}
		const selectedSurface = this.getSelectedSketchSurface()
		if (!selectedSurface) {
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
			rawHeight: height,
			origin: {
				x: selectedSurface.mesh.position.x,
				y: selectedSurface.mesh.position.y,
				z: selectedSurface.mesh.position.z
			},
			rotation: {
				x: selectedSurface.mesh.quaternion.x,
				y: selectedSurface.mesh.quaternion.y,
				z: selectedSurface.mesh.quaternion.z,
				w: selectedSurface.mesh.quaternion.w
			},
			startOffset: 0
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
		const distance = Math.max(PREVIEW_MIN_CAMERA_DISTANCE, Math.abs(this.getEffectivePreviewCameraDistance()))
		const verticalFovRadians = THREE.MathUtils.degToRad(this.previewCamera.fov)
		const visibleHeight = 2 * distance * Math.tan(verticalFovRadians / 2)
		const unitsPerPixelY = visibleHeight / height
		const unitsPerPixelX = (visibleHeight * this.previewCamera.aspect) / width
		return { x: unitsPerPixelX, y: unitsPerPixelY }
	}

	private drawPreview() {
		this.previewCamera.position.z = this.getEffectivePreviewCameraDistance()
		const quickActionsOffset = this.getQuickActionsPreviewOffset()
		this.previewRootGroup.position.set(this.previewPan.x + quickActionsOffset.x, this.previewPan.y + quickActionsOffset.y, 0)
		this.previewRootGroup.rotation.set(this.previewRotation.pitch, this.previewRotation.yaw, 0)
		this.previewRenderer.render(this.previewScene, this.previewCamera)
	}

	private updateReferencePlaneHover(clientX: number, clientY: number) {
		if (this.activeResizeHandle) {
			this.previewCanvas.style.cursor = "nwse-resize"
			return
		}
		if (this.activeTool === "sketch") {
			const overSelected = this.isPointInsideSelectedSketchSurface(clientX, clientY)
			this.pointerOverSelectedPlane = overSelected
			this.setHoveredReferencePlane(null)
			this.setHoveredExtrudedFace(null)
			this.updatePreviewCursor()
			return
		}
		this.pointerOverSelectedPlane = false
		if (this.getReferenceHandleAt(clientX, clientY)) {
			this.setHoveredReferencePlane(this.selectedReferencePlane)
			this.previewCanvas.style.cursor = "nwse-resize"
			return
		}
		const hoveredFace = this.getExtrudedFaceAt(clientX, clientY)
		if (hoveredFace) {
			this.setHoveredReferencePlane(null)
			this.setHoveredExtrudedFace(hoveredFace)
			return
		}
		this.setHoveredExtrudedFace(null)
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
			this.previewReferencePlanes.filter((plane) => plane.mesh.visible).map((plane) => plane.mesh),
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

	private getExtrudedFaceAt(clientX: number, clientY: number): THREE.Mesh | null {
		if (this.previewSolids.length === 0 || this.previewExtrudedFaces.length === 0) {
			return null
		}
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		this.previewPointer.x = ((clientX - rect.left) / rect.width) * 2 - 1
		this.previewPointer.y = -((clientY - rect.top) / rect.height) * 2 + 1
		this.previewRaycaster.setFromCamera(this.previewPointer, this.previewCamera)
		const intersection = this.previewRaycaster.intersectObjects(
			this.previewSolids.map((solid) => solid.mesh),
			false
		)[0]
		if (!intersection?.face) {
			return null
		}
		const hitNormal = intersection.face.normal.clone().transformDirection(intersection.object.matrixWorld).normalize()
		const hitPoint = intersection.point.clone()
		let bestFace: ExtrudedFaceVisual | null = null
		let bestScore = Number.NEGATIVE_INFINITY
		for (const face of this.previewExtrudedFaces) {
			if (face.solidMesh !== intersection.object) {
				continue
			}
			const faceNormal = face.normal.clone().applyQuaternion(this.previewRootGroup.quaternion).normalize()
			const faceOrigin = face.mesh.getWorldPosition(new THREE.Vector3())
			const plane = new THREE.Plane().setFromNormalAndCoplanarPoint(faceNormal, faceOrigin)
			const distance = Math.abs(plane.distanceToPoint(hitPoint))
			const normalScore = Math.abs(faceNormal.dot(hitNormal))
			const score = normalScore - distance * 12
			if (score > bestScore) {
				bestScore = score
				bestFace = face
			}
		}
		return bestFace?.mesh ?? null
	}

	private setHoveredExtrudedFace(mesh: THREE.Mesh | null) {
		if (this.hoveredExtrudedFace === mesh) {
			return
		}
		this.hoveredExtrudedFace = mesh
		this.refreshExtrudedFaceStyles()
		this.updatePreviewCursor()
		this.drawPreview()
	}

	private setSelectedReferencePlane(mesh: THREE.Mesh | null) {
		if (this.selectedReferencePlane === mesh) {
			return
		}
		this.selectedReferencePlane = mesh
		if (mesh) {
			this.selectedExtrudedFace = null
		}
		this.pointerOverSelectedPlane = false
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		this.selectedSketchPointIndex = null
		this.refreshReferencePlaneStyles()
		this.refreshExtrudedFaceStyles()
		this.updateReferencePlaneHandles()
		if (mesh && this.activeTool === "sketch") {
			this.handleSelectedSketchSurfaceChanged()
		}
		this.updateSketchOverlay()
		this.updatePreviewCursor()
		this.updateQuickActionsRail()
		this.drawPreview()
	}

	private setSelectedExtrudedFace(mesh: THREE.Mesh | null) {
		if (this.selectedExtrudedFace === mesh) {
			return
		}
		this.selectedExtrudedFace = mesh
		if (mesh) {
			this.selectedReferencePlane = null
		}
		this.pointerOverSelectedPlane = false
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		this.selectedSketchPointIndex = null
		this.refreshReferencePlaneStyles()
		this.refreshExtrudedFaceStyles()
		this.updateReferencePlaneHandles()
		if (mesh && this.activeTool === "sketch") {
			this.handleSelectedSketchSurfaceChanged()
		}
		this.updateSketchOverlay()
		this.updatePreviewCursor()
		this.updateQuickActionsRail()
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

	private getPointOnFaceMesh(clientX: number, clientY: number, mesh: THREE.Mesh): THREE.Vector3 | null {
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		this.previewPointer.x = ((clientX - rect.left) / rect.width) * 2 - 1
		this.previewPointer.y = -((clientY - rect.top) / rect.height) * 2 + 1
		this.previewRaycaster.setFromCamera(this.previewPointer, this.previewCamera)
		const intersection = this.previewRaycaster.intersectObject(mesh, false)[0]
		if (!intersection) {
			return null
		}
		return mesh.worldToLocal(intersection.point.clone())
	}

	private getPointOnSelectedSketchSurface(clientX: number, clientY: number): THREE.Vector3 | null {
		const selectedSurface = this.getSelectedSketchSurface()
		if (!selectedSurface) {
			return null
		}
		if (selectedSurface.kind === "solid-face") {
			return this.getPointOnFaceMesh(clientX, clientY, selectedSurface.mesh)
		}
		return this.getPointOnReferencePlane(clientX, clientY, selectedSurface.mesh)
	}

	private isPointInsideSelectedSketchSurface(clientX: number, clientY: number): boolean {
		return this.getPointOnSelectedSketchSurface(clientX, clientY) !== null
	}

	private isPointInsideReferencePlane(clientX: number, clientY: number, plane: THREE.Mesh): boolean {
		const localPoint = this.getPointOnReferencePlane(clientX, clientY, plane)
		if (!localPoint) {
			return false
		}
		const { width, height } = this.getSelectedSketchSurfaceSize()
		return Math.abs(localPoint.x) <= width / 2 && Math.abs(localPoint.y) <= height / 2
	}

	private getSketchPointIndexAtClient(clientX: number, clientY: number): number | null {
		if (this.sketchPoints.length === 0) {
			return null
		}
		const localPoint = this.getPointOnSelectedSketchSurface(clientX, clientY)
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
		const { width, height } = this.getSelectedSketchSurfaceSize()
		return {
			x: (point.x / width + 0.5) * SKETCH_CANVAS_SIZE,
			y: (0.5 - point.y / height) * SKETCH_CANVAS_SIZE
		}
	}

	private sketchPointToPlaneLocal(point: Point2D): Point2D {
		const { width, height } = this.getSelectedSketchSurfaceSize()
		return {
			x: (point.x / SKETCH_CANVAS_SIZE - 0.5) * width,
			y: (0.5 - point.y / SKETCH_CANVAS_SIZE) * height
		}
	}

	private clampPointToPlane(x: number, y: number): Point2D {
		const { width, height } = this.getSelectedSketchSurfaceSize()
		return {
			x: THREE.MathUtils.clamp(x, -width / 2, width / 2),
			y: THREE.MathUtils.clamp(y, -height / 2, height / 2)
		}
	}

	private handleSketchPlanePointInput(clientX: number, clientY: number) {
		const selectedInteractionMesh = this.getSelectedSketchSurface()?.interactionMesh ?? null
		if (!selectedInteractionMesh || this.isSketchClosed) {
			return
		}
		if (!this.activeSketchTool) {
			return
		}
		this.selectedSketchPointIndex = null
		const localPoint = this.getPointOnSelectedSketchSurface(clientX, clientY)
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

	private removeSketchPointAt(index: number) {
		if (index < 0 || index >= this.sketchPoints.length) {
			return
		}
		this.sketchPoints = this.sketchPoints.filter((_, pointIndex) => pointIndex !== index)
		this.reindexSketchBreaksAfterPointRemoval(index)
		if (this.pendingLineStartSourceIndex !== null) {
			if (this.pendingLineStartSourceIndex === index) {
				this.pendingLineStartSourceIndex = null
			} else if (this.pendingLineStartSourceIndex > index) {
				this.pendingLineStartSourceIndex -= 1
			}
		}
		if (this.selectedSketchPointIndex !== null) {
			if (this.selectedSketchPointIndex === index) {
				this.selectedSketchPointIndex = this.sketchPoints.length > 0 ? Math.min(index, this.sketchPoints.length - 1) : null
			} else if (this.selectedSketchPointIndex > index) {
				this.selectedSketchPointIndex -= 1
			}
		}
		if (this.draggingSketchPointIndex !== null) {
			if (this.draggingSketchPointIndex === index) {
				this.draggingSketchPointIndex = null
			} else if (this.draggingSketchPointIndex > index) {
				this.draggingSketchPointIndex -= 1
			}
		}
		this.sketchHoverPoint = null
		this.sketchHoverSnapIndex = null
		if (this.sketchPoints.length < 3 || this.sketchBreakIndices.size > 0) {
			this.isSketchClosed = false
		}
		this.drawSketch()
		this.updateSketchOverlay()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private reindexSketchBreaksAfterPointRemoval(removedIndex: number) {
		if (this.sketchBreakIndices.size === 0) {
			return
		}
		const nextBreaks: number[] = []
		for (const breakIndex of this.sketchBreakIndices) {
			let nextIndex = breakIndex
			if (breakIndex > removedIndex) {
				nextIndex = breakIndex - 1
			} else if (breakIndex === removedIndex) {
				nextIndex = removedIndex
			}
			if (nextIndex > 0 && nextIndex < this.sketchPoints.length) {
				nextBreaks.push(nextIndex)
			}
		}
		this.sketchBreakIndices.clear()
		for (const breakIndex of nextBreaks) {
			this.sketchBreakIndices.add(breakIndex)
		}
	}

	private handleLinePointInput(point: Point2D): boolean {
		this.sketchHoverSnapIndex = null
		const anchor = this.getCurrentLineAnchor()
		const snapTarget = this.findNearestSketchPoint(point, SKETCH_SNAP_DISTANCE, anchor?.index ?? null)
		const nextPoint = snapTarget ? { x: snapTarget.point.x, y: snapTarget.point.y } : point
		if (this.lineToolNeedsFreshStart) {
			this.pendingLineStart = nextPoint
			this.pendingLineStartSourceIndex = snapTarget?.index ?? null
			this.lineToolNeedsFreshStart = false
			return false
		}
		if (this.pendingLineStart) {
			const start = this.pendingLineStart
			this.pendingLineStart = null
			this.pendingLineStartSourceIndex = null
			if (this.sketchPoints.length > 0) {
				this.sketchBreakIndices.add(this.sketchPoints.length)
			}
			this.sketchPoints = [...this.sketchPoints, start, nextPoint]
			return true
		}
		this.sketchPoints = [...this.sketchPoints, nextPoint]
		return true
	}

	private getCurrentLineAnchor(): { point: Point2D; index: number | null } | null {
		if (this.pendingLineStart) {
			return {
				point: this.pendingLineStart,
				index: this.pendingLineStartSourceIndex
			}
		}
		if (this.lineToolNeedsFreshStart || this.sketchPoints.length === 0) {
			return null
		}
		const index = this.sketchPoints.length - 1
		const last = this.sketchPoints[index]
		if (!last) {
			return null
		}
		return {
			point: { x: last.x, y: last.y },
			index
		}
	}

	private findNearestSketchPoint(point: Point2D, maxDistancePx: number, excludeIndex?: number | null): { point: Point2D; index: number } | null {
		if (this.sketchPoints.length === 0) {
			return null
		}
		const maxDistanceSquared = maxDistancePx * maxDistancePx
		let bestIndex: number | null = null
		let bestDistanceSquared = maxDistanceSquared
		for (let index = 0; index < this.sketchPoints.length; index += 1) {
			const candidate = this.sketchPoints[index]
			if (!candidate) {
				continue
			}
			if (typeof excludeIndex === "number" && index === excludeIndex) {
				continue
			}
			const dx = candidate.x - point.x
			const dy = candidate.y - point.y
			const distanceSquared = dx * dx + dy * dy
			if (distanceSquared <= bestDistanceSquared) {
				bestDistanceSquared = distanceSquared
				bestIndex = index
			}
		}
		if (bestIndex === null) {
			return null
		}
		const bestPoint = this.sketchPoints[bestIndex]
		if (!bestPoint) {
			return null
		}
		return {
			point: { x: bestPoint.x, y: bestPoint.y },
			index: bestIndex
		}
	}

	private getLineSnapPointIfNeeded(point: Point2D): Point2D {
		if (this.activeSketchTool !== "line" || this.isSketchClosed) {
			this.sketchHoverSnapIndex = null
			return point
		}
		const anchor = this.getCurrentLineAnchor()
		const snapTarget = this.findNearestSketchPoint(point, SKETCH_SNAP_DISTANCE, anchor?.index ?? null)
		if (!snapTarget) {
			this.sketchHoverSnapIndex = null
			return point
		}
		this.sketchHoverSnapIndex = snapTarget.index
		return { x: snapTarget.point.x, y: snapTarget.point.y }
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

	private createSketchSnapIndicatorObject(point: Point2D, color: number, zOffset: number, sizePx = SKETCH_SNAP_MARKER_SIZE): THREE.LineLoop {
		const center = this.sketchPointToPlaneLocal(point)
		const { width, height } = this.getSelectedSketchSurfaceSize()
		const halfSize = (sizePx / SKETCH_CANVAS_SIZE) * Math.max(width, height) * 0.5
		const vertices = [
			center.x - halfSize,
			center.y - halfSize,
			zOffset,
			center.x + halfSize,
			center.y - halfSize,
			zOffset,
			center.x + halfSize,
			center.y + halfSize,
			zOffset,
			center.x - halfSize,
			center.y + halfSize,
			zOffset
		]
		const geometry = new THREE.BufferGeometry()
		geometry.setAttribute("position", new THREE.Float32BufferAttribute(vertices, 3))
		const material = new THREE.LineBasicMaterial({
			color,
			transparent: true,
			opacity: 1,
			depthTest: false
		})
		return new THREE.LineLoop(geometry, material)
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
		const selectedSurface = this.getSelectedSketchSurface()
		const selectedInteractionMesh = selectedSurface?.interactionMesh ?? null
		const shouldShow = this.activeTool === "sketch" && this.sketchVisible && !!selectedInteractionMesh
		if (!shouldShow || !selectedInteractionMesh) {
			this.sketchOverlayGroup.visible = false
			this.disposeSketchOverlayObject(this.sketchOverlayCommittedLine)
			this.disposeSketchOverlayObject(this.sketchOverlayPreviewLine)
			this.disposeSketchOverlayObject(this.sketchOverlayPoints)
			this.disposeSketchOverlayObject(this.sketchOverlaySnapIndicator)
			this.disposeSketchOverlayObject(this.sketchOverlaySelectedIndicator)
			this.disposeSketchOverlayObject(this.sketchOverlayLabel)
			this.sketchOverlayCommittedLine = null
			this.sketchOverlayPreviewLine = null
			this.sketchOverlayPoints = null
			this.sketchOverlaySnapIndicator = null
			this.sketchOverlaySelectedIndicator = null
			this.sketchOverlayLabel = null
			return
		}

		if (this.sketchOverlayParentPlane !== selectedInteractionMesh) {
			this.sketchOverlayGroup.removeFromParent()
			selectedInteractionMesh.add(this.sketchOverlayGroup)
			this.sketchOverlayParentPlane = selectedInteractionMesh
		}
		this.sketchOverlayGroup.visible = true
		this.sketchOverlayGroup.position.set(0, 0, 0.004)
		const palette = this.getSketchOverlayPalette()

		this.disposeSketchOverlayObject(this.sketchOverlayCommittedLine)
		if (this.sketchBreakIndices.size === 0) {
			this.sketchOverlayCommittedLine = this.createSketchLineObject(this.sketchPoints, palette.committedLine, 0, this.isSketchClosed)
		} else {
			this.sketchOverlayCommittedLine = this.createSketchSegmentObject(this.sketchPoints, palette.committedLine, 0)
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
		this.sketchOverlayPreviewLine = previewPoints ? this.createSketchLineObject(previewPoints, palette.previewLine, 0.001, this.activeSketchTool === "rectangle") : null
		if (this.sketchOverlayPreviewLine) {
			const material = this.sketchOverlayPreviewLine.material as THREE.LineBasicMaterial
			material.opacity = 0.7
			this.sketchOverlayPreviewLine.renderOrder = 7
			this.sketchOverlayGroup.add(this.sketchOverlayPreviewLine)
		}

		this.disposeSketchOverlayObject(this.sketchOverlayPoints)
		const visibleSketchPoints = this.getVisibleSketchPoints()
		if (visibleSketchPoints.length > 0) {
			const vertices = visibleSketchPoints.flatMap((point) => {
				const local = this.sketchPointToPlaneLocal(point)
				return [local.x, local.y, 0.002]
			})
			const geometry = new THREE.BufferGeometry()
			geometry.setAttribute("position", new THREE.Float32BufferAttribute(vertices, 3))
			const material = new THREE.PointsMaterial({
				color: palette.points,
				size: palette.pointSize,
				sizeAttenuation: true,
				depthTest: false
			})
			this.sketchOverlayPoints = new THREE.Points(geometry, material)
			this.sketchOverlayPoints.renderOrder = 8
			this.sketchOverlayGroup.add(this.sketchOverlayPoints)
		} else {
			this.sketchOverlayPoints = null
		}

		this.disposeSketchOverlayObject(this.sketchOverlaySnapIndicator)
		if (this.sketchHoverSnapIndex !== null) {
			const snapPoint = this.sketchPoints[this.sketchHoverSnapIndex]
			if (snapPoint) {
				this.sketchOverlaySnapIndicator = this.createSketchSnapIndicatorObject(snapPoint, 0xf59e0b, 0.003)
				this.sketchOverlaySnapIndicator.renderOrder = 9
				this.sketchOverlayGroup.add(this.sketchOverlaySnapIndicator)
			} else {
				this.sketchOverlaySnapIndicator = null
			}
		} else {
			this.sketchOverlaySnapIndicator = null
		}

		this.disposeSketchOverlayObject(this.sketchOverlaySelectedIndicator)
		if (this.selectedSketchPointIndex !== null) {
			const selectedPoint = this.sketchPoints[this.selectedSketchPointIndex]
			if (selectedPoint) {
				this.sketchOverlaySelectedIndicator = this.createSketchSnapIndicatorObject(selectedPoint, 0xf59e0b, 0.0035, SKETCH_SELECTED_MARKER_SIZE)
				this.sketchOverlaySelectedIndicator.renderOrder = 9
				this.sketchOverlayGroup.add(this.sketchOverlaySelectedIndicator)
			} else {
				this.sketchOverlaySelectedIndicator = null
			}
		} else {
			this.sketchOverlaySelectedIndicator = null
		}

		this.disposeSketchOverlayObject(this.sketchOverlayLabel)
		this.sketchOverlayLabel = this.createReferenceLabelSprite(this.sketchName)
		const { width, height } = this.getSelectedSketchSurfaceSize()
		this.sketchOverlayLabel.position.set(-width / 2 + 0.16, height / 2 - 0.08, 0.003)
		this.sketchOverlayLabel.renderOrder = 11
		this.sketchOverlayGroup.add(this.sketchOverlayLabel)

		this.drawPreview()
	}

	private refreshReferencePlaneStyles() {
		for (const plane of this.previewReferencePlanes) {
			const isVisible = this.referencePlaneVisibility[plane.name]
			plane.mesh.visible = isVisible
			plane.edge.visible = isVisible
			plane.label.visible = isVisible
			if (!isVisible) {
				continue
			}
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

	private refreshExtrudedFaceStyles() {
		for (const face of this.previewExtrudedFaces) {
			const isSelected = face.mesh === this.selectedExtrudedFace
			const isHovered = face.mesh === this.hoveredExtrudedFace
			if (isSelected) {
				face.fillMaterial.color.setHex(this.activeTool === "sketch" ? 0xffffff : 0xf59e0b)
				face.fillMaterial.opacity = this.activeTool === "sketch" ? 0.08 : 0.22
			} else if (isHovered) {
				face.fillMaterial.color.setHex(0xf59e0b)
				face.fillMaterial.opacity = 0.12
			} else {
				face.fillMaterial.color.setHex(0xffffff)
				face.fillMaterial.opacity = 0.001
			}
			face.mesh.renderOrder = isSelected ? 5 : isHovered ? 4 : 2
			face.interactionMaterial.color.setHex(this.activeTool === "sketch" ? 0xffffff : 0x7dd3fc)
			face.interactionMaterial.opacity = isSelected && this.activeTool === "sketch" ? 0.28 : 0.1
			face.interactionPlane.visible = isSelected && this.activeTool === "sketch"
		}
	}

	private updateReferencePlaneHandles() {
		if (!this.selectedReferencePlane || !this.previewReferenceGroup.visible || !this.selectedReferencePlane.visible) {
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
		this.previewCanvas.style.cursor = this.hoveredReferencePlane || this.hoveredExtrudedFace ? "pointer" : "grab"
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

	private clearExtrudedFaceVisuals() {
		for (const face of this.previewExtrudedFaces) {
			face.mesh.removeFromParent()
			face.mesh.geometry.dispose()
			face.fillMaterial.dispose()
			face.interactionPlane.removeFromParent()
			face.interactionPlane.geometry.dispose()
			face.interactionPlane.material.dispose()
		}
		this.previewExtrudedFaces.length = 0
		this.hoveredExtrudedFace = null
		this.selectedExtrudedFace = null
	}

	private createExtrudedFaceVisual(solidMesh: THREE.Mesh, name: string, vertices: THREE.Vector3[]): ExtrudedFaceVisual | null {
		if (vertices.length < 3) {
			return null
		}
		const origin = vertices[0]?.clone()
		const edgeX = origin && vertices[1] ? vertices[1].clone().sub(origin) : null
		const edgeYSource = origin && vertices[2] ? vertices[2].clone().sub(origin) : null
		if (!origin || !edgeX || !edgeYSource || edgeX.lengthSq() < 1e-6 || edgeYSource.lengthSq() < 1e-6) {
			return null
		}
		const normal = edgeX.clone().cross(edgeYSource).normalize()
		if (normal.lengthSq() < 1e-6) {
			return null
		}
		const basisX = edgeX.normalize()
		const basisY = normal.clone().cross(basisX).normalize()
		const projected = vertices.map((vertex) => {
			const relative = vertex.clone().sub(origin)
			return new THREE.Vector2(relative.dot(basisX), relative.dot(basisY))
		})
		const xs = projected.map((point) => point.x)
		const ys = projected.map((point) => point.y)
		const minX = Math.min(...xs)
		const maxX = Math.max(...xs)
		const minY = Math.min(...ys)
		const maxY = Math.max(...ys)
		const width = Math.max(maxX - minX, 0.2)
		const height = Math.max(maxY - minY, 0.2)
		const center = new THREE.Vector2((minX + maxX) / 2, (minY + maxY) / 2)
		const shape = new THREE.Shape(projected.map((point) => point.clone().sub(center)))
		const geometry = new THREE.ShapeGeometry(shape)
		const fillMaterial = new THREE.MeshBasicMaterial({
			color: 0xffffff,
			transparent: true,
			opacity: 0.001,
			side: THREE.DoubleSide,
			depthWrite: false,
			depthTest: false
		})
		const mesh = new THREE.Mesh(geometry, fillMaterial)
		mesh.position.copy(origin.clone().addScaledVector(basisX, center.x).addScaledVector(basisY, center.y))
		mesh.quaternion.setFromRotationMatrix(new THREE.Matrix4().makeBasis(basisX, basisY, normal))

		const interactionMaterial = new THREE.MeshBasicMaterial({
			color: 0xffffff,
			transparent: true,
			opacity: 0.1,
			side: THREE.DoubleSide,
			depthWrite: false,
			depthTest: false
		})
		const interactionPlane = new THREE.Mesh(new THREE.PlaneGeometry(width, height), interactionMaterial)
		interactionPlane.position.copy(mesh.position)
		interactionPlane.quaternion.copy(mesh.quaternion)
		interactionPlane.visible = false
		interactionPlane.renderOrder = 3

		this.previewExtrudedFaceGroup.add(mesh)
		this.previewExtrudedFaceGroup.add(interactionPlane)
		return {
			name,
			solidMesh,
			mesh,
			fillMaterial,
			interactionPlane,
			interactionMaterial,
			width,
			height,
			normal
		}
	}

	private syncPreviewGeometry() {
		for (const solid of this.previewSolids) {
			this.previewContentGroup.remove(solid.mesh)
			solid.mesh.geometry.dispose()
			const meshMaterial = solid.mesh.material
			if (Array.isArray(meshMaterial)) {
				for (const material of meshMaterial) {
					material.dispose()
				}
			} else {
				meshMaterial.dispose()
			}
			this.previewContentGroup.remove(solid.edges)
			solid.edges.geometry.dispose()
			const edgeMaterial = solid.edges.material
			if (Array.isArray(edgeMaterial)) {
				for (const material of edgeMaterial) {
					material.dispose()
				}
			} else {
				edgeMaterial.dispose()
			}
		}
		this.previewSolids.length = 0

		this.clearExtrudedFaceVisuals()

		if (this.extrudedModels.length === 0) {
			this.previewReferenceGroup.visible = true
			return
		}

		this.extrudedModels.forEach((model, modelIndex) => {
			if (model.base.length < 3) {
				return
			}
			const { base, height } = model
			const startOffset = typeof model.startOffset === "number" ? model.startOffset : -height / 2
			const endOffset = startOffset + height
			const shapePoints = base.map((point) => new THREE.Vector2(point.x, point.y))
			const shape = new THREE.Shape(shapePoints)
			const geometry = new THREE.ExtrudeGeometry(shape, {
				depth: height,
				bevelEnabled: false,
				steps: 1
			})
			geometry.translate(0, 0, startOffset)
			geometry.computeVertexNormals()

			const material = new THREE.MeshStandardMaterial({
				color: 0x3b82f6,
				roughness: 0.35,
				metalness: 0.05
			})
			const mesh = new THREE.Mesh(geometry, material)
			if (model.origin) {
				mesh.position.set(model.origin.x, model.origin.y, model.origin.z)
			}
			if (model.rotation) {
				mesh.quaternion.set(model.rotation.x, model.rotation.y, model.rotation.z, model.rotation.w)
			}
			this.previewContentGroup.add(mesh)

			const edgeGeometry = new THREE.EdgesGeometry(geometry)
			const edgeMaterial = new THREE.LineBasicMaterial({
				color: 0xe2e8f0,
				transparent: true,
				opacity: 0.8
			})
			const edges = new THREE.LineSegments(edgeGeometry, edgeMaterial)
			edges.position.copy(mesh.position)
			edges.quaternion.copy(mesh.quaternion)
			this.previewContentGroup.add(edges)
			this.previewSolids.push({ mesh, edges })

			const localToWorld = (point: THREE.Vector3) => point.clone().applyQuaternion(mesh.quaternion).add(mesh.position)
			const topVertices = base.map((point) => localToWorld(new THREE.Vector3(point.x, point.y, endOffset)))
			const bottomVertices = [...base].reverse().map((point) => localToWorld(new THREE.Vector3(point.x, point.y, startOffset)))
			const topFace = this.createExtrudedFaceVisual(mesh, modelIndex === 0 ? "Top Face" : `Top Face ${modelIndex + 1}`, topVertices)
			if (topFace) {
				this.previewExtrudedFaces.push(topFace)
			}
			const bottomFace = this.createExtrudedFaceVisual(mesh, modelIndex === 0 ? "Bottom Face" : `Bottom Face ${modelIndex + 1}`, bottomVertices)
			if (bottomFace) {
				this.previewExtrudedFaces.push(bottomFace)
			}
			for (let index = 0; index < base.length; index += 1) {
				const start = base[index]
				const end = base[(index + 1) % base.length]
				if (!start || !end) {
					continue
				}
				const face = this.createExtrudedFaceVisual(mesh, `Side Face ${index + 1}`, [
					localToWorld(new THREE.Vector3(start.x, start.y, startOffset)),
					localToWorld(new THREE.Vector3(end.x, end.y, startOffset)),
					localToWorld(new THREE.Vector3(end.x, end.y, endOffset)),
					localToWorld(new THREE.Vector3(start.x, start.y, endOffset))
				])
				if (face) {
					this.previewExtrudedFaces.push(face)
				}
			}
		})

		this.previewReferenceGroup.visible = false
		this.setHoveredReferencePlane(null)
		this.setSelectedReferencePlane(null)
		this.refreshExtrudedFaceStyles()
	}
}
