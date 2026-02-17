import { UiComponent } from "./ui"

export type DockOrientation = "horizontal" | "vertical"

export type DockLayoutLeafState = {
	type: "leaf"
	paneId: string
}

export type DockLayoutSplitState = {
	type: "split"
	orientation: DockOrientation
	children: DockLayoutNodeState[]
}

export type DockLayoutNodeState = DockLayoutLeafState | DockLayoutSplitState

export type DockLayoutState = {
	root: DockLayoutNodeState
	activePaneId: string | null
}

export type DockDropPosition = "left" | "right" | "top" | "bottom" | "center"

type DockSplit = {
	type: "split"
	orientation: DockOrientation
	element: HTMLDivElement
	children: DockNode[]
}

type DockPaneState = {
	id: string
	element: HTMLDivElement
	header: HTMLDivElement
	title: HTMLSpanElement
	content: HTMLDivElement
	placeholder: HTMLDivElement
	externalDropIndicator: HTMLDivElement
	currentComponent: UiComponent<HTMLElement> | null
	closeButton: HTMLButtonElement
}

type DockLeaf = {
	type: "leaf"
	pane: DockPaneState
}

type DockNode = DockSplit | DockLeaf

let paneIdCounter = 0

type DragState = {
	source: DockLeaf
	overlay: HTMLDivElement
	ghost: HTMLDivElement
	overlayDragHandler: (event: DragEvent) => void
}

type PointerMoveState = {
	source: DockLeaf
	overlay: HTMLDivElement
	ghost: HTMLDivElement
	hoveredZone: HTMLDivElement | null
	moveHandler: (event: MouseEvent) => void
	upHandler: (event: MouseEvent) => void
}

export class DockLayout extends UiComponent<HTMLDivElement> {
	private rootNode: DockNode
	private readonly parentMap = new Map<DockNode, DockSplit | null>()
	private readonly panes = new Map<string, DockLeaf>()
	private activePaneId: string | null = null
	public onActivePaneChange: ((paneId: string) => void) | null = null
	public onPaneClosed: ((closedPaneId: string, nextActivePaneId: string | null) => void) | null = null
	public canAcceptExternalDrop: ((event: DragEvent) => boolean) | null = null
	public onExternalDrop: ((args: { paneId: string; position: DockDropPosition; event: DragEvent }) => void) | null = null
	private dragState: DragState | null = null
	private pointerMoveState: PointerMoveState | null = null
	private pendingPointerMoveCancel: (() => void) | null = null

	public constructor() {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.flexGrow = "1"
		this.root.style.minHeight = "0"
		this.root.style.minWidth = "0"
		this.root.style.position = "relative"
		this.root.addEventListener("dragleave", (event) => {
			const related = event.relatedTarget as Node | null
			if (!related || !this.root.contains(related)) {
				this.hideAllExternalDropIndicators()
			}
		})
		this.root.addEventListener("drop", () => {
			this.hideAllExternalDropIndicators()
		})

		const initialLeaf = this.createLeaf()
		this.rootNode = initialLeaf
		this.parentMap.set(initialLeaf, null)
		this.root.appendChild(initialLeaf.pane.element)
		this.setActivePane(initialLeaf.pane.id)
	}

	public getActivePaneId(): string | null {
		return this.activePaneId
	}

	public setActivePane(paneId: string | null): void {
		if (paneId === this.activePaneId) {
			return
		}

		if (this.activePaneId) {
			const previousLeaf = this.panes.get(this.activePaneId)
			if (previousLeaf) {
				this.setPaneActive(previousLeaf.pane, false)
			}
		}

		this.activePaneId = paneId

		if (!paneId) {
			return
		}

		const leaf = this.panes.get(paneId)
		if (!leaf) {
			return
		}

		this.setPaneActive(leaf.pane, true)
		if (this.onActivePaneChange) {
			this.onActivePaneChange(paneId)
		}
	}

	public splitPane(paneId: string, orientation: DockOrientation): string | null {
		const leaf = this.panes.get(paneId)
		if (!leaf) {
			return null
		}

		const parent = this.parentMap.get(leaf) ?? null
		const newLeaf = this.createLeaf()

		if (parent && parent.orientation === orientation) {
			const index = parent.children.indexOf(leaf)
			parent.children.splice(index + 1, 0, newLeaf)
			parent.element.insertBefore(newLeaf.pane.element, parent.element.children[index + 1] ?? null)
			this.parentMap.set(newLeaf, parent)
			this.setActivePane(newLeaf.pane.id)
			this.updatePaneCloseVisibility()
			return newLeaf.pane.id
		}

		const splitNode: DockSplit = {
			type: "split",
			orientation,
			element: this.createSplitElement(orientation),
			children: []
		}

		splitNode.children.push(leaf, newLeaf)

		this.parentMap.set(splitNode, parent)
		this.parentMap.set(leaf, splitNode)
		this.parentMap.set(newLeaf, splitNode)

		if (!parent) {
			this.rootNode = splitNode
			this.root.innerHTML = ""
			this.root.appendChild(splitNode.element)
		} else {
			const parentIndex = parent.children.indexOf(leaf)
			parent.children.splice(parentIndex, 1, splitNode)
			parent.element.replaceChild(splitNode.element, leaf.pane.element)
		}

		splitNode.element.appendChild(leaf.pane.element)
		splitNode.element.appendChild(newLeaf.pane.element)

		this.setActivePane(newLeaf.pane.id)
		return newLeaf.pane.id
	}

	public getPaneIds(): string[] {
		return Array.from(this.panes.keys())
	}

