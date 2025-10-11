import { UiComponent, VList } from "./ui"

type FlowchartShape = "startEnd" | "process" | "decision" | "inputOutput" | "predefinedProcess" | "manualInput" | "document" | "database"

interface FlowchartNode {
	id: number
	type: FlowchartShape
	x: number
	y: number
	width: number
	height: number
	text: string
	element: HTMLDivElement
	textElement: HTMLDivElement
	baseBorderColor: string
}

interface FlowchartConnection {
	id: number
	from: number
	to: number
	path: SVGPathElement
}

type PersistedFlowchartNode = {
	id: number
	type: FlowchartShape
	x: number
	y: number
	width: number
	height: number
	text: string
}

type PersistedFlowchartConnection = {
	id: number
	from: number
	to: number
}

type PersistedDiagramState = {
	nodes: PersistedFlowchartNode[]
	connections?: PersistedFlowchartConnection[]
	nextNodeId?: number
	nextConnectionId?: number
	selectedNodeId?: number | null
	selectedNodeIds?: number[]
	selectedConnectionId?: number | null
	connectMode?: boolean
	panX?: number
	panY?: number
	zoom?: number
}

type ShapeConfig = {
	label: string
	width: number
	height: number
	defaultText: string
}

type ScreenRect = {
	left: number
	top: number
	right: number
	bottom: number
}

const SHAPE_CONFIG: Record<FlowchartShape, ShapeConfig> = {
	startEnd: { label: "Start / End", width: 140, height: 60, defaultText: "Start" },
	process: { label: "Process", width: 160, height: 80, defaultText: "Process" },
	decision: { label: "Decision", width: 160, height: 160, defaultText: "Decision" },
	inputOutput: { label: "Input / Output", width: 170, height: 80, defaultText: "Data" },
	predefinedProcess: { label: "Subprocess", width: 180, height: 80, defaultText: "Subprocess" },
	manualInput: { label: "Manual Input", width: 180, height: 80, defaultText: "Manual Input" },
	document: { label: "Document", width: 200, height: 120, defaultText: "Document" },
	database: { label: "Database", width: 160, height: 110, defaultText: "Database" }
}

export class DiagramEditor extends UiComponent<HTMLDivElement> {
	private palette: VList
	private editorArea: HTMLDivElement
	private canvasArea: HTMLDivElement
	private nodesLayer: HTMLDivElement
	private svgLayer: SVGSVGElement
	private nodes: FlowchartNode[] = []
	private connections: FlowchartConnection[] = []
	private nextNodeId = 1
	private nextConnectionId = 1
	private selectedNodeIds: Set<number> = new Set()
	private selectedConnectionId: number | null = null
	private connectMode = false
	private connectStartNodeId: number | null = null
	private draggingNodes: FlowchartNode[] = []
	private dragStartPointer: { x: number; y: number } | null = null
	private dragInitialNodePositions: Map<number, { x: number; y: number }> = new Map()
	private selectionOverlay: HTMLDivElement
	private selectionRect: HTMLDivElement
	private isSelecting = false
	private selectionStartX = 0
	private selectionStartY = 0
	private selectionMoved = false
	private connectButton: HTMLButtonElement
	private deleteButton: HTMLButtonElement
	private readonly boundPointerMove = this.onPointerMove.bind(this)
	private readonly boundPointerUp = this.onPointerUp.bind(this)
	private readonly boundSelectionMove = this.onSelectionPointerMove.bind(this)
	private readonly boundSelectionUp = this.onSelectionPointerUp.bind(this)
	private readonly boundResize = this.updateConnectorPositions.bind(this)
	private readonly boundPanMove = this.onPanPointerMove.bind(this)
	private readonly boundPanUp = this.onPanPointerUp.bind(this)
	private readonly boundWheel = this.onWheel.bind(this)
	private readonly minZoom = 0.25
	private readonly maxZoom = 3
	private panPointerId: number | null = null
	private panStartClientX = 0
	private panStartClientY = 0
	private panStartX = 0
	private panStartY = 0
	private panX = 0
	private panY = 0
	private zoom = 1
	private databasePromise: Promise<IDBDatabase> | null = null
	private persistTimeout: number | null = null
	private isRestoring = false
	private static readonly DATABASE_NAME = "puppycad-diagram"
	private static readonly STORE_NAME = "diagramState"
	private static readonly STORE_KEY = "current"
	private static readonly PERSIST_DEBOUNCE_MS = 200
	private static readonly CONNECTION_STROKE_COLOR = "#475569"
	private static readonly SELECTED_CONNECTION_STROKE_COLOR = "#2563eb"
	private static readonly SELECTED_CONNECTION_GLOW = "drop-shadow(0 0 6px rgba(37, 99, 235, 0.45))"

