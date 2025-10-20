import { HList, UiComponent, VList } from "./ui"

export type EdgeName = "left" | "right" | "top" | "bottom"

export interface CanvasComponent<TData = unknown> {
	id: number
	x: number
	y: number
	width: number
	height: number
	data?: TData
}

export interface ConnectionEndpoint {
	componentId: number
	edge: EdgeName
	ratio: number
}

export type ConnectionStyle = "solid" | "dashed"

export interface Connection {
	from: ConnectionEndpoint
	to: ConnectionEndpoint
	style?: ConnectionStyle
}

type EdgeAnchor<TData> = {
	component: CanvasComponent<TData>
	edge: EdgeName
	point: { x: number; y: number }
}

type NewComponent<TData> = Omit<CanvasComponent<TData>, "id"> & Partial<Pick<CanvasComponent<TData>, "id">>

export interface EditorCanvasOptions<TData = unknown> {
	initialComponents?: CanvasComponent<TData>[]
	initialConnections?: Connection[]
	createConnection?: (from: ConnectionEndpoint, to: ConnectionEndpoint) => Connection
	createComponent?: (type: string, position: { x: number; y: number }, helpers: { createId: () => number }) => NewComponent<TData> | null | undefined
	renderComponent?: (ctx: CanvasRenderingContext2D, component: CanvasComponent<TData>, state: { selected: boolean }) => void
	getComponentLabel?: (component: CanvasComponent<TData>) => string
	gridSpacing?: number
	onSelectionChange?: (ids: number[]) => void
	onComponentsChange?: (components: CanvasComponent<TData>[]) => void
	onConnectionsChange?: (connections: Connection[]) => void
	renderConnection?: (
		ctx: CanvasRenderingContext2D,
		connection: Connection,
		state: {
			selected: boolean
			from: { x: number; y: number }
			to: { x: number; y: number }
		}
	) => void
}

const cloneValue = <T>(value: T): T => {
	if (value === undefined || value === null) {
		return value
	}
	if (typeof structuredClone === "function") {
		return structuredClone(value)
	}
	return JSON.parse(JSON.stringify(value)) as T
}

const cloneComponent = <TData>(component: CanvasComponent<TData>): CanvasComponent<TData> => ({
	...component,
	data: component.data === undefined ? undefined : cloneValue(component.data)
})

const cloneConnection = (connection: Connection): Connection => ({
	from: { ...connection.from },
	to: { ...connection.to },
	style: connection.style
})

const connectionEndpointsEqual = (a: ConnectionEndpoint, b: ConnectionEndpoint): boolean => a.componentId === b.componentId && a.edge === b.edge && a.ratio === b.ratio

const connectionsEqual = (a: Connection, b: Connection): boolean => connectionEndpointsEqual(a.from, b.from) && connectionEndpointsEqual(a.to, b.to) && a.style === b.style

export class EditorCanvas<TData = unknown> extends UiComponent<HTMLDivElement> {
	public readonly canvasElement: HTMLCanvasElement

	private readonly options: EditorCanvasOptions<TData>
	private readonly ctx: CanvasRenderingContext2D
	private scale = 1
	private originX = 0
	private originY = 0
	private components: CanvasComponent<TData>[]
	private connections: Connection[]
	private selectedIds: number[] = []
	private selectedConnectionIndex: number | null = null
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
	private connectionStartAnchor: EdgeAnchor<TData> | null = null
	private connectionPreviewPoint: { x: number; y: number } | null = null
	private hoveredEdge: EdgeAnchor<TData> | null = null
	private readonly edgeHitPadding = 40
	private readonly connectionHitPadding = 12
	private nextComponentId = 1
	private isPanning = false
	private panStartX = 0
	private panStartY = 0
	private movedDuringDrag = false

	private readonly handleKeydown = (event: KeyboardEvent): void => {
		if (event.key !== "Delete" && event.key !== "Backspace") {
			return
		}

		if (this.selectedIds.length > 0) {
			const ids = [...this.selectedIds]
			for (const id of ids) {
				this.removeComponent(id)
			}
			event.preventDefault()
			return
		}

		if (this.selectedConnectionIndex !== null) {
			this.removeConnectionAt(this.selectedConnectionIndex)
			event.preventDefault()
		}
	}

