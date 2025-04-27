import { HList, UiComponent, VList } from "./ui"

interface Component { id: number; x: number; y: number; width: number; height: number }

export class EditorCanvas extends UiComponent<HTMLDivElement> {
	private canvas: HTMLCanvasElement
	private ctx!: CanvasRenderingContext2D
	private scale = 1
	private originX = 0
	private originY = 0
	private components: Component[] = [
		{ id: 1, x: 100, y: 50, width: 80, height: 40 },
		// add more components here
	]
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
		this.ctx = this.canvas.getContext("2d")!

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
			}
		})

		// Multi-select and group dragging
		this.canvas.addEventListener("mousedown", (event) => {
			if (event.button === 0) {
				const rect = this.canvas.getBoundingClientRect()
				const wx = (event.clientX - rect.left - this.originX) / this.scale
				const wy = (event.clientY - rect.top - this.originY) / this.scale
				// Check if clicked on a component
				const clicked = this.components.find(c => wx >= c.x && wx <= c.x + c.width && wy >= c.y && wy <= c.y + c.height)
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
					this.originalPositions = this.selectedIds.map(id => {
						const comp = this.components.find(c => c.id === id)!
						return { id, x: comp.x, y: comp.y }
					})
				} else {
					// Start selection rectangle
					this.selectedIds = []
					this.isSelecting = true
					this.selectStartX = wx; this.selectStartY = wy
					this.selectCurrentX = wx; this.selectCurrentY = wy
				}
				this.drawScene()
			}
		})
		this.canvas.addEventListener("mousemove", (event) => {
			const rect = this.canvas.getBoundingClientRect()
			const wx = (event.clientX - rect.left - this.originX) / this.scale
			const wy = (event.clientY - rect.top - this.originY) / this.scale
			if (this.isDraggingGroup) {
				const dx = wx - this.groupDragStartX
				const dy = wy - this.groupDragStartY
				this.originalPositions.forEach(op => {
					const comp = this.components.find(c => c.id === op.id)!
					comp.x = op.x + dx
					comp.y = op.y + dy
				})
				this.drawScene()
			} else if (this.isSelecting) {
				this.selectCurrentX = wx; this.selectCurrentY = wy
				this.drawScene()
			}
		})
		this.canvas.addEventListener("mouseup", (event) => {
			if (event.button === 0) {
				if (this.isDraggingGroup) this.isDraggingGroup = false
				if (this.isSelecting) {
					const x1 = Math.min(this.selectStartX, this.selectCurrentX)
					const y1 = Math.min(this.selectStartY, this.selectCurrentY)
					const x2 = Math.max(this.selectStartX, this.selectCurrentX)
					const y2 = Math.max(this.selectStartY, this.selectCurrentY)
					this.selectedIds = this.components
						.filter(c => c.x < x2 && c.x + c.width > x1 && c.y < y2 && c.y + c.height > y1)
						.map(c => c.id)
					this.isSelecting = false
				}
				this.drawScene()
			}
		})

		this.canvas.addEventListener("mouseleave", () => {
			isPanning = false
		})

		// Allow dropping new components from sidebar
		this.canvas.addEventListener("dragover", event => {
			event.preventDefault()
		})
		this.canvas.addEventListener("drop", event => {
			event.preventDefault();
			const type = event.dataTransfer!.getData("component");
			const rect = this.canvas.getBoundingClientRect();
			const wx = (event.clientX - rect.left - this.originX) / this.scale;
			const wy = (event.clientY - rect.top - this.originY) / this.scale;
			this.addComponent(type, wx, wy);
		});
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

		// Draw components
		this.components.forEach(comp => {
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

		this.ctx.restore()
	}

	private fitAllClicked() {
		// Fit view to show all components
		const xs = this.components.map(c => c.x)
		const ys = this.components.map(c => c.y)
		const xsMax = this.components.map(c => c.x + c.width)
		const ysMax = this.components.map(c => c.y + c.height)
		const minX = Math.min(...xs), maxX = Math.max(...xsMax)
		const minY = Math.min(...ys), maxY = Math.max(...ysMax)
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
		const newId = Math.max(0, ...this.components.map(c => c.id)) + 1;
		let size = { width: 80, height: 40 };
		switch (type) {
			case "resistor":
				size = { width: 80, height: 40 };
				break;
			case "capacitor":
				size = { width: 60, height: 60 };
				break;
			// add more cases as needed
			default:
				console.warn(`Unknown component type: ${type}`);
				return;
		}
		const newComp: Component = {
			id: newId,
			x,
			y,
			width: size.width,
			height: size.height
		};
		this.components.push(newComp);
		this.drawScene();
	}
}