	public constructor() {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "row"
		this.root.style.width = "100%"
		this.root.style.height = "100%"
		this.root.style.boxSizing = "border-box"

		this.palette = new VList({
			style: {
				width: "200px",
				padding: "12px",
				borderRight: "1px solid #d1d5db",
				gap: "8px",
				background: "linear-gradient(180deg, #f8fafc 0%, #f1f5f9 100%)"
			}
		})
		const paletteTitle = document.createElement("h3")
		paletteTitle.textContent = "Flowchart Shapes"
		paletteTitle.style.margin = "0 0 8px 0"
		paletteTitle.style.fontSize = "16px"
		paletteTitle.style.fontWeight = "600"
		this.palette.add(paletteTitle)

		for (const shape of Object.keys(SHAPE_CONFIG) as FlowchartShape[]) {
			const button = document.createElement("button")
			button.textContent = SHAPE_CONFIG[shape].label
			button.style.padding = "8px 12px"
			button.style.borderRadius = "6px"
			button.style.border = "1px solid #94a3b8"
			button.style.backgroundColor = "#fff"
			button.style.cursor = "pointer"
			button.onmouseenter = () => {
				button.style.backgroundColor = "#e2e8f0"
			}
			button.onmouseleave = () => {
				button.style.backgroundColor = "#fff"
			}
			button.onclick = () => this.addNode(shape)
			button.draggable = true
			button.addEventListener("dragstart", (event) => {
				if (!event.dataTransfer) return
				event.dataTransfer.effectAllowed = "copy"
				event.dataTransfer.setData("application/x-flowchart-shape", shape)
				event.dataTransfer.setData("text/plain", shape)
			})
			this.palette.add(button)
		}

		this.editorArea = document.createElement("div")
		this.editorArea.style.display = "flex"
		this.editorArea.style.flexDirection = "column"
		this.editorArea.style.flexGrow = "1"
		this.editorArea.style.padding = "12px"
		this.editorArea.style.gap = "12px"
		this.editorArea.style.boxSizing = "border-box"

		const toolbar = document.createElement("div")
		toolbar.style.display = "flex"
		toolbar.style.gap = "8px"

		this.connectButton = document.createElement("button")
		this.connectButton.textContent = "Connect"
		this.connectButton.style.padding = "8px 12px"
		this.connectButton.style.borderRadius = "6px"
		this.connectButton.style.border = "1px solid #2563eb"
		this.connectButton.style.backgroundColor = "#eff6ff"
		this.connectButton.style.color = "#1d4ed8"
		this.connectButton.style.cursor = "pointer"
		this.connectButton.onclick = () => this.toggleConnectMode()

		this.deleteButton = document.createElement("button")
		this.deleteButton.textContent = "Delete"
		this.deleteButton.style.padding = "8px 12px"
		this.deleteButton.style.borderRadius = "6px"
		this.deleteButton.style.border = "1px solid #dc2626"
		this.deleteButton.style.backgroundColor = "#fee2e2"
		this.deleteButton.style.color = "#b91c1c"
		this.deleteButton.style.cursor = "pointer"
		this.deleteButton.onclick = () => this.deleteSelection()

		toolbar.append(this.connectButton, this.deleteButton)

		this.canvasArea = document.createElement("div")
		this.canvasArea.style.flexGrow = "1"
		this.canvasArea.style.position = "relative"
		this.canvasArea.style.border = "1px solid #cbd5f5"
		this.canvasArea.style.backgroundColor = "#ffffff"
		this.canvasArea.style.minHeight = "600px"
		this.canvasArea.style.borderRadius = "8px"
		this.canvasArea.style.overflow = "hidden"
		this.canvasArea.style.backgroundImage = "linear-gradient(0deg, rgba(226, 232, 240, 0.5) 1px, transparent 1px), linear-gradient(90deg, rgba(226, 232, 240, 0.5) 1px, transparent 1px)"
		this.canvasArea.style.backgroundSize = "40px 40px"

		this.svgLayer = document.createElementNS("http://www.w3.org/2000/svg", "svg")
		this.svgLayer.style.position = "absolute"
		this.svgLayer.style.top = "0"
		this.svgLayer.style.left = "0"
		this.svgLayer.style.width = "100%"
		this.svgLayer.style.height = "100%"
		this.svgLayer.style.pointerEvents = "auto"
		this.svgLayer.style.transformOrigin = "0 0"
		this.svgLayer.style.overflow = "visible"

		this.nodesLayer = document.createElement("div")
		this.nodesLayer.style.position = "absolute"
		this.nodesLayer.style.top = "0"
		this.nodesLayer.style.left = "0"
		this.nodesLayer.style.right = "0"
		this.nodesLayer.style.bottom = "0"
		this.nodesLayer.style.transformOrigin = "0 0"
		this.nodesLayer.style.pointerEvents = "none"

		this.canvasArea.appendChild(this.svgLayer)
		this.canvasArea.appendChild(this.nodesLayer)

		this.selectionOverlay = document.createElement("div")
		this.selectionOverlay.style.position = "absolute"
		this.selectionOverlay.style.top = "0"
		this.selectionOverlay.style.left = "0"
		this.selectionOverlay.style.right = "0"
		this.selectionOverlay.style.bottom = "0"
		this.selectionOverlay.style.pointerEvents = "none"
		this.selectionOverlay.style.zIndex = "10"

		this.selectionRect = document.createElement("div")
		this.selectionRect.style.position = "absolute"
		this.selectionRect.style.border = "1px dashed rgba(37, 99, 235, 0.8)"
		this.selectionRect.style.backgroundColor = "rgba(37, 99, 235, 0.15)"
		this.selectionRect.style.display = "none"

		this.selectionOverlay.appendChild(this.selectionRect)
		this.canvasArea.appendChild(this.selectionOverlay)

		this.editorArea.appendChild(toolbar)
		this.editorArea.appendChild(this.canvasArea)

		this.root.appendChild(this.palette.root)
		this.root.appendChild(this.editorArea)

		this.canvasArea.addEventListener("pointerdown", (event: PointerEvent) => {
			if (event.button === 1) {
				this.startPan(event)
				return
			}
			if (event.button !== 0) {
				return
			}
			const target = event.target as EventTarget | null
			if (target === this.canvasArea || target === this.svgLayer || target === this.selectionOverlay || target === this.nodesLayer) {
				this.startSelection(event)
			}
		})

		this.canvasArea.addEventListener("dragover", (event) => {
			if (!event.dataTransfer) return
			const hasShapeData = Array.from(event.dataTransfer.types).some((type) => type === "application/x-flowchart-shape" || type === "text/plain")
			if (!hasShapeData) return
			event.preventDefault()
			event.dataTransfer.dropEffect = "copy"
		})

		this.canvasArea.addEventListener("drop", (event) => {
			if (!event.dataTransfer) return
			const shapeData = event.dataTransfer.getData("application/x-flowchart-shape") || event.dataTransfer.getData("text/plain")
			if (!this.isFlowchartShape(shapeData)) return
			event.preventDefault()
			const worldPosition = this.screenToWorld(event.clientX, event.clientY)
			this.addNode(shapeData, worldPosition)
		})

		this.canvasArea.addEventListener("mousedown", (event) => {
			if (event.target === this.canvasArea) {
				event.preventDefault()
			}
		})

		this.canvasArea.addEventListener("wheel", this.boundWheel, { passive: false })

		this.updateCanvasTransform()
		window.addEventListener("resize", this.boundResize)

		void this.loadFromIndexedDB()
	}