	public constructor(options?: EditorCanvasOptions<TData>) {
		super(document.createElement("div"))
		this.options = options ?? {}
		this.components = (options?.initialComponents ?? [{ id: 1, x: 100, y: 50, width: 80, height: 40 }]).map((component) => cloneComponent(component))
		this.connections = (options?.initialConnections ?? []).map((connection) => cloneConnection(connection))
		this.nextComponentId = this.computeNextComponentId()

		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.gap = "12px"
		this.root.style.background = "#f8fafc"
		this.root.style.borderRadius = "12px"
		this.root.style.padding = "12px"
		this.root.style.boxSizing = "border-box"
		this.root.style.flex = "1 1 auto"
		this.root.style.minWidth = "0"
		this.root.style.minHeight = "0"

		const topButtonRow = new HList()
		const fitButton = document.createElement("button")
		fitButton.textContent = "Fit"
		fitButton.addEventListener("click", () => this.fitAllClicked())
		topButtonRow.add(fitButton)
		this.root.appendChild(topButtonRow.root)

		this.canvasElement = document.createElement("canvas")
		this.canvasElement.width = 1200
		this.canvasElement.height = 800
		this.canvasElement.style.width = "100%"
		this.canvasElement.style.maxHeight = "100%"
		this.canvasElement.style.flex = "1 1 auto"
		this.canvasElement.style.minWidth = "0"
		this.canvasElement.style.borderRadius = "8px"
		this.canvasElement.style.background = "#ffffff"
		this.canvasElement.style.boxShadow = "0 10px 25px rgba(15, 23, 42, 0.12)"
		this.canvasElement.style.cursor = "default"
		this.canvasElement.tabIndex = 0

		const context = this.canvasElement.getContext("2d")
		if (!context) {
			throw new Error("Unable to acquire 2D drawing context")
		}
		this.ctx = context

		const canvasRow = new HList()
		canvasRow.root.style.flex = "1 1 auto"
		canvasRow.root.style.minWidth = "0"
		canvasRow.root.style.minHeight = "0"
		canvasRow.root.style.alignItems = "stretch"
		const rightButtonRow = new VList({
			style: {
				gap: "8px",
				alignItems: "stretch"
			}
		})
		rightButtonRow.root.style.flex = "0 0 auto"
		canvasRow.add(this.canvasElement, rightButtonRow)
		this.root.appendChild(canvasRow.root)

		this.drawScene()
		this.setupEventHandlers()
		this.canvasElement.addEventListener("keydown", this.handleKeydown)
	}

	public redraw(): void {
		this.drawScene()
	}

	public setGridSpacing(spacing: number): void {
		if (!Number.isFinite(spacing) || spacing <= 0) {
			return
		}
		this.options.gridSpacing = spacing
		this.drawScene()
	}

	public getGridSpacing(): number {
		return this.options.gridSpacing ?? 100
	}

	public setComponents(components: CanvasComponent<TData>[]): void {
		this.components = components.map((component) => cloneComponent(component))
		this.nextComponentId = this.computeNextComponentId()
		const hadConnectionSelection = this.selectedConnectionIndex !== null
		this.selectedConnectionIndex = null
		this.drawScene()
		this.emitComponentsChange()
		if (hadConnectionSelection) {
			this.emitSelectionChange()
		}
	}

	public getComponents(): CanvasComponent<TData>[] {
		return this.components.map((component) => cloneComponent(component))
	}

	public setConnections(connections: Connection[]): void {
		this.connections = connections.map((connection) => cloneConnection(connection))
		const hadConnectionSelection = this.selectedConnectionIndex !== null
		this.selectedConnectionIndex = null
		this.drawScene()
		this.emitConnectionsChange()
		if (hadConnectionSelection) {
			this.emitSelectionChange()
		}
	}

	public getConnections(): Connection[] {
		return this.connections.map((connection) => cloneConnection(connection))
	}

	public addComponent(component: NewComponent<TData>): CanvasComponent<TData> {
		const newComponent: CanvasComponent<TData> = {
			...component,
			id: component.id ?? this.createComponentId(),
			data: component.data
		}
		this.components.push(cloneComponent(newComponent))
		this.drawScene()
		this.emitComponentsChange()
		return newComponent
	}

	public removeComponent(id: number): void {
		const originalLength = this.components.length
		this.components = this.components.filter((component) => component.id !== id)
		if (this.components.length !== originalLength) {
			this.connections = this.connections.filter((connection) => connection.from.componentId !== id && connection.to.componentId !== id)
			this.selectedIds = this.selectedIds.filter((selectedId) => selectedId !== id)
			this.selectedConnectionIndex = null
			this.drawScene()
			this.emitComponentsChange()
			this.emitConnectionsChange()
			this.emitSelectionChange()
			this.nextComponentId = this.computeNextComponentId()
		}
	}

