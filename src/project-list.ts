export type ProjectListEntry = ProjectListFileEntry | ProjectListFolderEntry

export type ProjectListFileEntry = {
	kind: "file"
	id: string
	name: string
	metadata?: unknown
}

export type ProjectListFolderEntry = {
	kind: "folder"
	id: string
	name: string
	items: ProjectListEntry[]
	metadata?: unknown
}

type ProjectListOptions = {
	onMove?: (args: { sourceId: string; destinationId: string | null }) => void
	canMove?: (args: { sourceId: string; destinationId: string | null }) => boolean
	onSelect?: (args: { id: string }) => void
}

abstract class ProjectItemView<T extends ProjectListEntry> {
	public readonly entry: T
	public readonly root: HTMLElement
	public parentFolder: ProjectFolderView | null = null

	public constructor(
		protected readonly list: ProjectList,
		entry: T
	) {
		this.entry = entry
		this.root = this.createRoot()
		this.root.dataset.projectItemId = entry.id
	}

	protected abstract createRoot(): HTMLElement

	public setSelected(isSelected: boolean) {
		if (isSelected) {
			this.root.style.outline = "2px solid #1c7ed6"
			this.root.style.backgroundColor = "#edf2ff"
		} else {
			this.root.style.outline = ""
			this.root.style.backgroundColor = ""
		}
	}
}

class ProjectFileView extends ProjectItemView<ProjectListFileEntry> {
	protected createRoot(): HTMLElement {
		const root = this.list.doc.createElement("div")
		root.classList.add("project-file")
		root.draggable = true
		root.textContent = this.entry.name
		root.addEventListener("dragstart", (event) => {
			this.list.beginDrag(this, event)
		})
		root.addEventListener("dragend", () => {
			this.list.endDrag()
		})
		root.addEventListener("click", (event) => {
			event.stopPropagation()
			this.list.selectItem(this)
		})
		return root
	}
}

class ProjectFolderView extends ProjectItemView<ProjectListFolderEntry> {
	private declare expandIcon: HTMLElement
	private declare titleElement: HTMLElement
	private declare itemsContainer: HTMLElement
	private expanded = true
	private readonly childViews: ProjectItemView<ProjectListEntry>[] = []
	private dropHintActive = false
	private dropHintDirect = false

	protected createRoot(): HTMLElement {
		const root = this.list.doc.createElement("div")
		root.classList.add("project-folder")
		root.addEventListener("dragover", (event) => this.onDragOver(event))
		root.addEventListener("dragleave", (event) => this.onDragLeave(event))
		root.addEventListener("drop", (event) => this.onDrop(event))

		this.titleElement = this.list.doc.createElement("div")
		this.titleElement.style.display = "flex"
		this.titleElement.style.alignItems = "center"

		this.expandIcon = this.list.doc.createElement("span")
		this.expandIcon.textContent = this.expanded ? "[-]" : "[+]"
		this.expandIcon.style.display = "inline-block"
		this.expandIcon.style.width = "20px"
		this.expandIcon.style.cursor = "pointer"
		this.expandIcon.style.userSelect = "none"
		this.expandIcon.addEventListener("click", (event) => {
			event.stopPropagation()
			this.toggleExpanded()
		})
		this.expandIcon.addEventListener("mousedown", (event) => {
			event.stopPropagation()
		})
		this.expandIcon.draggable = false

		const titleText = this.list.doc.createElement("span")
		titleText.textContent = this.entry.name
		titleText.style.flexGrow = "1"
		titleText.style.userSelect = "none"

		this.titleElement.draggable = true
		this.titleElement.addEventListener("dragstart", (event) => {
			this.list.beginDrag(this, event)
		})
		this.titleElement.addEventListener("dragend", () => {
			this.list.endDrag()
		})
		this.titleElement.addEventListener("click", (event) => {
			event.stopPropagation()
			if (event.detail === 1) {
				this.list.selectItem(this)
			}
			if (event.detail === 2) {
				this.toggleExpanded()
			}
		})

		this.itemsContainer = this.list.doc.createElement("div")
		this.itemsContainer.style.paddingLeft = "16px"
		this.itemsContainer.style.display = "block"

		this.titleElement.appendChild(this.expandIcon)
		this.titleElement.appendChild(titleText)
		root.appendChild(this.titleElement)
		root.appendChild(this.itemsContainer)
		this.updateExpandState()
		return root
	}

