import { HList, UiComponent, VList } from "./ui"

interface Component {
	id: number
	x: number
	y: number
	width: number
	height: number
}

type EdgeName = "left" | "right" | "top" | "bottom"

interface EdgeAnchor {
	component: Component
	edge: EdgeName
	point: { x: number; y: number }
}

interface ConnectionEndpoint {
	componentId: number
	edge: EdgeName
	ratio: number
}

interface Connection {
	from: ConnectionEndpoint
	to: ConnectionEndpoint
}

export class EditorCanvas extends UiComponent<HTMLDivElement> {
	private canvas: HTMLCanvasElement
	private ctx!: CanvasRenderingContext2D
	private scale = 1
	private originX = 0
	private originY = 0
	private components: Component[] = [
		{ id: 1, x: 100, y: 50, width: 80, height: 40 }
		// add more components here
	]
	private connections: Connection[] = []
	private selectedIds: number[] = []
	private isDraggingGroup = false
	private groupDragStartX = 0
	private groupDragStartY = 0
	private originalPositions: { id: number; x: number; y: number }[] = []
	private isSelecting = false
	private selectStartX = 0
	private selectStartY = 0
	private selectCurrentX = 0
	private selectCurrentY = 0
	private isConnecting = false
	private connectionStartAnchor: EdgeAnchor | null = null
	private connectionPreviewPoint: { x: number; y: number } | null = null
	private hoveredEdge: EdgeAnchor | null = null
	private readonly edgeHitPadding = 40