	public removeConnectionAt(index: number): void {
		if (index < 0 || index >= this.connections.length) {
			return
		}

		let selectionChanged = false
		this.connections.splice(index, 1)

		if (this.selectedConnectionIndex === index) {
			this.selectedConnectionIndex = null
			selectionChanged = true
		} else if (this.selectedConnectionIndex !== null && this.selectedConnectionIndex > index) {
			this.selectedConnectionIndex -= 1
		}

		this.drawScene()
		this.emitConnectionsChange()
		if (selectionChanged) {
			this.emitSelectionChange()
		}
	}

	public getSelectedConnection(): Connection | null {
		if (this.selectedConnectionIndex === null) {
			return null
		}

		const connection = this.connections[this.selectedConnectionIndex]
		return connection ? cloneConnection(connection) : null
	}

	public updateSelectedConnection(updater: (connection: Connection) => void): Connection | null {
		if (this.selectedConnectionIndex === null) {
			return null
		}

		const connection = this.connections[this.selectedConnectionIndex]
		if (!connection) {
			return null
		}

		const before = cloneConnection(connection)
		updater(connection)
		const changed = !connectionsEqual(before, connection)
		if (changed) {
			this.drawScene()
			this.emitConnectionsChange()
		}
		return cloneConnection(connection)
	}

	public updateComponent(id: number, update: Partial<CanvasComponent<TData>>): CanvasComponent<TData> | null {
		const component = this.components.find((candidate) => candidate.id === id)
		if (!component) {
			return null
		}
		Object.assign(component, update)
		this.drawScene()
		this.emitComponentsChange()
		return cloneComponent(component)
	}

	public clearSelection(): void {
		if (this.selectedIds.length === 0 && this.selectedConnectionIndex === null) {
			return
		}
		this.selectedIds = []
		this.selectedConnectionIndex = null
		this.drawScene()
		this.emitSelectionChange()
	}

	public setSelection(ids: number[]): void {
		this.selectedConnectionIndex = null
		this.selectedIds = [...new Set(ids)]
		this.drawScene()
		this.emitSelectionChange()
	}

	public getSelection(): number[] {
		return [...this.selectedIds]
	}

	public toWorldPoint(clientX: number, clientY: number): { x: number; y: number } {
		const canvasPoint = this.clientToCanvasPoint(clientX, clientY)
		return {
			x: (canvasPoint.x - this.originX) / this.scale,
			y: (canvasPoint.y - this.originY) / this.scale
		}
	}

	private clientToCanvasPoint(clientX: number, clientY: number): { x: number; y: number } {
		const rect = this.canvasElement.getBoundingClientRect()
		const scaleX = rect.width === 0 ? 1 : this.canvasElement.width / rect.width
		const scaleY = rect.height === 0 ? 1 : this.canvasElement.height / rect.height
		return {
			x: (clientX - rect.left) * scaleX,
			y: (clientY - rect.top) * scaleY
		}
	}

	private clientDeltaToCanvas(deltaX: number, deltaY: number): { x: number; y: number } {
		const rect = this.canvasElement.getBoundingClientRect()
		const scaleX = rect.width === 0 ? 1 : this.canvasElement.width / rect.width
		const scaleY = rect.height === 0 ? 1 : this.canvasElement.height / rect.height
		return {
			x: deltaX * scaleX,
			y: deltaY * scaleY
		}
	}

