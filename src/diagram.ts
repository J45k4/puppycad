import { UiComponent, VList } from "./ui"

type FlowchartShape = "startEnd" | "process" | "decision" | "inputOutput"

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

type ShapeConfig = {
        label: string
        width: number
        height: number
        defaultText: string
}

const SHAPE_CONFIG: Record<FlowchartShape, ShapeConfig> = {
        startEnd: { label: "Start / End", width: 140, height: 60, defaultText: "Start" },
        process: { label: "Process", width: 160, height: 80, defaultText: "Process" },
        decision: { label: "Decision", width: 160, height: 160, defaultText: "Decision" },
        inputOutput: { label: "Input / Output", width: 170, height: 80, defaultText: "Data" }
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
        private selectedNodeId: number | null = null
        private connectMode = false
        private connectStartNodeId: number | null = null
        private draggingNode: FlowchartNode | null = null
        private dragOffsetX = 0
        private dragOffsetY = 0
        private connectButton: HTMLButtonElement
        private deleteButton: HTMLButtonElement
        private readonly boundPointerMove = this.onPointerMove.bind(this)
        private readonly boundPointerUp = this.onPointerUp.bind(this)
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
                        button.onmouseenter = () => button.style.backgroundColor = "#e2e8f0"
                        button.onmouseleave = () => button.style.backgroundColor = "#fff"
                        button.onclick = () => this.addNode(shape)
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
                this.deleteButton.onclick = () => this.deleteSelectedNode()

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
                this.svgLayer.style.pointerEvents = "none"
                this.svgLayer.style.transformOrigin = "0 0"

                this.nodesLayer = document.createElement("div")
                this.nodesLayer.style.position = "absolute"
                this.nodesLayer.style.top = "0"
                this.nodesLayer.style.left = "0"
                this.nodesLayer.style.right = "0"
                this.nodesLayer.style.bottom = "0"
                this.nodesLayer.style.transformOrigin = "0 0"

                this.canvasArea.appendChild(this.svgLayer)
                this.canvasArea.appendChild(this.nodesLayer)

                this.editorArea.appendChild(toolbar)
                this.editorArea.appendChild(this.canvasArea)

                this.root.appendChild(this.palette.root)
                this.root.appendChild(this.editorArea)

                this.canvasArea.addEventListener("pointerdown", (event: PointerEvent) => {
                        if (event.button === 1) {
                                this.startPan(event)
                        }
                })

                this.canvasArea.addEventListener("mousedown", (event) => {
                        if (event.target === this.canvasArea) {
                                this.selectNode(null)
                                if (this.connectMode && this.connectStartNodeId !== null) {
                                        this.connectStartNodeId = null
                                        this.updateNodeStyles()
                                }
                        }
                })

                this.canvasArea.addEventListener("wheel", this.boundWheel, { passive: false })

                this.updateCanvasTransform()
                window.addEventListener("resize", this.boundResize)
        }

        private getShapeConfig(shape: FlowchartShape): ShapeConfig {
                return SHAPE_CONFIG[shape]
        }

        private addNode(shape: FlowchartShape) {
                const config = this.getShapeConfig(shape)
                const rect = this.canvasArea.getBoundingClientRect()
                const screenCenterX = rect.width ? rect.width / 2 : 80
                const screenCenterY = rect.height ? rect.height / 2 : 80
                const worldCenterX = (screenCenterX - this.panX) / this.zoom
                const worldCenterY = (screenCenterY - this.panY) / this.zoom
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
                        this.selectNode(node.id)
                        element.style.cursor = "grabbing"
                        const pointerPosition = this.screenToWorld(event.clientX, event.clientY)
                        this.draggingNode = node
                        this.dragOffsetX = pointerPosition.x - node.x
                        this.dragOffsetY = pointerPosition.y - node.y
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
                        }
                })
        }

        private applyShapeStyle(element: HTMLDivElement, shape: FlowchartShape): string {
                element.style.borderRadius = "8px"
                element.style.clipPath = "none"
                element.style.transform = "none"
                element.style.background = "linear-gradient(180deg, #ffffff 0%, #f8fafc 100%)"

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
                }

                return borderColor
        }

        private screenToWorld(clientX: number, clientY: number) {
                const rect = this.canvasArea.getBoundingClientRect()
                const x = (clientX - rect.left - this.panX) / this.zoom
                const y = (clientY - rect.top - this.panY) / this.zoom
                return { x, y }
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
                        return
                }
                event.preventDefault()
                this.panX -= deltaX
                this.panY -= deltaY
                this.updateCanvasTransform()
        }

        private onPointerMove(event: PointerEvent) {
                if (!this.draggingNode) return
                const pointerPosition = this.screenToWorld(event.clientX, event.clientY)
                const newX = pointerPosition.x - this.dragOffsetX
                const newY = pointerPosition.y - this.dragOffsetY
                this.setNodePosition(this.draggingNode, newX, newY)
        }

        private onPointerUp() {
                if (this.draggingNode) {
                        this.draggingNode.element.style.cursor = "grab"
                        this.draggingNode = null
                }
                document.removeEventListener("pointermove", this.boundPointerMove)
                document.removeEventListener("pointerup", this.boundPointerUp)
        }

        private setNodePosition(node: FlowchartNode, x: number, y: number) {
                node.x = x
                node.y = y
                node.element.style.left = `${x}px`
                node.element.style.top = `${y}px`
                this.updateConnectorPositions()
        }

        private selectNode(nodeId: number | null) {
                this.selectedNodeId = nodeId
                this.updateNodeStyles()
        }

        private updateNodeStyles() {
                this.nodes.forEach(node => {
                        const isSelected = node.id === this.selectedNodeId
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
                })
                this.updateConnectButtonState()
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
                const exists = this.connections.some(conn => conn.from === fromId && conn.to === toId)
                if (exists) return
                const fromNode = this.nodes.find(node => node.id === fromId)
                const toNode = this.nodes.find(node => node.id === toId)
                if (!fromNode || !toNode) return

                const path = document.createElementNS("http://www.w3.org/2000/svg", "path")
                path.setAttribute("fill", "none")
                path.setAttribute("stroke", "#475569")
                path.setAttribute("stroke-width", "3")
                path.setAttribute("stroke-linecap", "round")
                path.setAttribute("vector-effect", "non-scaling-stroke")

                const connection: FlowchartConnection = {
                        id: this.nextConnectionId++,
                        from: fromId,
                        to: toId,
                        path
                }

                this.connections.push(connection)
                this.svgLayer.appendChild(path)
                this.updateConnectorPositions()
        }

        private updateConnectorPositions() {
                this.connections.forEach(connection => {
                        const fromNode = this.nodes.find(node => node.id === connection.from)
                        const toNode = this.nodes.find(node => node.id === connection.to)
                        if (!fromNode || !toNode) return
                        const fromPoint = this.getNodeConnectionPoint(fromNode, "bottom")
                        const toPoint = this.getNodeConnectionPoint(toNode, "top")
                        const midX = (fromPoint.x + toPoint.x) / 2
                        const pathData = `M ${fromPoint.x} ${fromPoint.y} L ${midX} ${fromPoint.y} L ${midX} ${toPoint.y} L ${toPoint.x} ${toPoint.y}`
                        connection.path.setAttribute("d", pathData)
                })
        }

        private getNodeConnectionPoint(node: FlowchartNode, position: "top" | "bottom") {
                const x = node.x + node.width / 2
                const y = position === "top" ? node.y : node.y + node.height
                return { x, y }
        }

        private deleteSelectedNode() {
                if (this.selectedNodeId === null) return
                const index = this.nodes.findIndex(node => node.id === this.selectedNodeId)
                if (index === -1) return
                const [node] = this.nodes.splice(index, 1)
                if (!node) return
                node.element.remove()
                this.connections = this.connections.filter(connection => {
                        if (connection.from === node.id || connection.to === node.id) {
                                connection.path.remove()
                                return false
                        }
                        return true
                })
                this.selectedNodeId = null
                this.updateNodeStyles()
                this.updateConnectorPositions()
        }
}
