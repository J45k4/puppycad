import type { PartProjectItemData, PartProjectPreviewRotation, PartProjectReferencePlaneVisibility } from "../contract"
import { extrudeSolidFeature, getExtrudedFaceDescriptors, resolveSketchTargetFrame, type ExtrudedFaceDescriptor, type ExtrudedSolid, type SketchFrame3D } from "../cad/extrude"
import type { PartAction } from "../part-actions"
import type { CadCommand } from "../project-commands"
import {
	createDefaultPartRuntimeState,
	createEdgeNodeFromReference,
	createFaceNodeFromReference,
	createPartRuntimeState,
	getReferencePlaneNodeId,
	materializePartFeatures,
	serializePCadState,
	type PartRuntimeState
} from "../pcad/part-state"
import { CadEditor, collectDependentNodeIds } from "../pcad/runtime"
import { PCadNodeEditor, type PCadGeneratedSelection, type PCadGeneratedState } from "./pcad-node-editor"
import { PART_PROJECT_DEFAULT_HEIGHT, PART_PROJECT_DEFAULT_PREVIEW_DISTANCE, PART_PROJECT_DEFAULT_ROTATION } from "../project-file"
import { derivePartQuickActionsModel, type PartQuickActionId, type ReferencePlaneName } from "../part-quick-actions"
import {
	REFERENCE_PLANE_TO_SKETCH_PLANE,
	SKETCH_PLANE_TO_REFERENCE_PLANE,
	type EdgeReference,
	type FaceReference,
	type PartTreeState,
	type PCadGraphNode,
	type PartFeature,
	type Sketch,
	type SketchDimension,
	type SketchEntity,
	type Solid,
	type SolidChamfer,
	type SolidExtrude
} from "../schema"
import type { Point2D, Vector3D } from "../types"
import { UiComponent, showTextPromptModal } from "./ui"
import * as THREE from "three"

type PartStudioTool = "view" | "sketch"
type PartStudioMode = "graphic" | "nodes"
type SketchTool = "line" | "rectangle"
type RectangleSide = "top" | "right" | "bottom" | "left"

type SketchEdgeSelection =
	| {
			type: "line"
			entityId: string
	  }
	| {
			type: "rectangleSide"
			entityId: string
			side: RectangleSide
	  }

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
	frame: SketchFrame3D
}

type PreviewEdgeVisual = {
	extrudeId: string
	edgeId: string
	label: string
	line: THREE.Line
	material: THREE.LineBasicMaterial
	highlight: THREE.Mesh
	highlightMaterial: THREE.MeshBasicMaterial
}

type PreviewCornerVisual = {
	extrudeId: string
	cornerId: string
	label: string
	marker: THREE.Mesh
	material: THREE.MeshBasicMaterial
}

type PreviewSolidVisual = {
	extrudeId: string
	mesh: THREE.Mesh
	fillMaterial: THREE.MeshStandardMaterial
	edges: PreviewEdgeVisual[]
	corners: PreviewCornerVisual[]
	faces: PreviewFaceVisual[]
}

type PreviewRendererLike = Pick<THREE.WebGLRenderer, "render" | "setClearColor" | "setPixelRatio" | "setSize">

export type PartEditorState = PartProjectItemData

export type PartEditorViewState = {
	sketchVisible: boolean
	referencePlaneVisibility: PartProjectReferencePlaneVisibility
	previewRotation: PartProjectPreviewRotation
	previewPan: Vector3D
	previewOrbitPivot: Vector3D
	previewBaseDistance: number
}

type PartEditorOptions = {
	initialState?: PartEditorState
	initialViewState?: PartEditorViewState
	onStateChange?: () => void
	onViewStateChange?: (state: PartEditorViewState) => void
	onCadCommand?: (command: CadCommand, previousState: PartEditorState) => void
	createPreviewRenderer?: (canvas: HTMLCanvasElement) => PreviewRendererLike
}

type SketchListEntry = {
	id: string
	name: string
	targetLabel: string
	dirty: boolean
}

type ExtrudeListEntry = {
	id: string
	name: string
	depth: number
}

type ChamferListEntry = {
	id: string
	name: string
	d1: number
	d2?: number
}

type FeatureTreeListEntry =
	| {
			type: "sketch"
			id: string
			name: string
			dirty: boolean
	  }
	| {
			type: "extrude"
			id: string
			name: string
			depth: number
	  }
	| {
			type: "chamfer"
			id: string
			name: string
			d1: number
			d2?: number
	  }