	private setupEventHandlers(): void {
		this.canvasElement.addEventListener("wheel", (event) => {
			event.preventDefault()
			const { x: mouseX, y: mouseY } = this.toWorldPoint(event.clientX, event.clientY)
			const zoomFactor = 1 - event.deltaY * 0.001
			this.originX -= mouseX * (zoomFactor - 1) * this.scale
			this.originY -= mouseY * (zoomFactor - 1) * this.scale
			this.scale *= zoomFactor
			this.drawScene()
		})

		this.canvasElement.addEventListener("mousedown", (event) => {
			if (event.button === 1) {
				this.isPanning = true
				this.panStartX = event.clientX
				this.panStartY = event.clientY
				this.canvasElement.style.cursor = "grabbing"
				return
			}

			if (event.button !== 0) {
				return
			}

			this.canvasElement.focus({ preventScroll: true })

			const { x, y } = this.toWorldPoint(event.clientX, event.clientY)
			this.movedDuringDrag = false
			const clicked = this.components.find((component) => this.isPointInsideComponent(component, x, y))
			const hoveredEdge = this.findHoveredEdge(x, y)
			const shouldStartConnection = hoveredEdge !== null && (!clicked || this.isNearComponentEdge(clicked, x, y))

			if (shouldStartConnection && hoveredEdge) {
				const hadConnectionSelection = this.selectedConnectionIndex !== null
				this.isConnecting = true
				this.connectionStartAnchor = hoveredEdge
				this.connectionPreviewPoint = hoveredEdge.point
				this.hoveredEdge = hoveredEdge
				this.selectedConnectionIndex = null
				this.canvasElement.style.cursor = "crosshair"
				this.drawScene()
				if (hadConnectionSelection) {
					this.emitSelectionChange()
				}
				return
			}

			const connectionIndex = this.findConnectionNearPoint(x, y)

			if (connectionIndex !== null && !clicked) {
				const hadSelection = this.selectedConnectionIndex !== connectionIndex || this.selectedIds.length > 0
				this.selectedIds = []
				this.selectedConnectionIndex = connectionIndex
				this.isDraggingGroup = false
				this.isSelecting = false
				this.hoveredEdge = null
				this.canvasElement.style.cursor = "default"
				this.drawScene()
				if (hadSelection) {
					this.emitSelectionChange()
				}
				return
			}

			if (clicked) {
				this.hoveredEdge = null
				this.selectedConnectionIndex = null
				if (event.shiftKey) {
					if (this.selectedIds.includes(clicked.id)) {
						this.selectedIds = this.selectedIds.filter((id) => id !== clicked.id)
					} else {
						this.selectedIds = [...this.selectedIds, clicked.id]
					}
				} else {
					if (!this.selectedIds.includes(clicked.id)) {
						this.selectedIds = [clicked.id]
					}
				}
				this.isDraggingGroup = true
				this.groupDragStartX = x
				this.groupDragStartY = y
				this.originalPositions = this.selectedIds
					.map((id) => {
						const component = this.components.find((candidate) => candidate.id === id)
						if (!component) {
							return null
						}
						return { id, x: component.x, y: component.y }
					})
					.filter((value): value is { id: number; x: number; y: number } => value !== null)
			} else {
				this.selectedIds = []
				this.selectedConnectionIndex = null
				this.isSelecting = true
				this.selectStartX = x
				this.selectStartY = y
				this.selectCurrentX = x
				this.selectCurrentY = y
				this.hoveredEdge = null
				this.canvasElement.style.cursor = "default"
			}
			this.drawScene()
			this.emitSelectionChange()
		})

		this.canvasElement.addEventListener("mousemove", (event) => {
			if (this.isPanning) {
				event.preventDefault()
				const delta = this.clientDeltaToCanvas(event.clientX - this.panStartX, event.clientY - this.panStartY)
				this.originX += delta.x
				this.originY += delta.y
				this.panStartX = event.clientX
				this.panStartY = event.clientY
				this.drawScene()
				return
			}

			const { x, y } = this.toWorldPoint(event.clientX, event.clientY)
			const previousHover = this.hoveredEdge

			if (this.isConnecting && this.connectionStartAnchor) {
				const candidate = this.findHoveredEdge(x, y) ?? this.findNearestEdgeAnchor(x, y)
				const startAnchor = this.connectionStartAnchor
				const isValid = candidate && candidate.component.id !== startAnchor.component.id
				this.hoveredEdge = isValid ? candidate : null
				this.connectionPreviewPoint = this.hoveredEdge ? this.hoveredEdge.point : { x, y }
				this.canvasElement.style.cursor = "crosshair"
				this.drawScene()
				return
			}

			if (this.isDraggingGroup) {
				const dx = x - this.groupDragStartX
				const dy = y - this.groupDragStartY
				for (const original of this.originalPositions) {
					const component = this.components.find((candidate) => candidate.id === original.id)
					if (!component) {
						continue
					}
					component.x = original.x + dx
					component.y = original.y + dy
				}
				this.movedDuringDrag = true
				this.drawScene()
				return
			}

			if (this.isSelecting) {
				this.selectCurrentX = x
				this.selectCurrentY = y
				this.drawScene()
				return
			}

			let hovered = this.findHoveredEdge(x, y)
			if (hovered && this.isPointInsideComponent(hovered.component, x, y) && !this.isNearComponentEdge(hovered.component, x, y)) {
				hovered = null
			}
			this.hoveredEdge = hovered
			if (this.hoveredEdge || previousHover) {
				this.canvasElement.style.cursor = this.hoveredEdge ? "crosshair" : "default"
				this.drawScene()
			}
		})

		this.canvasElement.addEventListener("mouseup", (event) => {
			if (event.button === 1) {
				this.isPanning = false
				this.canvasElement.style.cursor = this.hoveredEdge ? "crosshair" : "default"
				return
			}

			if (event.button !== 0) {
				return
			}

			const { x, y } = this.toWorldPoint(event.clientX, event.clientY)
			if (this.isConnecting) {
				const startAnchor = this.connectionStartAnchor
				const dropAnchor = this.hoveredEdge ?? this.findNearestEdgeAnchor(x, y)
				this.isConnecting = false
				this.connectionStartAnchor = null
				this.connectionPreviewPoint = null
				if (startAnchor && dropAnchor && dropAnchor.component.id !== startAnchor.component.id) {
					const from = this.endpointFromAnchor(startAnchor)
					const to = this.endpointFromAnchor(dropAnchor)
					const created = this.options.createConnection?.(from, to) ?? { from, to }
					this.connections.push(cloneConnection(created))
					this.emitConnectionsChange()
				}
				this.hoveredEdge = this.findHoveredEdge(x, y)
				this.canvasElement.style.cursor = this.hoveredEdge ? "crosshair" : "default"
				this.drawScene()
				return
			}

			if (this.isDraggingGroup) {
				this.isDraggingGroup = false
				if (this.movedDuringDrag) {
					this.emitComponentsChange()
				}
			}

			if (this.isSelecting) {
				const x1 = Math.min(this.selectStartX, this.selectCurrentX)
				const y1 = Math.min(this.selectStartY, this.selectCurrentY)
				const x2 = Math.max(this.selectStartX, this.selectCurrentX)
				const y2 = Math.max(this.selectStartY, this.selectCurrentY)
				this.selectedIds = this.components
					.filter((component) => component.x < x2 && component.x + component.width > x1 && component.y < y2 && component.y + component.height > y1)
					.map((component) => component.id)
				this.isSelecting = false
				this.emitSelectionChange()
			}

			this.drawScene()
			if (!this.isConnecting) {
				this.canvasElement.style.cursor = this.hoveredEdge ? "crosshair" : "default"
			}
		})

		this.canvasElement.addEventListener("mouseleave", () => {
			this.isPanning = false
			if (this.isConnecting) {
				this.isConnecting = false
				this.connectionStartAnchor = null
				this.connectionPreviewPoint = null
			}
			const hadHover = this.hoveredEdge !== null
			this.hoveredEdge = null
			if (hadHover) {
				this.drawScene()
			}
			this.canvasElement.style.cursor = "default"
		})

		this.canvasElement.addEventListener("dragover", (event) => {
			event.preventDefault()
		})

		this.canvasElement.addEventListener("drop", (event) => {
			event.preventDefault()
			const type = event.dataTransfer?.getData("component")
			if (!type) {
				return
			}
			const position = this.toWorldPoint(event.clientX, event.clientY)
			const created = this.options.createComponent?.(type, position, {
				createId: () => this.createComponentId()
			})
			if (!created) {
				return
			}
			const component: CanvasComponent<TData> = {
				...created,
				id: created.id ?? this.createComponentId(),
				x: created.x ?? position.x,
				y: created.y ?? position.y
			}
			this.components.push(cloneComponent(component))
			this.nextComponentId = Math.max(this.nextComponentId, component.id + 1)
			this.drawScene()
			this.emitComponentsChange()
		})
	}

