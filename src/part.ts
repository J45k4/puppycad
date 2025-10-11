import { UiComponent } from "./ui"

type Point2D = { x: number; y: number }

type Point3D = { x: number; y: number; z: number }

type ExtrudedModel = {
	base: Point2D[]
	height: number
	scale: number
	rawHeight: number
}

const SKETCH_CANVAS_SIZE = 360
const LIGHT_DIRECTION: Point3D = normalizeVector({ x: 0.4, y: 0.9, z: 0.6 })

export class PartEditor extends UiComponent<HTMLDivElement> {
	private readonly sketchCanvas: HTMLCanvasElement
	private readonly sketchCtx: CanvasRenderingContext2D
	private readonly previewCanvas: HTMLCanvasElement
	private readonly previewCtx: CanvasRenderingContext2D
	private readonly heightInput: HTMLInputElement
	private readonly statusText: HTMLParagraphElement
	private readonly extrudeSummary: HTMLParagraphElement
	private readonly finishButton: HTMLButtonElement
	private readonly undoButton: HTMLButtonElement
	private readonly resetButton: HTMLButtonElement
	private readonly extrudeButton: HTMLButtonElement
	private readonly previewContainer: HTMLDivElement
	private sketchPoints: Point2D[] = []
	private isSketchClosed = false
	private extrudedModel: ExtrudedModel | null = null
	private readonly previewRotation = {
		yaw: Math.PI / 4,
		pitch: Math.PI / 5
	}
	private isRotatingPreview = false
	private lastRotationPointer: { x: number; y: number } | null = null
	private previewScale = 180
	private resizeObserver: ResizeObserver | null = null

