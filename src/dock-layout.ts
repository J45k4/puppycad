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
	currentComponent: UiComponent<HTMLElement> | null
	closeButton: HTMLButtonElement
}

type DockLeaf = {
	type: "leaf"
	pane: DockPaneState
}

type DockNode = DockSplit | DockLeaf

let paneIdCounter = 0

export class DockLayout extends UiComponent<HTMLDivElement> {
	private rootNode: DockNode
	private readonly parentMap = new Map<DockNode, DockSplit | null>()
	private readonly panes = new Map<string, DockLeaf>()
	private activePaneId: string | null = null
	public onActivePaneChange: ((paneId: string) => void) | null = null
	public onPaneClosed: ((closedPaneId: string, nextActivePaneId: string | null) => void) | null = null

	public constructor() {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.flexGrow = "1"
		this.root.style.minHeight = "0"
		this.root.style.minWidth = "0"

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
		pane.content.innerHTML = ""
		pane.content.appendChild(component.root)
		pane.placeholder.style.display = "none"
		pane.currentComponent = component
	}

	public clearPane(paneId: string): void {
		const leaf = this.panes.get(paneId)
		if (!leaf) {
			return
		}

		const pane = leaf.pane
		pane.content.innerHTML = ""
		pane.content.appendChild(pane.placeholder)
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

		const title = document.createElement("span")
		title.textContent = "Empty Pane"
		title.style.flexGrow = "1"
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

		content.appendChild(placeholder)

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

		const paneState: DockPaneState = {
			id: paneId,
			element,
			header,
			title,
			content,
			placeholder,
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

		closeButton.addEventListener("click", (event) => {
			event.stopPropagation()
			this.setActivePane(paneState.id)
			this.closePane(paneState.id)
		})

		this.panes.set(paneState.id, leaf)
		this.parentMap.set(leaf, null)

		this.updatePaneCloseVisibility()

		return leaf
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

	private removeLeaf(leaf: DockLeaf): void {
		const paneElement = leaf.pane.element
		const parent = this.parentMap.get(leaf)
		paneElement.remove()

		if (!parent) {
			return
		}

		const index = parent.children.indexOf(leaf)
		if (index >= 0) {
			parent.children.splice(index, 1)
		}

		this.trimSplit(parent)
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