	private isFlowchartShape(value: string | null | undefined): value is FlowchartShape {
		return (
			value === "startEnd" ||
			value === "process" ||
			value === "decision" ||
			value === "inputOutput" ||
			value === "predefinedProcess" ||
			value === "manualInput" ||
			value === "document" ||
			value === "database"
		)
	}

	private getShapeConfig(shape: FlowchartShape): ShapeConfig {
		return SHAPE_CONFIG[shape]
	}

	private addNode(shape: FlowchartShape, worldPosition?: { x: number; y: number }) {
		const config = this.getShapeConfig(shape)
		let worldCenterX: number
		let worldCenterY: number
		if (worldPosition) {
			worldCenterX = worldPosition.x
			worldCenterY = worldPosition.y
		} else {
			const rect = this.canvasArea.getBoundingClientRect()
			const screenCenterX = rect.width ? rect.width / 2 : 80
			const screenCenterY = rect.height ? rect.height / 2 : 80
			worldCenterX = (screenCenterX - this.panX) / this.zoom
			worldCenterY = (screenCenterY - this.panY) / this.zoom
		}
		const defaultX = worldCenterX - config.width / 2
		const defaultY = worldCenterY - config.height / 2

		const node: FlowchartNode = {
			id: this.nextNodeId++,
			type: shape,
			x: Math.max(16, defaultX),
			y: Math.max(16, defaultY),
			width: config.width,
			height: config.height,
			text: config.defaultText,
			element: document.createElement("div"),
			textElement: document.createElement("div"),
			baseBorderColor: "#1f2937"
		}

		this.setupNodeElement(node)
		this.nodes.push(node)
		this.nodesLayer.appendChild(node.element)
		this.selectNode(node.id)
		this.updateConnectorPositions()
		this.schedulePersist()
	}

	private setupNodeElement(node: FlowchartNode) {
		const element = node.element
		element.style.position = "absolute"
		element.style.left = `${node.x}px`
		element.style.top = `${node.y}px`
		element.style.width = `${node.width}px`
		element.style.height = `${node.height}px`
		element.style.display = "flex"
		element.style.alignItems = "center"
		element.style.justifyContent = "center"
		element.style.padding = "12px"
		element.style.boxSizing = "border-box"
		element.style.backgroundColor = "#ffffff"
		element.style.borderWidth = "2px"
		element.style.borderStyle = "solid"
		element.style.borderColor = "#1f2937"
		element.style.boxShadow = "0 1px 4px rgba(15, 23, 42, 0.15)"
		element.style.borderRadius = "8px"
		element.style.cursor = "grab"
		element.style.userSelect = "none"
		element.style.transition = "box-shadow 0.1s ease, border-color 0.1s ease"
		element.style.pointerEvents = "auto"

		const baseBorderColor = this.applyShapeStyle(element, node.type)
		node.baseBorderColor = baseBorderColor
		element.style.borderColor = baseBorderColor

		node.textElement.textContent = node.text
		node.textElement.style.textAlign = "center"
		node.textElement.style.fontFamily = "Inter, system-ui, -apple-system, sans-serif"
		node.textElement.style.fontSize = "14px"
		node.textElement.style.fontWeight = "500"
		node.textElement.style.color = "#0f172a"
		node.textElement.style.pointerEvents = "none"

		element.appendChild(node.textElement)

		element.addEventListener("pointerdown", (event: PointerEvent) => {
			if (event.button === 1) {
				this.startPan(event)
				return
			}
			if (event.button !== 0) {
				return
			}
			event.stopPropagation()
			if (this.connectMode) {
				this.handleConnectClick(node)
				return
			}
			const wasSelected = this.selectedNodeIds.has(node.id)
			if (!wasSelected) {
				this.setSelectedNodes([node.id])
			} else {
				this.selectConnection(null, false)
			}
			const pointerPosition = this.screenToWorld(event.clientX, event.clientY)
			this.dragStartPointer = pointerPosition
			this.dragInitialNodePositions.clear()
			this.draggingNodes = this.nodes.filter((n) => this.selectedNodeIds.has(n.id))
			if (this.draggingNodes.length === 0) {
				this.draggingNodes = [node]
			}
			for (const dragNode of this.draggingNodes) {
				this.dragInitialNodePositions.set(dragNode.id, {
					x: dragNode.x,
					y: dragNode.y
				})
			}
			for (const dragNode of this.draggingNodes) {
				dragNode.element.style.cursor = "grabbing"
			}
			document.addEventListener("pointermove", this.boundPointerMove)
			document.addEventListener("pointerup", this.boundPointerUp)
		})

		element.addEventListener("pointerup", () => {
			element.style.cursor = "grab"
		})

		element.addEventListener("dblclick", (event) => {
			event.stopPropagation()
			const value = prompt("Edit step", node.text)
			if (value !== null) {
				node.text = value
				node.textElement.textContent = value
				this.schedulePersist()
			}
		})
	}