	private drawScene(): void {
		this.ctx.clearRect(0, 0, this.canvasElement.width, this.canvasElement.height)
		this.ctx.save()
		this.ctx.setTransform(this.scale, 0, 0, this.scale, this.originX, this.originY)

		this.drawGrid()
		this.drawSelectionRectangle()
		this.drawConnections()
		this.drawConnectionPreview()
		this.drawComponents()
		this.drawAnchors()

		this.ctx.restore()
	}

	private drawGrid(): void {
		const gridSpacing = this.getGridSpacing()
		const worldLeft = -this.originX / this.scale
		const worldRight = (this.canvasElement.width - this.originX) / this.scale
		const worldTop = -this.originY / this.scale
		const worldBottom = (this.canvasElement.height - this.originY) / this.scale

		this.ctx.strokeStyle = "#e2e8f0"
		this.ctx.lineWidth = 1
		for (let x = Math.floor(worldLeft / gridSpacing) * gridSpacing; x <= worldRight; x += gridSpacing) {
			this.ctx.beginPath()
			this.ctx.moveTo(x, worldTop)
			this.ctx.lineTo(x, worldBottom)
			this.ctx.stroke()
		}
		for (let y = Math.floor(worldTop / gridSpacing) * gridSpacing; y <= worldBottom; y += gridSpacing) {
			this.ctx.beginPath()
			this.ctx.moveTo(worldLeft, y)
			this.ctx.lineTo(worldRight, y)
			this.ctx.stroke()
		}
	}