	public getState(): DockLayoutState {
		return {
			root: this.serializeNode(this.rootNode),
			activePaneId: this.activePaneId
		}
	}

	public restoreState(state: DockLayoutState): void {
		this.root.innerHTML = ""
		this.parentMap.clear()
		this.panes.clear()

		const rebuiltRoot = this.restoreNode(state.root, null)
		this.rootNode = rebuiltRoot

		if (rebuiltRoot.type === "leaf") {
			this.root.appendChild(rebuiltRoot.pane.element)
		} else {
			this.root.appendChild(rebuiltRoot.element)
		}

		this.activePaneId = null
		const desiredActivePane = state.activePaneId && this.panes.has(state.activePaneId) ? state.activePaneId : (this.getPaneIds()[0] ?? null)

		this.setActivePane(desiredActivePane)

		const maxPaneNumber = this.getMaxPaneNumber(state.root)
		if (maxPaneNumber >= 0) {
			paneIdCounter = Math.max(paneIdCounter, maxPaneNumber)
		}

		this.updatePaneCloseVisibility()
	}

	public setPaneContent(paneId: string, component: UiComponent<HTMLElement>): void {
		const leaf = this.panes.get(paneId)
		if (!leaf) {
			return
		}

		const pane = leaf.pane
		this.hideExternalDropIndicator(pane)
		pane.content.innerHTML = ""
		pane.content.appendChild(component.root)
		pane.content.appendChild(pane.externalDropIndicator)
		pane.placeholder.style.display = "none"
		pane.currentComponent = component
	}

	public clearPane(paneId: string): void {
		const leaf = this.panes.get(paneId)
		if (!leaf) {
			return
		}

		const pane = leaf.pane
		this.hideExternalDropIndicator(pane)
		pane.content.innerHTML = ""
		pane.content.appendChild(pane.placeholder)
		pane.content.appendChild(pane.externalDropIndicator)
		pane.placeholder.style.display = "flex"
		pane.currentComponent = null
	}

	public closePane(paneId: string): string | null {
		const leaf = this.panes.get(paneId)
		if (!leaf) {
			return this.activePaneId
		}

		if (this.panes.size <= 1) {
			this.clearPane(paneId)
			return paneId
		}

		const wasActive = this.activePaneId === paneId
		const fallbackLeaf = this.findFallbackLeaf(paneId)

		this.removeLeaf(leaf)

		this.panes.delete(paneId)
		this.parentMap.delete(leaf)
		leaf.pane.currentComponent = null

		let nextActivePaneId = this.activePaneId
		if (wasActive) {
			const candidate = fallbackLeaf ?? null
			nextActivePaneId = candidate?.pane.id ?? this.getPaneIds()[0] ?? null
			this.setActivePane(nextActivePaneId)
		}

		this.updatePaneCloseVisibility()

		if (this.onPaneClosed) {
			this.onPaneClosed(paneId, this.activePaneId)
		}

		return this.activePaneId
	}

	public setPaneTitle(paneId: string, title: string): void {
		const leaf = this.panes.get(paneId)
		if (!leaf) {
			return
		}

		leaf.pane.title.textContent = title
	}

	public setPanePlaceholder(paneId: string, text: string): void {
		const leaf = this.panes.get(paneId)
		if (!leaf) {
			return
		}

		leaf.pane.placeholder.textContent = text
	}

	public movePane(paneId: string, targetPaneId: string | null, position: DockDropPosition): void {
		const source = this.panes.get(paneId)
		if (!source) {
			return
		}

		if (position === "center") {
			if (targetPaneId) {
				this.swapPanePositions(paneId, targetPaneId)
				this.setActivePane(paneId)
			}
			return
		}

		if (!targetPaneId) {
			if (this.panes.size <= 1) {
				return
			}

			this.moveLeafToRoot(source, position)
			this.setActivePane(paneId)
			return
		}

		const target = this.panes.get(targetPaneId)
		if (!target || target === source) {
			return
		}

		this.moveLeafRelative(source, target, position)
		this.setActivePane(paneId)
	}

