export const createSchematicCanvas = (canvas: HTMLCanvasElement) => {
	// Create Fit All button
	const fitButton = document.createElement("button")
	fitButton.textContent = "Fit All"
	fitButton.style.position = "relative"
	fitButton.style.top = "10px"
	fitButton.style.left = "10px"
	document.body.appendChild(fitButton)
	fitButton.addEventListener("click", () => {
		// Fit view to show all components
		const xs = components.map(c => c.x)
		const ys = components.map(c => c.y)
		const xsMax = components.map(c => c.x + c.width)
		const ysMax = components.map(c => c.y + c.height)
		const minX = Math.min(...xs), maxX = Math.max(...xsMax)
		const minY = Math.min(...ys), maxY = Math.max(...ysMax)
		const bboxWidth = maxX - minX
		const bboxHeight = maxY - minY
		const bboxCenterX = minX + bboxWidth / 2
		const bboxCenterY = minY + bboxHeight / 2
		const scaleX = canvas.width / bboxWidth
		const scaleY = canvas.height / bboxHeight
		scale = Math.min(scaleX, scaleY) * 0.9
		originX = canvas.width / 2 - bboxCenterX * scale
		originY = canvas.height / 2 - bboxCenterY * scale
		drawScene()
	})

	const ctx = canvas.getContext("2d")!
	let scale = 1
	let originX = 0
	let originY = 0

	// Components and selection state
	interface Component { id: number; x: number; y: number; width: number; height: number }
	let components: Component[] = [
		{ id: 1, x: 100, y: 50, width: 80, height: 40 },
		// add more components here
	]
	let selectedIds: number[] = []

	// Group drag state
	let isDraggingGroup = false
	let groupDragStartX = 0
	let groupDragStartY = 0
	let originalPositions: { id: number; x: number; y: number }[] = []

	// Selection rectangle state
	let isSelecting = false
	let selectStartX = 0
	let selectStartY = 0
	let selectCurrentX = 0
	let selectCurrentY = 0

	function drawScene() {
		ctx.clearRect(0, 0, canvas.width, canvas.height)
		ctx.save()
		ctx.setTransform(scale, 0, 0, scale, originX, originY)

	    // Draw infinite grid
	    const gridSpacing = 100
	    const worldLeft = -originX / scale
	    const worldRight = (canvas.width - originX) / scale
	    const worldTop = -originY / scale
	    const worldBottom = (canvas.height - originY) / scale

	    ctx.strokeStyle = "#e0e0e0"
	    ctx.lineWidth = 1

	    // Vertical lines
	    for (let x = Math.floor(worldLeft / gridSpacing) * gridSpacing; x <= worldRight; x += gridSpacing) {
	        ctx.beginPath()
	        ctx.moveTo(x, worldTop)
	        ctx.lineTo(x, worldBottom)
	        ctx.stroke()
	    }
	    // Horizontal lines
	    for (let y = Math.floor(worldTop / gridSpacing) * gridSpacing; y <= worldBottom; y += gridSpacing) {
	        ctx.beginPath()
	        ctx.moveTo(worldLeft, y)
	        ctx.lineTo(worldRight, y)
	        ctx.stroke()
	    }

		// Draw selection rectangle
		if (isSelecting) {
			ctx.save()
			ctx.strokeStyle = "blue"
			ctx.lineWidth = 1
			ctx.setLineDash([5, 5])
			const sx = selectStartX
			const sy = selectStartY
			const ex = selectCurrentX
			const ey = selectCurrentY
			ctx.strokeRect(sx, sy, ex - sx, ey - sy)
			ctx.restore()
		}

		// Draw components
		components.forEach(comp => {
			ctx.fillStyle = "white"
			ctx.strokeStyle = "black"
			ctx.lineWidth = 2
			ctx.fillRect(comp.x, comp.y, comp.width, comp.height)
			ctx.strokeRect(comp.x, comp.y, comp.width, comp.height)
			if (selectedIds.includes(comp.id)) {
				ctx.strokeStyle = "blue"
				ctx.lineWidth = 2
				ctx.strokeRect(comp.x - 2, comp.y - 2, comp.width + 4, comp.height + 4)
			}
			ctx.font = "16px Arial"
			ctx.fillStyle = "black"
			ctx.fillText(`R${comp.id}`, comp.x + 10, comp.y + 25)
		})

		ctx.restore()
	}

	drawScene()

	canvas.addEventListener("wheel", (event) => {
		event.preventDefault()
		const rect = canvas.getBoundingClientRect()
		const mouseX = (event.clientX - rect.left - originX) / scale
		const mouseY = (event.clientY - rect.top - originY) / scale
		const zoomFactor = 1 - event.deltaY * 0.001
		originX -= mouseX * (zoomFactor - 1) * scale
		originY -= mouseY * (zoomFactor - 1) * scale
		scale *= zoomFactor
		drawScene()
	})
	
	let isPanning = false
	let panStartX = 0
	let panStartY = 0

	canvas.addEventListener("mousedown", (event) => {
		if (event.button === 1) {
			isPanning = true
			panStartX = event.clientX
			panStartY = event.clientY
		}
	})

	canvas.addEventListener("mousemove", (event) => {
		if (isPanning) {
			event.preventDefault()
			const dx = event.clientX - panStartX
			const dy = event.clientY - panStartY
			originX += dx
			originY += dy
			panStartX = event.clientX
			panStartY = event.clientY
			drawScene()
		}
	})

	canvas.addEventListener("mouseup", (event) => {
		if (event.button === 1) {
			isPanning = false
		}
	})

	// Multi-select and group dragging
	canvas.addEventListener("mousedown", (event) => {
		if (event.button === 0) {
			const rect = canvas.getBoundingClientRect()
			const wx = (event.clientX - rect.left - originX) / scale
			const wy = (event.clientY - rect.top - originY) / scale
			// Check if clicked on a component
			const clicked = components.find(c => wx >= c.x && wx <= c.x + c.width && wy >= c.y && wy <= c.y + c.height)
			if (clicked) {
				if (event.shiftKey) {
					const idx = selectedIds.indexOf(clicked.id)
					if (idx >= 0) selectedIds.splice(idx, 1)
					else selectedIds.push(clicked.id)
				} else {
					if (!selectedIds.includes(clicked.id)) selectedIds = [clicked.id]
				}
				// Start group drag
				isDraggingGroup = true
				groupDragStartX = wx
				groupDragStartY = wy
				originalPositions = selectedIds.map(id => {
					const comp = components.find(c => c.id === id)!
					return { id, x: comp.x, y: comp.y }
				})
			} else {
				// Start selection rectangle
				selectedIds = []
				isSelecting = true
				selectStartX = wx; selectStartY = wy
				selectCurrentX = wx; selectCurrentY = wy
			}
			drawScene()
		}
	})
	canvas.addEventListener("mousemove", (event) => {
		const rect = canvas.getBoundingClientRect()
		const wx = (event.clientX - rect.left - originX) / scale
		const wy = (event.clientY - rect.top - originY) / scale
		if (isDraggingGroup) {
			const dx = wx - groupDragStartX
			const dy = wy - groupDragStartY
			originalPositions.forEach(op => {
				const comp = components.find(c => c.id === op.id)!
				comp.x = op.x + dx
				comp.y = op.y + dy
			})
			drawScene()
		} else if (isSelecting) {
			selectCurrentX = wx; selectCurrentY = wy
			drawScene()
		}
	})
	canvas.addEventListener("mouseup", (event) => {
		if (event.button === 0) {
			if (isDraggingGroup) isDraggingGroup = false
			if (isSelecting) {
				const x1 = Math.min(selectStartX, selectCurrentX)
				const y1 = Math.min(selectStartY, selectCurrentY)
				const x2 = Math.max(selectStartX, selectCurrentX)
				const y2 = Math.max(selectStartY, selectCurrentY)
				selectedIds = components
					.filter(c => c.x < x2 && c.x + c.width > x1 && c.y < y2 && c.y + c.height > y1)
					.map(c => c.id)
				isSelecting = false
			}
			drawScene()
		}
	})

	canvas.addEventListener("mouseleave", () => {
		isPanning = false
	})

	// Allow dropping new components from sidebar
	canvas.addEventListener("dragover", event => {
		event.preventDefault()
	})
	canvas.addEventListener("drop", event => {
		event.preventDefault()
		const type = event.dataTransfer.getData("component")
		const rect = canvas.getBoundingClientRect()
		const wx = (event.clientX - rect.left - originX) / scale
		const wy = (event.clientY - rect.top - originY) / scale
		// Create new component at drop position
		const newId = Math.max(0, ...components.map(c => c.id)) + 1
		let newComp
		if (type === "resistor") {
			newComp = { id: newId, x: wx, y: wy, width: 80, height: 40 }
		}
		if (newComp) {
			components.push(newComp)
			drawScene()
		}
	})
}