	public addChildView(view: ProjectItemView<ProjectListEntry>) {
		this.childViews.push(view)
		view.parentFolder = this
		this.itemsContainer.appendChild(view.root)
	}

	public expand() {
		this.expanded = true
		this.updateExpandState()
	}

	public collapse() {
		this.expanded = false
		this.updateExpandState()
	}

	public isExpanded(): boolean {
		return this.expanded
	}

	public resetDropHint() {
		this.dropHintActive = false
		this.dropHintDirect = false
		this.updateDropHintStyles()
	}

	public showDropHint(isDirect: boolean) {
		const wasDirect = this.dropHintDirect
		this.dropHintActive = true
		this.dropHintDirect = isDirect
		this.updateDropHintStyles()
		if (isDirect || !wasDirect) {
			this.parentFolder?.showDropHint(false)
		}
	}

	public hideDropHint(fromChild = false) {
		if (!this.dropHintActive) {
			return
		}
		if (fromChild && this.dropHintDirect) {
			return
		}
		this.dropHintActive = false
		this.dropHintDirect = false
		this.updateDropHintStyles()
		this.parentFolder?.hideDropHint(true)
	}

	private toggleExpanded() {
		this.expanded = !this.expanded
		this.updateExpandState()
	}

	private onDragOver(event: DragEvent) {
		if (!this.list.canDropOn(this)) {
			return
		}
		event.preventDefault()
		const dataTransfer = event.dataTransfer
		if (dataTransfer) {
			dataTransfer.dropEffect = "move"
		}
		this.showDropHint(true)
	}

	private onDrop(event: DragEvent) {
		event.preventDefault()
		event.stopPropagation()
		this.hideDropHint()
		this.list.dropOnto(this)
	}

	private onDragLeave(event: DragEvent) {
		const related = event.relatedTarget as Node | null
		if (!related || !this.root.contains(related)) {
			this.hideDropHint()
		}
	}

	private updateDropHintStyles() {
		if (!this.dropHintActive) {
			this.root.style.backgroundColor = ""
			this.root.style.outline = ""
			return
		}
		if (this.dropHintDirect) {
			this.root.style.backgroundColor = "#74c0fc"
			this.root.style.outline = "2px solid #1c7ed6"
		} else {
			this.root.style.backgroundColor = ""
			this.root.style.outline = "2px dashed #4dabf7"
		}
	}

	private updateExpandState() {
		this.itemsContainer.style.display = this.expanded ? "block" : "none"
		if (this.expandIcon) {
			this.expandIcon.textContent = this.expanded ? "[-]" : "[+]"
		}
	}
}

export class ProjectList {
	public readonly root: HTMLElement
	public readonly doc: Document

	private readonly options: ProjectListOptions
	private readonly views = new Map<string, ProjectItemView<ProjectListEntry>>()
	private draggedView: ProjectItemView<ProjectListEntry> | null = null
	private selectedView: ProjectItemView<ProjectListEntry> | null = null

	constructor(doc: Document = document, options: ProjectListOptions = {}) {
		if (!doc) {
			throw new Error("ProjectList requires a document to render into.")
		}
		this.doc = doc
		this.options = options
		this.root = doc.createElement("div")
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.gap = "4px"
		this.root.addEventListener("dragover", (event) => this.onRootDragOver(event))
		this.root.addEventListener("dragleave", (event) => this.onRootDragLeave(event))
		this.root.addEventListener("drop", (event) => this.onRootDrop(event))
	}

	public setItems(items: ProjectListEntry[], selectedId: string | null = null) {
		this.views.clear()
		this.root.innerHTML = ""
		for (const item of items) {
			const view = this.createView(item)
			this.views.set(item.id, view)
			this.root.appendChild(view.root)
		}
		this.selectById(selectedId)
	}

	public selectById(id: string | null) {
		if (!id) {
			this.clearSelection()
			return
		}
		const view = this.views.get(id)
		if (!view) {
			this.clearSelection()
			return
		}
		this.selectItem(view)
	}