	private createLeaf(presetId?: string): DockLeaf {
		const element = document.createElement("div")
		element.style.display = "flex"
		element.style.flexDirection = "column"
		element.style.flex = "1 1 0"
		element.style.minHeight = "0"
		element.style.minWidth = "0"
		element.style.border = "1px solid #cbd5f5"
		element.style.borderRadius = "4px"
		element.style.margin = "4px"
		element.style.backgroundColor = "#ffffff"

		const header = document.createElement("div")
		header.style.display = "flex"
		header.style.alignItems = "center"
		header.style.padding = "8px 12px"
		header.style.gap = "8px"
		header.style.borderBottom = "1px solid #e2e8f0"
		header.style.backgroundColor = "#f8fafc"
		header.style.fontWeight = "500"
		header.style.fontSize = "0.9rem"
		header.style.userSelect = "none"
		header.style.cursor = "grab"

		const title = document.createElement("span")
		title.textContent = "Empty Pane"
		title.style.flexGrow = "1"
		title.style.userSelect = "none"
		header.appendChild(title)

		const closeButton = document.createElement("button")
		closeButton.type = "button"
		closeButton.title = "Close pane"
		closeButton.setAttribute("aria-label", "Close pane")
		closeButton.style.border = "none"
		closeButton.style.background = "transparent"
		closeButton.style.cursor = "pointer"
		closeButton.style.padding = "4px"
		closeButton.style.borderRadius = "4px"
		closeButton.style.display = "flex"
		closeButton.style.alignItems = "center"
		closeButton.style.justifyContent = "center"

		closeButton.addEventListener("mouseenter", () => {
			closeButton.style.backgroundColor = "#e2e8f0"
		})
		closeButton.addEventListener("mouseleave", () => {
			closeButton.style.backgroundColor = "transparent"
		})
		closeButton.addEventListener("mousedown", (event) => {
			event.stopPropagation()
			closeButton.style.backgroundColor = "#cbd5f5"
		})
		closeButton.addEventListener("mouseup", () => {
			closeButton.style.backgroundColor = "#e2e8f0"
		})
		closeButton.addEventListener("blur", () => {
			closeButton.style.backgroundColor = "transparent"
		})

		const closeIcon = document.createElementNS("http://www.w3.org/2000/svg", "svg")
		closeIcon.setAttribute("viewBox", "0 0 20 20")
		closeIcon.setAttribute("width", "16")
		closeIcon.setAttribute("height", "16")

		const closePath = document.createElementNS("http://www.w3.org/2000/svg", "path")
		closePath.setAttribute("d", "M6.5 6.5 L13.5 13.5 M13.5 6.5 L6.5 13.5")
		closePath.setAttribute("stroke", "#475569")
		closePath.setAttribute("stroke-width", "1.5")
		closePath.setAttribute("stroke-linecap", "round")

		closeIcon.appendChild(closePath)
		closeButton.appendChild(closeIcon)
		header.appendChild(closeButton)

		const content = document.createElement("div")
		content.style.flexGrow = "1"
		content.style.display = "flex"
		content.style.flexDirection = "column"
		content.style.minHeight = "0"
		content.style.minWidth = "0"
		content.style.position = "relative"

		const placeholder = document.createElement("div")
		placeholder.textContent = "Select an item to open it here."
		placeholder.style.display = "flex"
		placeholder.style.alignItems = "center"
		placeholder.style.justifyContent = "center"
		placeholder.style.flexGrow = "1"
		placeholder.style.color = "#64748b"
		placeholder.style.fontSize = "0.9rem"
		placeholder.style.padding = "12px"
		placeholder.style.textAlign = "center"

		const externalDropIndicator = document.createElement("div")
		externalDropIndicator.style.position = "absolute"
		externalDropIndicator.style.display = "none"
		externalDropIndicator.style.pointerEvents = "none"
		externalDropIndicator.style.backgroundColor = "rgba(59, 130, 246, 0.2)"
		externalDropIndicator.style.border = "2px solid rgba(59, 130, 246, 0.65)"
		externalDropIndicator.style.borderRadius = "4px"
		externalDropIndicator.style.zIndex = "2"

		content.appendChild(placeholder)
		content.appendChild(externalDropIndicator)

		element.appendChild(header)
		element.appendChild(content)

		const paneId = presetId ?? `pane-${++paneIdCounter}`
		if (presetId) {
			const match = /pane-(\d+)/.exec(presetId)
			if (match) {
				const numericId = Number(match[1])
				if (!Number.isNaN(numericId)) {
					paneIdCounter = Math.max(paneIdCounter, numericId)
				}
			}
		}
		element.dataset.paneId = paneId

		const paneState: DockPaneState = {
			id: paneId,
			element,
			header,
			title,
			content,
			placeholder,
			externalDropIndicator,
			currentComponent: null,
			closeButton
		}

		const leaf: DockLeaf = {
			type: "leaf",
			pane: paneState
		}

		const activate = () => {
			this.setActivePane(paneState.id)
		}

		header.addEventListener("mousedown", activate)
		content.addEventListener("mousedown", activate)
		content.addEventListener("dragenter", (event) => this.handleExternalContentDragEnter(event, paneState))
		content.addEventListener("dragover", (event) => this.handleExternalContentDragOver(event, paneState))
		content.addEventListener("dragleave", (event) => this.handleExternalContentDragLeave(event, paneState))
		content.addEventListener("drop", (event) => this.handleExternalContentDrop(event, paneState))

		closeButton.addEventListener("click", (event) => {
			event.stopPropagation()
			this.setActivePane(paneState.id)
			this.closePane(paneState.id)
		})

		header.draggable = true
		title.draggable = true
		closeButton.draggable = false

		const handlePaneDragStart = (event: DragEvent) => {
			this.cancelPendingPointerMove()
			this.endPointerMove()
			if (this.panes.size <= 1) {
				event.preventDefault()
				return
			}
			event.dataTransfer?.setData("text/plain", paneState.id)
			event.dataTransfer?.setDragImage(paneState.header, 0, 0)
			header.style.cursor = "grabbing"
			this.startDrag(leaf, event.clientX, event.clientY)
		}

		header.addEventListener("dragstart", handlePaneDragStart)
		title.addEventListener("dragstart", handlePaneDragStart)

		const handlePaneDragEnd = () => {
			header.style.cursor = "grab"
			this.endDrag()
		}

		header.addEventListener("dragend", handlePaneDragEnd)
		title.addEventListener("dragend", handlePaneDragEnd)

		header.addEventListener("mousedown", (event) => {
			if (event.button !== 0) {
				return
			}
			if (this.panes.size <= 1) {
				return
			}
			const target = event.target as Node | null
			if (target && closeButton.contains(target)) {
				return
			}
			this.schedulePointerMoveStart(leaf, event.clientX, event.clientY)
		})

		this.panes.set(paneState.id, leaf)
		this.parentMap.set(leaf, null)

		this.updatePaneCloseVisibility()

		return leaf
	}