	private drawSelectionRectangle(): void {
		if (!this.isSelecting) {
			return
		}
		this.ctx.save()
		this.ctx.strokeStyle = "#2563eb"
		this.ctx.lineWidth = 1
		this.ctx.setLineDash([5, 5])
		this.ctx.strokeRect(this.selectStartX, this.selectStartY, this.selectCurrentX - this.selectStartX, this.selectCurrentY - this.selectStartY)
		this.ctx.restore()
	}

	private drawConnections(): void {
		this.ctx.save()
		this.ctx.lineCap = "round"
		for (let index = 0; index < this.connections.length; index += 1) {
			const connection = this.connections[index]
			if (!connection) {
				continue
			}
			const fromPoint = this.pointFromEndpoint(connection.from)
			const toPoint = this.pointFromEndpoint(connection.to)
			if (!fromPoint || !toPoint) {
				continue
			}
			const isSelected = index === this.selectedConnectionIndex
			if (this.options.renderConnection) {
				this.ctx.save()
				this.options.renderConnection(this.ctx, connection, {
					selected: isSelected,
					from: fromPoint,
					to: toPoint
				})
				this.ctx.restore()
				continue
			}
			this.ctx.strokeStyle = isSelected ? "#2563eb" : "#475569"
			this.ctx.lineWidth = isSelected ? 4 : 2
			if (connection.style === "dashed") {
				this.ctx.setLineDash([10, 6])
			} else {
				this.ctx.setLineDash([])
			}
			this.ctx.beginPath()
			this.ctx.moveTo(fromPoint.x, fromPoint.y)
			this.ctx.lineTo(toPoint.x, toPoint.y)
			this.ctx.stroke()
			this.ctx.setLineDash([])
			if (isSelected) {
				this.ctx.lineWidth = 2
			}
		}
		this.ctx.restore()
	}

	private drawConnectionPreview(): void {
		if (!this.isConnecting || !this.connectionStartAnchor || !this.connectionPreviewPoint) {
			return
		}
		this.ctx.save()
		this.ctx.strokeStyle = "#94a3b8"
		this.ctx.lineWidth = 2
		this.ctx.setLineDash([4, 4])
		this.ctx.beginPath()
		this.ctx.moveTo(this.connectionStartAnchor.point.x, this.connectionStartAnchor.point.y)
		this.ctx.lineTo(this.connectionPreviewPoint.x, this.connectionPreviewPoint.y)
		this.ctx.stroke()
		this.ctx.restore()
	}

	private drawComponents(): void {
		for (const component of this.components) {
			const isSelected = this.selectedIds.includes(component.id)
			if (this.options.renderComponent) {
				this.options.renderComponent(this.ctx, component, { selected: isSelected })
				continue
			}
			this.drawDefaultComponent(component, isSelected)
		}
	}

	private drawAnchors(): void {
		if (this.connectionStartAnchor) {
			this.drawEdgeHighlight(this.connectionStartAnchor, "#1d4ed8")
			this.drawAnchorMarker(this.connectionStartAnchor.point, "#1d4ed8")
		}
		if (this.hoveredEdge) {
			const color = this.isConnecting ? "#15803d" : "#f97316"
			this.drawEdgeHighlight(this.hoveredEdge, color)
			this.drawAnchorMarker(this.hoveredEdge.point, color)
		}
	}

	private drawDefaultComponent(component: CanvasComponent<TData>, selected: boolean): void {
		this.ctx.save()
		this.ctx.fillStyle = "#ffffff"
		this.ctx.strokeStyle = selected ? "#2563eb" : "#1f2937"
		this.ctx.lineWidth = selected ? 3 : 2
		this.ctx.fillRect(component.x, component.y, component.width, component.height)
		this.ctx.strokeRect(component.x, component.y, component.width, component.height)

		const label = this.options.getComponentLabel?.(component) ?? `C${component.id}`
		this.ctx.fillStyle = "#0f172a"
		this.ctx.font = "16px Inter, Arial, sans-serif"
		this.ctx.textAlign = "center"
		this.ctx.textBaseline = "middle"
		this.ctx.fillText(label, component.x + component.width / 2, component.y + component.height / 2)
		this.ctx.restore()
	}