	private createView(entry: ProjectListEntry): ProjectItemView<ProjectListEntry> {
		if (entry.kind === "folder") {
			const folderView = new ProjectFolderView(this, entry)
			for (const child of entry.items) {
				const childView = this.createView(child)
				this.views.set(child.id, childView)
				folderView.addChildView(childView)
			}
			return folderView
		}
		return new ProjectFileView(this, entry)
	}

	public beginDrag(view: ProjectItemView<ProjectListEntry>, event: DragEvent) {
		this.draggedView = view
		const dataTransfer = event.dataTransfer
		if (dataTransfer) {
			dataTransfer.effectAllowed = "move"
			dataTransfer.setData("text/plain", view.entry.id)
		}
	}

	public endDrag() {
		this.finishDrag()
	}

	public canDropOn(folder: ProjectFolderView | null): boolean {
		const sourceId = this.draggedView?.entry.id
		if (!sourceId) {
			return false
		}
		const destinationId = folder?.entry.id ?? null
		if (this.options.canMove) {
			return this.options.canMove({ sourceId, destinationId })
		}
		return sourceId !== destinationId
	}

	public dropOnto(folder: ProjectFolderView | null) {
		const view = this.draggedView
		if (!view) {
			return
		}
		const sourceId = view.entry.id
		const destinationId = folder?.entry.id ?? null
		if (!this.canDropOn(folder)) {
			this.finishDrag()
			return
		}
		this.options.onMove?.({ sourceId, destinationId })
		this.finishDrag()
	}

	public selectItem(view: ProjectItemView<ProjectListEntry>) {
		if (this.selectedView === view) {
			return
		}
		if (this.selectedView) {
			this.selectedView.setSelected(false)
		}
		this.selectedView = view
		this.selectedView.setSelected(true)
		this.options.onSelect?.({ id: view.entry.id })
		if (view instanceof ProjectFolderView) {
			view.expand()
		}
		let ancestor = view.parentFolder
		while (ancestor) {
			ancestor.expand()
			ancestor = ancestor.parentFolder
		}
	}

	private clearSelection() {
		if (this.selectedView) {
			this.selectedView.setSelected(false)
		}
		this.selectedView = null
	}

	private finishDrag() {
		this.draggedView = null
		this.clearDropHints()
		this.hideRootDropHint()
	}

	private clearDropHints() {
		for (const view of this.views.values()) {
			if (view instanceof ProjectFolderView) {
				view.resetDropHint()
			}
		}
	}

	private onRootDragOver(event: DragEvent) {
		if (event.target !== this.root) {
			return
		}
		const view = this.draggedView
		if (!view) {
			return
		}
		if (!this.canDropOn(null)) {
			return
		}
		event.preventDefault()
		const dataTransfer = event.dataTransfer
		if (dataTransfer) {
			dataTransfer.dropEffect = "move"
		}
		this.showRootDropHint()
	}

	private onRootDrop(event: DragEvent) {
		if (event.target !== this.root) {
			return
		}
		event.preventDefault()
		this.dropOnto(null)
	}

	private onRootDragLeave(event: DragEvent) {
		if (event.target !== this.root) {
			return
		}
		this.hideRootDropHint()
	}

	private showRootDropHint() {
		this.root.style.outline = "2px dashed #4dabf7"
	}

	private hideRootDropHint() {
		this.root.style.outline = ""
	}

	public getCollapsedFolderIds(): Set<string> {
		const collapsed = new Set<string>()
		for (const view of this.views.values()) {
			if (view instanceof ProjectFolderView && !view.isExpanded()) {
				collapsed.add(view.entry.id)
			}
		}
		return collapsed
	}

	public applyCollapsedState(collapsedIds: Set<string>) {
		for (const id of collapsedIds) {
			const view = this.views.get(id)
			if (view instanceof ProjectFolderView) {
				view.collapse()
			}
		}
	}

	public expandToId(id: string | null) {
		if (!id) {
			return
		}
		const view = this.views.get(id)
		if (!view) {
			return
		}
		let folder: ProjectFolderView | null = view instanceof ProjectFolderView ? view : view.parentFolder
		while (folder) {
			folder.expand()
			folder = folder.parentFolder
		}
	}
}