	private canHandleExternalDrop(event: DragEvent): boolean {
		if (this.dragState) {
			return false
		}
		if (!this.canAcceptExternalDrop) {
			return false
		}
		return this.canAcceptExternalDrop(event)
	}

	private getExternalDropPosition(event: DragEvent, pane: DockPaneState): DockDropPosition {
		const rect = pane.content.getBoundingClientRect()
		const leftEdge = Math.min(Math.max(rect.width * 0.28, 40), 140)
		const topEdge = Math.min(Math.max(rect.height * 0.28, 40), 120)
		const offsetX = event.clientX - rect.left
		const offsetY = event.clientY - rect.top
		const distances: Array<{ position: DockDropPosition; score: number }> = []
		if (offsetY <= topEdge) {
			distances.push({ position: "top", score: offsetY / topEdge })
		}
		if (offsetY >= rect.height - topEdge) {
			distances.push({ position: "bottom", score: (rect.height - offsetY) / topEdge })
		}
		if (offsetX <= leftEdge) {
			distances.push({ position: "left", score: offsetX / leftEdge })
		}
		if (offsetX >= rect.width - leftEdge) {
			distances.push({ position: "right", score: (rect.width - offsetX) / leftEdge })
		}
		if (distances.length === 0) {
			return "center"
		}
		distances.sort((a, b) => a.score - b.score)
		const nearest = distances[0]
		return nearest?.position ?? "center"
	}

	private showExternalDropIndicator(pane: DockPaneState, position: DockDropPosition): void {
		const indicator = pane.externalDropIndicator
		const paneWidth = pane.content.clientWidth
		const paneHeight = pane.content.clientHeight
		const edgeWidth = Math.min(Math.max(paneWidth * 0.35, 48), 180)
		const edgeHeight = Math.min(Math.max(paneHeight * 0.35, 48), 180)
		indicator.style.display = "block"
		indicator.style.left = ""
		indicator.style.right = ""
		indicator.style.width = ""
		indicator.style.height = ""
		indicator.style.top = ""
		indicator.style.bottom = ""
		switch (position) {
			case "left":
				indicator.style.left = "0"
				indicator.style.top = "0"
				indicator.style.bottom = "0"
				indicator.style.width = `${Math.round(edgeWidth)}px`
				break
			case "right":
				indicator.style.right = "0"
				indicator.style.top = "0"
				indicator.style.bottom = "0"
				indicator.style.width = `${Math.round(edgeWidth)}px`
				break
			case "top":
				indicator.style.left = "0"
				indicator.style.right = "0"
				indicator.style.top = "0"
				indicator.style.height = `${Math.round(edgeHeight)}px`
				break
			case "bottom":
				indicator.style.left = "0"
				indicator.style.right = "0"
				indicator.style.bottom = "0"
				indicator.style.height = `${Math.round(edgeHeight)}px`
				break
			default:
				indicator.style.left = "0"
				indicator.style.right = "0"
				indicator.style.top = "0"
				indicator.style.bottom = "0"
				break
		}
	}

	private hideExternalDropIndicator(pane: DockPaneState): void {
		pane.externalDropIndicator.style.display = "none"
	}

	private hideAllExternalDropIndicators(): void {
		for (const leaf of this.panes.values()) {
			this.hideExternalDropIndicator(leaf.pane)
		}
	}

	private handleExternalContentDragEnter(event: DragEvent, pane: DockPaneState): void {
		if (!this.canHandleExternalDrop(event)) {
			this.hideExternalDropIndicator(pane)
			return
		}
		event.preventDefault()
		event.stopPropagation()
		const position = this.getExternalDropPosition(event, pane)
		this.hideAllExternalDropIndicators()
		this.showExternalDropIndicator(pane, position)
	}