	public constructor() {
		super(document.createElement("div"))
		this.root.style.width = "100%"
		this.root.style.height = "100%"
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
		body.style.flex = "1"
		body.style.minHeight = "0"
		body.style.gap = "16px"
		body.style.padding = "16px"
		body.style.boxSizing = "border-box"
		this.root.appendChild(body)

		const sketchPanel = document.createElement("div")
		sketchPanel.style.width = "320px"
		sketchPanel.style.flexShrink = "0"
		sketchPanel.style.display = "flex"
		sketchPanel.style.flexDirection = "column"
		sketchPanel.style.gap = "12px"
		body.appendChild(sketchPanel)

		const sketchHeader = document.createElement("div")
		sketchHeader.innerHTML =
			'<h3 style="margin:0;font-size:16px;">Sketch</h3><p style="margin:4px 0 0;color:#475569;font-size:13px;">Click inside the sketch area to create points. Add at least three points, then finish the sketch to extrude it.</p>'
		sketchPanel.appendChild(sketchHeader)

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
		this.sketchCanvas.style.width = `${SKETCH_CANVAS_SIZE}px`
		this.sketchCanvas.style.height = `${SKETCH_CANVAS_SIZE}px`
		sketchPanel.appendChild(this.sketchCanvas)

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
		sketchPanel.appendChild(controlsRow)

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
		sketchPanel.appendChild(extrudeControls)

		const heightLabel = document.createElement("label")
		heightLabel.textContent = "Extrude height"
		heightLabel.style.fontSize = "13px"
		heightLabel.style.color = "#0f172a"
		extrudeControls.appendChild(heightLabel)

		this.heightInput = document.createElement("input")
		this.heightInput.type = "number"
		this.heightInput.min = "1"
		this.heightInput.value = "30"
		this.heightInput.step = "1"
		this.heightInput.style.width = "80px"
		this.heightInput.style.padding = "4px 6px"
		this.heightInput.style.border = "1px solid #cbd5f5"
		this.heightInput.style.borderRadius = "4px"
		extrudeControls.appendChild(this.heightInput)

		this.extrudeButton = this.createButton("Extrude", this.handleExtrude)
		this.extrudeButton.style.gridColumn = "span 2"
		sketchPanel.appendChild(this.extrudeButton)

		this.statusText = document.createElement("p")
		this.statusText.style.margin = "4px 0 0"
		this.statusText.style.fontSize = "13px"
		this.statusText.style.color = "#475569"
		sketchPanel.appendChild(this.statusText)

		this.extrudeSummary = document.createElement("p")
		this.extrudeSummary.style.margin = "0"
		this.extrudeSummary.style.fontSize = "13px"
		this.extrudeSummary.style.color = "#0f172a"
		sketchPanel.appendChild(this.extrudeSummary)

		this.previewContainer = document.createElement("div")
		this.previewContainer.style.flex = "1"
		this.previewContainer.style.backgroundColor = "#1f2937"
		this.previewContainer.style.borderRadius = "12px"
		this.previewContainer.style.position = "relative"
		this.previewContainer.style.display = "flex"
		this.previewContainer.style.alignItems = "center"
		this.previewContainer.style.justifyContent = "center"
		this.previewContainer.style.boxShadow = "inset 0 0 0 1px rgba(255,255,255,0.05)"
		body.appendChild(this.previewContainer)

		this.previewCanvas = document.createElement("canvas")
		this.previewCanvas.style.width = "100%"
		this.previewCanvas.style.height = "100%"
		this.previewCanvas.style.cursor = "grab"
		this.previewContainer.appendChild(this.previewCanvas)

		const previewCtx = this.previewCanvas.getContext("2d")
		if (!previewCtx) {
			throw new Error("Failed to initialize preview canvas context")
		}
		this.previewCtx = previewCtx

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

		this.updateStatus()
		this.updateControls()
		this.drawSketch()
		this.drawPreview()

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

	private handleSketchCanvasClick = (event: MouseEvent) => {
		if (this.isSketchClosed) {
			return
		}
		const rect = this.sketchCanvas.getBoundingClientRect()
		const x = event.clientX - rect.left
		const y = event.clientY - rect.top
		this.sketchPoints = [...this.sketchPoints, { x, y }]
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
	}

	private handleSketchHover = (event: MouseEvent) => {
		const rect = this.sketchCanvas.getBoundingClientRect()
		const x = event.clientX - rect.left
		const y = event.clientY - rect.top
		this.drawSketch({ x, y, active: event.type === "mousemove" })
	}

	private handleUndo = () => {
		if (!this.sketchPoints.length || this.isSketchClosed) {
			return
		}
		this.sketchPoints = this.sketchPoints.slice(0, -1)
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
	}

	private handleReset = () => {
		this.sketchPoints = []
		this.isSketchClosed = false
		this.extrudedModel = null
		this.drawSketch()
		this.drawPreview()
		this.updateStatus()
		this.updateControls()
	}

	private handleFinishSketch = () => {
		if (this.sketchPoints.length < 3) {
			return
		}
		this.isSketchClosed = true
		this.drawSketch()
		this.updateStatus()
		this.updateControls()
	}

	private handleExtrude = () => {
		if (!this.isSketchClosed || this.sketchPoints.length < 3) {
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
		this.drawPreview()
		this.updateStatus()
		this.updateControls()
	}

	private handlePreviewPointerDown = (event: PointerEvent) => {
		this.isRotatingPreview = true
		this.lastRotationPointer = { x: event.clientX, y: event.clientY }
		this.previewCanvas.setPointerCapture(event.pointerId)
		this.previewCanvas.style.cursor = "grabbing"
	}

	private handlePreviewPointerMove = (event: PointerEvent) => {
		if (!this.isRotatingPreview || !this.lastRotationPointer) {
			return
		}
		const dx = event.clientX - this.lastRotationPointer.x
		const dy = event.clientY - this.lastRotationPointer.y
		this.lastRotationPointer = { x: event.clientX, y: event.clientY }
		this.previewRotation.yaw += dx * 0.01
		this.previewRotation.pitch += dy * 0.01
		const limit = Math.PI / 2 - 0.1
		this.previewRotation.pitch = Math.min(limit, Math.max(-limit, this.previewRotation.pitch))
		this.drawPreview()
	}

	private handlePreviewPointerUp = (event: PointerEvent) => {
		if (!this.isRotatingPreview) {
			return
		}
		if (this.previewCanvas.hasPointerCapture(event.pointerId)) {
			this.previewCanvas.releasePointerCapture(event.pointerId)
		}
		this.isRotatingPreview = false
		this.lastRotationPointer = null
		this.previewCanvas.style.cursor = "grab"
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
		this.finishButton.disabled = this.isSketchClosed || this.sketchPoints.length < 3
		this.undoButton.disabled = this.isSketchClosed || this.sketchPoints.length === 0
		this.resetButton.disabled = this.sketchPoints.length === 0 && !this.extrudedModel
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
			const first = this.sketchPoints[0]
			if (!first) {
				return
			}
			this.sketchCtx.lineWidth = 2
			this.sketchCtx.strokeStyle = "#2563eb"
			this.sketchCtx.beginPath()
			this.sketchCtx.moveTo(first.x, first.y)
			for (const point of this.sketchPoints.slice(1)) {
				this.sketchCtx.lineTo(point.x, point.y)
			}
			if (this.isSketchClosed) {
				this.sketchCtx.closePath()
			}
			this.sketchCtx.stroke()

			if (this.isSketchClosed && this.sketchPoints.length >= 3) {
				this.sketchCtx.fillStyle = "rgba(59,130,246,0.2)"
				this.sketchCtx.fill()
			}

			for (const point of this.sketchPoints) {
				this.drawSketchPoint(point, "#1d4ed8")
			}
		}

		if (hover?.active && !this.isSketchClosed && this.sketchPoints.length > 0) {
			const last = this.sketchPoints[this.sketchPoints.length - 1]
			if (last) {
				this.sketchCtx.setLineDash([4, 4])
				this.sketchCtx.strokeStyle = "rgba(15,23,42,0.4)"
				this.sketchCtx.beginPath()
				this.sketchCtx.moveTo(last.x, last.y)
				this.sketchCtx.lineTo(hover.x, hover.y)
				this.sketchCtx.stroke()
				this.sketchCtx.setLineDash([])
			}
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
		if (this.sketchPoints.length < 3) {
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
		const dpr = Number.isFinite(window.devicePixelRatio) && window.devicePixelRatio > 0 ? window.devicePixelRatio : 1
		const targetW = Math.max(1, Math.round(width * dpr))
		const targetH = Math.max(1, Math.round(height * dpr))
		if (this.previewCanvas.width !== targetW || this.previewCanvas.height !== targetH) {
			this.previewCanvas.width = targetW
			this.previewCanvas.height = targetH
		}
		this.previewCtx.setTransform(1, 0, 0, 1, 0, 0)
		this.previewCtx.scale(dpr, dpr)
		this.previewScale = Math.min(width, height) * 0.35
		this.drawPreview()
	}

	private drawPreview() {
		const rect = this.previewCanvas.getBoundingClientRect()
		this.previewCtx.clearRect(0, 0, rect.width, rect.height)

		if (!this.extrudedModel) {
			this.drawPreviewPlaceholder(rect)
			return
		}

		const { base, height } = this.extrudedModel
		if (base.length < 3) {
			this.drawPreviewPlaceholder(rect)
			return
		}
		const bottom = base.map((point) => ({ x: point.x, y: point.y, z: -height / 2 }))
		const top = base.map((point) => ({ x: point.x, y: point.y, z: height / 2 }))

		const faces: { vertices: Point3D[]; type: "side" | "top" | "bottom" }[] = []
		for (let i = 0; i < base.length; i += 1) {
			const next = (i + 1) % base.length
			const bottomCurrent = bottom[i]
			const bottomNext = bottom[next]
			const topNext = top[next]
			const topCurrent = top[i]
			if (!bottomCurrent || !bottomNext || !topNext || !topCurrent) {
				continue
			}
			faces.push({
				vertices: [bottomCurrent, bottomNext, topNext, topCurrent],
				type: "side"
			})
		}
		faces.push({ vertices: top.slice().reverse(), type: "top" })
		faces.push({ vertices: bottom, type: "bottom" })

		const rotatedFaces = faces
			.map((face) => {
				const rotated = face.vertices.map((vertex) => this.rotateVertex(vertex))
				const projected = rotated.map((vertex) => this.projectVertex(vertex, rect))
				const averageZ = rotated.reduce((sum, vertex) => sum + vertex.z, 0) / rotated.length
				const normal = computeFaceNormal(rotated)
				return { ...face, rotated, projected, averageZ, normal }
			})
			.sort((a, b) => a.averageZ - b.averageZ)

		for (const face of rotatedFaces) {
			if (!face.projected.length) {
				continue
			}
			let intensity = 0.6
			if (face.normal) {
				const dot = dotProduct(normalizeVector(face.normal), LIGHT_DIRECTION)
				const clamped = Math.max(0.2, Math.min(1, dot))
				intensity = Number.isFinite(clamped) ? clamped : 0.6
			}
			const baseColor: [number, number, number] = face.type === "top" ? [96, 165, 250] : [30, 64, 175]
			const fillColor = `rgba(${Math.round(baseColor[0] * intensity)}, ${Math.round(
				baseColor[1] * intensity
			)}, ${Math.round(baseColor[2] * intensity)}, ${face.type === "bottom" ? 0.4 : 0.9})`
			const firstPoint = face.projected[0]
			if (!firstPoint) {
				continue
			}
			this.previewCtx.beginPath()
			this.previewCtx.moveTo(firstPoint.x, firstPoint.y)
			for (let i = 1; i < face.projected.length; i += 1) {
				const projectedPoint = face.projected[i]
				if (!projectedPoint) {
					continue
				}
				this.previewCtx.lineTo(projectedPoint.x, projectedPoint.y)
			}
			this.previewCtx.closePath()
			this.previewCtx.fillStyle = fillColor
			this.previewCtx.fill()
			this.previewCtx.strokeStyle = "rgba(15,23,42,0.6)"
			this.previewCtx.lineWidth = 1.5
			this.previewCtx.stroke()
		}

		this.previewCtx.strokeStyle = "rgba(241,245,249,0.8)"
		this.previewCtx.lineWidth = 1
		for (const point of [...top, ...bottom]) {
			const rotated = this.rotateVertex(point)
			const projected = this.projectVertex(rotated, rect)
			this.previewCtx.beginPath()
			this.previewCtx.arc(projected.x, projected.y, 2.5, 0, Math.PI * 2)
			this.previewCtx.stroke()
		}
	}

	private drawPreviewPlaceholder(rect: DOMRect) {
		const baseSquare: Point3D[] = [
			{ x: -0.6, y: -0.4, z: -0.6 },
			{ x: 0.6, y: -0.4, z: -0.6 },
			{ x: 0.6, y: -0.4, z: 0.6 },
			{ x: -0.6, y: -0.4, z: 0.6 }
		]
		const topSquare = baseSquare.map((point) => ({ ...point, y: point.y + 0.8 }))
		const faces: Point3D[][] = []
		for (let i = 0; i < baseSquare.length; i += 1) {
			const next = (i + 1) % baseSquare.length
			const baseCurrent = baseSquare[i]
			const baseNext = baseSquare[next]
			const topNext = topSquare[next]
			const topCurrent = topSquare[i]
			if (!baseCurrent || !baseNext || !topNext || !topCurrent) {
				continue
			}
			faces.push([baseCurrent, baseNext, topNext, topCurrent])
		}
		faces.push(topSquare.slice().reverse())
		faces.push(baseSquare)

		for (const face of faces) {
			const rotated = face.map((vertex) => this.rotateVertex(vertex))
			const projected = rotated.map((vertex) => this.projectVertex(vertex, rect))
			const first = projected[0]
			if (!first) {
				continue
			}
			this.previewCtx.beginPath()
			this.previewCtx.moveTo(first.x, first.y)
			for (let i = 1; i < projected.length; i += 1) {
				const point = projected[i]
				if (!point) {
					continue
				}
				this.previewCtx.lineTo(point.x, point.y)
			}
			this.previewCtx.closePath()
			this.previewCtx.fillStyle = "rgba(255,255,255,0.1)"
			this.previewCtx.fill()
			this.previewCtx.strokeStyle = "rgba(255,255,255,0.25)"
			this.previewCtx.lineWidth = 1.5
			this.previewCtx.stroke()
		}

		this.previewCtx.fillStyle = "rgba(255,255,255,0.75)"
		this.previewCtx.font = "14px Inter, system-ui, sans-serif"
		this.previewCtx.textAlign = "center"
		this.previewCtx.fillText("Sketch a profile and extrude to see it here", rect.width / 2, rect.height / 2 + Math.min(rect.width, rect.height) * 0.3)
	}

	private rotateVertex(vertex: Point3D): Point3D {
		const cosYaw = Math.cos(this.previewRotation.yaw)
		const sinYaw = Math.sin(this.previewRotation.yaw)
		const cosPitch = Math.cos(this.previewRotation.pitch)
		const sinPitch = Math.sin(this.previewRotation.pitch)

		const xzRotatedX = vertex.x * cosYaw - vertex.z * sinYaw
		const xzRotatedZ = vertex.x * sinYaw + vertex.z * cosYaw

		const yRotatedY = vertex.y * cosPitch - xzRotatedZ * sinPitch
		const yRotatedZ = vertex.y * sinPitch + xzRotatedZ * cosPitch

		return {
			x: xzRotatedX,
			y: yRotatedY,
			z: yRotatedZ
		}
	}

	private projectVertex(vertex: Point3D, rect: DOMRect): Point2D {
		const distance = 3
		const perspective = distance / Math.max(0.01, distance - vertex.z)
		const x = rect.width / 2 + vertex.x * this.previewScale * perspective
		const y = rect.height / 2 - vertex.y * this.previewScale * perspective
		return { x, y }
	}
}

function normalizeVector(vector: Point3D): Point3D {
	const length = Math.hypot(vector.x, vector.y, vector.z) || 1
	return { x: vector.x / length, y: vector.y / length, z: vector.z / length }
}

function dotProduct(a: Point3D, b: Point3D): number {
	return a.x * b.x + a.y * b.y + a.z * b.z
}

function computeFaceNormal(vertices: Point3D[]): Point3D | null {
	if (vertices.length < 3) {
		return null
	}
	const v0 = vertices[0]
	const v1 = vertices[1]
	const v2 = vertices[vertices.length - 1]
	if (!v0 || !v1 || !v2) {
		return null
	}
	const edge1 = {
		x: v1.x - v0.x,
		y: v1.y - v0.y,
		z: v1.z - v0.z
	}
	const edge2 = {
		x: v2.x - v0.x,
		y: v2.y - v0.y,
		z: v2.z - v0.z
	}
	const normal = {
		x: edge1.y * edge2.z - edge1.z * edge2.y,
		y: edge1.z * edge2.x - edge1.x * edge2.z,
		z: edge1.x * edge2.y - edge1.y * edge2.x
	}
	return normal
}