	private findHoveredEdge(x: number, y: number): EdgeAnchor<TData> | null {
		let closest: EdgeAnchor<TData> | null = null
		let closestDistance = Number.POSITIVE_INFINITY
		const tolerance = this.edgeHitPadding / this.scale
		for (const component of this.components) {
			const anchor = this.getEdgeAnchor(component, x, y, tolerance)
			if (!anchor) {
				continue
			}
			const distance = this.distanceToAnchor(anchor, x, y)
			if (distance < closestDistance) {
				closest = anchor
				closestDistance = distance
			}
		}
		return closest
	}

	private findNearestEdgeAnchor(x: number, y: number): EdgeAnchor<TData> | null {
		const insideComponent = this.components.find((component) => this.isPointInsideComponent(component, x, y))
		if (insideComponent) {
			return this.getNearestEdgeAnchor(insideComponent, x, y)
		}
		const tolerance = this.edgeHitPadding / this.scale
		let closest: EdgeAnchor<TData> | null = null
		let closestDistance = Number.POSITIVE_INFINITY
		for (const component of this.components) {
			const anchor = this.getEdgeAnchor(component, x, y, tolerance)
			if (!anchor) {
				continue
			}
			const distance = this.distanceBetweenPoints(anchor.point, { x, y })
			if (distance < closestDistance) {
				closest = anchor
				closestDistance = distance
			}
		}
		return closest
	}

	private findConnectionNearPoint(x: number, y: number): number | null {
		let closestIndex: number | null = null
		let closestDistance = Number.POSITIVE_INFINITY
		const tolerance = this.connectionHitPadding / this.scale

		for (let index = 0; index < this.connections.length; index += 1) {
			const connection = this.connections[index]
			if (!connection) {
				continue
			}
			const fromPoint = this.pointFromEndpoint(connection.from)
			const toPoint = this.pointFromEndpoint(connection.to)
			if (!fromPoint || !toPoint) {
				continue
			}
			const distance = this.distanceFromPointToSegment({ x, y }, fromPoint, toPoint)
			if (distance <= tolerance && distance < closestDistance) {
				closestDistance = distance
				closestIndex = index
			}
		}

		return closestIndex
	}

	private getEdgeAnchor(component: CanvasComponent<TData>, x: number, y: number, tolerance = this.edgeHitPadding / this.scale): EdgeAnchor<TData> | null {
		const expandedLeft = component.x - tolerance
		const expandedRight = component.x + component.width + tolerance
		const expandedTop = component.y - tolerance
		const expandedBottom = component.y + component.height + tolerance
		if (x < expandedLeft || x > expandedRight || y < expandedTop || y > expandedBottom) {
			return null
		}
		const { edge, point, distance } = this.computeAnchor(component, x, y)
		if (distance > tolerance) {
			return null
		}
		return { component, edge, point }
	}

	private getNearestEdgeAnchor(component: CanvasComponent<TData>, x: number, y: number): EdgeAnchor<TData> {
		const { edge, point } = this.computeAnchor(component, x, y)
		return { component, edge, point }
	}

	private computeAnchor(component: CanvasComponent<TData>, x: number, y: number): { edge: EdgeName; point: { x: number; y: number }; distance: number } {
		const distances = [
			{ edge: "left" as const, distance: Math.abs(x - component.x) },
			{ edge: "right" as const, distance: Math.abs(x - (component.x + component.width)) },
			{ edge: "top" as const, distance: Math.abs(y - component.y) },
			{ edge: "bottom" as const, distance: Math.abs(y - (component.y + component.height)) }
		]
		const closest = distances.reduce((previous, current) => (current.distance < previous.distance ? current : previous))
		return {
			edge: closest.edge,
			point: this.anchorForEdge(component, closest.edge, x, y),
			distance: closest.distance
		}
	}

	private anchorForEdge(component: CanvasComponent<TData>, edge: EdgeName, x: number, y: number): { x: number; y: number } {
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
	}