	private handleExternalContentDragOver(event: DragEvent, pane: DockPaneState): void {
		if (!this.canHandleExternalDrop(event)) {
			this.hideExternalDropIndicator(pane)
			return
		}
		event.preventDefault()
		event.stopPropagation()
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = "move"
		}
		const position = this.getExternalDropPosition(event, pane)
		this.hideAllExternalDropIndicators()
		this.showExternalDropIndicator(pane, position)
	}

	private handleExternalContentDragLeave(event: DragEvent, pane: DockPaneState): void {
		const related = event.relatedTarget as Node | null
		if (related && pane.content.contains(related)) {
			return
		}
		this.hideExternalDropIndicator(pane)
	}

	private handleExternalContentDrop(event: DragEvent, pane: DockPaneState): void {
		if (!this.canHandleExternalDrop(event)) {
			this.hideExternalDropIndicator(pane)
			return
		}
		event.preventDefault()
		event.stopPropagation()
		const position = this.getExternalDropPosition(event, pane)
		this.hideAllExternalDropIndicators()
		this.setActivePane(pane.id)
		this.onExternalDrop?.({
			paneId: pane.id,
			position,
			event
		})
	}

	private createPaneDragGhost(source: DockLeaf): HTMLDivElement {
		const ghost = document.createElement("div")
		ghost.textContent = source.pane.title.textContent ?? "Pane"
		ghost.style.position = "absolute"
		ghost.style.pointerEvents = "none"
		ghost.style.padding = "8px 12px"
		ghost.style.borderRadius = "8px"
		ghost.style.border = "1px solid rgba(59, 130, 246, 0.45)"
		ghost.style.background = "rgba(248, 250, 252, 0.95)"
		ghost.style.color = "#0f172a"
		ghost.style.fontSize = "13px"
		ghost.style.fontWeight = "600"
		ghost.style.boxShadow = "0 10px 24px rgba(15, 23, 42, 0.22)"
		ghost.style.zIndex = "1002"
		ghost.style.maxWidth = "240px"
		ghost.style.whiteSpace = "nowrap"
		ghost.style.overflow = "hidden"
		ghost.style.textOverflow = "ellipsis"
		return ghost
	}

	private positionPaneDragGhost(ghost: HTMLDivElement, clientX: number, clientY: number): void {
		const rootRect = this.root.getBoundingClientRect()
		ghost.style.left = `${Math.round(clientX - rootRect.left + 12)}px`
		ghost.style.top = `${Math.round(clientY - rootRect.top + 12)}px`
	}

	private schedulePointerMoveStart(source: DockLeaf, startX: number, startY: number): void {
		this.cancelPendingPointerMove()
		const moveThreshold = 2
		const moveGuard = (event: MouseEvent) => {
			const distance = Math.hypot(event.clientX - startX, event.clientY - startY)
			if (distance > moveThreshold) {
				cleanup()
				this.beginPointerMove(source, event.clientX, event.clientY)
			}
		}
		const upGuard = () => {
			cleanup()
		}
		const cleanup = () => {
			window.removeEventListener("mousemove", moveGuard, true)
			window.removeEventListener("mouseup", upGuard, true)
			if (this.pendingPointerMoveCancel === cleanup) {
				this.pendingPointerMoveCancel = null
			}
		}
		this.pendingPointerMoveCancel = cleanup
		window.addEventListener("mousemove", moveGuard, true)
		window.addEventListener("mouseup", upGuard, true)
	}

	private cancelPendingPointerMove(): void {
		if (!this.pendingPointerMoveCancel) {
			return
		}
		const cancel = this.pendingPointerMoveCancel
		this.pendingPointerMoveCancel = null
		cancel()
	}

	private beginPointerMove(source: DockLeaf, clientX: number, clientY: number): void {
		if (this.dragState || this.pointerMoveState) {
			return
		}
		const overlay = document.createElement("div")
		overlay.style.position = "absolute"
		overlay.style.top = "0"
		overlay.style.left = "0"
		overlay.style.right = "0"
		overlay.style.bottom = "0"
		overlay.style.pointerEvents = "auto"
		overlay.style.zIndex = "1000"
		this.root.appendChild(overlay)
		this.buildDropTargets(overlay, source)
		const ghost = this.createPaneDragGhost(source)
		overlay.appendChild(ghost)
		this.positionPaneDragGhost(ghost, clientX, clientY)

		const moveHandler = (event: MouseEvent) => {
			this.updatePointerHoveredZone(event.clientX, event.clientY)
			this.positionPaneDragGhost(ghost, event.clientX, event.clientY)
		}
		const upHandler = () => {
			this.commitPointerMove()
		}
		this.pointerMoveState = {
			source,
			overlay,
			ghost,
			hoveredZone: null,
			moveHandler,
			upHandler
		}
		window.addEventListener("mousemove", moveHandler, true)
		window.addEventListener("mouseup", upHandler, true)
		this.updatePointerHoveredZone(clientX, clientY)
	}

	private updatePointerHoveredZone(clientX: number, clientY: number): void {
		const state = this.pointerMoveState
		if (!state) {
			return
		}
		let zone: HTMLDivElement | null = null
		const stackedElements = document.elementsFromPoint(clientX, clientY)
		for (const stackedElement of stackedElements) {
			const candidate = stackedElement.closest<HTMLDivElement>("[data-dock-drop-position]")
			if (candidate && state.overlay.contains(candidate)) {
				zone = candidate
				break
			}
		}
		if (!zone) {
			const element = document.elementFromPoint(clientX, clientY) as HTMLElement | null
			zone = element?.closest<HTMLDivElement>("[data-dock-drop-position]") ?? null
		}
		const hoveredZone = zone && state.overlay.contains(zone) ? zone : null
		if (state.hoveredZone && state.hoveredZone !== hoveredZone) {
			this.setDropZoneHighlight(state.hoveredZone, false)
		}
		state.hoveredZone = hoveredZone
		if (state.hoveredZone) {
			this.setDropZoneHighlight(state.hoveredZone, true)
		}
	}

	private commitPointerMove(): void {
		const state = this.pointerMoveState
		if (!state) {
			return
		}
		const zone = state.hoveredZone
		this.endPointerMove()
		if (!zone) {
			return
		}
		const position = zone.dataset.dockDropPosition as DockDropPosition | undefined
		const targetPaneId = zone.dataset.dockDropPaneId || null
		if (!position) {
			return
		}
		this.movePane(state.source.pane.id, targetPaneId, position)
	}

	private endPointerMove(): void {
		const state = this.pointerMoveState
		if (!state) {
			return
		}
		if (state.hoveredZone) {
			this.setDropZoneHighlight(state.hoveredZone, false)
		}
		window.removeEventListener("mousemove", state.moveHandler, true)
		window.removeEventListener("mouseup", state.upHandler, true)
		state.overlay.remove()
		this.pointerMoveState = null
	}

	private createSplitElement(orientation: DockOrientation): HTMLDivElement {
		const element = document.createElement("div")
		element.style.display = "flex"
		element.style.flex = "1 1 0"
		element.style.minHeight = "0"
		element.style.minWidth = "0"
		element.style.flexDirection = orientation === "horizontal" ? "row" : "column"
		return element
	}

	private setPaneActive(pane: DockPaneState, active: boolean): void {
		if (active) {
			pane.element.style.boxShadow = "0 0 0 2px #3b82f6"
			pane.element.style.borderColor = "#3b82f6"
		} else {
			pane.element.style.boxShadow = "none"
			pane.element.style.borderColor = "#cbd5f5"
		}
	}

	private serializeNode(node: DockNode): DockLayoutNodeState {
		if (node.type === "leaf") {
			return {
				type: "leaf",
				paneId: node.pane.id
			}
		}

		return {
			type: "split",
			orientation: node.orientation,
			children: node.children.map((child) => this.serializeNode(child))
		}
	}

	private restoreNode(node: DockLayoutNodeState, parent: DockSplit | null): DockNode {
		if (node.type === "leaf") {
			const leaf = this.createLeaf(node.paneId)
			this.parentMap.set(leaf, parent)
			return leaf
		}

		const splitNode: DockSplit = {
			type: "split",
			orientation: node.orientation,
			element: this.createSplitElement(node.orientation),
			children: []
		}

		this.parentMap.set(splitNode, parent)

		for (const childState of node.children) {
			const childNode = this.restoreNode(childState, splitNode)
			splitNode.children.push(childNode)
			if (childNode.type === "leaf") {
				splitNode.element.appendChild(childNode.pane.element)
			} else {
				splitNode.element.appendChild(childNode.element)
			}
		}

		return splitNode
	}

	private getMaxPaneNumber(node: DockLayoutNodeState): number {
		if (node.type === "leaf") {
			const match = /pane-(\d+)/.exec(node.paneId)
			if (match) {
				const numericId = Number(match[1])
				if (!Number.isNaN(numericId)) {
					return numericId
				}
			}
			return -1
		}

		return node.children.reduce((max, child) => Math.max(max, this.getMaxPaneNumber(child)), -1)
	}

	private startDrag(source: DockLeaf, initialClientX?: number, initialClientY?: number): void {
		this.cancelPendingPointerMove()
		this.endPointerMove()
		this.hideAllExternalDropIndicators()
		this.endDrag()

		const overlay = document.createElement("div")
		overlay.style.position = "absolute"
		overlay.style.top = "0"
		overlay.style.left = "0"
		overlay.style.right = "0"
		overlay.style.bottom = "0"
		overlay.style.pointerEvents = "auto"
		overlay.style.zIndex = "1000"

		this.root.appendChild(overlay)
		this.buildDropTargets(overlay, source)
		const ghost = this.createPaneDragGhost(source)
		overlay.appendChild(ghost)

		const sourceRect = source.pane.element.getBoundingClientRect()
		const ghostX = typeof initialClientX === "number" && Number.isFinite(initialClientX) && initialClientX > 0 ? initialClientX : sourceRect.left + sourceRect.width / 2
		const ghostY = typeof initialClientY === "number" && Number.isFinite(initialClientY) && initialClientY > 0 ? initialClientY : sourceRect.top + sourceRect.height / 2
		this.positionPaneDragGhost(ghost, ghostX, ghostY)

		const overlayDragHandler = (event: DragEvent) => {
			if (!Number.isFinite(event.clientX) || !Number.isFinite(event.clientY)) {
				return
			}
			this.positionPaneDragGhost(ghost, event.clientX, event.clientY)
		}
		overlay.addEventListener("dragover", overlayDragHandler, true)
		overlay.addEventListener("dragenter", overlayDragHandler, true)

		this.dragState = { source, overlay, ghost, overlayDragHandler }
	}

	private endDrag(): void {
		if (!this.dragState) {
			return
		}

		this.dragState.overlay.removeEventListener("dragover", this.dragState.overlayDragHandler, true)
		this.dragState.overlay.removeEventListener("dragenter", this.dragState.overlayDragHandler, true)
		this.dragState.overlay.remove()
		this.dragState = null
		this.hideAllExternalDropIndicators()
	}

	private buildDropTargets(overlay: HTMLDivElement, source: DockLeaf): void {
		if (this.panes.size <= 1) {
			return
		}

		const rootRect = this.root.getBoundingClientRect()
		const edgeRatio = 0.35

		for (const leaf of this.panes.values()) {
			if (leaf === source) {
				continue
			}

			const rect = leaf.pane.element.getBoundingClientRect()
			const container = document.createElement("div")
			container.style.position = "absolute"
			container.style.left = `${rect.left - rootRect.left}px`
			container.style.top = `${rect.top - rootRect.top}px`
			container.style.width = `${rect.width}px`
			container.style.height = `${rect.height}px`
			container.style.pointerEvents = "none"

			overlay.appendChild(container)

			this.createDropZone(container, leaf.pane.id, "left", () => this.handleDrop(leaf, "left"), {
				left: 0,
				top: 0,
				width: rect.width * edgeRatio,
				height: rect.height
			})

			this.createDropZone(container, leaf.pane.id, "right", () => this.handleDrop(leaf, "right"), {
				left: rect.width * (1 - edgeRatio),
				top: 0,
				width: rect.width * edgeRatio,
				height: rect.height
			})

			this.createDropZone(container, leaf.pane.id, "top", () => this.handleDrop(leaf, "top"), {
				left: 0,
				top: 0,
				width: rect.width,
				height: rect.height * edgeRatio
			})

			this.createDropZone(container, leaf.pane.id, "bottom", () => this.handleDrop(leaf, "bottom"), {
				left: 0,
				top: rect.height * (1 - edgeRatio),
				width: rect.width,
				height: rect.height * edgeRatio
			})

			this.createDropZone(container, leaf.pane.id, "center", () => this.handleDrop(leaf, "center"), {
				left: rect.width * edgeRatio,
				top: rect.height * edgeRatio,
				width: rect.width * (1 - edgeRatio * 2),
				height: rect.height * (1 - edgeRatio * 2)
			})
		}

		const container = document.createElement("div")
		container.style.position = "absolute"
		container.style.left = "0"
		container.style.top = "0"
		container.style.right = "0"
		container.style.bottom = "0"
		container.style.pointerEvents = "none"

		overlay.appendChild(container)

		const rootWidth = rootRect.width
		const rootHeight = rootRect.height
		const rootEdgeWidth = Math.min(rootWidth * edgeRatio, 200)
		const rootEdgeHeight = Math.min(rootHeight * edgeRatio, 200)

		this.createDropZone(container, null, "left", () => this.handleDrop(null, "left"), {
			left: 0,
			top: 0,
			width: rootEdgeWidth,
			height: rootHeight
		})

		this.createDropZone(container, null, "right", () => this.handleDrop(null, "right"), {
			left: Math.max(rootWidth - rootEdgeWidth, 0),
			top: 0,
			width: rootEdgeWidth,
			height: rootHeight
		})

		this.createDropZone(container, null, "top", () => this.handleDrop(null, "top"), {
			left: 0,
			top: 0,
			width: rootWidth,
			height: rootEdgeHeight
		})

		this.createDropZone(container, null, "bottom", () => this.handleDrop(null, "bottom"), {
			left: 0,
			top: Math.max(rootHeight - rootEdgeHeight, 0),
			width: rootWidth,
			height: rootEdgeHeight
		})
	}

	private createDropZone(
		container: HTMLDivElement,
		targetPaneId: string | null,
		position: DockDropPosition,
		onDrop: () => void,
		dimensions: { left: number; top: number; width: number; height: number }
	): void {
		if (dimensions.width <= 0 || dimensions.height <= 0) {
			return
		}

		const zone = document.createElement("div")
		zone.style.position = "absolute"
		zone.style.left = `${dimensions.left}px`
		zone.style.top = `${dimensions.top}px`
		zone.style.width = `${dimensions.width}px`
		zone.style.height = `${dimensions.height}px`
		zone.style.border = "2px solid transparent"
		zone.style.backgroundColor = position === "center" ? "rgba(59, 130, 246, 0.1)" : "rgba(59, 130, 246, 0.15)"
		zone.style.opacity = "0"
		zone.style.transition = "opacity 0.1s ease"
		zone.style.pointerEvents = "auto"
		zone.style.borderRadius = "4px"
		zone.dataset.dockDropPosition = position
		zone.dataset.dockDropPaneId = targetPaneId ?? ""

		zone.addEventListener("dragenter", (event) => {
			event.preventDefault()
			this.setDropZoneHighlight(zone, true)
		})

		zone.addEventListener("dragleave", () => {
			this.setDropZoneHighlight(zone, false)
		})

		zone.addEventListener("dragover", (event) => {
			event.preventDefault()
			if (event.dataTransfer) {
				event.dataTransfer.dropEffect = "move"
			}
		})

		zone.addEventListener("drop", (event) => {
			event.preventDefault()
			this.setDropZoneHighlight(zone, false)
			onDrop()
		})

		container.appendChild(zone)
	}

	private setDropZoneHighlight(zone: HTMLDivElement, isActive: boolean): void {
		zone.style.opacity = isActive ? "1" : "0"
		zone.style.borderColor = isActive ? "rgba(59, 130, 246, 0.6)" : "transparent"
	}

	private handleDrop(target: DockLeaf | null, position: DockDropPosition): void {
		const dragState = this.dragState
		if (!dragState) {
			return
		}

		const source = dragState.source
		this.endDrag()

		if (target) {
			this.movePane(source.pane.id, target.pane.id, position)
		} else {
			this.movePane(source.pane.id, null, position)
		}
	}

	private swapPanePositions(sourcePaneId: string, targetPaneId: string): void {
		if (sourcePaneId === targetPaneId) {
			return
		}
		const source = this.panes.get(sourcePaneId)
		const target = this.panes.get(targetPaneId)
		if (!source || !target) {
			return
		}
		const sourceParent = this.parentMap.get(source)
		const targetParent = this.parentMap.get(target)
		if (!sourceParent || !targetParent) {
			return
		}

		const sourceIndex = sourceParent.children.indexOf(source)
		const targetIndex = targetParent.children.indexOf(target)
		if (sourceIndex < 0 || targetIndex < 0) {
			return
		}

		sourceParent.children[sourceIndex] = target
		targetParent.children[targetIndex] = source
		this.parentMap.set(source, targetParent)
		this.parentMap.set(target, sourceParent)

		const sourceElement = source.pane.element
		const targetElement = target.pane.element
		const sourcePlaceholder = document.createElement("div")
		const targetPlaceholder = document.createElement("div")
		sourceParent.element.replaceChild(sourcePlaceholder, sourceElement)
		targetParent.element.replaceChild(targetPlaceholder, targetElement)
		sourceParent.element.replaceChild(targetElement, sourcePlaceholder)
		targetParent.element.replaceChild(sourceElement, targetPlaceholder)
	}

	private getOrientationForPosition(position: DockDropPosition): DockOrientation | null {
		switch (position) {
			case "left":
			case "right":
				return "horizontal"
			case "top":
			case "bottom":
				return "vertical"
			default:
				return null
		}
	}

	private moveLeafRelative(source: DockLeaf, target: DockLeaf, position: DockDropPosition): void {
		const orientation = this.getOrientationForPosition(position)
		if (!orientation) {
			return
		}

		const insertBefore = position === "left" || position === "top"

		this.detachLeaf(source)

		const parent = this.parentMap.get(target) ?? null

		if (parent && parent.orientation === orientation) {
			const targetIndex = parent.children.indexOf(target)
			const insertIndex = insertBefore ? targetIndex : targetIndex + 1
			const reference = parent.element.children[insertIndex] ?? null

			parent.children.splice(insertIndex, 0, source)
			parent.element.insertBefore(source.pane.element, reference)
			this.parentMap.set(source, parent)
			return
		}

		const splitNode: DockSplit = {
			type: "split",
			orientation,
			element: this.createSplitElement(orientation),
			children: []
		}

		const order: DockNode[] = insertBefore ? [source, target] : [target, source]

		if (!parent) {
			this.parentMap.set(splitNode, null)
			this.parentMap.set(target, splitNode)
			this.parentMap.set(source, splitNode)
			this.rootNode = splitNode
			this.root.innerHTML = ""
			this.root.appendChild(splitNode.element)
		} else {
			const index = parent.children.indexOf(target)
			if (index >= 0) {
				parent.children.splice(index, 1, splitNode)
			}
			this.parentMap.set(splitNode, parent)
			this.parentMap.set(target, splitNode)
			this.parentMap.set(source, splitNode)
			parent.element.replaceChild(splitNode.element, target.pane.element)
		}

		for (const child of order) {
			splitNode.children.push(child)
			if (child.type === "leaf") {
				splitNode.element.appendChild(child.pane.element)
			} else {
				splitNode.element.appendChild(child.element)
				this.parentMap.set(child, splitNode)
			}
		}
	}

	private moveLeafToRoot(source: DockLeaf, position: DockDropPosition): void {
		const orientation = this.getOrientationForPosition(position)
		if (!orientation) {
			return
		}

		this.detachLeaf(source)

		if (this.rootNode.type === "split" && this.rootNode.orientation === orientation) {
			const insertBefore = position === "left" || position === "top"
			if (insertBefore) {
				this.rootNode.children.unshift(source)
				this.rootNode.element.insertBefore(source.pane.element, this.rootNode.element.firstChild)
			} else {
				this.rootNode.children.push(source)
				this.rootNode.element.appendChild(source.pane.element)
			}
			this.parentMap.set(source, this.rootNode)
			return
		}

		const existingRoot = this.rootNode
		const splitNode: DockSplit = {
			type: "split",
			orientation,
			element: this.createSplitElement(orientation),
			children: []
		}

		this.parentMap.set(splitNode, null)

		const order: DockNode[] = position === "left" || position === "top" ? [source, existingRoot] : [existingRoot, source]

		this.rootNode = splitNode
		this.root.innerHTML = ""
		this.root.appendChild(splitNode.element)

		for (const child of order) {
			splitNode.children.push(child)
			if (child.type === "leaf") {
				splitNode.element.appendChild(child.pane.element)
				this.parentMap.set(child, splitNode)
			} else {
				splitNode.element.appendChild(child.element)
				this.parentMap.set(child, splitNode)
			}
		}

		this.parentMap.set(source, splitNode)
	}

	private detachLeaf(leaf: DockLeaf): void {
		const parent = this.parentMap.get(leaf) ?? null

		leaf.pane.element.remove()
		this.parentMap.set(leaf, null)

		if (!parent) {
			return
		}

		const index = parent.children.indexOf(leaf)
		if (index >= 0) {
			parent.children.splice(index, 1)
		}

		this.trimSplit(parent)
	}

	private removeLeaf(leaf: DockLeaf): void {
		this.detachLeaf(leaf)
	}

	private trimSplit(split: DockSplit): void {
		const grandParent = this.parentMap.get(split) ?? null

		if (split.children.length > 1) {
			return
		}

		if (split.children.length === 1) {
			const surviving = split.children[0]
			if (!surviving) {
				return
			}
			this.parentMap.set(surviving, grandParent)

			if (!grandParent) {
				this.rootNode = surviving
				this.root.innerHTML = ""
				if (surviving.type === "leaf") {
					this.root.appendChild(surviving.pane.element)
				} else {
					this.root.appendChild(surviving.element)
				}
			} else {
				const index = grandParent.children.indexOf(split)
				if (index >= 0) {
					grandParent.children.splice(index, 1, surviving)
				}
				const replacement = surviving.type === "leaf" ? surviving.pane.element : surviving.element
				grandParent.element.replaceChild(replacement, split.element)
			}

			split.children = []
			this.parentMap.delete(split)
			if (grandParent) {
				this.trimSplit(grandParent)
			}
			return
		}

		if (!grandParent) {
			return
		}

		const index = grandParent.children.indexOf(split)
		if (index >= 0) {
			grandParent.children.splice(index, 1)
		}
		grandParent.element.removeChild(split.element)
		this.parentMap.delete(split)
		if (grandParent) {
			this.trimSplit(grandParent)
		}
	}

	private findFallbackLeaf(excludePaneId: string): DockLeaf | null {
		for (const [id, leaf] of this.panes) {
			if (id !== excludePaneId) {
				return leaf
			}
		}
		return null
	}

	private updatePaneCloseVisibility(): void {
		const closable = this.panes.size > 1
		for (const leaf of this.panes.values()) {
			leaf.pane.closeButton.style.visibility = closable ? "visible" : "hidden"
		}
	}
}