	public constructor() {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		const fitButton = document.createElement("button")
		fitButton.textContent = "FIT"
		fitButton.onclick = this.fitAllClicked.bind(this)
		const topButtonRow = new HList()
		topButtonRow.add(fitButton)
		this.root.appendChild(topButtonRow.root)

	this.canvas = document.createElement("canvas")
	this.canvas.width = 800
	this.canvas.height = 600
	const context = this.canvas.getContext("2d")
	if (!context) {
		throw new Error("Unable to acquire 2D drawing context")
	}
	this.ctx = context

		// Append canvas into the layout
		const middle = new HList()
		const rightButtonRow = new VList()
		middle.add(this.canvas, rightButtonRow)
		this.root.appendChild(middle.root)

		// Initial draw
		this.drawScene()

		this.canvas.addEventListener("wheel", (event) => {
			event.preventDefault()
			const rect = this.canvas.getBoundingClientRect()
			const mouseX = (event.clientX - rect.left - this.originX) / this.scale
			const mouseY = (event.clientY - rect.top - this.originY) / this.scale
			const zoomFactor = 1 - event.deltaY * 0.001
			this.originX -= mouseX * (zoomFactor - 1) * this.scale
			this.originY -= mouseY * (zoomFactor - 1) * this.scale
			this.scale *= zoomFactor
			this.drawScene()
		})

		let isPanning = false
		let panStartX = 0
		let panStartY = 0

		this.canvas.addEventListener("mousedown", (event) => {
			if (event.button === 1) {
				isPanning = true
				panStartX = event.clientX
				panStartY = event.clientY
				this.canvas.style.cursor = "grabbing"
			}
		})

		this.canvas.addEventListener("mousemove", (event) => {
			if (isPanning) {
				event.preventDefault()
				const dx = event.clientX - panStartX
				const dy = event.clientY - panStartY
				this.originX += dx
				this.originY += dy
				panStartX = event.clientX
				panStartY = event.clientY
				this.drawScene()
			}
		})

		this.canvas.addEventListener("mouseup", (event) => {
			if (event.button === 1) {
				isPanning = false
				this.canvas.style.cursor = this.hoveredEdge ? "crosshair" : "default"
			}
		})

		// Multi-select and group dragging
		this.canvas.addEventListener("mousedown", (event) => {
			if (event.button === 0) {
				console.log("left mouse down")
				const { wx, wy } = this.worldPointFromEvent(event)
				const hoverAnchor = this.findHoveredEdge(wx, wy)
				if (hoverAnchor) {
					this.isConnecting = true
					this.connectionStartAnchor = hoverAnchor
					this.connectionPreviewPoint = hoverAnchor.point
					this.hoveredEdge = hoverAnchor
					this.canvas.style.cursor = "crosshair"
					this.drawScene()
					return
				}
				// Check if clicked on a component
				const clicked = this.components.find((c) => wx >= c.x && wx <= c.x + c.width && wy >= c.y && wy <= c.y + c.height)
				if (clicked) {
					if (event.shiftKey) {
						const idx = this.selectedIds.indexOf(clicked.id)
						if (idx >= 0) this.selectedIds.splice(idx, 1)
						else this.selectedIds.push(clicked.id)
					} else {
						if (!this.selectedIds.includes(clicked.id)) this.selectedIds = [clicked.id]
					}
					// Start group drag
				this.isDraggingGroup = true
				this.groupDragStartX = wx
				this.groupDragStartY = wy
				const positions: { id: number; x: number; y: number }[] = []
				for (const id of this.selectedIds) {
					const comp = this.components.find((c) => c.id === id)
					if (comp) {
						positions.push({ id, x: comp.x, y: comp.y })
					}
				}
				this.originalPositions = positions
				} else {
					// Start selection rectangle
					this.selectedIds = []
					this.isSelecting = true
					this.selectStartX = wx
					this.selectStartY = wy
					this.selectCurrentX = wx
					this.selectCurrentY = wy
					this.hoveredEdge = null
					this.canvas.style.cursor = "default"
				}
				this.drawScene()
			}
		})
		this.canvas.addEventListener("mousemove", (event) => {
			if (isPanning) {
				this.canvas.style.cursor = "grabbing"
				return
			}
			const { wx, wy } = this.worldPointFromEvent(event)
			const previousHover = this.hoveredEdge
			const hovered = this.findHoveredEdge(wx, wy)
			if (this.isConnecting) {
				const startAnchor = this.connectionStartAnchor
				const candidateAnchor = hovered ?? this.findNearestEdgeAnchor(wx, wy)
				const isValidDrop = candidateAnchor && (!startAnchor || candidateAnchor.component.id !== startAnchor.component.id)
				this.hoveredEdge = isValidDrop ? candidateAnchor : null
				if (this.hoveredEdge) {
					this.connectionPreviewPoint = this.hoveredEdge.point
				} else {
					this.connectionPreviewPoint = { x: wx, y: wy }
				}
				this.canvas.style.cursor = "crosshair"
				this.drawScene()
			} else if (this.isDraggingGroup) {
				this.canvas.style.cursor = "grabbing"
				const dx = wx - this.groupDragStartX
				const dy = wy - this.groupDragStartY
				this.originalPositions.forEach((op) => {
					const comp = this.components.find((c) => c.id === op.id)!
					comp.x = op.x + dx
					comp.y = op.y + dy
				})
				this.drawScene()
			} else if (this.isSelecting) {
				this.canvas.style.cursor = "default"
				this.selectCurrentX = wx
				this.selectCurrentY = wy
				this.drawScene()
			} else {
				this.hoveredEdge = hovered
				if (hovered) {
					this.canvas.style.cursor = "crosshair"
				} else {
					this.canvas.style.cursor = "default"
				}
				if (hovered || previousHover) {
					this.drawScene()
				}
			}
		})
		this.canvas.addEventListener("mouseup", (event) => {
			if (event.button === 0) {
				const { wx, wy } = this.worldPointFromEvent(event)
				if (this.isConnecting) {
					const startAnchor = this.connectionStartAnchor
					const dropAnchor = this.hoveredEdge ?? this.findNearestEdgeAnchor(wx, wy)
					this.isConnecting = false
					this.connectionStartAnchor = null
					this.connectionPreviewPoint = null
					if (startAnchor && dropAnchor && dropAnchor.component.id !== startAnchor.component.id) {
						this.connections.push({
							from: this.endpointFromAnchor(startAnchor),
							to: this.endpointFromAnchor(dropAnchor)
						})
					}
					this.hoveredEdge = this.findHoveredEdge(wx, wy)
					this.canvas.style.cursor = this.hoveredEdge ? "crosshair" : "default"
					this.drawScene()
					return
				}
				if (this.isDraggingGroup) this.isDraggingGroup = false
				if (this.isSelecting) {
					const x1 = Math.min(this.selectStartX, this.selectCurrentX)
					const y1 = Math.min(this.selectStartY, this.selectCurrentY)
					const x2 = Math.max(this.selectStartX, this.selectCurrentX)
					const y2 = Math.max(this.selectStartY, this.selectCurrentY)
					this.selectedIds = this.components.filter((c) => c.x < x2 && c.x + c.width > x1 && c.y < y2 && c.y + c.height > y1).map((c) => c.id)
					this.isSelecting = false
				}
				this.drawScene()
				if (!this.isConnecting) this.canvas.style.cursor = this.hoveredEdge ? "crosshair" : "default"
			}
		})

		this.canvas.addEventListener("mouseleave", () => {
			isPanning = false
			if (this.isConnecting) {
				this.isConnecting = false
				this.connectionStartAnchor = null
				this.connectionPreviewPoint = null
				this.drawScene()
			}
			const hadHover = this.hoveredEdge !== null
			this.hoveredEdge = null
			if (hadHover) this.drawScene()
			this.canvas.style.cursor = "default"
		})

		// Allow dropping new components from sidebar
		this.canvas.addEventListener("dragover", (event) => {
			event.preventDefault()
		})
		this.canvas.addEventListener("drop", (event) => {
			event.preventDefault()
			const type = event.dataTransfer!.getData("component")
			const rect = this.canvas.getBoundingClientRect()
			const wx = (event.clientX - rect.left - this.originX) / this.scale
			const wy = (event.clientY - rect.top - this.originY) / this.scale
			this.addComponent(type, wx, wy)
		})
	}