	private applyShapeStyle(element: HTMLDivElement, shape: FlowchartShape): string {
		element.style.borderRadius = "8px"
		element.style.clipPath = "none"
		element.style.transform = "none"
		element.style.background = "linear-gradient(180deg, #ffffff 0%, #f8fafc 100%)"
		element.style.backgroundSize = "auto"
		element.style.backgroundPosition = "center"
		element.style.backgroundRepeat = "no-repeat"

		let borderColor = "#1f2937"

		switch (shape) {
			case "startEnd": {
				element.style.borderRadius = "9999px"
				element.style.background = "linear-gradient(180deg, #dcfce7 0%, #bbf7d0 100%)"
				borderColor = "#15803d"
				break
			}
			case "process": {
				element.style.background = "linear-gradient(180deg, #e0f2fe 0%, #bae6fd 100%)"
				borderColor = "#1d4ed8"
				break
			}
			case "decision": {
				element.style.clipPath = "polygon(50% 0%, 100% 50%, 50% 100%, 0% 50%)"
				element.style.background = "linear-gradient(180deg, #fef3c7 0%, #fde68a 100%)"
				borderColor = "#f59e0b"
				break
			}
			case "inputOutput": {
				element.style.clipPath = "polygon(12% 0%, 100% 0%, 88% 100%, 0% 100%)"
				element.style.background = "linear-gradient(180deg, #cffafe 0%, #a5f3fc 100%)"
				borderColor = "#0ea5e9"
				break
			}
			case "predefinedProcess": {
				element.style.background =
					"linear-gradient(90deg, rgba(37, 99, 235, 0.18) 0px, rgba(37, 99, 235, 0.18) 12px, transparent 12px, transparent calc(100% - 12px), rgba(37, 99, 235, 0.18) calc(100% - 12px), rgba(37, 99, 235, 0.18) 100%), linear-gradient(180deg, #dbeafe 0%, #bfdbfe 100%)"
				element.style.backgroundSize = "100% 100%, 100% 100%"
				element.style.backgroundRepeat = "no-repeat"
				borderColor = "#1d4ed8"
				break
			}
			case "manualInput": {
				element.style.clipPath = "polygon(0% 22%, 100% 0%, 100% 100%, 0% 100%)"
				element.style.background = "linear-gradient(180deg, #ede9fe 0%, #ddd6fe 100%)"
				borderColor = "#6d28d9"
				break
			}
			case "document": {
				element.style.borderRadius = "16px"
				element.style.clipPath = "polygon(0 0, 100% 0, 100% 82%, 80% 100%, 0 100%)"
				element.style.background = "linear-gradient(180deg, #fef9c3 0%, #fde68a 100%)"
				borderColor = "#d97706"
				break
			}
			case "database": {
				element.style.borderRadius = "9999px / 20%"
				element.style.background =
					"radial-gradient(120% 120% at 50% 0%, rgba(255, 255, 255, 0.75) 0%, rgba(255, 255, 255, 0) 60%), linear-gradient(180deg, #c7d2fe 0%, #a5b4fc 100%)"
				element.style.backgroundSize = "100% 65%, 100% 100%"
				element.style.backgroundPosition = "center top, center"
				element.style.backgroundRepeat = "no-repeat"
				borderColor = "#4338ca"
				break
			}
		}

		return borderColor
	}

	private screenToWorld(clientX: number, clientY: number) {
		const rect = this.canvasArea.getBoundingClientRect()
		const x = (clientX - rect.left - this.panX) / this.zoom
		const y = (clientY - rect.top - this.panY) / this.zoom
		return { x, y }
	}

	private worldToScreen(x: number, y: number) {
		return {
			x: x * this.zoom + this.panX,
			y: y * this.zoom + this.panY
		}
	}

	private updateCanvasTransform() {
		const transform = `matrix(${this.zoom}, 0, 0, ${this.zoom}, ${this.panX}, ${this.panY})`
		this.nodesLayer.style.transform = transform
		this.svgLayer.style.transform = transform
		const gridSize = 40 * this.zoom
		this.canvasArea.style.backgroundSize = `${gridSize}px ${gridSize}px`
		this.canvasArea.style.backgroundPosition = `${this.panX}px ${this.panY}px`
	}