	private drawAnchorMarker(point: { x: number; y: number }, color: string): void {
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

	private drawEdgeHighlight(anchor: EdgeAnchor<TData>, color: string): void {
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

	private isNearComponentEdge(component: CanvasComponent<TData>, x: number, y: number): boolean {
		const { distance } = this.computeAnchor(component, x, y)
		const edgeThreshold = Math.min(component.width, component.height) * 0.25
		const maxDistance = Math.min(edgeThreshold, this.edgeHitPadding / this.scale)
		return distance <= maxDistance
	}

	private endpointFromAnchor(anchor: EdgeAnchor<TData>): ConnectionEndpoint {
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
		const component = this.components.find((candidate) => candidate.id === endpoint.componentId)
		if (!component) {
			return null
		}
		return this.anchorPointForRatio(component, endpoint.edge, endpoint.ratio)
	}

	private anchorPointForRatio(component: CanvasComponent<TData>, edge: EdgeName, ratio: number): { x: number; y: number } {
		const clamped = this.clamp(ratio, 0, 1)
		switch (edge) {
			case "left":
				return { x: component.x, y: component.y + component.height * clamped }
			case "right":
				return { x: component.x + component.width, y: component.y + component.height * clamped }
			case "top":
				return { x: component.x + component.width * clamped, y: component.y }
			case "bottom":
				return { x: component.x + component.width * clamped, y: component.y + component.height }
		}
	}

	private safeRatio(delta: number, size: number): number {
		return size === 0 ? 0 : delta / size
	}

	private distanceFromPointToSegment(point: { x: number; y: number }, start: { x: number; y: number }, end: { x: number; y: number }): number {
		const segmentLengthSquared = (end.x - start.x) ** 2 + (end.y - start.y) ** 2
		if (segmentLengthSquared === 0) {
			return this.distanceBetweenPoints(point, start)
		}

		let t = ((point.x - start.x) * (end.x - start.x) + (point.y - start.y) * (end.y - start.y)) / segmentLengthSquared
		t = this.clamp(t, 0, 1)

		const projection = {
			x: start.x + t * (end.x - start.x),
			y: start.y + t * (end.y - start.y)
		}

		return this.distanceBetweenPoints(point, projection)
	}

	private distanceBetweenPoints(a: { x: number; y: number }, b: { x: number; y: number }): number {
		return Math.hypot(a.x - b.x, a.y - b.y)
	}

	private distanceToAnchor(anchor: EdgeAnchor<TData>, x: number, y: number): number {
		switch (anchor.edge) {
			case "left":
			case "right":
				return Math.abs(x - anchor.point.x)
			case "top":
			case "bottom":
				return Math.abs(y - anchor.point.y)
		}
	}

	private isPointInsideComponent(component: CanvasComponent<TData>, x: number, y: number): boolean {
		return x >= component.x && x <= component.x + component.width && y >= component.y && y <= component.y + component.height
	}

	private clamp(value: number, min: number, max: number): number {
		return Math.max(min, Math.min(max, value))
	}

	private fitAllClicked(): void {
		if (this.components.length === 0) {
			return
		}
		const xs = this.components.map((component) => component.x)
		const ys = this.components.map((component) => component.y)
		const xsMax = this.components.map((component) => component.x + component.width)
		const ysMax = this.components.map((component) => component.y + component.height)
		const minX = Math.min(...xs)
		const maxX = Math.max(...xsMax)
		const minY = Math.min(...ys)
		const maxY = Math.max(...ysMax)
		const bboxWidth = maxX - minX || 1
		const bboxHeight = maxY - minY || 1
		const bboxCenterX = minX + bboxWidth / 2
		const bboxCenterY = minY + bboxHeight / 2
		const scaleX = this.canvasElement.width / bboxWidth
		const scaleY = this.canvasElement.height / bboxHeight
		this.scale = Math.min(scaleX, scaleY) * 0.9
		this.originX = this.canvasElement.width / 2 - bboxCenterX * this.scale
		this.originY = this.canvasElement.height / 2 - bboxCenterY * this.scale
		this.drawScene()
	}

	private createComponentId(): number {
		const id = this.nextComponentId
		this.nextComponentId += 1
		return id
	}

	private computeNextComponentId(): number {
		const maxId = this.components.reduce((max, component) => Math.max(max, component.id), 0)
		return maxId + 1
	}

	private emitSelectionChange(): void {
		this.options.onSelectionChange?.([...this.selectedIds])
	}

	private emitComponentsChange(): void {
		this.options.onComponentsChange?.(this.getComponents())
	}

	private emitConnectionsChange(): void {
		this.options.onConnectionsChange?.(this.getConnections())
	}
}