	private drawScene() {
		this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height)
		this.ctx.save()
		this.ctx.setTransform(this.scale, 0, 0, this.scale, this.originX, this.originY)

		// Draw infinite grid
		const gridSpacing = 100
		const worldLeft = -this.originX / this.scale
		const worldRight = (this.canvas.width - this.originX) / this.scale
		const worldTop = -this.originY / this.scale
		const worldBottom = (this.canvas.height - this.originY) / this.scale

		this.ctx.strokeStyle = "#e0e0e0"
		this.ctx.lineWidth = 1

		// Vertical lines
		for (let x = Math.floor(worldLeft / gridSpacing) * gridSpacing; x <= worldRight; x += gridSpacing) {
			this.ctx.beginPath()
			this.ctx.moveTo(x, worldTop)
			this.ctx.lineTo(x, worldBottom)
			this.ctx.stroke()
		}
		// Horizontal lines
		for (let y = Math.floor(worldTop / gridSpacing) * gridSpacing; y <= worldBottom; y += gridSpacing) {
			this.ctx.beginPath()
			this.ctx.moveTo(worldLeft, y)
			this.ctx.lineTo(worldRight, y)
			this.ctx.stroke()
		}

		// Draw selection rectangle
		if (this.isSelecting) {
			this.ctx.save()
			this.ctx.strokeStyle = "blue"
			this.ctx.lineWidth = 1
			this.ctx.setLineDash([5, 5])
			const sx = this.selectStartX
			const sy = this.selectStartY
			const ex = this.selectCurrentX
			const ey = this.selectCurrentY
			this.ctx.strokeRect(sx, sy, ex - sx, ey - sy)
			this.ctx.restore()
		}