	private startPan(event: PointerEvent) {
		if (this.panPointerId !== null) return
		this.panPointerId = event.pointerId
		this.panStartClientX = event.clientX
		this.panStartClientY = event.clientY
		this.panStartX = this.panX
		this.panStartY = this.panY
		this.canvasArea.style.cursor = "grabbing"
		document.addEventListener("pointermove", this.boundPanMove)
		document.addEventListener("pointerup", this.boundPanUp)
		document.addEventListener("pointercancel", this.boundPanUp)
		event.preventDefault()
		event.stopPropagation()
	}

	private onPanPointerMove(event: PointerEvent) {
		if (this.panPointerId !== event.pointerId) return
		const deltaX = event.clientX - this.panStartClientX
		const deltaY = event.clientY - this.panStartClientY
		this.panX = this.panStartX + deltaX
		this.panY = this.panStartY + deltaY
		this.updateCanvasTransform()
	}

	private onPanPointerUp(event: PointerEvent) {
		if (this.panPointerId !== event.pointerId) return
		document.removeEventListener("pointermove", this.boundPanMove)
		document.removeEventListener("pointerup", this.boundPanUp)
		document.removeEventListener("pointercancel", this.boundPanUp)
		this.canvasArea.style.cursor = ""
		this.panPointerId = null
		this.schedulePersist()
	}

	private onWheel(event: WheelEvent) {
		const deltaX = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? event.deltaX * 16 : event.deltaX
		const deltaY = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? event.deltaY * 16 : event.deltaY
		if (event.ctrlKey || event.metaKey) {
			event.preventDefault()
			const rect = this.canvasArea.getBoundingClientRect()
			const screenX = event.clientX - rect.left
			const screenY = event.clientY - rect.top
			const worldX = (screenX - this.panX) / this.zoom
			const worldY = (screenY - this.panY) / this.zoom
			const zoomFactor = Math.exp(-deltaY * 0.001)
			const newZoom = Math.min(this.maxZoom, Math.max(this.minZoom, this.zoom * zoomFactor))
			this.zoom = newZoom
			this.panX = screenX - worldX * this.zoom
			this.panY = screenY - worldY * this.zoom
			this.updateCanvasTransform()
			this.schedulePersist()
			return
		}
		event.preventDefault()
		this.panX -= deltaX
		this.panY -= deltaY
		this.updateCanvasTransform()
		this.schedulePersist()
	}

	private onPointerMove(event: PointerEvent) {
		if (this.draggingNodes.length === 0 || !this.dragStartPointer) {
			return
		}
		const pointerPosition = this.screenToWorld(event.clientX, event.clientY)
		const deltaX = pointerPosition.x - this.dragStartPointer.x
		const deltaY = pointerPosition.y - this.dragStartPointer.y
		for (const node of this.draggingNodes) {
			const initial = this.dragInitialNodePositions.get(node.id)
			if (!initial) {
				return
			}
			this.setNodePosition(node, initial.x + deltaX, initial.y + deltaY, false)
		}
		this.updateConnectorPositions()
		this.schedulePersist()
	}

	private onPointerUp() {
		if (this.draggingNodes.length > 0) {
			for (const node of this.draggingNodes) {
				node.element.style.cursor = "grab"
			}
			this.draggingNodes = []
		}
		this.dragInitialNodePositions.clear()
		this.dragStartPointer = null
		document.removeEventListener("pointermove", this.boundPointerMove)
		document.removeEventListener("pointerup", this.boundPointerUp)
	}

	private setNodePosition(node: FlowchartNode, x: number, y: number, updateConnections = true) {
		node.x = x
		node.y = y
		node.element.style.left = `${x}px`
		node.element.style.top = `${y}px`
		if (updateConnections) {
			this.updateConnectorPositions()
			this.schedulePersist()
		}
	}

	private selectNode(nodeId: number | null) {
		if (nodeId === null) {
			this.setSelectedNodes([])
		} else {
			this.setSelectedNodes([nodeId])
		}
	}

	private updateNodeStyles() {
		for (const node of this.nodes) {
			const isSelected = this.selectedNodeIds.has(node.id)
			const isConnectStart = node.id === this.connectStartNodeId
			if (isConnectStart) {
				node.element.style.borderColor = "#16a34a"
				node.element.style.boxShadow = "0 0 0 3px rgba(22, 163, 74, 0.25)"
			} else if (isSelected) {
				node.element.style.borderColor = "#2563eb"
				node.element.style.boxShadow = "0 0 0 3px rgba(37, 99, 235, 0.25)"
			} else {
				node.element.style.borderColor = node.baseBorderColor
				node.element.style.boxShadow = "0 1px 4px rgba(15, 23, 42, 0.15)"
			}
		}
		this.updateConnectButtonState()
	}

	private updateConnectionStyles() {
		for (const connection of this.connections) {
			const isSelected = connection.id === this.selectedConnectionId
			if (isSelected) {
				connection.path.style.stroke = DiagramEditor.SELECTED_CONNECTION_STROKE_COLOR
				connection.path.style.strokeWidth = "4px"
				connection.path.style.filter = DiagramEditor.SELECTED_CONNECTION_GLOW
			} else {
				connection.path.style.stroke = DiagramEditor.CONNECTION_STROKE_COLOR
				connection.path.style.strokeWidth = "3px"
				connection.path.style.filter = "none"
				connection.path.removeAttribute("stroke-dasharray")
			}
		}
	}