const SKETCH_CANVAS_SIZE = 360
const REFERENCE_PLANE_SIZE = 18
const PREVIEW_FIELD_OF_VIEW = 60
const PREVIEW_MIN_CAMERA_DISTANCE = 0.5
const PREVIEW_MAX_CAMERA_DISTANCE = 50
const PREVIEW_ZOOM_SENSITIVITY = 0.0015
const SKETCH_SNAP_DISTANCE = 0.45
const SKETCH_CANVAS_PADDING = 24
const SKETCH_EDGE_HIT_TOLERANCE = 16
const PREVIEW_EDGE_HIT_TOLERANCE = 10
const PREVIEW_EDGE_HOVER_RADIUS = 0.028
const PREVIEW_EDGE_SELECTED_RADIUS = 0.04
const PREVIEW_CORNER_HIT_TOLERANCE = 12
const PREVIEW_CORNER_HOVER_RADIUS = 0.11
const PREVIEW_CORNER_SELECTED_RADIUS = 0.15
const PREVIEW_SELECTION_OCCLUSION_DEPTH_EPSILON = 0.0001
const PREVIEW_SKETCH_SURFACE_OFFSET = 0.035
const PREVIEW_SKETCH_MARKER_OFFSET = 0.05
const PREVIEW_SKETCH_LABEL_OFFSET = 0.08

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
	private readonly graphicModeButton: HTMLButtonElement
	private readonly nodeModeButton: HTMLButtonElement
	private readonly bodyContainer: HTMLDivElement
	private readonly previewContainer: HTMLDivElement
	private readonly sketchPanel: HTMLDivElement
	private readonly nodeEditorPanel: HTMLDivElement
	private readonly nodePreviewResizeHandle: HTMLDivElement
	private readonly nodeEditor: PCadNodeEditor
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
	private readonly onViewStateChange?: (state: PartEditorViewState) => void
	private readonly onCadCommand?: (command: CadCommand, previousState: PartEditorState) => void
	private readonly previewRotation: PartProjectPreviewRotation = {
		yaw: PART_PROJECT_DEFAULT_ROTATION.yaw,
		pitch: PART_PROJECT_DEFAULT_ROTATION.pitch
	}
	private readonly previewPan = new THREE.Vector3()
	private readonly previewOrbitPivot = new THREE.Vector3()
	private readonly previewPointer = new THREE.Vector2()
	private readonly previewRaycaster = new THREE.Raycaster()
	private readonly migrationWarnings: string[]
	private referencePlaneVisibility: PartProjectReferencePlaneVisibility = {
		Front: true,
		Top: true,
		Right: true
	}
	private sketchVisible = true
	private cadEditor = new CadEditor(createDefaultPartRuntimeState().cad)
	private partTreeState: Required<PartTreeState> = {
		orderedNodeIds: [],
		dirtySketchIds: []
	}
	private features: PartFeature[] = []
	private solids: Solid[] = []
	private previewBaseDistance = PART_PROJECT_DEFAULT_PREVIEW_DISTANCE
	private selectedReferencePlane: ReferencePlaneName | null = "Front"
	private selectedSketchId: string | null = null
	private selectedExtrudeId: string | null = null
	private selectedFaceId: string | null = null
	private selectedEdgeId: string | null = null
	private selectedCornerId: string | null = null
	private activeTool: PartStudioTool = "view"
	private activeSketchTool: SketchTool | null = "line"
	private selectedSketchEdge: SketchEdgeSelection | null = null
	private pendingRectangleStart: Point2D | null = null
	private pendingLineStart: Point2D | null = null
	private sketchHoverPoint: Point2D | null = null
	private isRotatingPreview = false
	private isPanningPreview = false
	private reverseRotatePreview = false
	private lastRotationPointer: { x: number; y: number } | null = null
	private resizeObserver: ResizeObserver | null = null
	private nodePreviewSplitRatio = 0.7
	private nodePreviewResizeBodyCursor = ""
	private nodePreviewResizeBodyUserSelect = ""
	private hoveredReferencePlane: ReferencePlaneName | null = null
	private hoveredExtrudeId: string | null = null
	private hoveredFaceId: string | null = null
	private hoveredEdgeId: string | null = null
	private hoveredCornerId: string | null = null
	private studioMode: PartStudioMode = "graphic"
	private selectedPCadNodeId: string | null = null
	private readonly handleNodePreviewResizeMove = (event: MouseEvent): void => {
		this.updateNodePreviewSplitFromClientX(event.clientX)
	}
	private readonly handleNodePreviewResizeEnd = (): void => {
		document.removeEventListener("mousemove", this.handleNodePreviewResizeMove)
		document.removeEventListener("mouseup", this.handleNodePreviewResizeEnd)
		document.body.style.cursor = this.nodePreviewResizeBodyCursor
		document.body.style.userSelect = this.nodePreviewResizeBodyUserSelect
	}

	public constructor(options?: PartEditorOptions) {
		super(document.createElement("div"))
		this.onStateChange = options?.onStateChange
		this.onViewStateChange = options?.onViewStateChange
		this.onCadCommand = options?.onCadCommand
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
		this.root.appendChild(header)

		const headerRow = document.createElement("div")
		headerRow.style.display = "flex"
		headerRow.style.alignItems = "center"
		headerRow.style.justifyContent = "flex-start"
		headerRow.style.gap = "12px"
		headerRow.style.flexWrap = "wrap"
		header.appendChild(headerRow)

		const title = document.createElement("h2")
		title.textContent = "Part Studio"
		title.style.margin = "0"
		title.style.fontSize = "18px"
		headerRow.appendChild(title)

		const modeSwitch = document.createElement("div")
		modeSwitch.style.display = "flex"
		modeSwitch.style.gap = "4px"
		modeSwitch.style.padding = "3px"
		modeSwitch.style.border = "1px solid #cbd5e1"
		modeSwitch.style.borderRadius = "8px"
		modeSwitch.style.background = "#f8fafc"
		headerRow.appendChild(modeSwitch)

		this.graphicModeButton = this.createModeButton("Graphic", "graphic")
		this.nodeModeButton = this.createModeButton("Nodes", "nodes")
		modeSwitch.append(this.graphicModeButton, this.nodeModeButton)

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
		this.bodyContainer = body

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
		this.sketchCanvas.addEventListener("click", this.handleSketchCanvasClick)
		this.sketchCanvas.addEventListener("mousemove", this.handleSketchCanvasHover)
		this.sketchCanvas.addEventListener("mouseleave", () => {
			this.sketchHoverPoint = null
			this.drawSketch()
		})

		this.nodeEditorPanel = document.createElement("div")
		this.nodeEditorPanel.style.flex = "1 1 620px"
		this.nodeEditorPanel.style.minWidth = "0"
		this.nodeEditorPanel.style.minHeight = "320px"
		this.nodeEditorPanel.style.display = "none"
		this.nodeEditorPanel.style.alignItems = "stretch"
		this.nodeEditorPanel.style.overflow = "hidden"
		body.appendChild(this.nodeEditorPanel)

		this.nodeEditor = new PCadNodeEditor({
			state: this.cadEditor.getState(),
			generatedState: this.getPCadGeneratedState(),
			selectedNodeId: this.getSelectedPCadNodeId(),
			onSelectNode: (nodeId) => this.selectPCadNode(nodeId),
			onSelectGenerated: (selection) => this.selectPCadGeneratedNode(selection),
			onRenameNode: (nodeId, name) => this.renamePCadNode(nodeId, name),
			onSetExtrudeDepth: (nodeId, depth) => this.setPCadExtrudeDepth(nodeId, depth),
			onSetChamferDistances: (nodeId, d1, d2) => this.setPCadChamferDistances(nodeId, d1, d2),
			onDeleteNode: (nodeId) => this.deletePCadNode(nodeId)
		})
		this.nodeEditorPanel.appendChild(this.nodeEditor.root)

		this.nodePreviewResizeHandle = document.createElement("div")
		this.nodePreviewResizeHandle.className = "part-node-preview-resize-handle"
		this.nodePreviewResizeHandle.setAttribute("role", "separator")
		this.nodePreviewResizeHandle.setAttribute("aria-orientation", "vertical")
		this.nodePreviewResizeHandle.setAttribute("aria-label", "Resize node and graphic views")
		this.nodePreviewResizeHandle.style.display = "none"
		this.nodePreviewResizeHandle.style.flex = "0 0 10px"
		this.nodePreviewResizeHandle.style.alignSelf = "stretch"
		this.nodePreviewResizeHandle.style.cursor = "col-resize"
		this.nodePreviewResizeHandle.style.borderRadius = "999px"
		this.nodePreviewResizeHandle.style.position = "relative"
		this.nodePreviewResizeHandle.style.touchAction = "none"
		this.nodePreviewResizeHandle.style.background = "transparent"
		const resizeHandleBar = document.createElement("div")
		resizeHandleBar.style.position = "absolute"
		resizeHandleBar.style.left = "4px"
		resizeHandleBar.style.top = "8px"
		resizeHandleBar.style.bottom = "8px"
		resizeHandleBar.style.width = "2px"
		resizeHandleBar.style.borderRadius = "999px"
		resizeHandleBar.style.background = "#cbd5e1"
		this.nodePreviewResizeHandle.appendChild(resizeHandleBar)
		this.nodePreviewResizeHandle.addEventListener("mousedown", this.handleNodePreviewResizeStart)
		body.appendChild(this.nodePreviewResizeHandle)

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
		if (options?.initialViewState) {
			this.applyViewState(options.initialViewState)
		}

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
			cad: serializePCadState(this.cadEditor.getState()),
			tree: {
				orderedNodeIds: [...this.partTreeState.orderedNodeIds],
				dirtySketchIds: [...this.partTreeState.dirtySketchIds]
			},
			features: structuredClone(this.features) as PartFeature[],
			...(this.solids.length > 0 ? { solids: structuredClone(this.solids) as Solid[] } : {}),
			...(this.migrationWarnings.length > 0 ? { migrationWarnings: [...this.migrationWarnings] } : {})
		}
	}

	public getViewState(): PartEditorViewState {
		return {
			sketchVisible: this.sketchVisible,
			referencePlaneVisibility: {
				Front: this.referencePlaneVisibility.Front,
				Top: this.referencePlaneVisibility.Top,
				Right: this.referencePlaneVisibility.Right
			},
			previewRotation: {
				yaw: this.previewRotation.yaw,
				pitch: this.previewRotation.pitch
			},
			previewPan: {
				x: this.previewPan.x,
				y: this.previewPan.y,
				z: this.previewPan.z
			},
			previewOrbitPivot: {
				x: this.previewOrbitPivot.x,
				y: this.previewOrbitPivot.y,
				z: this.previewOrbitPivot.z
			},
			previewBaseDistance: this.previewBaseDistance
		}
	}

	public applyViewState(state: PartEditorViewState): void {
		this.sketchVisible = state.sketchVisible
		this.referencePlaneVisibility = {
			Front: state.referencePlaneVisibility.Front,
			Top: state.referencePlaneVisibility.Top,
			Right: state.referencePlaneVisibility.Right
		}
		if (this.selectedReferencePlane && !this.referencePlaneVisibility[this.selectedReferencePlane]) {
			this.selectedReferencePlane = this.getFirstVisibleReferencePlane()
		}
		this.previewRotation.yaw = state.previewRotation.yaw
		this.previewRotation.pitch = state.previewRotation.pitch
		this.previewPan.set(state.previewPan.x, state.previewPan.y, state.previewPan.z)
		this.previewOrbitPivot.set(state.previewOrbitPivot.x, state.previewOrbitPivot.y, state.previewOrbitPivot.z)
		this.previewBaseDistance = THREE.MathUtils.clamp(state.previewBaseDistance, PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
		this.refreshReferencePlaneStyles()
		this.syncPreviewGeometry()
		this.updateControls()
		this.drawPreview()
	}

	private createModeButton(label: string, mode: PartStudioMode): HTMLButtonElement {
		const button = document.createElement("button")
		button.type = "button"
		button.textContent = label
		button.style.border = "0"
		button.style.borderRadius = "6px"
		button.style.padding = "6px 10px"
		button.style.fontSize = "13px"
		button.style.fontWeight = "700"
		button.style.cursor = "pointer"
		button.addEventListener("click", () => this.setStudioMode(mode))
		return button
	}

	private setStudioMode(mode: PartStudioMode): void {
		if (this.studioMode === mode) {
			return
		}
		this.studioMode = mode
		if (mode === "nodes") {
			this.selectedPCadNodeId = this.getSelectedPCadNodeId()
		}
		this.updateModeButtons()
		this.updateControls()
		queueFrame(() => this.updatePreviewSize())
	}

	private updateModeButtons(): void {
		const updateButton = (button: HTMLButtonElement, active: boolean) => {
			button.style.backgroundColor = active ? "#2563eb" : "transparent"
			button.style.color = active ? "#ffffff" : "#334155"
			button.style.boxShadow = active ? "0 1px 3px rgba(15,23,42,0.18)" : "none"
		}
		updateButton(this.graphicModeButton, this.studioMode === "graphic")
		updateButton(this.nodeModeButton, this.studioMode === "nodes")
	}

	private readonly handleNodePreviewResizeStart = (event: MouseEvent): void => {
		if (this.studioMode !== "nodes") {
			return
		}
		event.preventDefault()
		this.nodePreviewResizeBodyCursor = document.body.style.cursor
		this.nodePreviewResizeBodyUserSelect = document.body.style.userSelect
		document.body.style.cursor = "col-resize"
		document.body.style.userSelect = "none"
		document.addEventListener("mousemove", this.handleNodePreviewResizeMove)
		document.addEventListener("mouseup", this.handleNodePreviewResizeEnd)
		this.updateNodePreviewSplitFromClientX(event.clientX)
	}

	private updateNodePreviewSplitFromClientX(clientX: number): void {
		const rect = this.bodyContainer.getBoundingClientRect()
		if (rect.width <= 0) {
			return
		}
		const relativeX = clientX - rect.left
		this.nodePreviewSplitRatio = Math.min(0.84, Math.max(0.36, relativeX / rect.width))
		this.applyNodePreviewSplit()
		queueFrame(() => this.updatePreviewSize())
	}

	private applyNodePreviewSplit(): void {
		const nodeBasis = `${(this.nodePreviewSplitRatio * 100).toFixed(2)}%`
		const previewBasis = `${((1 - this.nodePreviewSplitRatio) * 100).toFixed(2)}%`
		this.nodeEditorPanel.style.flex = `0 1 calc(${nodeBasis} - 13px)`
		this.previewContainer.style.flex = `1 1 calc(${previewBasis} - 13px)`
		this.nodePreviewResizeHandle.setAttribute("aria-valuemin", "36")
		this.nodePreviewResizeHandle.setAttribute("aria-valuemax", "84")
		this.nodePreviewResizeHandle.setAttribute("aria-valuenow", String(Math.round(this.nodePreviewSplitRatio * 100)))
	}

	public dispatchPartAction(action: PartAction): void {
		const deletedExtrude = action.type === "deleteExtrude" ? this.resolveExtrude(action.extrudeId) : null
		const previousState = this.getState()
		const previousCadState = this.cadEditor.getState()
		const previousTreeState = this.partTreeState
		if (!this.applyCadEditorAction(action)) {
			return
		}
		if (this.cadEditor.getState() === previousCadState && this.partTreeState === previousTreeState) {
			return
		}

		this.syncDerivedPartState()
		this.syncSelectionAfterPartAction(action, deletedExtrude)
		this.selectedPCadNodeId = this.getSelectedPCadNodeId()
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
		this.onCadCommand?.(action, previousState)
	}

	private applyCadEditorAction(action: PartAction): boolean {
		try {
			switch (action.type) {
				case "createSketch":
					return this.applyCreateSketchAction(action)
				case "renameSketch":
					if (!action.name.trim() || !this.resolveSketch(action.sketchId)) {
						return false
					}
					this.cadEditor.renameNode(action.sketchId, action.name)
					return true
				case "addSketchEntity":
					if (!this.isDirtySketch(action.sketchId)) {
						return false
					}
					this.cadEditor.addSketchEntity(action.sketchId, action.entity)
					return true
				case "undoSketchEntity":
					if (!this.isDirtySketch(action.sketchId) || !this.resolveSketch(action.sketchId)?.entities.length) {
						return false
					}
					this.cadEditor.removeLastSketchEntity(action.sketchId)
					return true
				case "resetSketch":
					if (!this.isDirtySketch(action.sketchId) || !this.resolveSketch(action.sketchId)?.entities.length) {
						return false
					}
					this.cadEditor.clearSketch(action.sketchId)
					return true
				case "finishSketch": {
					const sketch = this.resolveSketch(action.sketchId)
					if (!sketch || !this.isDirtySketch(sketch.id) || sketch.profiles.length !== 1) {
						return false
					}
					this.setDirtySketch(sketch.id, false)
					return true
				}
				case "createExtrude":
					return this.applyCreateExtrudeAction(action)
				case "setExtrudeDepth": {
					const extrude = this.resolveExtrude(action.extrudeId)
					if (!extrude || !Number.isFinite(action.depth) || action.depth <= 0 || extrude.depth === action.depth) {
						return false
					}
					this.cadEditor.setExtrudeDepth(action.extrudeId, action.depth)
					return true
				}
				case "createChamfer":
					return this.applyCreateChamferAction(action)
				case "setChamferDistances":
					return this.applySetChamferDistancesAction(action)
				case "setSketchDimension":
					if (!Number.isFinite(action.dimension.value) || action.dimension.value <= 0 || !this.resolveSketch(action.sketchId)) {
						return false
					}
					this.cadEditor.setSketchDimension(action.sketchId, action.dimension)
					return true
				case "deleteSketch":
					return this.applyDeleteNodeAction(action.sketchId, "sketch")
				case "deleteExtrude":
					return this.applyDeleteNodeAction(action.extrudeId, "extrude")
			}
		} catch (_error) {
			return false
		}
	}

	private applyCreateSketchAction(action: Extract<PartAction, { type: "createSketch" }>): boolean {
		const nextName = action.name.trim()
		if (!nextName || this.cadEditor.getState().nodes.has(action.sketchId) || this.features.some((feature) => feature.type === "sketch" && feature.dirty)) {
			return false
		}

		const targetId =
			action.target.type === "plane"
				? getReferencePlaneNodeId(SKETCH_PLANE_TO_REFERENCE_PLANE[action.target.plane])
				: (() => {
						const faceNode = createFaceNodeFromReference(action.target.face)
						this.cadEditor.addFace(faceNode)
						return faceNode.id
					})()

		this.cadEditor.addSketch({
			id: action.sketchId,
			name: nextName,
			targetId
		})
		this.partTreeState = {
			...this.partTreeState,
			orderedNodeIds: [...this.partTreeState.orderedNodeIds, action.sketchId],
			dirtySketchIds: [...this.partTreeState.dirtySketchIds, action.sketchId]
		}
		return true
	}

	private applyCreateExtrudeAction(action: Extract<PartAction, { type: "createExtrude" }>): boolean {
		const nextName = action.name.trim()
		const sketch = this.resolveSketch(action.sketchId)
		if (!nextName || this.cadEditor.getState().nodes.has(action.extrudeId) || !sketch || sketch.dirty || sketch.profiles.length !== 1 || sketch.profiles[0]?.id !== action.profileId) {
			return false
		}
		if (!Number.isFinite(action.depth) || action.depth <= 0) {
			return false
		}
		this.cadEditor.extrudeSketchProfile({
			id: action.extrudeId,
			name: nextName,
			sketchId: sketch.id,
			profileId: action.profileId,
			operation: "newBody",
			depth: action.depth
		})
		this.partTreeState = {
			...this.partTreeState,
			orderedNodeIds: [...this.partTreeState.orderedNodeIds, action.extrudeId]
		}
		return true
	}

	private applyCreateChamferAction(action: Extract<PartAction, { type: "createChamfer" }>): boolean {
		const nextName = action.name.trim()
		if (
			!nextName ||
			this.cadEditor.getState().nodes.has(action.chamferId) ||
			!Number.isFinite(action.d1) ||
			action.d1 <= 0 ||
			(action.d2 !== undefined && (!Number.isFinite(action.d2) || action.d2 <= 0))
		) {
			return false
		}
		if (this.getChamferForEdge(action.target.edge)) {
			return false
		}
		const edgeNode = createEdgeNodeFromReference(action.target.edge)
		this.cadEditor.addEdge(edgeNode)
		this.cadEditor.chamferEdge({
			id: action.chamferId,
			name: nextName,
			edgeId: edgeNode.id,
			d1: action.d1,
			...(action.d2 === undefined ? {} : { d2: action.d2 })
		})
		this.partTreeState = {
			...this.partTreeState,
			orderedNodeIds: [...this.partTreeState.orderedNodeIds, action.chamferId]
		}
		return true
	}

	private applySetChamferDistancesAction(action: Extract<PartAction, { type: "setChamferDistances" }>): boolean {
		const chamfer = this.features.find((feature): feature is SolidChamfer => feature.type === "chamfer" && feature.id === action.chamferId)
		if (
			!chamfer ||
			!Number.isFinite(action.d1) ||
			action.d1 <= 0 ||
			(action.d2 !== undefined && (!Number.isFinite(action.d2) || action.d2 <= 0)) ||
			(chamfer.d1 === action.d1 && chamfer.d2 === action.d2)
		) {
			return false
		}
		this.cadEditor.setChamferDistance(action.chamferId, action.d1, action.d2)
		return true
	}

	private applyDeleteNodeAction(nodeId: string, type: "sketch" | "extrude"): boolean {
		const node = this.cadEditor.getState().nodes.get(nodeId)
		if (!node || node.type !== type) {
			return false
		}
		const deletedIds = collectDependentNodeIds(this.cadEditor.getState(), [nodeId])
		this.cadEditor.deleteNodeCascade(nodeId)
		this.partTreeState = {
			orderedNodeIds: this.partTreeState.orderedNodeIds.filter((id) => !deletedIds.has(id)),
			dirtySketchIds: this.partTreeState.dirtySketchIds.filter((id) => !deletedIds.has(id))
		}
		return true
	}

	private syncSelectionAfterPartAction(action: PartAction, deletedExtrude: SolidExtrude | null): void {
		switch (action.type) {
			case "createSketch": {
				const sketch = this.resolveSketch(action.sketchId)
				if (!sketch) {
					break
				}
				this.selectedSketchId = sketch.id
				this.selectedExtrudeId = sketch.target.type === "face" ? sketch.target.face.extrudeId : null
				this.selectedFaceId = sketch.target.type === "face" ? sketch.target.face.faceId : null
				this.selectedEdgeId = null
				this.selectedCornerId = null
				this.selectedReferencePlane = sketch.target.type === "plane" ? SKETCH_PLANE_TO_REFERENCE_PLANE[sketch.target.plane] : null
				this.activeTool = "sketch"
				this.activeSketchTool = "line"
				this.selectedSketchEdge = null
				break
			}
			case "createExtrude":
				this.selectedExtrudeId = action.extrudeId
				this.selectedFaceId = null
				this.selectedEdgeId = null
				this.selectedCornerId = null
				this.selectedSketchId = null
				this.selectedSketchEdge = null
				this.activeTool = "view"
				break
			case "deleteExtrude": {
				const targetSketch = deletedExtrude ? this.resolveSketch(deletedExtrude.target.sketchId) : null
				if (targetSketch) {
					this.selectedReferencePlane = this.getReferencePlaneForSketchTarget(targetSketch.target)
				}
				this.selectedSketchEdge = null
				this.activeTool = "view"
				break
			}
			default:
				break
		}

		if (this.selectedSketchId && !this.resolveSketch(this.selectedSketchId)) {
			this.selectedSketchId = null
			this.selectedSketchEdge = null
			this.activeTool = "view"
		}
		if (this.selectedExtrudeId && !this.resolveExtrude(this.selectedExtrudeId)) {
			this.selectedExtrudeId = null
			this.selectedFaceId = null
			this.selectedEdgeId = null
			this.selectedCornerId = null
		}
		if (this.selectedFaceId && !this.selectedExtrudeId) {
			this.selectedFaceId = null
		}
		if (this.selectedEdgeId && !this.selectedExtrudeId) {
			this.selectedEdgeId = null
		}
		if (this.selectedCornerId && !this.selectedExtrudeId) {
			this.selectedCornerId = null
		}
		if (this.selectedSketchEdge && !this.getSelectedSketchEdgeEntity()) {
			this.selectedSketchEdge = null
		}
	}

	private getDefaultSketchTool(_sketch: Sketch | null): SketchTool {
		return "line"
	}

	private static sketchTargetsEqual(left: Sketch["target"], right: Sketch["target"]): boolean {
		if (left.type !== right.type) {
			return false
		}
		if (left.type === "plane" && right.type === "plane") {
			return left.plane === right.plane
		}
		if (left.type === "face" && right.type === "face") {
			return left.face.extrudeId === right.face.extrudeId && left.face.faceId === right.face.faceId
		}
		return false
	}

	public enterSketchMode(): void {
		const selectedFace = this.getSelectedFaceReference()
		const selectedPlane = selectedFace ? null : (this.selectedReferencePlane ?? this.getFirstVisibleReferencePlane())
		const requestedTarget: Sketch["target"] | null = selectedFace
			? {
					type: "face",
					face: selectedFace
				}
			: selectedPlane
				? {
						type: "plane",
						plane: REFERENCE_PLANE_TO_SKETCH_PLANE[selectedPlane]
					}
				: null
		const existingDirtySketch = this.features.find((feature) => feature.type === "sketch" && feature.dirty)
		if (existingDirtySketch && existingDirtySketch.type === "sketch") {
			if (requestedTarget && !PartEditor.sketchTargetsEqual(existingDirtySketch.target, requestedTarget)) {
				if (!this.closeDirtySketchBeforeStartingAnother(existingDirtySketch)) {
					this.updateStatus()
					this.updateControls()
					return
				}
			} else {
				this.selectedSketchId = existingDirtySketch.id
				this.selectedExtrudeId = existingDirtySketch.target.type === "face" ? existingDirtySketch.target.face.extrudeId : null
				this.selectedFaceId = existingDirtySketch.target.type === "face" ? existingDirtySketch.target.face.faceId : null
				this.selectedEdgeId = null
				this.selectedCornerId = null
				this.selectedReferencePlane = existingDirtySketch.target.type === "plane" ? SKETCH_PLANE_TO_REFERENCE_PLANE[existingDirtySketch.target.plane] : null
				this.activeTool = "sketch"
				this.activeSketchTool = this.getDefaultSketchTool(existingDirtySketch)
				this.selectedSketchEdge = null
				this.pendingLineStart = null
				this.pendingRectangleStart = null
				this.sketchHoverPoint = null
				if (this.selectedReferencePlane) {
					this.focusReferencePlaneForSketch(this.selectedReferencePlane)
				}
				this.drawSketch()
				this.updateStatus()
				this.updateControls()
				return
			}
		}

		if (!requestedTarget) {
			return
		}

		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		if (selectedPlane) {
			this.focusReferencePlaneForSketch(selectedPlane)
		}
		this.dispatchPartAction({
			type: "createSketch",
			sketchId: `sketch-${this.features.filter((feature) => feature.type === "sketch").length + 1}-${createId()}`,
			name: this.getNextSketchName(),
			target: requestedTarget
		})
	}

	private closeDirtySketchBeforeStartingAnother(sketch: Sketch): boolean {
		if (sketch.profiles.length === 1) {
			this.dispatchPartAction({
				type: "finishSketch",
				sketchId: sketch.id
			})
			return !this.features.some((feature) => feature.type === "sketch" && feature.dirty)
		}
		if (sketch.entities.length === 0) {
			this.dispatchPartAction({
				type: "deleteSketch",
				sketchId: sketch.id
			})
			return !this.features.some((feature) => feature.type === "sketch" && feature.dirty)
		}
		return false
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
		this.selectedEdgeId = null
		this.selectedCornerId = null
		this.selectedSketchEdge = null
		this.selectedPCadNodeId = getReferencePlaneNodeId(planeName)
		this.activeTool = "view"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.syncPreviewSketchGeometry()
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
		this.selectedExtrudeId = sketch.target.type === "face" ? sketch.target.face.extrudeId : null
		this.selectedFaceId = sketch.target.type === "face" ? sketch.target.face.faceId : null
		this.selectedEdgeId = null
		this.selectedCornerId = null
		this.selectedSketchEdge = null
		this.selectedPCadNodeId = sketch.id
		this.selectedReferencePlane = sketch.target.type === "plane" ? SKETCH_PLANE_TO_REFERENCE_PLANE[sketch.target.plane] : null
		this.activeTool = "sketch"
		this.activeSketchTool = this.getDefaultSketchTool(sketch)
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		if (sketch.dirty && this.selectedReferencePlane) {
			this.focusReferencePlaneForSketch(this.selectedReferencePlane)
		}
		this.syncPreviewSketchGeometry()
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
		this.emitViewStateChange()
	}

	public setSketchVisible(visible: boolean): void {
		if (this.sketchVisible === visible) {
			return
		}
		this.sketchVisible = visible
		this.syncPreviewGeometry()
		this.updateControls()
		this.drawPreview()
		this.emitViewStateChange()
	}

	public listSketches(): SketchListEntry[] {
		return this.features
			.filter((feature): feature is Sketch => feature.type === "sketch")
			.map((sketch) => ({
				id: sketch.id,
				name: sketch.name?.trim() || "Sketch",
				targetLabel: this.getSketchTargetLabel(sketch.target),
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

	public listChamfers(): ChamferListEntry[] {
		return this.features
			.filter((feature): feature is SolidChamfer => feature.type === "chamfer")
			.map((chamfer, index) => ({
				id: chamfer.id,
				name: chamfer.name?.trim() || `Chamfer ${index + 1}`,
				d1: chamfer.d1,
				...(chamfer.d2 === undefined ? {} : { d2: chamfer.d2 })
			}))
	}

	public listFeatureTreeEntries(): FeatureTreeListEntry[] {
		return this.partTreeState.orderedNodeIds
			.map((nodeId): FeatureTreeListEntry | null => {
				const feature = this.features.find((candidate) => candidate.id === nodeId)
				if (!feature) {
					return null
				}
				if (feature.type === "sketch") {
					return {
						type: "sketch",
						id: feature.id,
						name: feature.name?.trim() || "Sketch",
						dirty: feature.dirty
					}
				}
				if (feature.type === "extrude") {
					return {
						type: "extrude",
						id: feature.id,
						name: feature.name?.trim() || "Extrude",
						depth: feature.depth
					}
				}
				return {
					type: "chamfer",
					id: feature.id,
					name: feature.name?.trim() || "Chamfer",
					d1: feature.d1,
					...(feature.d2 === undefined ? {} : { d2: feature.d2 })
				}
			})
			.filter((entry): entry is FeatureTreeListEntry => entry !== null)
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
		this.selectedEdgeId = null
		this.selectedCornerId = null
		this.selectedSketchId = null
		this.selectedSketchEdge = null
		this.selectedPCadNodeId = faceId ? (this.findFaceNodeId(extrude.id, faceId) ?? this.findGeneratedSolidFaceGraphId(extrude.id, faceId)) : extrude.id
		this.selectedReferencePlane = targetSketch?.type === "sketch" ? this.getReferencePlaneForSketchTarget(targetSketch.target) : this.selectedReferencePlane
		this.activeTool = "view"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.heightInput.value = String(extrude.depth)
		this.syncPreviewSketchGeometry()
		this.refreshReferencePlaneStyles()
		this.updateStatus()
		this.updateControls()
		this.drawPreview()
	}

	private selectExtrudeEdge(extrudeId: string, edgeId: string): void {
		const extrude = this.features.find((feature) => feature.type === "extrude" && feature.id === extrudeId)
		if (!extrude || extrude.type !== "extrude") {
			return
		}
		const targetSketch = this.features.find((feature) => feature.type === "sketch" && feature.id === extrude.target.sketchId)
		this.selectedExtrudeId = extrude.id
		this.selectedFaceId = null
		this.selectedEdgeId = edgeId
		this.selectedCornerId = null
		this.selectedSketchId = null
		this.selectedSketchEdge = null
		this.selectedPCadNodeId = this.findEdgeNodeId(extrude.id, edgeId) ?? this.findGeneratedSolidEdgeGraphId(extrude.id, edgeId)
		this.selectedReferencePlane = targetSketch?.type === "sketch" ? this.getReferencePlaneForSketchTarget(targetSketch.target) : this.selectedReferencePlane
		this.activeTool = "view"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.heightInput.value = String(extrude.depth)
		this.syncPreviewSketchGeometry()
		this.refreshEdgeStyles()
		this.refreshFaceStyles()
		this.refreshSolidStyles()
		this.refreshReferencePlaneStyles()
		this.updateStatus()
		this.updateControls()
		this.drawPreview()
	}

	private selectExtrudeCorner(extrudeId: string, cornerId: string): void {
		const extrude = this.features.find((feature) => feature.type === "extrude" && feature.id === extrudeId)
		if (!extrude || extrude.type !== "extrude") {
			return
		}
		const targetSketch = this.features.find((feature) => feature.type === "sketch" && feature.id === extrude.target.sketchId)
		this.selectedExtrudeId = extrude.id
		this.selectedFaceId = null
		this.selectedEdgeId = null
		this.selectedCornerId = cornerId
		this.selectedSketchId = null
		this.selectedSketchEdge = null
		this.selectedPCadNodeId = this.findGeneratedSolidVertexGraphId(extrude.id, cornerId) ?? extrude.id
		this.selectedReferencePlane = targetSketch?.type === "sketch" ? this.getReferencePlaneForSketchTarget(targetSketch.target) : this.selectedReferencePlane
		this.activeTool = "view"
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.heightInput.value = String(extrude.depth)
		this.syncPreviewSketchGeometry()
		this.refreshCornerStyles()
		this.refreshEdgeStyles()
		this.refreshFaceStyles()
		this.refreshSolidStyles()
		this.refreshReferencePlaneStyles()
		this.updateStatus()
		this.updateControls()
		this.drawPreview()
	}

	private clearViewSelection(): void {
		this.selectedReferencePlane = null
		this.selectedSketchId = null
		this.selectedExtrudeId = null
		this.selectedFaceId = null
		this.selectedEdgeId = null
		this.selectedCornerId = null
		this.selectedPCadNodeId = null
		this.selectedSketchEdge = null
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.activeTool = "view"
		this.syncPreviewSketchGeometry()
		this.refreshCornerStyles()
		this.refreshEdgeStyles()
		this.refreshFaceStyles()
		this.refreshSolidStyles()
		this.refreshReferencePlaneStyles()
		this.updateStatus()
		this.updateControls()
		this.drawPreview()
	}

	private selectPCadNode(nodeId: string | null): void {
		this.selectedPCadNodeId = nodeId
		if (!nodeId) {
			this.clearViewSelection()
			return
		}
		const node = this.cadEditor.getState().nodes.get(nodeId)
		if (!node) {
			this.selectedPCadNodeId = null
			this.updateControls()
			return
		}
		switch (node.type) {
			case "referencePlane":
				this.selectReferencePlane(SKETCH_PLANE_TO_REFERENCE_PLANE[node.plane])
				break
			case "sketch":
				this.selectSketch(node.id)
				break
			case "sketchLine":
			case "sketchCornerRectangle":
				this.selectSketch(node.sketchId)
				this.selectSketchEdge(this.getSketchGraphEdgeSelection(node.sketchId, node.id))
				break
			case "extrude":
				this.selectExtrude(node.id)
				break
			case "face":
				this.selectExtrudeFace(node.sourceId, node.faceId)
				break
			case "edge":
				this.selectExtrudeEdge(node.sourceId, node.edgeId)
				break
			case "chamfer": {
				const edge = this.cadEditor.getState().nodes.get(node.edgeId)
				if (edge?.type === "edge") {
					this.selectExtrudeEdge(edge.sourceId, edge.edgeId)
				}
				break
			}
		}
		this.selectedPCadNodeId = node.id
		this.refreshNodeEditor()
	}

	private selectPCadGeneratedNode(selection: PCadGeneratedSelection): void {
		switch (selection.type) {
			case "sketch":
				this.selectSketch(selection.sketchId)
				break
			case "sketchEntity":
				this.selectSketch(selection.sketchId)
				this.selectSketchEdge(this.getSketchGraphEdgeSelection(selection.sketchId, selection.entityId))
				break
			case "sketchDimension":
				this.selectSketch(selection.sketchId)
				this.selectSketchEdge(this.getSketchGraphEdgeSelection(selection.sketchId, selection.entityId, selection.dimensionType))
				break
			case "solidFace":
				this.selectExtrudeFace(selection.extrudeId, selection.faceId)
				break
			case "solidEdge":
				this.selectExtrudeEdge(selection.extrudeId, selection.edgeId)
				break
			case "solidVertex":
				this.selectExtrudeCorner(selection.extrudeId, selection.vertexId)
				break
		}
		this.selectedPCadNodeId = selection.graphId
		this.refreshNodeEditor()
	}

	private getSketchGraphEdgeSelection(sketchId: string, entityId: string, dimensionType?: SketchDimension["type"]): SketchEdgeSelection | null {
		const sketch = this.features.find((feature): feature is Sketch => feature.type === "sketch" && feature.id === sketchId)
		const entity = sketch?.entities.find((candidate) => candidate.id === entityId)
		if (!entity) {
			return null
		}
		if (entity.type === "line") {
			return {
				type: "line",
				entityId
			}
		}
		const side: RectangleSide = dimensionType === "rectangleHeight" ? "right" : "bottom"
		return {
			type: "rectangleSide",
			entityId,
			side
		}
	}

	private renamePCadNode(nodeId: string, name: string): void {
		const node = this.cadEditor.getState().nodes.get(nodeId)
		const trimmedName = name.trim()
		if (!node || !trimmedName || !this.canRenamePCadNode(node) || node.name === trimmedName) {
			return
		}
		const previousState = this.getState()
		try {
			this.cadEditor.renameNode(nodeId, trimmedName)
			this.syncAfterPCadNodeMutation(nodeId)
			this.onCadCommand?.({ type: "renameNode", nodeId, name: trimmedName }, previousState)
		} catch (_error) {
			return
		}
	}

	private setPCadExtrudeDepth(nodeId: string, depth: number): void {
		const node = this.cadEditor.getState().nodes.get(nodeId)
		if (!node || node.type !== "extrude" || !Number.isFinite(depth) || depth <= 0 || node.depth === depth) {
			return
		}
		const previousState = this.getState()
		try {
			this.cadEditor.setExtrudeDepth(nodeId, depth)
			this.selectedExtrudeId = nodeId
			this.syncAfterPCadNodeMutation(nodeId)
			this.onCadCommand?.({ type: "setExtrudeDepth", extrudeId: nodeId, depth }, previousState)
		} catch (_error) {
			return
		}
	}

	private setPCadChamferDistances(nodeId: string, d1: number, d2?: number): void {
		const node = this.cadEditor.getState().nodes.get(nodeId)
		if (!node || node.type !== "chamfer" || !Number.isFinite(d1) || d1 <= 0 || (d2 !== undefined && (!Number.isFinite(d2) || d2 <= 0)) || (node.d1 === d1 && node.d2 === d2)) {
			return
		}
		const previousState = this.getState()
		try {
			this.cadEditor.setChamferDistance(nodeId, d1, d2)
			this.syncAfterPCadNodeMutation(nodeId)
			this.onCadCommand?.({ type: "setChamferDistances", chamferId: nodeId, d1, ...(d2 === undefined ? {} : { d2 }) }, previousState)
		} catch (_error) {
			return
		}
	}

	private deletePCadNode(nodeId: string): void {
		const node = this.cadEditor.getState().nodes.get(nodeId)
		if (!node || !this.canDeletePCadNode(node)) {
			return
		}
		const previousState = this.getState()
		const deletedIds = collectDependentNodeIds(this.cadEditor.getState(), [nodeId])
		try {
			this.cadEditor.deleteNodeCascade(nodeId)
		} catch (_error) {
			return
		}
		this.partTreeState = {
			orderedNodeIds: this.partTreeState.orderedNodeIds.filter((id) => !deletedIds.has(id)),
			dirtySketchIds: this.partTreeState.dirtySketchIds.filter((id) => !deletedIds.has(id))
		}
		this.syncAfterPCadNodeMutation(null)
		this.onCadCommand?.({ type: "deleteNodeCascade", nodeId }, previousState)
	}

	private syncAfterPCadNodeMutation(selectedNodeId: string | null): void {
		this.syncDerivedPartState()
		this.normalizeSelectionAfterCadGraphChange()
		this.selectedPCadNodeId = selectedNodeId && this.cadEditor.getState().nodes.has(selectedNodeId) ? selectedNodeId : this.getSelectedPCadNodeId()
		this.syncPreviewGeometry()
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
		this.emitStateChange()
	}

	private normalizeSelectionAfterCadGraphChange(): void {
		if (this.selectedSketchId && !this.resolveSketch(this.selectedSketchId)) {
			this.selectedSketchId = null
			this.selectedSketchEdge = null
			this.activeTool = "view"
		}
		if (this.selectedExtrudeId && !this.resolveExtrude(this.selectedExtrudeId)) {
			this.selectedExtrudeId = null
			this.selectedFaceId = null
			this.selectedEdgeId = null
			this.selectedCornerId = null
		}
		if (this.selectedFaceId && !this.selectedExtrudeId) {
			this.selectedFaceId = null
		}
		if (this.selectedEdgeId && !this.selectedExtrudeId) {
			this.selectedEdgeId = null
		}
		if (this.selectedCornerId && !this.selectedExtrudeId) {
			this.selectedCornerId = null
		}
		if (this.selectedSketchEdge && !this.getSelectedSketchEdgeEntity()) {
			this.selectedSketchEdge = null
		}
	}

	private canRenamePCadNode(node: PCadGraphNode): boolean {
		return node.type === "sketch" || node.type === "extrude" || node.type === "chamfer"
	}

	private canDeletePCadNode(node: PCadGraphNode): boolean {
		return node.type === "sketch" || node.type === "extrude" || node.type === "chamfer"
	}

	private refreshNodeEditor(): void {
		this.nodeEditor.update(this.cadEditor.getState(), this.getSelectedPCadNodeId(), this.getPCadGeneratedState())
	}

	private getPCadGeneratedState(): PCadGeneratedState {
		return {
			features: this.features,
			solids: this.solids
		}
	}

	private getSelectedPCadNodeId(): string | null {
		const state = this.cadEditor.getState()
		if (this.selectedPCadNodeId && !state.nodes.has(this.selectedPCadNodeId) && this.selectedPCadNodeId.startsWith("generated:")) {
			return this.selectedPCadNodeId
		}
		const explicitNode = this.selectedPCadNodeId ? state.nodes.get(this.selectedPCadNodeId) : null
		if (explicitNode?.type === "chamfer" || explicitNode?.type === "sketchLine" || explicitNode?.type === "sketchCornerRectangle") {
			return explicitNode.id
		}
		if (this.selectedExtrudeId && this.selectedEdgeId) {
			return (
				this.findEdgeNodeId(this.selectedExtrudeId, this.selectedEdgeId) ??
				this.findGeneratedSolidEdgeGraphId(this.selectedExtrudeId, this.selectedEdgeId) ??
				this.selectedExtrudeId
			)
		}
		if (this.selectedExtrudeId && this.selectedFaceId) {
			return (
				this.findFaceNodeId(this.selectedExtrudeId, this.selectedFaceId) ??
				this.findGeneratedSolidFaceGraphId(this.selectedExtrudeId, this.selectedFaceId) ??
				this.selectedExtrudeId
			)
		}
		if (this.selectedExtrudeId && this.selectedCornerId) {
			return this.findGeneratedSolidVertexGraphId(this.selectedExtrudeId, this.selectedCornerId) ?? this.selectedExtrudeId
		}
		if (this.selectedExtrudeId && state.nodes.has(this.selectedExtrudeId)) {
			return this.selectedExtrudeId
		}
		if (this.selectedSketchId && this.selectedSketchEdge && state.nodes.has(this.selectedSketchEdge.entityId)) {
			return this.selectedSketchEdge.entityId
		}
		if (this.selectedSketchId && state.nodes.has(this.selectedSketchId)) {
			return this.selectedSketchId
		}
		return this.selectedReferencePlane ? getReferencePlaneNodeId(this.selectedReferencePlane) : null
	}

	private findFaceNodeId(extrudeId: string, faceId: string): string | null {
		for (const node of this.cadEditor.getState().nodes.values()) {
			if (node.type === "face" && node.sourceId === extrudeId && node.faceId === faceId) {
				return node.id
			}
		}
		return null
	}

	private findEdgeNodeId(extrudeId: string, edgeId: string): string | null {
		for (const node of this.cadEditor.getState().nodes.values()) {
			if (node.type === "edge" && node.sourceId === extrudeId && node.edgeId === edgeId) {
				return node.id
			}
		}
		return null
	}

	private findGeneratedSolidFaceGraphId(extrudeId: string, faceId: string): string | null {
		const solid = this.solids.find((candidate) => candidate.featureId === extrudeId && candidate.faces.some((face) => face.id === faceId))
		return solid ? `generated:${solid.id}:solid-face:${faceId}` : null
	}

	private findGeneratedSolidEdgeGraphId(extrudeId: string, edgeId: string): string | null {
		const solid = this.solids.find((candidate) => candidate.featureId === extrudeId && candidate.edges.some((edge) => edge.id === edgeId))
		return solid ? `generated:${solid.id}:solid-edge:${edgeId}` : null
	}

	private findGeneratedSolidVertexGraphId(extrudeId: string, vertexId: string): string | null {
		const solid = this.solids.find((candidate) => candidate.featureId === extrudeId && candidate.vertices.some((vertex) => vertex.id === vertexId))
		return solid ? `generated:${solid.id}:solid-vertex:${vertexId}` : null
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
		this.dispatchPartAction({
			type: "renameSketch",
			sketchId: sketch.id,
			name: trimmed
		})
	}

	public deleteSketch(sketchId?: string): void {
		const sketch = this.resolveSketch(sketchId)
		if (!sketch) {
			return
		}
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.dispatchPartAction({
			type: "deleteSketch",
			sketchId: sketch.id
		})
	}

	public deleteExtrude(extrudeId?: string): void {
		const extrude = this.resolveExtrude(extrudeId)
		if (!extrude) {
			return
		}
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.dispatchPartAction({
			type: "deleteExtrude",
			extrudeId: extrude.id
		})
	}

	private restoreState(state?: PartEditorState): void {
		this.solids = []
		const runtimeState: PartRuntimeState = createPartRuntimeState(state)
		this.cadEditor = new CadEditor(runtimeState.cad)
		this.partTreeState = runtimeState.tree
		this.syncDerivedPartState()
		this.selectedSketchId = this.features.find((feature) => feature.type === "sketch" && feature.dirty)?.id ?? this.getLastSketchId()
		this.selectedExtrudeId = null
		this.selectedFaceId = null
		this.selectedEdgeId = null
		this.selectedCornerId = null
		this.selectedSketchEdge = null
		const selectedSketch = this.getSelectedSketch()
		this.selectedReferencePlane = selectedSketch ? this.getReferencePlaneForSketchTarget(selectedSketch.target) : "Front"
		this.activeTool = selectedSketch?.dirty ? "sketch" : "view"
		this.activeSketchTool = this.getDefaultSketchTool(selectedSketch)
		this.selectedPCadNodeId = this.getSelectedPCadNodeId()
		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.sketchVisible = true
		this.referencePlaneVisibility = {
			Front: true,
			Top: true,
			Right: true
		}
		if (this.activeTool === "sketch" && this.selectedReferencePlane) {
			this.focusReferencePlaneForSketch(this.selectedReferencePlane, { resetView: true })
		}
		this.warningText.textContent = this.migrationWarnings.join(" ")
		this.updateModeButtons()
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
		if (actionId === "start-sketch" || actionId === "extrude" || actionId === "chamfer") {
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
				this.selectedSketchEdge = null
				this.pendingLineStart = null
				this.pendingRectangleStart = null
				this.sketchHoverPoint = null
				this.syncPreviewSketchGeometry()
				this.drawSketch()
				this.updateStatus()
				this.updateControls()
				return
			case "tool-line":
				this.activeSketchTool = "line"
				this.selectedSketchEdge = null
				this.pendingRectangleStart = null
				this.pendingLineStart = null
				this.sketchHoverPoint = null
				this.syncPreviewSketchGeometry()
				this.drawSketch()
				this.updateControls()
				return
			case "tool-rectangle":
				this.activeSketchTool = "rectangle"
				this.selectedSketchEdge = null
				this.pendingRectangleStart = null
				this.pendingLineStart = null
				this.sketchHoverPoint = null
				this.syncPreviewSketchGeometry()
				this.drawSketch()
				this.updateControls()
				return
			case "dimension":
				void this.handleDimension()
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
			case "chamfer":
				void this.handleChamfer()
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

		this.dispatchPartAction({
			type: "undoSketchEntity",
			sketchId: sketch.id
		})
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

		this.dispatchPartAction({
			type: "resetSketch",
			sketchId: sketch.id
		})
	}

	private handleFinishSketch = (): void => {
		const sketch = this.getEditableSketch()
		if (!sketch || !this.canFinishSketch()) {
			return
		}

		this.pendingLineStart = null
		this.pendingRectangleStart = null
		this.sketchHoverPoint = null
		this.dispatchPartAction({
			type: "finishSketch",
			sketchId: sketch.id
		})
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

		const extrudeCount = this.features.filter((feature) => feature.type === "extrude").length
		this.dispatchPartAction({
			type: "createExtrude",
			extrudeId: `extrude-${extrudeCount + 1}-${createId()}`,
			name: `Extrude ${extrudeCount + 1}`,
			sketchId: sketch.id,
			profileId: profile.id,
			depth
		})
	}

	private async handleChamfer(): Promise<void> {
		const edge = this.getSelectedEdgeReference()
		if (!edge || typeof window === "undefined") {
			return
		}

		const existingChamfer = this.getChamferForEdge(edge)
		const input = await showTextPromptModal({
			title: "Chamfer",
			initialValue: formatSketchDimensionValue(existingChamfer?.d1 ?? 1),
			confirmText: "Apply",
			cancelText: "Cancel"
		})
		if (!input) {
			return
		}

		const distance = Number.parseFloat(input)
		if (!Number.isFinite(distance) || distance <= 0) {
			return
		}

		if (existingChamfer) {
			this.dispatchPartAction({
				type: "setChamferDistances",
				chamferId: existingChamfer.id,
				d1: distance
			})
			return
		}

		const chamferCount = this.features.filter((feature) => feature.type === "chamfer").length
		this.dispatchPartAction({
			type: "createChamfer",
			chamferId: `chamfer-${chamferCount + 1}-${createId()}`,
			name: `Chamfer ${chamferCount + 1}`,
			target: {
				edge
			},
			d1: distance
		})
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
		this.dispatchPartAction({
			type: "setExtrudeDepth",
			extrudeId: extrude.id,
			depth
		})
		this.heightInput.value = String(depth)
	}

	private handleSketchCanvasClick = (event: MouseEvent): void => {
		if (this.activeTool !== "sketch") {
			return
		}
		const cursorPoint = this.getCanvasCursorPoint(event)
		if (!cursorPoint) {
			return
		}
		const point = this.canvasPointToSketchPoint(cursorPoint)
		if (!this.pendingLineStart && !this.pendingRectangleStart && !this.isNearSketchAnchor(point)) {
			this.selectSketchEdgeAtCanvasPoint(cursorPoint)
			if (this.selectedSketchEdge) {
				return
			}
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
			const point = this.getSelectedSketchPoint(event.clientX, event.clientY)
			if (!this.pendingLineStart && !this.pendingRectangleStart && point && !this.isNearSketchAnchor(point)) {
				const selectedEdge = this.getSketchEdgeAtPreviewPoint(event.clientX, event.clientY)
				if (selectedEdge) {
					event.preventDefault()
					this.selectSketchEdge(selectedEdge)
					return
				}
			}
			if (point) {
				event.preventDefault()
				this.handleSketchPoint(point)
				return
			}
		}

		if (isLeftMouseClick && this.activeTool === "view") {
			const cornerSelection = this.getExtrudeCornerAt(event.clientX, event.clientY)
			if (cornerSelection) {
				event.preventDefault()
				this.selectExtrudeCorner(cornerSelection.extrudeId, cornerSelection.cornerId)
				return
			}
			const edgeSelection = this.getExtrudeEdgeAt(event.clientX, event.clientY)
			if (edgeSelection) {
				event.preventDefault()
				this.selectExtrudeEdge(edgeSelection.extrudeId, edgeSelection.edgeId)
				return
			}
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
				this.selectedEdgeId = null
				this.selectedCornerId = null
				this.selectedSketchEdge = null
				this.refreshReferencePlaneStyles()
				this.updateControls()
				this.drawPreview()
				return
			}
			event.preventDefault()
			this.clearViewSelection()
			return
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
		if (isRotatePointer) {
			this.setPreviewOrbitPivot(this.getOrbitAnchorPoint(event.clientX, event.clientY))
		}
		this.previewCanvas.setPointerCapture(event.pointerId)
		this.previewCanvas.style.cursor = "grabbing"
	}

	private handlePreviewPointerMove = (event: PointerEvent): void => {
		if ((!this.isRotatingPreview && !this.isPanningPreview) || !this.lastRotationPointer) {
			if (this.activeTool === "sketch") {
				this.hoveredReferencePlane = null
				this.hoveredExtrudeId = null
				this.hoveredFaceId = null
				this.hoveredEdgeId = null
				this.hoveredCornerId = null
				const point = this.getSelectedSketchPoint(event.clientX, event.clientY)
				this.sketchHoverPoint = point ? this.snapPoint(point) : null
				this.refreshCornerStyles()
				this.refreshEdgeStyles()
				this.updatePreviewCursor()
				this.drawSketch()
				return
			}
			const hoveredCorner = this.activeTool === "view" ? this.getExtrudeCornerAt(event.clientX, event.clientY) : null
			const hoveredEdge = hoveredCorner ? null : this.activeTool === "view" ? this.getExtrudeEdgeAt(event.clientX, event.clientY) : null
			const hoveredFace = hoveredCorner || hoveredEdge ? null : this.activeTool === "view" ? this.getExtrudeFaceAt(event.clientX, event.clientY) : null
			const hoveredExtrudeId =
				hoveredCorner?.extrudeId ?? hoveredEdge?.extrudeId ?? hoveredFace?.extrudeId ?? (this.activeTool === "view" ? this.getExtrudeAt(event.clientX, event.clientY) : null)
			const hoveredPlane = hoveredExtrudeId ? null : this.activeTool === "view" ? this.getReferencePlaneAt(event.clientX, event.clientY) : null
			if (
				hoveredPlane !== this.hoveredReferencePlane ||
				hoveredExtrudeId !== this.hoveredExtrudeId ||
				hoveredFace?.faceId !== this.hoveredFaceId ||
				hoveredEdge?.edgeId !== this.hoveredEdgeId ||
				hoveredCorner?.cornerId !== this.hoveredCornerId
			) {
				this.hoveredReferencePlane = hoveredPlane
				this.hoveredExtrudeId = hoveredExtrudeId
				this.hoveredFaceId = hoveredFace?.faceId ?? null
				this.hoveredEdgeId = hoveredEdge?.edgeId ?? null
				this.hoveredCornerId = hoveredCorner?.cornerId ?? null
				this.refreshCornerStyles()
				this.refreshEdgeStyles()
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
		this.emitViewStateChange()
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
			this.hoveredEdgeId = null
			this.hoveredCornerId = null
			this.sketchHoverPoint = null
			this.refreshCornerStyles()
			this.refreshEdgeStyles()
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
		this.emitViewStateChange()
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
		this.dispatchPartAction({
			type: "addSketchEntity",
			sketchId: sketch.id,
			entity
		})
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
				this.selectedSketchEdge = null
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
			this.selectedSketchEdge = null
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

	private getSelectedSketch(): Sketch | null {
		if (!this.selectedSketchId) {
			return null
		}
		const sketch = this.features.find((feature) => feature.type === "sketch" && feature.id === this.selectedSketchId)
		return sketch?.type === "sketch" ? sketch : null
	}

	private syncDerivedPartState(): void {
		this.features = materializePartFeatures(this.cadEditor.getState(), this.partTreeState)
	}

	private isDirtySketch(sketchId: string): boolean {
		return this.partTreeState.dirtySketchIds.includes(sketchId)
	}

	private setDirtySketch(sketchId: string, dirty: boolean): void {
		const dirtySketchIds = dirty ? [...new Set([...this.partTreeState.dirtySketchIds, sketchId])] : this.partTreeState.dirtySketchIds.filter((id) => id !== sketchId)
		this.partTreeState = {
			...this.partTreeState,
			dirtySketchIds
		}
	}

	private getSelectedExtrude(): SolidExtrude | null {
		if (!this.selectedExtrudeId) {
			return null
		}
		const extrude = this.features.find((feature) => feature.type === "extrude" && feature.id === this.selectedExtrudeId)
		return extrude?.type === "extrude" ? extrude : null
	}

	private getSelectedFaceReference(): FaceReference | null {
		if (!this.selectedExtrudeId || !this.selectedFaceId) {
			return null
		}
		return {
			type: "extrudeFace",
			extrudeId: this.selectedExtrudeId,
			faceId: this.selectedFaceId
		}
	}

	private getSelectedEdgeReference(): EdgeReference | null {
		if (!this.selectedExtrudeId || !this.selectedEdgeId) {
			return null
		}
		return {
			type: "extrudeEdge",
			extrudeId: this.selectedExtrudeId,
			edgeId: this.selectedEdgeId
		}
	}

	private getSelectedFaceLabel(): string | null {
		return this.selectedExtrudeId && this.selectedFaceId ? this.getPreviewFaceLabel(this.selectedExtrudeId, this.selectedFaceId) : null
	}

	private getSelectedEdgeLabel(): string | null {
		return this.selectedExtrudeId && this.selectedEdgeId ? this.getPreviewEdgeLabel(this.selectedExtrudeId, this.selectedEdgeId) : null
	}

	private getSelectedCornerLabel(): string | null {
		return this.selectedExtrudeId && this.selectedCornerId ? this.getPreviewCornerLabel(this.selectedExtrudeId, this.selectedCornerId) : null
	}

	private getReferencePlaneForSketchTarget(target: Sketch["target"]): ReferencePlaneName | null {
		return target.type === "plane" ? SKETCH_PLANE_TO_REFERENCE_PLANE[target.plane] : null
	}

	private getSketchTargetLabel(target: Sketch["target"]): string {
		if (target.type === "plane") {
			return SKETCH_PLANE_TO_REFERENCE_PLANE[target.plane]
		}
		return this.getPreviewFaceLabel(target.face.extrudeId, target.face.faceId) ?? "Face"
	}

	private resolveSketchFrame(target: Sketch["target"]): SketchFrame3D | null {
		try {
			return resolveSketchTargetFrame(
				{
					features: this.features
				},
				target
			)
		} catch (_error) {
			return null
		}
	}

	private worldPointToSketchPoint(worldPoint: THREE.Vector3, frame: SketchFrame3D): Point2D {
		const partPoint = this.previewContentGroup.worldToLocal(worldPoint.clone())
		const origin = vector3DToThree(frame.origin)
		const offset = partPoint.sub(origin)
		const xAxis = vector3DToThree(frame.xAxis).normalize()
		const yAxis = vector3DToThree(frame.yAxis).normalize()
		return {
			x: offset.dot(xAxis),
			y: offset.dot(yAxis)
		}
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

	private getSelectedEdgeChamfer(): SolidChamfer | null {
		const edge = this.getSelectedEdgeReference()
		return edge ? this.getChamferForEdge(edge) : null
	}

	private getChamferForEdge(edge: EdgeReference): SolidChamfer | null {
		const chamfer = this.features.find((feature) => feature.type === "chamfer" && edgeReferencesEqual(feature.target.edge, edge))
		return chamfer?.type === "chamfer" ? chamfer : null
	}

	private canFinishSketch(): boolean {
		const sketch = this.getEditableSketch()
		return !!sketch && !this.pendingLineStart && !this.pendingRectangleStart && sketch.profiles.length === 1
	}

	private canExtrude(): boolean {
		const sketch = this.getSelectedSketch()
		return !!sketch && !sketch.dirty && sketch.profiles.length === 1
	}

	private canDimension(): boolean {
		return !!this.getSelectedSketch() && !!this.getSelectedSketchDimensionCandidate()
	}

	private getCanvasCursorPoint(event: MouseEvent): Point2D | null {
		const rect = this.sketchCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		return {
			x: event.clientX - rect.left,
			y: event.clientY - rect.top
		}
	}

	private getCanvasPoint(event: MouseEvent): Point2D | null {
		const point = this.getCanvasCursorPoint(event)
		return point ? this.canvasPointToSketchPoint(point) : null
	}

	private getSketchCanvasScale(): number {
		return (SKETCH_CANVAS_SIZE - SKETCH_CANVAS_PADDING * 2) / REFERENCE_PLANE_SIZE
	}

	private sketchPointToCanvasPoint(point: Point2D): Point2D {
		const scale = this.getSketchCanvasScale()
		return {
			x: SKETCH_CANVAS_SIZE / 2 + point.x * scale,
			y: SKETCH_CANVAS_SIZE / 2 - point.y * scale
		}
	}

	private canvasPointToSketchPoint(point: Point2D): Point2D {
		const scale = this.getSketchCanvasScale()
		return {
			x: (point.x - SKETCH_CANVAS_SIZE / 2) / scale,
			y: (SKETCH_CANVAS_SIZE / 2 - point.y) / scale
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

	private isNearSketchAnchor(point: Point2D): boolean {
		const maxDistanceSquared = SKETCH_SNAP_DISTANCE * SKETCH_SNAP_DISTANCE
		for (const candidate of this.getActiveAnchorPoints()) {
			const dx = candidate.x - point.x
			const dy = candidate.y - point.y
			if (dx * dx + dy * dy <= maxDistanceSquared) {
				return true
			}
		}
		return false
	}

	private getSelectedSketchEdgeEntity(): SketchEntity | null {
		const sketch = this.getSelectedSketch()
		if (!sketch || !this.selectedSketchEdge) {
			return null
		}
		return sketch.entities.find((entity) => entity.id === this.selectedSketchEdge?.entityId) ?? null
	}

	private selectSketchEdgeAtSketchPoint(point: Point2D): void {
		this.selectSketchEdgeAtCanvasPoint(this.sketchPointToCanvasPoint(point))
	}

	private selectSketchEdge(selection: SketchEdgeSelection | null): void {
		this.selectedSketchEdge = selection
		this.syncPreviewSketchGeometry()
		this.drawSketch()
		this.updateControls()
	}

	private selectSketchEdgeAtCanvasPoint(point: Point2D): void {
		this.selectSketchEdge(this.getSketchEdgeAtCanvasPoint(point))
	}

	private getSketchEdgeAtCanvasPoint(point: Point2D): SketchEdgeSelection | null {
		const sketch = this.getSelectedSketch()
		if (!sketch) {
			return null
		}

		let bestSelection: SketchEdgeSelection | null = null
		let bestDistanceSquared = SKETCH_EDGE_HIT_TOLERANCE * SKETCH_EDGE_HIT_TOLERANCE
		for (const entity of sketch.entities) {
			if (entity.type === "line") {
				const distanceSquared = distanceToSegmentSquared(point, this.sketchPointToCanvasPoint(entity.p0), this.sketchPointToCanvasPoint(entity.p1))
				if (distanceSquared <= bestDistanceSquared) {
					bestDistanceSquared = distanceSquared
					bestSelection = {
						type: "line",
						entityId: entity.id
					}
				}
				continue
			}

			const corners = rectangleCorners(entity)
			const sides: RectangleSide[] = ["bottom", "right", "top", "left"]
			for (let index = 0; index < corners.length; index += 1) {
				const start = this.sketchPointToCanvasPoint(corners[index] ?? corners[0] ?? { x: 0, y: 0 })
				const end = this.sketchPointToCanvasPoint(corners[(index + 1) % corners.length] ?? corners[0] ?? { x: 0, y: 0 })
				const distanceSquared = distanceToSegmentSquared(point, start, end)
				if (distanceSquared <= bestDistanceSquared) {
					bestDistanceSquared = distanceSquared
					bestSelection = {
						type: "rectangleSide",
						entityId: entity.id,
						side: sides[index] ?? "bottom"
					}
				}
			}
		}

		return bestSelection
	}

	private getSketchEdgeAtPreviewPoint(clientX: number, clientY: number): SketchEdgeSelection | null {
		const sketch = this.getSelectedSketch()
		if (!sketch) {
			return null
		}
		const frame = this.resolveSketchFrame(sketch.target)
		if (!frame) {
			return null
		}

		const point = { x: clientX, y: clientY }
		let bestSelection: SketchEdgeSelection | null = null
		let bestDistanceSquared = SKETCH_EDGE_HIT_TOLERANCE * SKETCH_EDGE_HIT_TOLERANCE
		for (const entity of sketch.entities) {
			if (entity.type === "line") {
				const start = this.projectSketchPointToPreviewPoint(entity.p0, frame)
				const end = this.projectSketchPointToPreviewPoint(entity.p1, frame)
				if (!start || !end) {
					continue
				}
				const distanceSquared = distanceToSegmentSquared(point, start, end)
				if (distanceSquared <= bestDistanceSquared) {
					bestDistanceSquared = distanceSquared
					bestSelection = {
						type: "line",
						entityId: entity.id
					}
				}
				continue
			}

			const sides: RectangleSide[] = ["bottom", "right", "top", "left"]
			for (const side of sides) {
				const segment = getRectangleSideSegment(entity, side)
				const start = this.projectSketchPointToPreviewPoint(segment.start, frame)
				const end = this.projectSketchPointToPreviewPoint(segment.end, frame)
				if (!start || !end) {
					continue
				}
				const distanceSquared = distanceToSegmentSquared(point, start, end)
				if (distanceSquared <= bestDistanceSquared) {
					bestDistanceSquared = distanceSquared
					bestSelection = {
						type: "rectangleSide",
						entityId: entity.id,
						side
					}
				}
			}
		}

		return bestSelection
	}

	private projectSketchPointToPreviewPoint(point: Point2D, frame: SketchFrame3D): Point2D | null {
		return this.projectPreviewLocalPointToClient(this.sketchPointToPreviewWorld(point, frame))
	}

	private projectPreviewLocalPointToClient(localPoint: THREE.Vector3): Point2D | null {
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}
		this.syncPreviewView()
		this.previewScene.updateMatrixWorld(true)
		this.previewCamera.updateMatrixWorld(true)
		const projected = this.previewContentGroup.localToWorld(localPoint.clone()).project(this.previewCamera)
		if (!Number.isFinite(projected.x) || !Number.isFinite(projected.y)) {
			return null
		}
		return {
			x: rect.left + ((projected.x + 1) / 2) * rect.width,
			y: rect.top + ((1 - projected.y) / 2) * rect.height
		}
	}

	private getSelectedSketchDimensionCandidate(): {
		id: string
		type: SketchDimension["type"]
		entityId: string
		value: number
		label: string
	} | null {
		const sketch = this.getSelectedSketch()
		if (!sketch || !this.selectedSketchEdge) {
			return null
		}

		if (this.selectedSketchEdge.type === "line") {
			const entity = sketch.entities.find((candidate) => candidate.id === this.selectedSketchEdge?.entityId)
			if (!entity || entity.type !== "line") {
				return null
			}
			const existing = sketch.dimensions.find((dimension) => dimension.entityId === entity.id && dimension.type === "lineLength")
			return {
				id: existing?.id ?? `dimension-${createId()}`,
				type: "lineLength",
				entityId: entity.id,
				value: existing?.value ?? Math.hypot(entity.p1.x - entity.p0.x, entity.p1.y - entity.p0.y),
				label: "Line Length"
			}
		}

		const selectedEdge = this.selectedSketchEdge
		if (!selectedEdge || selectedEdge.type !== "rectangleSide") {
			return null
		}

		const entity = sketch.entities.find((candidate) => candidate.id === selectedEdge.entityId)
		if (!entity || entity.type !== "cornerRectangle") {
			return null
		}

		const dimensionType = selectedEdge.side === "top" || selectedEdge.side === "bottom" ? "rectangleWidth" : "rectangleHeight"
		const existing = sketch.dimensions.find((dimension) => dimension.entityId === entity.id && dimension.type === dimensionType)
		return {
			id: existing?.id ?? `dimension-${createId()}`,
			type: dimensionType,
			entityId: entity.id,
			value: existing?.value ?? (dimensionType === "rectangleWidth" ? Math.abs(entity.p1.x - entity.p0.x) : Math.abs(entity.p1.y - entity.p0.y)),
			label: dimensionType === "rectangleWidth" ? "Rectangle Width" : "Rectangle Height"
		}
	}

	private async handleDimension(): Promise<void> {
		const sketch = this.getSelectedSketch()
		const candidate = this.getSelectedSketchDimensionCandidate()
		if (!sketch || !candidate || typeof window === "undefined") {
			return
		}

		const input = await showTextPromptModal({
			title: candidate.label,
			initialValue: formatSketchDimensionValue(candidate.value),
			confirmText: "Save",
			cancelText: "Cancel"
		})
		if (!input) {
			return
		}

		const value = Number.parseFloat(input)
		if (!Number.isFinite(value) || value <= 0) {
			return
		}

		this.dispatchPartAction({
			type: "setSketchDimension",
			sketchId: sketch.id,
			dimension: {
				id: candidate.id,
				type: candidate.type,
				entityId: candidate.entityId,
				value
			}
		})
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

		if (this.selectedSketchEdge) {
			this.drawSelectedSketchEdge(sketch, this.selectedSketchEdge)
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
					const canvasPoints = points.map((point) => this.sketchPointToCanvasPoint(point))
					this.sketchCtx.beginPath()
					this.sketchCtx.moveTo(canvasPoints[0]?.x ?? 0, canvasPoints[0]?.y ?? 0)
					for (const point of canvasPoints.slice(1)) {
						this.sketchCtx.lineTo(point.x, point.y)
					}
					this.sketchCtx.closePath()
					this.sketchCtx.fill()
				}
			}
		}

		this.drawSketchDimensions(sketch)

		if (this.pendingRectangleStart && this.sketchHoverPoint) {
			const start = this.sketchPointToCanvasPoint(this.pendingRectangleStart)
			const hover = this.sketchPointToCanvasPoint(this.sketchHoverPoint)
			this.sketchCtx.setLineDash([4, 4])
			this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
			this.sketchCtx.strokeRect(Math.min(start.x, hover.x), Math.min(start.y, hover.y), Math.abs(hover.x - start.x), Math.abs(hover.y - start.y))
			this.sketchCtx.setLineDash([])
		}

		if (this.pendingLineStart) {
			this.drawSketchPoint(this.pendingLineStart, "#f59e0b")
			if (this.sketchHoverPoint) {
				const start = this.sketchPointToCanvasPoint(this.pendingLineStart)
				const hover = this.sketchPointToCanvasPoint(this.sketchHoverPoint)
				this.sketchCtx.setLineDash([4, 4])
				this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
				this.sketchCtx.beginPath()
				this.sketchCtx.moveTo(start.x, start.y)
				this.sketchCtx.lineTo(hover.x, hover.y)
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
			const start = this.sketchPointToCanvasPoint(entity.p0)
			const end = this.sketchPointToCanvasPoint(entity.p1)
			this.sketchCtx.beginPath()
			this.sketchCtx.moveTo(start.x, start.y)
			this.sketchCtx.lineTo(end.x, end.y)
			this.sketchCtx.stroke()
			return
		}
		const corners = rectangleCorners(entity).map((point) => this.sketchPointToCanvasPoint(point))
		this.sketchCtx.beginPath()
		this.sketchCtx.moveTo(corners[0]?.x ?? 0, corners[0]?.y ?? 0)
		for (const point of corners.slice(1)) {
			this.sketchCtx.lineTo(point.x, point.y)
		}
		this.sketchCtx.closePath()
		this.sketchCtx.stroke()
	}

	private drawSketchPoint(point: Point2D, color: string): void {
		const canvasPoint = this.sketchPointToCanvasPoint(point)
		this.sketchCtx.fillStyle = color
		this.sketchCtx.beginPath()
		this.sketchCtx.arc(canvasPoint.x, canvasPoint.y, 5, 0, Math.PI * 2)
		this.sketchCtx.fill()
		this.sketchCtx.strokeStyle = "#ffffff"
		this.sketchCtx.lineWidth = 1.5
		this.sketchCtx.beginPath()
		this.sketchCtx.arc(canvasPoint.x, canvasPoint.y, 5, 0, Math.PI * 2)
		this.sketchCtx.stroke()
	}

	private drawSelectedSketchEdge(sketch: Sketch, selection: SketchEdgeSelection): void {
		const entity = sketch.entities.find((candidate) => candidate.id === selection.entityId)
		if (!entity) {
			return
		}

		this.sketchCtx.strokeStyle = "#f59e0b"
		this.sketchCtx.lineWidth = 3
		if (selection.type === "line" && entity.type === "line") {
			const start = this.sketchPointToCanvasPoint(entity.p0)
			const end = this.sketchPointToCanvasPoint(entity.p1)
			this.sketchCtx.beginPath()
			this.sketchCtx.moveTo(start.x, start.y)
			this.sketchCtx.lineTo(end.x, end.y)
			this.sketchCtx.stroke()
			return
		}

		if (selection.type === "rectangleSide" && entity.type === "cornerRectangle") {
			const segment = getRectangleSideSegment(entity, selection.side)
			const start = this.sketchPointToCanvasPoint(segment.start)
			const end = this.sketchPointToCanvasPoint(segment.end)
			this.sketchCtx.beginPath()
			this.sketchCtx.moveTo(start.x, start.y)
			this.sketchCtx.lineTo(end.x, end.y)
			this.sketchCtx.stroke()
		}
	}

	private drawSketchDimensions(sketch: Sketch): void {
		for (const dimension of sketch.dimensions) {
			const labelPoint = getSketchDimensionLabelPoint(sketch, dimension)
			if (!labelPoint) {
				continue
			}
			const labelPosition = this.sketchPointToCanvasPoint(labelPoint)
			const isSelected = isSketchDimensionSelected(this.selectedSketchEdge, dimension)
			const text = formatSketchDimensionValue(dimension.value)
			this.sketchCtx.font = "12px sans-serif"
			const metrics = this.sketchCtx.measureText(text)
			const width = Math.max(24, metrics.width + 10)
			const height = 20

			this.sketchCtx.fillStyle = isSelected ? "rgba(245,158,11,0.95)" : "rgba(255,255,255,0.95)"
			this.sketchCtx.strokeStyle = isSelected ? "#f59e0b" : "#cbd5e1"
			this.sketchCtx.lineWidth = 1.5
			this.sketchCtx.fillRect(labelPosition.x - width / 2, labelPosition.y - height / 2, width, height)
			this.sketchCtx.strokeRect(labelPosition.x - width / 2, labelPosition.y - height / 2, width, height)

			this.sketchCtx.fillStyle = isSelected ? "#ffffff" : "#0f172a"
			this.sketchCtx.textAlign = "center"
			this.sketchCtx.textBaseline = "middle"
			this.sketchCtx.fillText(text, labelPosition.x, labelPosition.y + 0.5)
		}
	}

	private updateStatus(): void {
		const sketch = this.getSelectedSketch()
		const extrudeCount = this.features.filter((feature) => feature.type === "extrude").length
		if (this.activeTool === "sketch" && sketch) {
			const entityCount = sketch.entities.length
			const profileCount = sketch.profiles.length
			const sketchState = sketch.dirty ? "Sketch open" : "Sketch finished"
			this.statusText.textContent = `${sketchState}. ${entityCount} entit${entityCount === 1 ? "y" : "ies"}. ${profileCount} profile${profileCount === 1 ? "" : "s"}.`
			this.summaryText.textContent = extrudeCount > 0 ? `${extrudeCount} extrude${extrudeCount === 1 ? "" : "s"} in the part.` : ""
			return
		}

		const extrude = this.getSelectedExtrude()
		if (extrude) {
			const extrudeLabel = extrude.name?.trim() || "Extrude"
			const selectedFaceLabel = this.getSelectedFaceLabel()
			const selectedEdgeLabel = this.getSelectedEdgeLabel()
			const selectedCornerLabel = this.getSelectedCornerLabel()
			if (selectedFaceLabel) {
				this.statusText.textContent = `${selectedFaceLabel} selected.`
				this.summaryText.textContent = `On ${extrudeLabel}. Depth ${extrude.depth.toFixed(1)}.`
				return
			}
			if (selectedEdgeLabel) {
				const selectedChamfer = this.getSelectedEdgeChamfer()
				this.statusText.textContent = `${selectedEdgeLabel} selected.`
				this.summaryText.textContent = selectedChamfer
					? `On ${extrudeLabel}. Depth ${extrude.depth.toFixed(1)}. Chamfer ${formatSketchDimensionValue(selectedChamfer.d1)}.`
					: `On ${extrudeLabel}. Depth ${extrude.depth.toFixed(1)}.`
				return
			}
			if (selectedCornerLabel) {
				this.statusText.textContent = `${selectedCornerLabel} selected.`
				this.summaryText.textContent = `On ${extrudeLabel}. Depth ${extrude.depth.toFixed(1)}.`
				return
			}
			this.statusText.textContent = `Extrude selected. Depth ${extrude.depth.toFixed(1)}.`
			this.summaryText.textContent = `Editing ${extrudeLabel}.`
			return
		}

		if (!sketch) {
			const selectedFaceLabel = this.getSelectedFaceLabel()
			this.statusText.textContent = selectedFaceLabel
				? `${selectedFaceLabel} selected.`
				: this.selectedReferencePlane
					? `Plane selected: ${this.selectedReferencePlane}.`
					: "Select a reference plane or face to start."
			this.summaryText.textContent = extrudeCount > 0 ? `${extrudeCount} extrude${extrudeCount === 1 ? "" : "s"} in the part.` : ""
			return
		}
	}

	private updateQuickActionsRail(): void {
		const selectedExtrude = this.activeTool === "view" ? this.getSelectedExtrude() : null
		const selectedFaceLabel = this.getSelectedFaceLabel()
		const selectedEdgeLabel = this.getSelectedEdgeLabel()
		const selectedCornerLabel = this.getSelectedCornerLabel()
		const sketch = this.getSelectedSketch()
		const selectedPlaneVisible = !!this.selectedReferencePlane && this.referencePlaneVisibility[this.selectedReferencePlane]
		const selectedTargetVisible = sketch?.target.type === "face" || !!selectedFaceLabel ? true : selectedPlaneVisible
		const model = derivePartQuickActionsModel({
			activeTool: this.activeTool,
			selectedExtrudeLabel: selectedExtrude?.name?.trim() || (selectedExtrude ? "Extrude" : null),
			selectedFaceLabel: selectedFaceLabel,
			selectedEdgeLabel: selectedEdgeLabel,
			selectedCornerLabel: selectedCornerLabel,
			selectedPlaneLabel: this.activeTool === "sketch" ? (sketch ? this.getSketchTargetLabel(sketch.target) : this.selectedReferencePlane) : this.selectedReferencePlane,
			selectedPlaneVisible: selectedTargetVisible,
			activeSketchTool: this.activeSketchTool,
			canDimension: this.canDimension(),
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
	}

	private updateControls(): void {
		if (this.studioMode === "nodes") {
			this.bodyContainer.style.flexDirection = "row"
			this.bodyContainer.style.flexWrap = "nowrap"
			this.nodeEditorPanel.style.display = "flex"
			this.nodeEditorPanel.style.minHeight = "0"
			this.nodePreviewResizeHandle.style.display = "block"
			this.applyNodePreviewSplit()
			this.previewContainer.style.width = "auto"
			this.previewContainer.style.height = "100%"
			this.previewContainer.style.minWidth = "0"
			this.previewContainer.style.minHeight = "0"
			this.sketchPanel.style.display = "none"
			this.updateQuickActionsRail()
			this.refreshNodeEditor()
			this.refreshCornerStyles()
			this.refreshEdgeStyles()
			this.refreshFaceStyles()
			this.refreshSolidStyles()
			this.refreshReferencePlaneStyles()
			this.updatePreviewCursor()
			queueFrame(() => this.drawPreview())
			return
		}

		this.nodeEditorPanel.style.display = "none"
		this.nodePreviewResizeHandle.style.display = "none"
		this.bodyContainer.style.flexDirection = "row"
		this.bodyContainer.style.flexWrap = "wrap"
		this.previewContainer.style.flex = "1 1 640px"
		this.previewContainer.style.width = "auto"
		this.previewContainer.style.height = "100%"
		this.previewContainer.style.minWidth = "0"
		this.previewContainer.style.minHeight = "0"
		const selectedExtrude = this.activeTool === "view" ? this.getSelectedExtrude() : null
		const selectedFaceLabel = this.getSelectedFaceLabel()
		const selectedEdgeLabel = this.getSelectedEdgeLabel()
		const selectedCornerLabel = this.getSelectedCornerLabel()
		const sketch = this.getSelectedSketch()
		const selectedPlaneVisible = !!this.selectedReferencePlane && this.referencePlaneVisibility[this.selectedReferencePlane]
		const selectedTargetVisible = sketch?.target.type === "face" || !!selectedFaceLabel ? true : selectedPlaneVisible
		const model = derivePartQuickActionsModel({
			activeTool: this.activeTool,
			selectedExtrudeLabel: selectedExtrude?.name?.trim() || (selectedExtrude ? "Extrude" : null),
			selectedFaceLabel: selectedFaceLabel,
			selectedEdgeLabel: selectedEdgeLabel,
			selectedCornerLabel: selectedCornerLabel,
			selectedPlaneLabel: this.activeTool === "sketch" ? (sketch ? this.getSketchTargetLabel(sketch.target) : this.selectedReferencePlane) : this.selectedReferencePlane,
			selectedPlaneVisible: selectedTargetVisible,
			activeSketchTool: this.activeSketchTool,
			canDimension: this.canDimension(),
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
		this.sketchCanvas.style.cursor = "crosshair"
		this.refreshCornerStyles()
		this.refreshEdgeStyles()
		this.refreshFaceStyles()
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
		const cameraDistance = THREE.MathUtils.clamp(this.previewBaseDistance, PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
		const distance = Math.max(PREVIEW_MIN_CAMERA_DISTANCE, Math.abs(cameraDistance - (this.previewPan.z + this.previewOrbitPivot.z)))
		const verticalFovRadians = THREE.MathUtils.degToRad(this.previewCamera.fov)
		const visibleHeight = 2 * distance * Math.tan(verticalFovRadians / 2)
		return {
			x: (visibleHeight * this.previewCamera.aspect) / width,
			y: visibleHeight / height
		}
	}

	private getPreviewPointerPosition(clientX: number, clientY: number): THREE.Vector2 | null {
		const rect = this.previewCanvas.getBoundingClientRect()
		const width = rect.width || this.previewContainer.clientWidth || 960
		const height = rect.height || this.previewContainer.clientHeight || 640
		if (width <= 0 || height <= 0) {
			return null
		}
		return new THREE.Vector2(((clientX - rect.left) / width) * 2 - 1, -((clientY - rect.top) / height) * 2 + 1)
	}

	private getPreviewViewportCenterClientPoint(): { x: number; y: number } | null {
		const rect = this.previewCanvas.getBoundingClientRect()
		const width = rect.width || this.previewContainer.clientWidth || 960
		const height = rect.height || this.previewContainer.clientHeight || 640
		if (width <= 0 || height <= 0) {
			return null
		}
		return {
			x: rect.left + width / 2,
			y: rect.top + height / 2
		}
	}

	private getReferencePlaneAt(clientX: number, clientY: number): ReferencePlaneName | null {
		return this.getReferencePlaneIntersection(clientX, clientY)?.name ?? null
	}

	private getPreviewFaceIntersection(clientX: number, clientY: number, reference?: FaceReference): { face: PreviewFaceVisual; intersectionPoint: THREE.Vector3 } | null {
		if (!this.setPreviewRaycaster(clientX, clientY)) {
			return null
		}
		const candidateFaces = this.previewSolids.flatMap((solid) => solid.faces).filter((face) => !reference || (face.extrudeId === reference.extrudeId && face.faceId === reference.faceId))
		const intersections = this.previewRaycaster.intersectObjects(
			candidateFaces.map((face) => face.mesh),
			false
		)
		const intersection = intersections[0]
		const mesh = intersection?.object
		if (!(mesh instanceof THREE.Mesh) || !intersection) {
			return null
		}
		const face = candidateFaces.find((entry) => entry.mesh === mesh)
		return face
			? {
					face,
					intersectionPoint: intersection.point.clone()
				}
			: null
	}

	private getExtrudeFaceAt(clientX: number, clientY: number): { extrudeId: string; faceId: string } | null {
		const face = this.getPreviewFaceIntersection(clientX, clientY)?.face
		if (face) {
			return {
				extrudeId: face.extrudeId,
				faceId: face.faceId
			}
		}
		return null
	}

	private getPreviewCornerIntersection(clientX: number, clientY: number): { corner: PreviewCornerVisual; distanceSquared: number; depth: number } | null {
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}

		this.syncPreviewView()
		this.previewScene.updateMatrixWorld(true)
		this.previewCamera.updateMatrixWorld(true)

		const pointer = { x: clientX, y: clientY }
		const maxDistanceSquared = PREVIEW_CORNER_HIT_TOLERANCE * PREVIEW_CORNER_HIT_TOLERANCE
		const occludingDepth = this.getNearestPreviewSolidDepth(clientX, clientY, rect)
		let best: { corner: PreviewCornerVisual; distanceSquared: number; depth: number } | null = null
		for (const solid of this.previewSolids) {
			if (!solid.mesh.visible) {
				continue
			}
			for (const corner of solid.corners) {
				const worldPoint = corner.marker.getWorldPosition(new THREE.Vector3())
				const projected = this.projectWorldPointToClient(worldPoint, rect)
				if (!projected) {
					continue
				}
				const offsetX = pointer.x - projected.x
				const offsetY = pointer.y - projected.y
				const distanceSquared = offsetX * offsetX + offsetY * offsetY
				if (distanceSquared > maxDistanceSquared) {
					continue
				}
				if (occludingDepth !== null && projected.depth > occludingDepth + PREVIEW_SELECTION_OCCLUSION_DEPTH_EPSILON) {
					continue
				}
				if (!best || distanceSquared < best.distanceSquared - 1e-6 || (Math.abs(distanceSquared - best.distanceSquared) <= 1e-6 && projected.depth < best.depth)) {
					best = {
						corner,
						distanceSquared,
						depth: projected.depth
					}
				}
			}
		}
		return best
	}

	private getExtrudeCornerAt(clientX: number, clientY: number): { extrudeId: string; cornerId: string } | null {
		const corner = this.getPreviewCornerIntersection(clientX, clientY)?.corner
		if (corner) {
			return {
				extrudeId: corner.extrudeId,
				cornerId: corner.cornerId
			}
		}
		return null
	}

	private getPreviewEdgeIntersection(clientX: number, clientY: number): { edge: PreviewEdgeVisual; distanceSquared: number; depth: number } | null {
		const rect = this.previewCanvas.getBoundingClientRect()
		if (rect.width <= 0 || rect.height <= 0) {
			return null
		}

		this.syncPreviewView()
		this.previewScene.updateMatrixWorld(true)
		this.previewCamera.updateMatrixWorld(true)

		const pointer = { x: clientX, y: clientY }
		const maxDistanceSquared = PREVIEW_EDGE_HIT_TOLERANCE * PREVIEW_EDGE_HIT_TOLERANCE
		const occludingDepth = this.getNearestPreviewSolidDepth(clientX, clientY, rect)
		let best: { edge: PreviewEdgeVisual; distanceSquared: number; depth: number } | null = null
		for (const solid of this.previewSolids) {
			if (!solid.mesh.visible) {
				continue
			}
			for (const edge of solid.edges) {
				if (!edge.line.visible) {
					continue
				}
				const position = edge.line.geometry.getAttribute("position")
				if (!position || position.count < 2) {
					continue
				}
				const startWorld = edge.line.localToWorld(new THREE.Vector3().fromBufferAttribute(position, 0))
				const endWorld = edge.line.localToWorld(new THREE.Vector3().fromBufferAttribute(position, 1))
				const start = this.projectWorldPointToClient(startWorld, rect)
				const end = this.projectWorldPointToClient(endWorld, rect)
				if (!start || !end) {
					continue
				}
				const hit = getSegmentHitInfo(pointer, start, end)
				if (hit.distanceSquared > maxDistanceSquared) {
					continue
				}
				const depth = THREE.MathUtils.lerp(start.depth, end.depth, hit.ratio)
				if (occludingDepth !== null && depth > occludingDepth + PREVIEW_SELECTION_OCCLUSION_DEPTH_EPSILON) {
					continue
				}
				if (!best || hit.distanceSquared < best.distanceSquared - 1e-6 || (Math.abs(hit.distanceSquared - best.distanceSquared) <= 1e-6 && depth < best.depth)) {
					best = {
						edge,
						distanceSquared: hit.distanceSquared,
						depth
					}
				}
			}
		}
		return best
	}

	private getNearestPreviewSolidDepth(clientX: number, clientY: number, rect: DOMRect): number | null {
		if (!this.setPreviewRaycaster(clientX, clientY)) {
			return null
		}
		const visibleSolids = this.previewSolids.filter((solid) => solid.mesh.visible)
		const intersections = this.previewRaycaster.intersectObjects(
			visibleSolids.map((solid) => solid.mesh),
			false
		)
		const intersection = intersections[0]
		if (!intersection) {
			return null
		}
		return this.projectWorldPointToClient(intersection.point, rect)?.depth ?? null
	}

	private getExtrudeEdgeAt(clientX: number, clientY: number): { extrudeId: string; edgeId: string } | null {
		const edge = this.getPreviewEdgeIntersection(clientX, clientY)?.edge
		if (edge) {
			return {
				extrudeId: edge.extrudeId,
				edgeId: edge.edgeId
			}
		}
		return null
	}

	private projectWorldPointToClient(worldPoint: THREE.Vector3, rect: DOMRect): (Point2D & { depth: number }) | null {
		const projected = worldPoint.clone().project(this.previewCamera)
		if (!Number.isFinite(projected.x) || !Number.isFinite(projected.y) || !Number.isFinite(projected.z) || projected.z < -1 || projected.z > 1) {
			return null
		}
		return {
			x: rect.left + ((projected.x + 1) / 2) * rect.width,
			y: rect.top + ((1 - projected.y) / 2) * rect.height,
			depth: projected.z
		}
	}

	private getOrbitAnchorPoint(clientX: number, clientY: number): THREE.Vector3 | null {
		const sketch = this.getEditableSketch()
		if (sketch) {
			if (sketch.target.type === "face") {
				const faceHit = this.getPreviewFaceIntersection(clientX, clientY, sketch.target.face)
				return faceHit ? this.previewContentGroup.worldToLocal(faceHit.intersectionPoint.clone()) : null
			}
			const selectedPlane = SKETCH_PLANE_TO_REFERENCE_PLANE[sketch.target.plane]
			const planeHit = this.getReferencePlaneIntersection(clientX, clientY, selectedPlane)
			return planeHit ? this.previewContentGroup.worldToLocal(planeHit.intersectionPoint.clone()) : null
		}

		const faceHit = this.getPreviewFaceIntersection(clientX, clientY)
		if (faceHit) {
			return this.previewContentGroup.worldToLocal(faceHit.intersectionPoint.clone())
		}
		const solidHit = this.getPreviewSolidIntersection(clientX, clientY)
		if (solidHit) {
			return this.previewContentGroup.worldToLocal(solidHit.intersectionPoint.clone())
		}
		const planeHit = this.getReferencePlaneIntersection(clientX, clientY)
		if (planeHit) {
			return this.previewContentGroup.worldToLocal(planeHit.intersectionPoint.clone())
		}
		const viewportCenter = this.getPreviewViewportCenterClientPoint()
		if (viewportCenter && (Math.abs(viewportCenter.x - clientX) > 1e-3 || Math.abs(viewportCenter.y - clientY) > 1e-3)) {
			const centerOrbitAnchor = this.getOrbitAnchorPoint(viewportCenter.x, viewportCenter.y)
			if (centerOrbitAnchor) {
				return centerOrbitAnchor
			}
		}
		return this.getPreviewDepthAnchorPoint(viewportCenter?.x ?? clientX, viewportCenter?.y ?? clientY)
	}

	private setPreviewOrbitPivot(nextPivot: THREE.Vector3 | null): void {
		const targetPivot = nextPivot ? nextPivot.clone() : new THREE.Vector3()
		if (this.previewOrbitPivot.distanceToSquared(targetPivot) <= 1e-12) {
			return
		}
		const rotation = new THREE.Euler(this.previewRotation.pitch, this.previewRotation.yaw, 0, this.previewRootGroup.rotation.order)
		const currentRotatedPivot = this.previewOrbitPivot.clone().applyEuler(rotation)
		const nextRotatedPivot = targetPivot.clone().applyEuler(rotation)
		this.previewPan.add(this.previewOrbitPivot).sub(currentRotatedPivot).sub(targetPivot).add(nextRotatedPivot)
		this.previewOrbitPivot.copy(targetPivot)
	}

	private getPreviewDepthAnchorPoint(clientX: number, clientY: number): THREE.Vector3 | null {
		if (!this.setPreviewRaycaster(clientX, clientY)) {
			return null
		}
		const fallbackDistance = Math.max(6, Math.min(24, this.previewBaseDistance * 0.65))
		const cameraForward = this.previewCamera.getWorldDirection(new THREE.Vector3()).normalize()
		const planePoint =
			this.previewOrbitPivot.lengthSq() > 1e-12
				? this.previewContentGroup.localToWorld(this.previewOrbitPivot.clone())
				: this.previewCamera.position.clone().add(cameraForward.clone().multiplyScalar(fallbackDistance))
		const denominator = this.previewRaycaster.ray.direction.dot(cameraForward)
		if (Math.abs(denominator) <= 1e-6) {
			const fallbackPoint = this.previewRaycaster.ray.origin.clone().add(this.previewRaycaster.ray.direction.clone().multiplyScalar(fallbackDistance))
			return this.previewContentGroup.worldToLocal(fallbackPoint)
		}
		const distanceAlongRay = planePoint.clone().sub(this.previewRaycaster.ray.origin).dot(cameraForward) / denominator
		const anchorWorldPoint =
			distanceAlongRay > 0
				? this.previewRaycaster.ray.origin.clone().add(this.previewRaycaster.ray.direction.clone().multiplyScalar(distanceAlongRay))
				: this.previewRaycaster.ray.origin.clone().add(this.previewRaycaster.ray.direction.clone().multiplyScalar(fallbackDistance))
		return this.previewContentGroup.worldToLocal(anchorWorldPoint)
	}

	private getExtrudeAt(clientX: number, clientY: number): string | null {
		return this.getPreviewSolidIntersection(clientX, clientY)?.solid.extrudeId ?? null
	}

	private getSelectedSketchPoint(clientX: number, clientY: number): Point2D | null {
		const sketch = this.getSelectedSketch()
		if (!sketch) {
			return null
		}
		if (sketch.target.type === "plane") {
			const selectedPlane = SKETCH_PLANE_TO_REFERENCE_PLANE[sketch.target.plane]
			if (!this.referencePlaneVisibility[selectedPlane]) {
				return null
			}
			return this.getReferencePlaneIntersection(clientX, clientY, selectedPlane)?.point ?? null
		}
		const hit = this.getPreviewFaceIntersection(clientX, clientY, sketch.target.face)
		return hit ? this.worldPointToSketchPoint(hit.intersectionPoint, hit.face.frame) : null
	}

	private getPreviewSolidIntersection(clientX: number, clientY: number): { solid: PreviewSolidVisual; intersectionPoint: THREE.Vector3 } | null {
		if (!this.setPreviewRaycaster(clientX, clientY)) {
			return null
		}
		const visibleSolids = this.previewSolids.filter((solid) => solid.mesh.visible)
		const intersections = this.previewRaycaster.intersectObjects(
			visibleSolids.map((solid) => solid.mesh),
			false
		)
		const intersection = intersections[0]
		const mesh = intersection?.object
		if (!(mesh instanceof THREE.Mesh) || !intersection) {
			return null
		}
		const solid = visibleSolids.find((entry) => entry.mesh === mesh)
		return solid
			? {
					solid,
					intersectionPoint: intersection.point.clone()
				}
			: null
	}

	private getReferencePlaneIntersection(clientX: number, clientY: number, onlyPlane?: ReferencePlaneName): { name: ReferencePlaneName; point: Point2D; intersectionPoint: THREE.Vector3 } | null {
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
			},
			intersectionPoint: intersection.point.clone()
		}
	}

	private setPreviewRaycaster(clientX: number, clientY: number): boolean {
		const pointer = this.getPreviewPointerPosition(clientX, clientY)
		if (!pointer) {
			return false
		}
		this.previewPointer.copy(pointer)
		this.syncPreviewView()
		this.previewCamera.updateMatrixWorld()
		this.previewScene.updateMatrixWorld(true)
		this.previewRaycaster.setFromCamera(this.previewPointer, this.previewCamera)
		return true
	}

	private focusReferencePlaneForSketch(planeName: ReferencePlaneName | null, options?: { resetView?: boolean }): void {
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
		const shouldResetView = options?.resetView === true || (options?.resetView !== false && this.isPreviewViewAtDefaultFraming())
		if (!shouldResetView) {
			return
		}
		this.previewPan.set(0, 0, 0)
		this.previewOrbitPivot.set(0, 0, 0)
		this.previewBaseDistance = Math.min(this.previewBaseDistance, 28)
	}

	private isPreviewViewAtDefaultFraming(): boolean {
		return this.previewPan.lengthSq() <= 1e-12 && this.previewOrbitPivot.lengthSq() <= 1e-12 && Math.abs(this.previewBaseDistance - PART_PROJECT_DEFAULT_PREVIEW_DISTANCE) <= 1e-9
	}

	private syncPreviewGeometry(): void {
		for (const solid of this.previewSolids) {
			this.previewSolidsGroup.remove(solid.mesh)
			for (const edge of solid.edges) {
				this.previewSolidsGroup.remove(edge.line)
				this.previewSolidsGroup.remove(edge.highlight)
				edge.line.geometry.dispose()
				disposeMaterial(edge.line.material)
				edge.highlight.geometry.dispose()
				disposeMaterial(edge.highlight.material)
			}
			for (const corner of solid.corners) {
				this.previewSolidsGroup.remove(corner.marker)
				corner.marker.geometry.dispose()
				disposeMaterial(corner.marker.material)
			}
			for (const face of solid.faces) {
				this.previewSolidsGroup.remove(face.mesh)
				face.mesh.geometry.dispose()
				disposeMaterial(face.mesh.material)
			}
			solid.mesh.geometry.dispose()
			disposeMaterial(solid.mesh.material)
		}
		this.previewSolids.length = 0

		this.syncPreviewSketchGeometry()

		const partState: PartProjectItemData = {
			features: structuredClone(this.features) as PartFeature[]
		}
		const nextSolids: Solid[] = []
		for (const extrude of this.features.filter((feature): feature is SolidExtrude => feature.type === "extrude")) {
			try {
				const extrusion = extrudeSolidFeature(partState, extrude)
				nextSolids.push(structuredClone(extrusion.solid) as Solid)
				const chamfers = this.features.filter((feature): feature is SolidChamfer => feature.type === "chamfer" && feature.target.edge.extrudeId === extrude.id)
				const visual = this.createExtrudedVisual(extrusion, extrude.id, chamfers)
				this.previewSolids.push(visual)
				this.previewSolidsGroup.add(visual.mesh)
				for (const edge of visual.edges) {
					this.previewSolidsGroup.add(edge.line)
					this.previewSolidsGroup.add(edge.highlight)
				}
				for (const corner of visual.corners) {
					this.previewSolidsGroup.add(corner.marker)
				}
				for (const face of visual.faces) {
					this.previewSolidsGroup.add(face.mesh)
				}
			} catch (_error) {
				// Skip invalid extrusions during preview replay.
			}
		}
		this.solids = nextSolids

		if (this.selectedFaceId && !this.previewSolids.some((solid) => solid.faces.some((face) => face.faceId === this.selectedFaceId))) {
			this.selectedFaceId = null
		}
		if (this.selectedEdgeId && !this.previewSolids.some((solid) => solid.extrudeId === this.selectedExtrudeId && solid.edges.some((edge) => edge.edgeId === this.selectedEdgeId))) {
			this.selectedEdgeId = null
		}
		if (
			this.selectedCornerId &&
			!this.previewSolids.some((solid) => solid.extrudeId === this.selectedExtrudeId && solid.corners.some((corner) => corner.cornerId === this.selectedCornerId))
		) {
			this.selectedCornerId = null
		}
		if (this.hoveredFaceId && !this.previewSolids.some((solid) => solid.faces.some((face) => face.faceId === this.hoveredFaceId))) {
			this.hoveredFaceId = null
		}
		if (this.hoveredEdgeId && !this.previewSolids.some((solid) => solid.edges.some((edge) => edge.edgeId === this.hoveredEdgeId))) {
			this.hoveredEdgeId = null
		}
		if (this.hoveredCornerId && !this.previewSolids.some((solid) => solid.corners.some((corner) => corner.cornerId === this.hoveredCornerId))) {
			this.hoveredCornerId = null
		}

		this.refreshCornerStyles()
		this.refreshEdgeStyles()
		this.refreshFaceStyles()
		this.refreshSolidStyles()
		this.refreshReferencePlaneStyles()
		this.syncSketchDraftPreview()
		this.drawPreview()
	}

	private syncPreviewSketchGeometry(): void {
		while (this.previewSketchGroup.children.length > 0) {
			const child = this.previewSketchGroup.children[0]
			if (!child) {
				break
			}
			this.previewSketchGroup.remove(child)
			disposeObject3D(child)
		}

		if (!this.sketchVisible) {
			return
		}

		for (const sketch of this.features.filter((feature): feature is Sketch => feature.type === "sketch")) {
			const sketchVisual = this.createSketchVisual(sketch)
			if (sketchVisual) {
				this.previewSketchGroup.add(sketchVisual)
			}
		}
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
			const draft = this.createDraftLineVisual(sketch.target, this.pendingLineStart, this.sketchHoverPoint)
			if (draft) {
				this.previewSketchDraftGroup.add(draft)
			}
		}

		if (this.pendingRectangleStart) {
			const draft = this.createDraftRectangleVisual(sketch.target, this.pendingRectangleStart, this.sketchHoverPoint)
			if (draft) {
				this.previewSketchDraftGroup.add(draft)
			}
		}
	}

	private createSketchVisual(sketch: Sketch): THREE.Object3D | null {
		if (sketch.entities.length === 0) {
			return null
		}
		const frame = this.resolveSketchFrame(sketch.target)
		if (!frame) {
			return null
		}
		const isSelected = sketch.id === this.selectedSketchId
		const sketchColor = isSelected ? (sketch.target.type === "face" ? 0xf8fafc : 0x1d4ed8) : sketch.dirty ? 0x93c5fd : 0xcbd5e1
		const markerColor = isSelected ? (sketch.target.type === "face" ? 0xffffff : 0x1d4ed8) : sketchColor

		const segments: number[] = []
		for (const entity of sketch.entities) {
			if (entity.type === "line") {
				const start = this.sketchPointToPreviewWorld(entity.p0, frame)
				const end = this.sketchPointToPreviewWorld(entity.p1, frame)
				segments.push(start.x, start.y, start.z, end.x, end.y, end.z)
				continue
			}
			const corners = rectangleCorners(entity)
			for (let index = 0; index < corners.length; index += 1) {
				const start = this.sketchPointToPreviewWorld(corners[index] ?? corners[0] ?? { x: 0, y: 0 }, frame)
				const end = this.sketchPointToPreviewWorld(corners[(index + 1) % corners.length] ?? corners[0] ?? { x: 0, y: 0 }, frame)
				segments.push(start.x, start.y, start.z, end.x, end.y, end.z)
			}
		}
		if (segments.length === 0) {
			return null
		}

		const geometry = new THREE.BufferGeometry()
		geometry.setAttribute("position", new THREE.Float32BufferAttribute(segments, 3))
		const material = new THREE.LineBasicMaterial({
			color: sketchColor,
			transparent: true,
			opacity: isSelected ? 1 : sketch.dirty ? 0.95 : 0.8,
			depthWrite: false,
			depthTest: !isSelected
		})
		const lines = new THREE.LineSegments(geometry, material)
		lines.renderOrder = 6

		const group = new THREE.Group()
		group.add(lines)
		if (isSelected) {
			for (const point of this.getSketchEntityPoints(sketch)) {
				group.add(this.createSketchPointMarker(point, frame, markerColor))
			}
		}
		if (isSelected && this.selectedSketchEdge) {
			const selectedEdge = this.createSelectedSketchEdgeVisual(sketch, this.selectedSketchEdge, frame)
			if (selectedEdge) {
				group.add(selectedEdge)
			}
		}
		if (isSelected) {
			for (const dimension of sketch.dimensions) {
				const label = this.createSketchDimensionVisual(sketch, dimension, frame)
				if (label) {
					group.add(label)
				}
			}
		}
		if (sketch.name) {
			const label = this.createSketchLabelSprite(sketch.name)
			const labelPoint = sketch.vertices[0] ?? extractSketchLabelPoint(sketch)
			const localPoint = this.sketchPointToPreviewWorld(labelPoint, frame, PREVIEW_SKETCH_LABEL_OFFSET)
			label.position.set(localPoint.x, localPoint.y, localPoint.z)
			group.add(label)
		}
		return group
	}

	private createSelectedSketchEdgeVisual(sketch: Sketch, selection: SketchEdgeSelection, frame: SketchFrame3D): THREE.Object3D | null {
		const entity = sketch.entities.find((candidate) => candidate.id === selection.entityId)
		if (!entity) {
			return null
		}

		const points =
			selection.type === "line" && entity.type === "line"
				? [entity.p0, entity.p1]
				: selection.type === "rectangleSide" && entity.type === "cornerRectangle"
					? (() => {
							const segment = getRectangleSideSegment(entity, selection.side)
							return [segment.start, segment.end]
						})()
					: null
		if (!points || points.length !== 2) {
			return null
		}
		const startPoint = points[0]
		const endPoint = points[1]
		if (!startPoint || !endPoint) {
			return null
		}

		const start = this.sketchPointToPreviewWorld(startPoint, frame, PREVIEW_SKETCH_MARKER_OFFSET)
		const end = this.sketchPointToPreviewWorld(endPoint, frame, PREVIEW_SKETCH_MARKER_OFFSET)
		const geometry = new THREE.BufferGeometry().setFromPoints([start, end])
		const material = new THREE.LineBasicMaterial({
			color: 0xf59e0b,
			transparent: true,
			opacity: 1,
			depthWrite: false,
			depthTest: false
		})
		const highlight = new THREE.Line(geometry, material)
		highlight.renderOrder = 9

		const group = new THREE.Group()
		group.add(highlight)
		group.add(this.createSketchPointMarker(startPoint, frame, 0xf59e0b))
		group.add(this.createSketchPointMarker(endPoint, frame, 0xf59e0b))
		return group
	}

	private createSketchDimensionVisual(sketch: Sketch, dimension: SketchDimension, frame: SketchFrame3D): THREE.Object3D | null {
		const labelPoint = getSketchDimensionLabelPoint(sketch, dimension)
		if (!labelPoint) {
			return null
		}

		const isSelected = isSketchDimensionSelected(this.selectedSketchEdge, dimension)
		const sprite = this.createDimensionLabelSprite(formatSketchDimensionValue(dimension.value), isSelected)
		const localPoint = this.sketchPointToPreviewWorld(labelPoint, frame, PREVIEW_SKETCH_LABEL_OFFSET + 0.02)
		sprite.position.copy(localPoint)
		sprite.renderOrder = 10
		return sprite
	}

	private createDraftLineVisual(target: Sketch["target"], start: Point2D, end: Point2D | null): THREE.Object3D | null {
		const frame = this.resolveSketchFrame(target)
		if (!frame) {
			return null
		}
		const group = new THREE.Group()
		const startMarker = this.createSketchPointMarker(start, frame, 0xf59e0b)
		group.add(startMarker)
		if (!end) {
			return group
		}

		const geometry = new THREE.BufferGeometry().setFromPoints([
			this.sketchPointToPreviewWorld(start, frame, PREVIEW_SKETCH_MARKER_OFFSET),
			this.sketchPointToPreviewWorld(end, frame, PREVIEW_SKETCH_MARKER_OFFSET)
		])
		const material = new THREE.LineBasicMaterial({
			color: 0xf59e0b,
			transparent: true,
			opacity: 0.9,
			depthWrite: false,
			depthTest: false
		})
		const line = new THREE.Line(geometry, material)
		line.renderOrder = 8
		group.add(line)
		group.add(this.createSketchPointMarker(end, frame, 0xf59e0b))
		return group
	}

	private createDraftRectangleVisual(target: Sketch["target"], start: Point2D, end: Point2D | null): THREE.Object3D | null {
		const frame = this.resolveSketchFrame(target)
		if (!frame) {
			return null
		}
		const group = new THREE.Group()
		group.add(this.createSketchPointMarker(start, frame, 0xf59e0b))
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
			const segmentStart = this.sketchPointToPreviewWorld(corners[index] ?? corners[0] ?? { x: 0, y: 0 }, frame, PREVIEW_SKETCH_MARKER_OFFSET)
			const segmentEnd = this.sketchPointToPreviewWorld(corners[(index + 1) % corners.length] ?? corners[0] ?? { x: 0, y: 0 }, frame, PREVIEW_SKETCH_MARKER_OFFSET)
			segments.push(segmentStart.x, segmentStart.y, segmentStart.z, segmentEnd.x, segmentEnd.y, segmentEnd.z)
		}

		const geometry = new THREE.BufferGeometry()
		geometry.setAttribute("position", new THREE.Float32BufferAttribute(segments, 3))
		const material = new THREE.LineBasicMaterial({
			color: 0xf59e0b,
			transparent: true,
			opacity: 0.9,
			depthWrite: false,
			depthTest: false
		})
		const lines = new THREE.LineSegments(geometry, material)
		lines.renderOrder = 8
		group.add(lines)
		for (const point of corners) {
			group.add(this.createSketchPointMarker(point, frame, 0xf59e0b))
		}
		return group
	}

	private createSketchPointMarker(point: Point2D, frame: SketchFrame3D, color: number): THREE.Mesh {
		const marker = new THREE.Mesh(
			new THREE.SphereGeometry(0.12, 16, 12),
			new THREE.MeshBasicMaterial({
				color,
				depthWrite: false,
				depthTest: false
			})
		)
		const localPoint = this.sketchPointToPreviewWorld(point, frame, PREVIEW_SKETCH_MARKER_OFFSET)
		marker.position.set(localPoint.x, localPoint.y, localPoint.z)
		marker.renderOrder = 8
		return marker
	}

	private createExtrudedVisual(extrusion: ExtrudedSolid, extrudeId: string, chamfers: SolidChamfer[] = []): PreviewSolidVisual {
		const geometry = createExtrudedMeshGeometry(extrusion, chamfers)
		const material = new THREE.MeshStandardMaterial({
			color: 0x3b82f6,
			roughness: 0.35,
			metalness: 0.05,
			side: THREE.DoubleSide
		})
		const mesh = new THREE.Mesh(geometry, material)
		const sketchQuaternion = getSketchFrameQuaternion(extrusion.frame)
		mesh.position.copy(vector3DToThree(extrusion.frame.origin))
		mesh.quaternion.copy(sketchQuaternion)

		const edges = createPreviewEdgeVisuals(extrusion, extrudeId)
		const corners = createPreviewCornerVisuals(extrusion, extrudeId)
		const faces = createPreviewFaceVisuals(extrusion, extrudeId, mesh.position, sketchQuaternion)
		return {
			extrudeId,
			mesh,
			fillMaterial: material,
			edges,
			corners,
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

	private createSketchLabelSprite(text: string): THREE.Sprite {
		const canvas = document.createElement("canvas")
		canvas.width = 208
		canvas.height = 64
		const ctx = canvas.getContext("2d")
		if (ctx) {
			ctx.clearRect(0, 0, canvas.width, canvas.height)
			ctx.fillStyle = "rgba(248,250,252,0.94)"
			ctx.strokeStyle = "rgba(15,23,42,0.16)"
			ctx.lineWidth = 3
			if (typeof ctx.roundRect === "function") {
				ctx.beginPath()
				ctx.roundRect(10, 10, canvas.width - 20, canvas.height - 20, 18)
				ctx.fill()
				ctx.stroke()
			} else {
				ctx.fillRect(10, 10, canvas.width - 20, canvas.height - 20)
				ctx.strokeRect(10, 10, canvas.width - 20, canvas.height - 20)
			}
			ctx.fillStyle = "#0f172a"
			ctx.font = "700 24px sans-serif"
			ctx.textAlign = "center"
			ctx.textBaseline = "middle"
			ctx.fillText(text, canvas.width / 2, canvas.height / 2)
		}
		const texture = new THREE.CanvasTexture(canvas)
		texture.minFilter = THREE.LinearFilter
		texture.magFilter = THREE.LinearFilter
		texture.generateMipmaps = false
		const material = new THREE.SpriteMaterial({
			map: texture,
			transparent: true,
			depthWrite: false,
			depthTest: false
		})
		const sprite = new THREE.Sprite(material)
		sprite.scale.set(0.82, 0.25, 1)
		return sprite
	}

	private createDimensionLabelSprite(text: string, isSelected: boolean): THREE.Sprite {
		const canvas = document.createElement("canvas")
		canvas.width = 196
		canvas.height = 64
		const ctx = canvas.getContext("2d")
		if (ctx) {
			ctx.clearRect(0, 0, canvas.width, canvas.height)
			ctx.fillStyle = isSelected ? "#f59e0b" : "rgba(255,255,255,0.95)"
			ctx.strokeStyle = isSelected ? "#f59e0b" : "#cbd5e1"
			ctx.lineWidth = 3
			ctx.fillRect(10, 12, canvas.width - 20, canvas.height - 24)
			ctx.strokeRect(10, 12, canvas.width - 20, canvas.height - 24)
			ctx.fillStyle = isSelected ? "#ffffff" : "#0f172a"
			ctx.font = "700 24px sans-serif"
			ctx.textAlign = "center"
			ctx.textBaseline = "middle"
			ctx.fillText(text, canvas.width / 2, canvas.height / 2)
		}
		const texture = new THREE.CanvasTexture(canvas)
		texture.minFilter = THREE.LinearFilter
		texture.magFilter = THREE.LinearFilter
		texture.generateMipmaps = false
		const material = new THREE.SpriteMaterial({
			map: texture,
			transparent: true,
			depthWrite: false,
			depthTest: false
		})
		const sprite = new THREE.Sprite(material)
		sprite.scale.set(1.5, 0.5, 1)
		return sprite
	}

	private getSketchEntityPoints(sketch: Sketch): Point2D[] {
		const uniquePoints = new Map<string, Point2D>()
		for (const entity of sketch.entities) {
			const points = entity.type === "line" ? [entity.p0, entity.p1] : rectangleCorners(entity)
			for (const point of points) {
				uniquePoints.set(`${point.x}:${point.y}`, point)
			}
		}
		return [...uniquePoints.values()]
	}

	private sketchPointToPreviewWorld(point: Point2D, frame: SketchFrame3D, offset = PREVIEW_SKETCH_SURFACE_OFFSET): THREE.Vector3 {
		const localPoint = sketchPointToWorld(point, frame)
		return localPoint.add(vector3DToThree(frame.normal).normalize().multiplyScalar(offset))
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
			const hasSelectedFace = solid.extrudeId === this.selectedExtrudeId && !!this.selectedFaceId
			const hasSelectedEdge = solid.extrudeId === this.selectedExtrudeId && !!this.selectedEdgeId
			const hasSelectedCorner = solid.extrudeId === this.selectedExtrudeId && !!this.selectedCornerId
			const isSelected = solid.extrudeId === this.selectedExtrudeId && !hasSelectedFace && !hasSelectedEdge && !hasSelectedCorner
			const isHovered = solid.extrudeId === this.hoveredExtrudeId
			if (isSelected) {
				solid.fillMaterial.color.setHex(0xf59e0b)
				continue
			}
			if (hasSelectedFace || hasSelectedEdge || hasSelectedCorner) {
				solid.fillMaterial.color.setHex(0x3b82f6)
				continue
			}
			if (isHovered) {
				solid.fillMaterial.color.setHex(0x60a5fa)
				continue
			}
			solid.fillMaterial.color.setHex(0x3b82f6)
		}
	}

	private refreshCornerStyles(): void {
		for (const solid of this.previewSolids) {
			for (const corner of solid.corners) {
				const isSelected = solid.extrudeId === this.selectedExtrudeId && corner.cornerId === this.selectedCornerId
				const isHovered = corner.cornerId === this.hoveredCornerId
				if (isSelected) {
					corner.marker.visible = true
					corner.marker.scale.setScalar(PREVIEW_CORNER_SELECTED_RADIUS)
					corner.material.color.setHex(0xf59e0b)
					corner.material.opacity = 0.96
					continue
				}
				if (isHovered) {
					corner.marker.visible = true
					corner.marker.scale.setScalar(PREVIEW_CORNER_HOVER_RADIUS)
					corner.material.color.setHex(0x7dd3fc)
					corner.material.opacity = 0.85
					continue
				}
				corner.marker.visible = false
			}
		}
	}

	private refreshEdgeStyles(): void {
		for (const solid of this.previewSolids) {
			const isSolidSelected = solid.extrudeId === this.selectedExtrudeId && !this.selectedFaceId && !this.selectedEdgeId && !this.selectedCornerId
			const isSolidHovered = solid.extrudeId === this.hoveredExtrudeId && !this.hoveredFaceId && !this.hoveredEdgeId && !this.hoveredCornerId
			for (const edge of solid.edges) {
				const isSelected = solid.extrudeId === this.selectedExtrudeId && edge.edgeId === this.selectedEdgeId
				const isHovered = edge.edgeId === this.hoveredEdgeId
				if (isSelected) {
					edge.material.color.setHex(0xf59e0b)
					edge.material.opacity = 1
					edge.highlight.visible = true
					edge.highlight.scale.set(PREVIEW_EDGE_SELECTED_RADIUS, 1, PREVIEW_EDGE_SELECTED_RADIUS)
					edge.highlightMaterial.color.setHex(0xf59e0b)
					edge.highlightMaterial.opacity = 0.96
					continue
				}
				if (isHovered) {
					edge.material.color.setHex(0x7dd3fc)
					edge.material.opacity = 1
					edge.highlight.visible = true
					edge.highlight.scale.set(PREVIEW_EDGE_HOVER_RADIUS, 1, PREVIEW_EDGE_HOVER_RADIUS)
					edge.highlightMaterial.color.setHex(0x7dd3fc)
					edge.highlightMaterial.opacity = 0.85
					continue
				}
				edge.highlight.visible = false
				if (isSolidSelected) {
					edge.material.color.setHex(0x7c2d12)
					edge.material.opacity = 1
					continue
				}
				if (isSolidHovered) {
					edge.material.color.setHex(0xffffff)
					edge.material.opacity = 1
					continue
				}
				edge.material.color.setHex(0xe2e8f0)
				edge.material.opacity = 0.8
			}
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
		this.previewCanvas.style.cursor = this.hoveredCornerId || this.hoveredEdgeId || this.hoveredFaceId || this.hoveredExtrudeId || this.hoveredReferencePlane ? "pointer" : "grab"
	}

	private getPreviewFaceLabel(extrudeId: string, faceId: string): string | null {
		return this.previewSolids.find((solid) => solid.extrudeId === extrudeId)?.faces.find((face) => face.faceId === faceId)?.label ?? null
	}

	private getPreviewEdgeLabel(extrudeId: string, edgeId: string): string | null {
		return this.previewSolids.find((solid) => solid.extrudeId === extrudeId)?.edges.find((edge) => edge.edgeId === edgeId)?.label ?? null
	}

	private getPreviewCornerLabel(extrudeId: string, cornerId: string): string | null {
		return this.previewSolids.find((solid) => solid.extrudeId === extrudeId)?.corners.find((corner) => corner.cornerId === cornerId)?.label ?? null
	}

	private syncPreviewView(): void {
		this.previewCamera.position.z = THREE.MathUtils.clamp(this.previewBaseDistance, PREVIEW_MIN_CAMERA_DISTANCE, PREVIEW_MAX_CAMERA_DISTANCE)
		this.previewRootGroup.position.copy(this.previewPan).add(this.previewOrbitPivot)
		this.previewRootGroup.rotation.set(this.previewRotation.pitch, this.previewRotation.yaw, 0)
		this.previewContentGroup.position.copy(this.previewOrbitPivot).multiplyScalar(-1)
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

	private emitViewStateChange(): void {
		this.onViewStateChange?.(this.getViewState())
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

function getPlaneQuaternion(plane: "XY" | "YZ" | "XZ"): THREE.Quaternion {
	switch (plane) {
		case "XZ":
			return new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(new THREE.Vector3(1, 0, 0), new THREE.Vector3(0, 0, -1), new THREE.Vector3(0, 1, 0)))
		case "YZ":
			return new THREE.Quaternion().setFromRotationMatrix(new THREE.Matrix4().makeBasis(new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, 0, 1), new THREE.Vector3(1, 0, 0)))
		default:
			return new THREE.Quaternion()
	}
}

type PreviewTriangle = [THREE.Vector3, THREE.Vector3, THREE.Vector3]

type ChamferClipPlane = {
	normal: THREE.Vector3
	constant: number
}

type ChamferEdgeDescriptor =
	| {
			kind: "bottom" | "top"
			edgeId: string
			point: Point2D
			inwardNormal: THREE.Vector2
	  }
	| {
			kind: "vertical"
			edgeId: string
			point: Point2D
			previousInwardNormal: THREE.Vector2
			nextInwardNormal: THREE.Vector2
	  }

function createExtrudedMeshGeometry(extrusion: ExtrudedSolid, chamfers: SolidChamfer[]): THREE.BufferGeometry {
	const baseGeometry = createBaseExtrudedGeometry(extrusion)
	const clipPlanes = createChamferClipPlanes(extrusion, chamfers)
	if (clipPlanes.length === 0) {
		return baseGeometry
	}

	const clippedGeometry = clipGeometryWithPlanes(baseGeometry, clipPlanes)
	baseGeometry.dispose()
	return clippedGeometry
}

function createBaseExtrudedGeometry(extrusion: ExtrudedSolid): THREE.BufferGeometry {
	const outerLoop = extrusion.profileLoops[0] ?? []
	const shape = new THREE.Shape(outerLoop.map((point) => new THREE.Vector2(point.x, point.y)))
	for (const hole of extrusion.profileLoops.slice(1)) {
		shape.holes.push(new THREE.Path(hole.map((point) => new THREE.Vector2(point.x, point.y))))
	}
	return new THREE.ExtrudeGeometry(shape, {
		depth: extrusion.depth,
		bevelEnabled: false,
		steps: 1
	})
}

function createChamferClipPlanes(extrusion: ExtrudedSolid, chamfers: SolidChamfer[]): ChamferClipPlane[] {
	const descriptors = createChamferEdgeDescriptors(extrusion)
	const planes: ChamferClipPlane[] = []
	for (const chamfer of chamfers) {
		const descriptor = descriptors.get(chamfer.target.edge.edgeId)
		if (!descriptor || !Number.isFinite(chamfer.d1) || chamfer.d1 <= 0) {
			continue
		}
		planes.push(createChamferClipPlane(descriptor, extrusion.depth, chamfer.d1))
	}
	return planes
}

function createChamferEdgeDescriptors(extrusion: ExtrudedSolid): Map<string, ChamferEdgeDescriptor> {
	const descriptors = new Map<string, ChamferEdgeDescriptor>()
	let edgeIndex = 0

	for (const loop of extrusion.profileLoops) {
		const area = signedPolygonArea(loop)
		const outwardNormals = loop.map((point, pointIndex) => getOutwardSegmentNormal(point, loop[(pointIndex + 1) % loop.length] ?? point, area))

		for (let pointIndex = 0; pointIndex < loop.length; pointIndex += 1) {
			const point = loop[pointIndex]
			const nextPoint = loop[(pointIndex + 1) % loop.length]
			const outwardNormal = outwardNormals[pointIndex]
			if (!point || !nextPoint || !outwardNormal) {
				edgeIndex += 4
				continue
			}

			const bottomEdge = extrusion.solid.edges[edgeIndex]
			const topEdge = extrusion.solid.edges[edgeIndex + 1]
			const sideStartEdge = extrusion.solid.edges[edgeIndex + 2]
			const sideEndEdge = extrusion.solid.edges[edgeIndex + 3]
			const inwardNormal = outwardNormal.clone().multiplyScalar(-1)

			if (bottomEdge) {
				descriptors.set(bottomEdge.id, {
					kind: "bottom",
					edgeId: bottomEdge.id,
					point,
					inwardNormal
				})
			}
			if (topEdge) {
				descriptors.set(topEdge.id, {
					kind: "top",
					edgeId: topEdge.id,
					point,
					inwardNormal
				})
			}
			if (sideStartEdge) {
				descriptors.set(sideStartEdge.id, createVerticalChamferEdgeDescriptor(sideStartEdge.id, loop, outwardNormals, pointIndex))
			}
			if (sideEndEdge) {
				descriptors.set(sideEndEdge.id, createVerticalChamferEdgeDescriptor(sideEndEdge.id, loop, outwardNormals, (pointIndex + 1) % loop.length))
			}

			edgeIndex += 4
		}
	}

	return descriptors
}

function createVerticalChamferEdgeDescriptor(edgeId: string, loop: Point2D[], outwardNormals: THREE.Vector2[], pointIndex: number): ChamferEdgeDescriptor {
	const point = loop[pointIndex] ?? { x: 0, y: 0 }
	const previousNormal = outwardNormals[(pointIndex - 1 + outwardNormals.length) % outwardNormals.length] ?? new THREE.Vector2(0, 0)
	const nextNormal = outwardNormals[pointIndex] ?? new THREE.Vector2(0, 0)
	return {
		kind: "vertical",
		edgeId,
		point,
		previousInwardNormal: previousNormal.clone().multiplyScalar(-1),
		nextInwardNormal: nextNormal.clone().multiplyScalar(-1)
	}
}

function createChamferClipPlane(descriptor: ChamferEdgeDescriptor, depth: number, distance: number): ChamferClipPlane {
	if (descriptor.kind === "vertical") {
		const normal = new THREE.Vector3(descriptor.previousInwardNormal.x + descriptor.nextInwardNormal.x, descriptor.previousInwardNormal.y + descriptor.nextInwardNormal.y, 0)
		const pointOnEdge = new THREE.Vector3(descriptor.point.x, descriptor.point.y, 0)
		return {
			normal,
			constant: -normal.dot(pointOnEdge) - distance
		}
	}

	const z = descriptor.kind === "bottom" ? 0 : depth
	const zDirection = descriptor.kind === "bottom" ? 1 : -1
	const normal = new THREE.Vector3(descriptor.inwardNormal.x, descriptor.inwardNormal.y, zDirection)
	const pointOnEdge = new THREE.Vector3(descriptor.point.x, descriptor.point.y, z)
	return {
		normal,
		constant: -normal.dot(pointOnEdge) - distance
	}
}

function clipGeometryWithPlanes(geometry: THREE.BufferGeometry, planes: ChamferClipPlane[]): THREE.BufferGeometry {
	let triangles = extractGeometryTriangles(geometry)
	for (const plane of planes) {
		const clipped = clipTrianglesWithPlane(triangles, plane)
		triangles = [...clipped.triangles, ...createCapTriangles(clipped.capPoints, plane)]
	}
	return createGeometryFromTriangles(triangles)
}

function extractGeometryTriangles(geometry: THREE.BufferGeometry): PreviewTriangle[] {
	const positions = geometry.getAttribute("position")
	const index = geometry.getIndex()
	const triangles: PreviewTriangle[] = []
	if (!positions) {
		return triangles
	}

	const readVertex = (vertexIndex: number) => new THREE.Vector3().fromBufferAttribute(positions, vertexIndex)
	if (index) {
		for (let i = 0; i + 2 < index.count; i += 3) {
			triangles.push([readVertex(index.getX(i)), readVertex(index.getX(i + 1)), readVertex(index.getX(i + 2))])
		}
		return triangles
	}

	for (let i = 0; i + 2 < positions.count; i += 3) {
		triangles.push([readVertex(i), readVertex(i + 1), readVertex(i + 2)])
	}
	return triangles
}

function clipTrianglesWithPlane(triangles: PreviewTriangle[], plane: ChamferClipPlane): { triangles: PreviewTriangle[]; capPoints: THREE.Vector3[] } {
	const clippedTriangles: PreviewTriangle[] = []
	const capPoints: THREE.Vector3[] = []
	for (const triangle of triangles) {
		const clippedPolygon = clipPolygonWithPlane(triangle, plane, capPoints)
		for (let index = 1; index + 1 < clippedPolygon.length; index += 1) {
			clippedTriangles.push([
				clippedPolygon[0]?.clone() ?? new THREE.Vector3(),
				clippedPolygon[index]?.clone() ?? new THREE.Vector3(),
				clippedPolygon[index + 1]?.clone() ?? new THREE.Vector3()
			])
		}
	}
	return {
		triangles: clippedTriangles,
		capPoints
	}
}

function clipPolygonWithPlane(polygon: THREE.Vector3[], plane: ChamferClipPlane, capPoints: THREE.Vector3[]): THREE.Vector3[] {
	const clipped: THREE.Vector3[] = []
	const epsilon = 1e-7
	for (let index = 0; index < polygon.length; index += 1) {
		const current = polygon[index]
		const previous = polygon[(index - 1 + polygon.length) % polygon.length]
		if (!current || !previous) {
			continue
		}

		const currentDistance = signedPlaneDistance(current, plane)
		const previousDistance = signedPlaneDistance(previous, plane)
		const currentInside = currentDistance >= -epsilon
		const previousInside = previousDistance >= -epsilon

		if (currentInside !== previousInside) {
			const intersection = interpolatePlaneIntersection(previous, current, previousDistance, currentDistance)
			clipped.push(intersection)
			capPoints.push(intersection.clone())
		}
		if (currentInside) {
			clipped.push(current.clone())
		}
	}
	return clipped
}

function createCapTriangles(points: THREE.Vector3[], plane: ChamferClipPlane): PreviewTriangle[] {
	const uniquePoints = uniqueVectorPoints(points)
	if (uniquePoints.length < 3) {
		return []
	}

	const centroid = uniquePoints.reduce((sum, point) => sum.add(point), new THREE.Vector3()).multiplyScalar(1 / uniquePoints.length)
	const planeNormal = plane.normal.clone().normalize()
	const capNormal = planeNormal.clone().multiplyScalar(-1)
	const basisU = getPerpendicularUnitVector(planeNormal)
	const basisV = new THREE.Vector3().crossVectors(planeNormal, basisU).normalize()
	const ordered = uniquePoints
		.map((point) => ({
			point,
			angle: Math.atan2(point.clone().sub(centroid).dot(basisV), point.clone().sub(centroid).dot(basisU))
		}))
		.sort((a, b) => a.angle - b.angle)
		.map((entry) => entry.point)

	const triangles: PreviewTriangle[] = []
	for (let index = 1; index + 1 < ordered.length; index += 1) {
		const a = ordered[0]?.clone()
		const b = ordered[index]?.clone()
		const c = ordered[index + 1]?.clone()
		if (!a || !b || !c || areTrianglePointsCollinear(a, b, c)) {
			continue
		}
		const normal = new THREE.Vector3().crossVectors(b.clone().sub(a), c.clone().sub(a))
		triangles.push(normal.dot(capNormal) >= 0 ? [a, b, c] : [a, c, b])
	}
	return triangles
}

function createGeometryFromTriangles(triangles: PreviewTriangle[]): THREE.BufferGeometry {
	const coordinates: number[] = []
	for (const triangle of triangles) {
		for (const point of triangle) {
			coordinates.push(point.x, point.y, point.z)
		}
	}
	const geometry = new THREE.BufferGeometry()
	geometry.setAttribute("position", new THREE.Float32BufferAttribute(coordinates, 3))
	geometry.computeVertexNormals()
	return geometry
}

function uniqueVectorPoints(points: THREE.Vector3[]): THREE.Vector3[] {
	const seen = new Set<string>()
	const unique: THREE.Vector3[] = []
	for (const point of points) {
		const key = `${point.x.toFixed(6)}:${point.y.toFixed(6)}:${point.z.toFixed(6)}`
		if (seen.has(key)) {
			continue
		}
		seen.add(key)
		unique.push(point.clone())
	}
	return unique
}

function getOutwardSegmentNormal(start: Point2D, end: Point2D, polygonArea: number): THREE.Vector2 {
	const direction = new THREE.Vector2(end.x - start.x, end.y - start.y)
	if (direction.lengthSq() <= 1e-12) {
		return new THREE.Vector2(0, 0)
	}
	direction.normalize()
	return polygonArea >= 0 ? new THREE.Vector2(direction.y, -direction.x) : new THREE.Vector2(-direction.y, direction.x)
}

function signedPolygonArea(points: Point2D[]): number {
	let area = 0
	for (let index = 0; index < points.length; index += 1) {
		const current = points[index]
		const next = points[(index + 1) % points.length]
		if (!current || !next) {
			continue
		}
		area += current.x * next.y - next.x * current.y
	}
	return area / 2
}

function signedPlaneDistance(point: THREE.Vector3, plane: ChamferClipPlane): number {
	return plane.normal.dot(point) + plane.constant
}

function interpolatePlaneIntersection(a: THREE.Vector3, b: THREE.Vector3, aDistance: number, bDistance: number): THREE.Vector3 {
	const denominator = aDistance - bDistance
	const ratio = Math.abs(denominator) <= 1e-12 ? 0 : aDistance / denominator
	return a.clone().lerp(b, THREE.MathUtils.clamp(ratio, 0, 1))
}

function getPerpendicularUnitVector(normal: THREE.Vector3): THREE.Vector3 {
	const reference = Math.abs(normal.z) < 0.9 ? new THREE.Vector3(0, 0, 1) : new THREE.Vector3(1, 0, 0)
	return new THREE.Vector3().crossVectors(normal, reference).normalize()
}

function areTrianglePointsCollinear(a: THREE.Vector3, b: THREE.Vector3, c: THREE.Vector3): boolean {
	return new THREE.Vector3().crossVectors(b.clone().sub(a), c.clone().sub(a)).lengthSq() <= 1e-12
}

function createPreviewEdgeVisuals(extrusion: ExtrudedSolid, extrudeId: string): PreviewEdgeVisual[] {
	const verticesById = new Map(extrusion.solid.vertices.map((vertex) => [vertex.id, vertex]))
	const edges: PreviewEdgeVisual[] = []

	for (let edgeIndex = 0; edgeIndex < extrusion.solid.edges.length; edgeIndex += 1) {
		const edge = extrusion.solid.edges[edgeIndex]
		const startVertexId = edge?.vertexIds[0]
		const endVertexId = edge?.vertexIds[1]
		const startVertex = startVertexId ? verticesById.get(startVertexId) : undefined
		const endVertex = endVertexId ? verticesById.get(endVertexId) : undefined
		if (!edge || !startVertex || !endVertex) {
			continue
		}

		const geometry = new THREE.BufferGeometry().setFromPoints([vector3DToThree(startVertex.position), vector3DToThree(endVertex.position)])
		const material = new THREE.LineBasicMaterial({
			color: 0xe2e8f0,
			transparent: true,
			opacity: 0.8
		})
		const line = new THREE.Line(geometry, material)
		line.renderOrder = 7
		const startPoint = vector3DToThree(startVertex.position)
		const endPoint = vector3DToThree(endVertex.position)
		const highlight = createPreviewEdgeHighlight(startPoint, endPoint)
		const highlightMaterial = highlight.material as THREE.MeshBasicMaterial
		edges.push({
			extrudeId,
			edgeId: edge.id,
			label: `Edge ${edgeIndex + 1}`,
			line,
			material,
			highlight,
			highlightMaterial
		})
	}

	return edges
}

function createPreviewEdgeHighlight(start: THREE.Vector3, end: THREE.Vector3): THREE.Mesh {
	const direction = end.clone().sub(start)
	const length = direction.length()
	const material = new THREE.MeshBasicMaterial({
		color: 0xf59e0b,
		transparent: true,
		opacity: 0.96,
		depthWrite: false,
		depthTest: true,
		polygonOffset: true,
		polygonOffsetFactor: -2,
		polygonOffsetUnits: -2
	})
	const geometry = new THREE.CylinderGeometry(1, 1, Math.max(length, 1e-6), 16, 1, false)
	const highlight = new THREE.Mesh(geometry, material)
	highlight.position.copy(start).add(end).multiplyScalar(0.5)
	if (length > 1e-9) {
		highlight.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), direction.normalize())
	}
	highlight.scale.set(PREVIEW_EDGE_SELECTED_RADIUS, 1, PREVIEW_EDGE_SELECTED_RADIUS)
	highlight.renderOrder = 11
	highlight.visible = false
	return highlight
}

function createPreviewCornerVisuals(extrusion: ExtrudedSolid, extrudeId: string): PreviewCornerVisual[] {
	const corners: PreviewCornerVisual[] = []
	for (let cornerIndex = 0; cornerIndex < extrusion.solid.vertices.length; cornerIndex += 1) {
		const vertex = extrusion.solid.vertices[cornerIndex]
		if (!vertex) {
			continue
		}
		const marker = createPreviewCornerMarker(vector3DToThree(vertex.position))
		const material = marker.material as THREE.MeshBasicMaterial
		corners.push({
			extrudeId,
			cornerId: vertex.id,
			label: `Corner ${cornerIndex + 1}`,
			marker,
			material
		})
	}
	return corners
}

function createPreviewCornerMarker(position: THREE.Vector3): THREE.Mesh {
	const material = new THREE.MeshBasicMaterial({
		color: 0xf59e0b,
		transparent: true,
		opacity: 0.96,
		depthWrite: false,
		depthTest: true
	})
	const marker = new THREE.Mesh(new THREE.SphereGeometry(1, 16, 12), material)
	marker.position.copy(position)
	marker.scale.setScalar(PREVIEW_CORNER_SELECTED_RADIUS)
	marker.renderOrder = 12
	marker.visible = false
	return marker
}

function createPreviewFaceVisuals(extrusion: ExtrudedSolid, extrudeId: string, position: THREE.Vector3, quaternion: THREE.Quaternion): PreviewFaceVisual[] {
	const faceDescriptors = getExtrudedFaceDescriptors(extrusion)
	const faceVisuals: PreviewFaceVisual[] = []
	let faceIndex = 0
	const totalSideFaces = extrusion.profileLoops.reduce((count, loop) => count + loop.length, 0)

	for (const loop of extrusion.profileLoops) {
		for (let pointIndex = 0; pointIndex < loop.length; pointIndex += 1) {
			const descriptor = faceDescriptors[faceIndex]
			const start = loop[pointIndex]
			const end = loop[(pointIndex + 1) % loop.length]
			if (descriptor && start && end) {
				const geometry = createQuadFaceGeometry(
					new THREE.Vector3(start.x, start.y, 0),
					new THREE.Vector3(end.x, end.y, 0),
					new THREE.Vector3(end.x, end.y, extrusion.depth),
					new THREE.Vector3(start.x, start.y, extrusion.depth)
				)
				faceVisuals.push(createPreviewFaceVisual(extrudeId, descriptor, geometry, position, quaternion))
			}
			faceIndex += 1
		}
	}

	const bottomDescriptor = faceDescriptors[totalSideFaces]
	if (bottomDescriptor) {
		faceVisuals.push(createPreviewFaceVisual(extrudeId, bottomDescriptor, createPlanarFaceGeometry(extrusion.profileLoops, 0), position, quaternion))
	}
	const topDescriptor = faceDescriptors[totalSideFaces + 1]
	if (topDescriptor) {
		faceVisuals.push(createPreviewFaceVisual(extrudeId, topDescriptor, createPlanarFaceGeometry(extrusion.profileLoops, extrusion.depth), position, quaternion))
	}

	return faceVisuals
}

function createPreviewFaceVisual(extrudeId: string, descriptor: ExtrudedFaceDescriptor, geometry: THREE.BufferGeometry, position: THREE.Vector3, quaternion: THREE.Quaternion): PreviewFaceVisual {
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
	mesh.position.copy(position)
	mesh.quaternion.copy(quaternion)
	mesh.renderOrder = 8
	return {
		extrudeId,
		faceId: descriptor.faceId,
		label: descriptor.label,
		mesh,
		material,
		frame: descriptor.frame
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

function getSketchFrameQuaternion(frame: SketchFrame3D): THREE.Quaternion {
	return new THREE.Quaternion().setFromRotationMatrix(
		new THREE.Matrix4().makeBasis(vector3DToThree(frame.xAxis).normalize(), vector3DToThree(frame.yAxis).normalize(), vector3DToThree(frame.normal).normalize())
	)
}

function sketchPointToWorld(point: Point2D, frame: SketchFrame3D): THREE.Vector3 {
	return vector3DToThree(addScaledVector(addScaledVector(frame.origin, frame.xAxis, point.x), frame.yAxis, point.y))
}

function vector3DToThree(vector: Vector3D): THREE.Vector3 {
	return new THREE.Vector3(vector.x, vector.y, vector.z)
}

function addScaledVector(origin: Vector3D, axis: Vector3D, scalar: number): Vector3D {
	return {
		x: origin.x + axis.x * scalar,
		y: origin.y + axis.y * scalar,
		z: origin.z + axis.z * scalar
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

function getRectangleSideSegment(entity: Extract<SketchEntity, { type: "cornerRectangle" }>, side: RectangleSide): { start: Point2D; end: Point2D } {
	const [bottomLeft, bottomRight, topRight, topLeft] = rectangleCorners(entity)
	switch (side) {
		case "top":
			return {
				start: topLeft ?? { x: 0, y: 0 },
				end: topRight ?? { x: 0, y: 0 }
			}
		case "right":
			return {
				start: bottomRight ?? { x: 0, y: 0 },
				end: topRight ?? { x: 0, y: 0 }
			}
		case "left":
			return {
				start: bottomLeft ?? { x: 0, y: 0 },
				end: topLeft ?? { x: 0, y: 0 }
			}
		default:
			return {
				start: bottomLeft ?? { x: 0, y: 0 },
				end: bottomRight ?? { x: 0, y: 0 }
			}
	}
}

function getSketchDimensionLabelPoint(sketch: Sketch, dimension: SketchDimension): Point2D | null {
	const entity = sketch.entities.find((candidate) => candidate.id === dimension.entityId)
	if (!entity) {
		return null
	}

	if (dimension.type === "lineLength" && entity.type === "line") {
		const midpoint = {
			x: (entity.p0.x + entity.p1.x) / 2,
			y: (entity.p0.y + entity.p1.y) / 2
		}
		const dx = entity.p1.x - entity.p0.x
		const dy = entity.p1.y - entity.p0.y
		const length = Math.hypot(dx, dy)
		return length <= 1e-9
			? { x: midpoint.x, y: midpoint.y + 0.8 }
			: {
					x: midpoint.x + (-dy / length) * 0.8,
					y: midpoint.y + (dx / length) * 0.8
				}
	}

	if (entity.type !== "cornerRectangle") {
		return null
	}

	if (dimension.type === "rectangleWidth") {
		const segment = getRectangleSideSegment(entity, "top")
		return {
			x: (segment.start.x + segment.end.x) / 2,
			y: Math.max(segment.start.y, segment.end.y) + 0.8
		}
	}

	if (dimension.type === "rectangleHeight") {
		const segment = getRectangleSideSegment(entity, "right")
		return {
			x: Math.max(segment.start.x, segment.end.x) + 0.8,
			y: (segment.start.y + segment.end.y) / 2
		}
	}

	return null
}

function isSketchDimensionSelected(selection: SketchEdgeSelection | null, dimension: SketchDimension): boolean {
	if (!selection || selection.entityId !== dimension.entityId) {
		return false
	}
	if (selection.type === "line") {
		return dimension.type === "lineLength"
	}
	return dimension.type === "rectangleWidth" ? selection.side === "top" || selection.side === "bottom" : selection.side === "left" || selection.side === "right"
}

function formatSketchDimensionValue(value: number): string {
	return Number.isInteger(value) ? String(value) : value.toFixed(2).replace(/\.?0+$/, "")
}

function distanceToSegmentSquared(point: Point2D, start: Point2D, end: Point2D): number {
	return getSegmentHitInfo(point, start, end).distanceSquared
}

function getSegmentHitInfo(point: Point2D, start: Point2D, end: Point2D): { distanceSquared: number; ratio: number } {
	const dx = end.x - start.x
	const dy = end.y - start.y
	if (dx === 0 && dy === 0) {
		const offsetX = point.x - start.x
		const offsetY = point.y - start.y
		return {
			distanceSquared: offsetX * offsetX + offsetY * offsetY,
			ratio: 0
		}
	}

	const projection = ((point.x - start.x) * dx + (point.y - start.y) * dy) / (dx * dx + dy * dy)
	const clamped = Math.max(0, Math.min(1, projection))
	const closestX = start.x + dx * clamped
	const closestY = start.y + dy * clamped
	const offsetX = point.x - closestX
	const offsetY = point.y - closestY
	return {
		distanceSquared: offsetX * offsetX + offsetY * offsetY,
		ratio: clamped
	}
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

function edgeReferencesEqual(a: EdgeReference, b: EdgeReference): boolean {
	return a.extrudeId === b.extrudeId && a.edgeId === b.edgeId
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
