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

type ProjectListAction = {
	label: string
	onSelect: () => void
}

type ProjectListOptions = {
	onMove?: (args: { sourceId: string; destinationId: string | null }) => void
	canMove?: (args: { sourceId: string; destinationId: string | null }) => boolean
	onSelect?: (args: { id: string }) => void
	getActions?: (entry: ProjectListEntry) => ProjectListAction[]
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
		this.root.classList.add("project-item")
	}

	protected abstract createRoot(): HTMLElement

	public setSelected(isSelected: boolean) {
		this.root.classList.toggle("project-item--selected", isSelected)
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
			this.list.handleItemLeftClick(this)
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
		this.titleElement.classList.add("project-folder__title")

		this.expandIcon = this.list.doc.createElement("span")
		this.expandIcon.textContent = this.expanded ? "▾" : "▸"
		this.expandIcon.classList.add("project-folder__toggle")
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
			if (event.detail === 2) {
				this.toggleExpanded()
				return
			}
			this.list.handleItemLeftClick(this)
		})
		this.itemsContainer = this.list.doc.createElement("div")
		this.itemsContainer.classList.add("project-folder__children")

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
		const isActive = this.dropHintActive
		const isDirect = this.dropHintActive && this.dropHintDirect
		this.root.classList.toggle("project-folder--drop-target", isActive)
		this.root.classList.toggle("project-folder--drop-target-direct", isDirect)
	}

	private updateExpandState() {
		this.itemsContainer.style.display = this.expanded ? "" : "none"
		if (this.expandIcon) {
			this.expandIcon.textContent = this.expanded ? "▾" : "▸"
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
	private readonly menu: HTMLDivElement
	private menuVisibleForId: string | null = null
	private readonly onDocumentClick = (event: MouseEvent) => {
		if (!this.menuVisibleForId) {
			return
		}
		const target = event.target as Node | null
		if (!target) {
			return
		}
		if (!this.root.contains(target)) {
			this.hideMenu()
		}
	}
	private readonly onRootContextMenu = (event: MouseEvent) => {
		const target = event.target as HTMLElement | null
		if (!target) {
			return
		}
		if (this.menu.contains(target)) {
			return
		}
		const itemElement = target.closest<HTMLElement>("[data-project-item-id]")
		if (!itemElement) {
			this.hideMenu()
			return
		}
		const itemId = itemElement.dataset.projectItemId
		if (!itemId) {
			this.hideMenu()
			return
		}
		const view = this.views.get(itemId)
		if (!view) {
			this.hideMenu()
			return
		}
		event.preventDefault()
		event.stopPropagation()
		this.handleItemContextMenu(view, event)
	}

	constructor(doc: Document = document, options: ProjectListOptions = {}) {
		if (!doc) {
			throw new Error("ProjectList requires a document to render into.")
		}
		this.doc = doc
		this.options = options
		this.root = doc.createElement("div")
		this.root.classList.add("project-tree")
		this.root.addEventListener("dragover", (event) => this.onRootDragOver(event))
		this.root.addEventListener("dragleave", (event) => this.onRootDragLeave(event))
		this.root.addEventListener("drop", (event) => this.onRootDrop(event))
		this.root.addEventListener("contextmenu", this.onRootContextMenu)
		this.root.addEventListener("click", () => {
			if (!this.menuVisibleForId) {
				return
			}
			this.hideMenu()
		})

		this.menu = doc.createElement("div")
		this.menu.className = "project-tree-menu"
		this.menu.addEventListener("click", (event) => {
			event.stopPropagation()
		})
		this.root.appendChild(this.menu)

		this.doc.addEventListener("click", this.onDocumentClick)
	}

	public setItems(items: ProjectListEntry[], selectedId: string | null = null) {
		this.views.clear()
		this.root.innerHTML = ""
		this.root.appendChild(this.menu)
		this.hideMenu()
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
			dataTransfer.setData("application/x-puppycad-project-node", view.entry.id)
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

	public handleItemLeftClick(view: ProjectItemView<ProjectListEntry>) {
		this.selectItem(view)
		this.hideMenu()
	}

	public handleItemContextMenu(view: ProjectItemView<ProjectListEntry>, event: MouseEvent) {
		event.preventDefault()
		event.stopPropagation()
		this.selectItem(view)
		const actions = this.options.getActions?.(view.entry) ?? []
		if (!actions.length) {
			this.hideMenu()
			return
		}
		this.showMenu(view, actions, event)
	}

	private showMenu(view: ProjectItemView<ProjectListEntry>, actions: ProjectListAction[], event: MouseEvent) {
		this.menu.innerHTML = ""
		for (const action of actions) {
			const button = this.doc.createElement("button")
			button.textContent = action.label
			button.type = "button"
			button.className = "project-tree-menu__action"
			button.addEventListener("click", (clickEvent) => {
				clickEvent.stopPropagation()
				this.hideMenu()
				action.onSelect()
			})
			this.menu.appendChild(button)
		}
		this.menu.style.display = "flex"
		this.menuVisibleForId = view.entry.id

		const rootRect = this.root.getBoundingClientRect()
		const itemRect = view.root.getBoundingClientRect()
		const scrollTop = this.root.scrollTop
		const scrollLeft = this.root.scrollLeft
		const top = itemRect.top - rootRect.top + scrollTop
		const preferredLeft = event.clientX - rootRect.left + scrollLeft

		this.menu.style.top = `${Math.max(0, top)}px`
		this.menu.style.left = "0px"

		const menuWidth = this.menu.offsetWidth
		let left = preferredLeft
		const maxLeft = Math.max(0, this.root.clientWidth - menuWidth - 4)
		if (left > maxLeft) {
			left = maxLeft
		}
		if (left < 0) {
			left = 0
		}
		this.menu.style.left = `${left}px`
	}

	private hideMenu() {
		this.menuVisibleForId = null
		this.menu.style.display = "none"
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
		this.root.classList.add("project-tree--root-drop-target")
	}

	private hideRootDropHint() {
		this.root.classList.remove("project-tree--root-drop-target")
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