	private updateSelectionStyles() {
		this.updateNodeStyles()
		this.updateConnectionStyles()
	}

	private setSelectedNodes(nodeIds: Iterable<number>, persist = true) {
		this.selectedNodeIds = new Set(nodeIds)
		if (this.selectedConnectionId !== null) {
			this.selectedConnectionId = null
		}
		this.updateSelectionStyles()
		if (persist) {
			this.schedulePersist()
		}
	}

	private selectConnection(connectionId: number | null, persist = true) {
		if (connectionId !== null) {
			this.selectedNodeIds.clear()
		}
		this.selectedConnectionId = connectionId
		this.updateSelectionStyles()
		if (persist) {
			this.schedulePersist()
		}
	}

	private clearSelection(persist = true) {
		this.selectedNodeIds.clear()
		this.selectedConnectionId = null
		this.updateSelectionStyles()
		if (persist) {
			this.schedulePersist()
		}
	}

	private startSelection(event: PointerEvent) {
		if (this.isSelecting) {
			return
		}
		event.preventDefault()
		const rect = this.canvasArea.getBoundingClientRect()
		this.isSelecting = true
		this.selectionMoved = false
		this.selectionStartX = event.clientX - rect.left
		this.selectionStartY = event.clientY - rect.top
		this.selectionRect.style.display = "block"
		this.updateSelectionRect(this.selectionStartX, this.selectionStartY, this.selectionStartX, this.selectionStartY)
		this.setSelectedNodes([], false)
		this.selectConnection(null, false)
		if (this.connectMode && this.connectStartNodeId !== null) {
			this.connectStartNodeId = null
			this.updateNodeStyles()
		}
		document.addEventListener("pointermove", this.boundSelectionMove)
		document.addEventListener("pointerup", this.boundSelectionUp)
	}

	private onSelectionPointerMove(event: PointerEvent) {
		if (!this.isSelecting) {
			return
		}
		const rect = this.canvasArea.getBoundingClientRect()
		const currentX = event.clientX - rect.left
		const currentY = event.clientY - rect.top
		const deltaX = Math.abs(currentX - this.selectionStartX)
		const deltaY = Math.abs(currentY - this.selectionStartY)
		if (deltaX > 2 || deltaY > 2) {
			this.selectionMoved = true
		}
		this.updateSelectionRect(this.selectionStartX, this.selectionStartY, currentX, currentY)
		const selectionRect = this.getSelectionRect(this.selectionStartX, this.selectionStartY, currentX, currentY)
		this.updateSelectionFromRect(selectionRect, false)
	}

	private onSelectionPointerUp() {
		if (!this.isSelecting) {
			return
		}
		document.removeEventListener("pointermove", this.boundSelectionMove)
		document.removeEventListener("pointerup", this.boundSelectionUp)
		this.isSelecting = false
		this.selectionRect.style.display = "none"
		if (this.selectionMoved) {
			this.schedulePersist()
		} else {
			this.clearSelection()
		}
	}

	private updateSelectionRect(startX: number, startY: number, currentX: number, currentY: number) {
		const selectionRect = this.getSelectionRect(startX, startY, currentX, currentY)
		this.selectionRect.style.left = `${selectionRect.left}px`
		this.selectionRect.style.top = `${selectionRect.top}px`
		this.selectionRect.style.width = `${selectionRect.right - selectionRect.left}px`
		this.selectionRect.style.height = `${selectionRect.bottom - selectionRect.top}px`
	}

	private getSelectionRect(startX: number, startY: number, currentX: number, currentY: number): ScreenRect {
		const left = Math.min(startX, currentX)
		const top = Math.min(startY, currentY)
		const right = Math.max(startX, currentX)
		const bottom = Math.max(startY, currentY)
		return { left, top, right, bottom }
	}

	private updateSelectionFromRect(rect: ScreenRect, persist = true) {
		const selectedIds: number[] = []
		for (const node of this.nodes) {
			const bounds = this.getNodeScreenBounds(node)
			if (this.rectanglesIntersect(rect, bounds)) {
				selectedIds.push(node.id)
			}
		}
		this.setSelectedNodes(selectedIds, persist)
	}

	private getNodeScreenBounds(node: FlowchartNode): ScreenRect {
		const topLeft = this.worldToScreen(node.x, node.y)
		const width = node.width * this.zoom
		const height = node.height * this.zoom
		return {
			left: topLeft.x,
			top: topLeft.y,
			right: topLeft.x + width,
			bottom: topLeft.y + height
		}
	}

	private rectanglesIntersect(a: ScreenRect, b: ScreenRect) {
		return a.left <= b.right && a.right >= b.left && a.top <= b.bottom && a.bottom >= b.top
	}

	private handleConnectClick(node: FlowchartNode) {
		if (!this.connectMode) return
		if (this.connectStartNodeId === null) {
			this.connectStartNodeId = node.id
			this.updateNodeStyles()
			return
		}
		if (this.connectStartNodeId === node.id) {
			this.connectStartNodeId = null
			this.updateNodeStyles()
			return
		}
		this.createConnection(this.connectStartNodeId, node.id)
		this.connectStartNodeId = null
		this.updateNodeStyles()
	}