		// Draw existing connections
		this.connections.forEach((conn) => {
			const fromPoint = this.pointFromEndpoint(conn.from)
			const toPoint = this.pointFromEndpoint(conn.to)
			if (!fromPoint || !toPoint) return
			this.ctx.strokeStyle = "#444"
			this.ctx.lineWidth = 2
			this.ctx.beginPath()
			this.ctx.moveTo(fromPoint.x, fromPoint.y)
			this.ctx.lineTo(toPoint.x, toPoint.y)
			this.ctx.stroke()
		})

		// Draw preview connection if connecting
		if (this.isConnecting && this.connectionStartAnchor && this.connectionPreviewPoint) {
			this.ctx.strokeStyle = "#888"
			this.ctx.lineWidth = 2
			this.ctx.setLineDash([4, 4])
			this.ctx.beginPath()
			this.ctx.moveTo(this.connectionStartAnchor.point.x, this.connectionStartAnchor.point.y)
			this.ctx.lineTo(this.connectionPreviewPoint.x, this.connectionPreviewPoint.y)
			this.ctx.stroke()
			this.ctx.setLineDash([])
		}

		// Draw components
		this.components.forEach((comp) => {
			this.ctx.fillStyle = "white"
			this.ctx.strokeStyle = "black"
			this.ctx.lineWidth = 2
			this.ctx.fillRect(comp.x, comp.y, comp.width, comp.height)
			this.ctx.strokeRect(comp.x, comp.y, comp.width, comp.height)
			if (this.selectedIds.includes(comp.id)) {
				this.ctx.strokeStyle = "blue"
				this.ctx.lineWidth = 2
				this.ctx.strokeRect(comp.x - 2, comp.y - 2, comp.width + 4, comp.height + 4)
			}
			this.ctx.font = "16px Arial"
			this.ctx.fillStyle = "black"
			this.ctx.fillText(`R${comp.id}`, comp.x + 10, comp.y + 25)
		})

		if (this.connectionStartAnchor) {
			this.drawEdgeHighlight(this.connectionStartAnchor, "#1565c0")
			this.drawAnchorMarker(this.connectionStartAnchor.point, "#1565c0")
		}
		if (this.hoveredEdge) {
			const color = this.isConnecting ? "#2e7d32" : "#ef6c00"
			this.drawEdgeHighlight(this.hoveredEdge, color)
			this.drawAnchorMarker(this.hoveredEdge.point, color)
		}