	private toggleConnectMode() {
		this.connectMode = !this.connectMode
		if (!this.connectMode) {
			this.connectStartNodeId = null
		}
		this.updateNodeStyles()
		this.schedulePersist()
	}

	private updateConnectButtonState() {
		if (this.connectMode) {
			this.connectButton.textContent = this.connectStartNodeId === null ? "Connect (Select start)" : "Connect (Select end)"
			this.connectButton.style.backgroundColor = "#2563eb"
			this.connectButton.style.color = "#ffffff"
		} else {
			this.connectButton.textContent = "Connect"
			this.connectButton.style.backgroundColor = "#eff6ff"
			this.connectButton.style.color = "#1d4ed8"
		}
	}

	private createConnection(fromId: number, toId: number) {
		if (fromId === toId) return
		const exists = this.connections.some((conn) => conn.from === fromId && conn.to === toId)
		if (exists) return
		const fromNode = this.nodes.find((node) => node.id === fromId)
		const toNode = this.nodes.find((node) => node.id === toId)
		if (!fromNode || !toNode) return

		const connection = this.createConnectionElement({
			id: this.nextConnectionId++,
			fromNode,
			toNode
		})

		this.connections.push(connection)
		this.updateConnectorPositions()
		this.schedulePersist()
	}

	private createConnectionFromPersisted(data: PersistedFlowchartConnection): FlowchartConnection | null {
		const fromNode = this.nodes.find((node) => node.id === data.from)
		const toNode = this.nodes.find((node) => node.id === data.to)
		if (!fromNode || !toNode) {
			return null
		}
		const connection = this.createConnectionElement({
			id: data.id,
			fromNode,
			toNode
		})
		return connection
	}

	private createConnectionElement(args: {
		id: number
		fromNode: FlowchartNode
		toNode: FlowchartNode
	}): FlowchartConnection {
		const path = document.createElementNS("http://www.w3.org/2000/svg", "path")
		path.setAttribute("fill", "none")
		path.setAttribute("stroke-linecap", "round")
		path.setAttribute("vector-effect", "non-scaling-stroke")
		path.style.cursor = "pointer"
		path.style.stroke = DiagramEditor.CONNECTION_STROKE_COLOR
		path.style.strokeWidth = "3px"
		path.style.transition = "stroke 120ms ease, stroke-width 120ms ease, filter 120ms ease"
		path.style.pointerEvents = "stroke"

		const connection: FlowchartConnection = {
			id: args.id,
			from: args.fromNode.id,
			to: args.toNode.id,
			path
		}

		path.addEventListener("pointerdown", (event: PointerEvent) => {
			if (event.button === 1) {
				this.startPan(event)
				return
			}
			if (event.button !== 0) {
				return
			}
			event.stopPropagation()
			this.selectConnection(connection.id)
		})

		this.svgLayer.appendChild(path)
		this.updateConnectionStyles()
		return connection
	}

	private updateConnectorPositions() {
		for (const connection of this.connections) {
			const fromNode = this.nodes.find((node) => node.id === connection.from)
			const toNode = this.nodes.find((node) => node.id === connection.to)
			if (!fromNode || !toNode) return
			const fromPoint = this.getNodeConnectionPoint(fromNode, "bottom")
			const toPoint = this.getNodeConnectionPoint(toNode, "top")
			const midX = (fromPoint.x + toPoint.x) / 2
			const pathData = `M ${fromPoint.x} ${fromPoint.y} L ${midX} ${fromPoint.y} L ${midX} ${toPoint.y} L ${toPoint.x} ${toPoint.y}`
			connection.path.setAttribute("d", pathData)
		}
	}

	private getNodeConnectionPoint(node: FlowchartNode, position: "top" | "bottom") {
		const x = node.x + node.width / 2
		const y = position === "top" ? node.y : node.y + node.height
		return { x, y }
	}

	private deleteSelection() {
		if (this.selectedNodeIds.size > 0) {
			const idsToDelete = new Set(this.selectedNodeIds)
			const remainingNodes: FlowchartNode[] = []
			for (const node of this.nodes) {
				if (idsToDelete.has(node.id)) {
					node.element.remove()
				} else {
					remainingNodes.push(node)
				}
			}
			this.nodes = remainingNodes
			this.connections = this.connections.filter((connection) => {
				if (idsToDelete.has(connection.from) || idsToDelete.has(connection.to)) {
					connection.path.remove()
					return false
				}
				return true
			})
			this.clearSelection(false)
			this.updateConnectorPositions()
			this.schedulePersist()
			return
		}
		if (this.selectedConnectionId !== null) {
			const index = this.connections.findIndex((connection) => connection.id === this.selectedConnectionId)
			if (index !== -1) {
				const [connection] = this.connections.splice(index, 1)
				if (connection) {
					connection.path.remove()
				}
			}
			this.selectedConnectionId = null
			this.updateSelectionStyles()
			this.schedulePersist()
		}
	}

	private schedulePersist() {
		if (this.isRestoring) {
			return
		}
		if (this.persistTimeout !== null) {
			window.clearTimeout(this.persistTimeout)
		}
		this.persistTimeout = window.setTimeout(() => {
			this.persistTimeout = null
			void this.saveToIndexedDB()
		}, DiagramEditor.PERSIST_DEBOUNCE_MS)
	}

	private async saveToIndexedDB() {
		try {
			const db = await this.getDatabase()
			const transaction = db.transaction(DiagramEditor.STORE_NAME, "readwrite")
			const store = transaction.objectStore(DiagramEditor.STORE_NAME)
			const selectedNodeIdsArray = Array.from(this.selectedNodeIds)
			const state: PersistedDiagramState = {
				nodes: this.nodes.map((node) => ({
					id: node.id,
					type: node.type,
					x: node.x,
					y: node.y,
					width: node.width,
					height: node.height,
					text: node.text
				})),
				connections: this.connections.map((connection) => ({
					id: connection.id,
					from: connection.from,
					to: connection.to
				})),
				nextNodeId: this.nextNodeId,
				nextConnectionId: this.nextConnectionId,
				selectedNodeId: selectedNodeIdsArray[0] ?? null,
				selectedNodeIds: selectedNodeIdsArray,
				selectedConnectionId: this.selectedConnectionId,
				connectMode: this.connectMode,
				panX: this.panX,
				panY: this.panY,
				zoom: this.zoom
			}
			store.put(state, DiagramEditor.STORE_KEY)
			await new Promise<void>((resolve, reject) => {
				transaction.oncomplete = () => resolve()
				transaction.onerror = () => reject(transaction.error)
				transaction.onabort = () => reject(transaction.error)
			})
		} catch (error) {
			console.error("Failed to save diagram state", error)
		}
	}

	private async loadFromIndexedDB() {
		try {
			const db = await this.getDatabase()
			const transaction = db.transaction(DiagramEditor.STORE_NAME, "readonly")
			const store = transaction.objectStore(DiagramEditor.STORE_NAME)
			const request = store.get(DiagramEditor.STORE_KEY)
			const result = await this.promisifyRequest<PersistedDiagramState | undefined>(request)
			await new Promise<void>((resolve, reject) => {
				transaction.oncomplete = () => resolve()
				transaction.onerror = () => reject(transaction.error)
				transaction.onabort = () => reject(transaction.error)
			})
			if (!result) {
				return
			}
			this.isRestoring = true
			this.nodesLayer.innerHTML = ""
			this.svgLayer.innerHTML = ""
			this.nodes = []
			this.connections = []
			let maxNodeId = 0
			for (const nodeData of result.nodes) {
				const node = this.createNodeFromPersisted(nodeData)
				this.nodes.push(node)
				this.nodesLayer.appendChild(node.element)
				if (node.id > maxNodeId) {
					maxNodeId = node.id
				}
			}
			let maxConnectionId = 0
			const connections = result.connections ?? []
			for (const connectionData of connections) {
				const connection = this.createConnectionFromPersisted(connectionData)
				if (connection) {
					this.connections.push(connection)
					if (connection.id > maxConnectionId) {
						maxConnectionId = connection.id
					}
				}
			}
			this.nextNodeId = Math.max(result.nextNodeId ?? 0, maxNodeId + 1)
			this.nextConnectionId = Math.max(result.nextConnectionId ?? 0, maxConnectionId + 1)
			const restoredSelectedNodes = result.selectedNodeIds ?? (result.selectedNodeId !== undefined && result.selectedNodeId !== null ? [result.selectedNodeId] : [])
			this.selectedNodeIds = new Set(restoredSelectedNodes)
			this.selectedConnectionId = result.selectedConnectionId ?? null
			this.connectMode = result.connectMode ?? false
			this.connectStartNodeId = null
			this.panX = result.panX ?? 0
			this.panY = result.panY ?? 0
			const persistedZoom = result.zoom ?? 1
			this.zoom = Math.min(this.maxZoom, Math.max(this.minZoom, persistedZoom))
			this.updateCanvasTransform()
			this.updateSelectionStyles()
			this.updateConnectorPositions()
		} catch (error) {
			console.error("Failed to load diagram state", error)
		} finally {
			this.isRestoring = false
		}
	}

	private createNodeFromPersisted(data: PersistedFlowchartNode): FlowchartNode {
		const node: FlowchartNode = {
			id: data.id,
			type: data.type,
			x: data.x,
			y: data.y,
			width: data.width,
			height: data.height,
			text: data.text,
			element: document.createElement("div"),
			textElement: document.createElement("div"),
			baseBorderColor: "#1f2937"
		}
		this.setupNodeElement(node)
		return node
	}

	private promisifyRequest<T>(request: IDBRequest<T>): Promise<T> {
		return new Promise<T>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result)
			request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"))
		})
	}

	private getDatabase(): Promise<IDBDatabase> {
		if (!this.databasePromise) {
			this.databasePromise = new Promise((resolve, reject) => {
				const request = indexedDB.open(DiagramEditor.DATABASE_NAME, 1)
				request.onerror = () => reject(request.error ?? new Error("Failed to open IndexedDB"))
				request.onsuccess = () => resolve(request.result)
				request.onupgradeneeded = () => {
					const db = request.result
					if (!db.objectStoreNames.contains(DiagramEditor.STORE_NAME)) {
						db.createObjectStore(DiagramEditor.STORE_NAME)
					}
				}
			})
		}
		return this.databasePromise
	}
}