		this.ctx.restore()
	}

	private getEdgeAnchor(component: Component, x: number, y: number, tolerance = this.edgeHitPadding / this.scale): EdgeAnchor | null {
		const expandedLeft = component.x - tolerance
		const expandedRight = component.x + component.width + tolerance
		const expandedTop = component.y - tolerance
		const expandedBottom = component.y + component.height + tolerance
		if (x < expandedLeft || x > expandedRight || y < expandedTop || y > expandedBottom) return null

		const { edge, point, distance } = this.computeAnchor(component, x, y)
		if (distance > tolerance) return null
		return { component, edge, point }
	}

	private getNearestEdgeAnchor(component: Component, x: number, y: number): EdgeAnchor {
		const { edge, point } = this.computeAnchor(component, x, y)
		return { component, edge, point }
	}

	private anchorForEdge(component: Component, edge: EdgeName, x: number, y: number): { x: number; y: number } {
		switch (edge) {
			case "left":
				return {
					x: component.x,
					y: this.clamp(y, component.y, component.y + component.height)
				}
			case "right":
				return {
					x: component.x + component.width,
					y: this.clamp(y, component.y, component.y + component.height)
				}
			case "top":
				return {
					x: this.clamp(x, component.x, component.x + component.width),
					y: component.y
				}
			case "bottom":
				return {
					x: this.clamp(x, component.x, component.x + component.width),
					y: component.y + component.height
				}
		}
		return { x: component.x, y: component.y }
	}

	private clamp(value: number, min: number, max: number) {
		return Math.max(min, Math.min(max, value))
	}

	private worldPointFromEvent(event: MouseEvent) {
		const rect = this.canvas.getBoundingClientRect()
		const wx = (event.clientX - rect.left - this.originX) / this.scale
		const wy = (event.clientY - rect.top - this.originY) / this.scale
		return { wx, wy }
	}

	private findHoveredEdge(x: number, y: number) {
		let closest: EdgeAnchor | null = null
		let closestDistance = Number.POSITIVE_INFINITY
		const tolerance = this.edgeHitPadding / this.scale
		for (const component of this.components) {
			const anchor = this.getEdgeAnchor(component, x, y, tolerance)
			if (!anchor) continue
			const distance = this.distanceToAnchor(anchor, x, y)
			if (distance < closestDistance) {
				closest = anchor
				closestDistance = distance
			}
		}
		console.log("Hovered edge:", closest)
		return closest
	}

	private findNearestEdgeAnchor(x: number, y: number): EdgeAnchor | null {
		const insideComponent = this.components.find((component) => this.isPointInsideComponent(component, x, y))
		if (insideComponent) {
			return this.getNearestEdgeAnchor(insideComponent, x, y)
		}

		const tolerance = this.edgeHitPadding / this.scale
		let closest: EdgeAnchor | null = null
		let closestDistance = Number.POSITIVE_INFINITY

		for (const component of this.components) {
			const anchor = this.getEdgeAnchor(component, x, y, tolerance)
			if (!anchor) continue
			const distance = this.distanceBetweenPoints(anchor.point, { x, y })
			if (distance < closestDistance) {
				closest = anchor
				closestDistance = distance
			}
		}
		return closest
	}

	private computeAnchor(component: Component, x: number, y: number): { edge: EdgeName; point: { x: number; y: number }; distance: number } {
		const distances = [
			{ edge: "left", distance: Math.abs(x - component.x) },
			{ edge: "right", distance: Math.abs(x - (component.x + component.width)) },
			{ edge: "top", distance: Math.abs(y - component.y) },
			{ edge: "bottom", distance: Math.abs(y - (component.y + component.height)) }
		]
		const closest = distances.reduce((prev, curr) => (curr.distance < prev.distance ? curr : prev))
		return {
			edge: closest.edge as EdgeName,
			point: this.anchorForEdge(component, closest.edge as EdgeName, x, y),
			distance: closest.distance
		}
	}

	private drawAnchorMarker(point: { x: number; y: number }, color: string) {
		this.ctx.save()
		const radius = 6 / this.scale
		this.ctx.fillStyle = color
		this.ctx.strokeStyle = "#ffffff"
		this.ctx.lineWidth = 2 / this.scale
		this.ctx.beginPath()
		this.ctx.arc(point.x, point.y, radius, 0, Math.PI * 2)
		this.ctx.fill()
		this.ctx.stroke()
		this.ctx.restore()
	}

	private drawEdgeHighlight(anchor: EdgeAnchor, color: string) {
		this.ctx.save()
		this.ctx.strokeStyle = color
		this.ctx.lineWidth = 4 / this.scale
		this.ctx.beginPath()
		const { component, edge } = anchor
		switch (edge) {
			case "left":
				this.ctx.moveTo(component.x, component.y)
				this.ctx.lineTo(component.x, component.y + component.height)
				break
			case "right":
				this.ctx.moveTo(component.x + component.width, component.y)
				this.ctx.lineTo(component.x + component.width, component.y + component.height)
				break
			case "top":
				this.ctx.moveTo(component.x, component.y)
				this.ctx.lineTo(component.x + component.width, component.y)
				break
			case "bottom":
				this.ctx.moveTo(component.x, component.y + component.height)
				this.ctx.lineTo(component.x + component.width, component.y + component.height)
				break
		}
		this.ctx.stroke()
		this.ctx.restore()
	}

	private distanceToAnchor(anchor: EdgeAnchor, x: number, y: number): number {
		switch (anchor.edge) {
			case "left":
			case "right":
				return Math.abs(x - anchor.point.x)
			case "top":
			case "bottom":
				return Math.abs(y - anchor.point.y)
		}
		return Number.POSITIVE_INFINITY
	}

	private endpointFromAnchor(anchor: EdgeAnchor): ConnectionEndpoint {
		const ratio =
			anchor.edge === "left" || anchor.edge === "right"
				? this.safeRatio(anchor.point.y - anchor.component.y, anchor.component.height)
				: this.safeRatio(anchor.point.x - anchor.component.x, anchor.component.width)
		return {
			componentId: anchor.component.id,
			edge: anchor.edge,
			ratio: this.clamp(ratio, 0, 1)
		}
	}

	private pointFromEndpoint(endpoint: ConnectionEndpoint): { x: number; y: number } | null {
		const component = this.components.find((c) => c.id === endpoint.componentId)
		if (!component) return null
		return this.anchorPointForRatio(component, endpoint.edge, endpoint.ratio)
	}

	private anchorPointForRatio(component: Component, edge: EdgeName, ratio: number) {
		const clamped = this.clamp(ratio, 0, 1)
		switch (edge) {
			case "left":
				return { x: component.x, y: component.y + component.height * clamped }
			case "right":
				return {
					x: component.x + component.width,
					y: component.y + component.height * clamped
				}
			case "top":
				return { x: component.x + component.width * clamped, y: component.y }
			case "bottom":
				return {
					x: component.x + component.width * clamped,
					y: component.y + component.height
				}
		}
		return { x: component.x, y: component.y }
	}

	private safeRatio(delta: number, size: number) {
		if (size === 0) return 0
		return delta / size
	}

	private distanceBetweenPoints(a: { x: number; y: number }, b: { x: number; y: number }) {
		return Math.hypot(a.x - b.x, a.y - b.y)
	}

	private isPointInsideComponent(component: Component, x: number, y: number) {
		return x >= component.x && x <= component.x + component.width && y >= component.y && y <= component.y + component.height
	}

	private fitAllClicked() {
		// Fit view to show all components
		const xs = this.components.map((c) => c.x)
		const ys = this.components.map((c) => c.y)
		const xsMax = this.components.map((c) => c.x + c.width)
		const ysMax = this.components.map((c) => c.y + c.height)
		const minX = Math.min(...xs),
			maxX = Math.max(...xsMax)
		const minY = Math.min(...ys),
			maxY = Math.max(...ysMax)
		const bboxWidth = maxX - minX
		const bboxHeight = maxY - minY
		const bboxCenterX = minX + bboxWidth / 2
		const bboxCenterY = minY + bboxHeight / 2
		const scaleX = this.canvas.width / bboxWidth
		const scaleY = this.canvas.height / bboxHeight
		this.scale = Math.min(scaleX, scaleY) * 0.9
		this.originX = this.canvas.width / 2 - bboxCenterX * this.scale
		this.originY = this.canvas.height / 2 - bboxCenterY * this.scale
		this.drawScene()
	}

	public addComponent(type: string, x: number, y: number) {
		const newId = Math.max(0, ...this.components.map((c) => c.id)) + 1
		let size = { width: 80, height: 40 }
		switch (type) {
			case "resistor":
				size = { width: 80, height: 40 }
				break
			case "capacitor":
				size = { width: 60, height: 60 }
				break
			// add more cases as needed
			default:
				console.warn(`Unknown component type: ${type}`)
				return
		}
		const newComp: Component = {
			id: newId,
			x,
			y,
			width: size.width,
			height: size.height
		}
		this.components.push(newComp)
		this.drawScene()
	}
}
