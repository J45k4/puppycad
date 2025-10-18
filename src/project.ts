import { AssemblyEditor } from "./assembly"
import { createDiagramEditor } from "./diagram"
import { PartEditor } from "./part"
import type { PartEditorState } from "./part"
import { PCBEditor } from "./pcb"
import { SchemanticEditor, type SchemanticEditorState } from "./schemantic"
import { PROJECT_FILE_MIME_TYPE, createProjectFile, normalizeProjectFile, serializeProjectFile } from "./project-file"
import type { ProjectFile, ProjectFileEntry, ProjectFileFolder, ProjectFileType } from "./project-file"
import { ItemList, Modal, UiComponent, TreeList, type TreeNode } from "./ui"
import { ProjectList, type ProjectListEntry } from "./project-list"

type BaseProjectItem = {
	type: ProjectFileType
	name: string
	editor: UiComponent<HTMLDivElement>
}

type SchemanticProjectItem = BaseProjectItem & {
	type: "schemantic"
	editor: SchemanticEditor
	getState: () => SchemanticEditorState
}

type PartProjectItem = BaseProjectItem & {
	type: "part"
	editor: PartEditor
	getState: () => PartEditorState
}

type OtherProjectItem = BaseProjectItem & {
	type: Exclude<ProjectFileType, "schemantic" | "part">
}

export type ProjectItem = SchemanticProjectItem | PartProjectItem | OtherProjectItem

type ProjectFolder = {
	kind: "folder"
	name: string
	children: ProjectNode[]
}

type ProjectNode = ProjectItem | ProjectFolder

type NewNodeType = ProjectFileType | "folder"

function isFolder(node: ProjectNode): node is ProjectFolder {
	return (node as ProjectFolder).kind === "folder"
}

function isProjectItem(node: ProjectNode): node is ProjectItem {
	return !isFolder(node)
}

class ProjectTreeView extends UiComponent<HTMLDivElement> {
	private items: ProjectNode[] = []
	private modal: Modal
	private projectList: ProjectList
	private selectedPath: number[] | null = null
	private readonly onItemSelected?: (item: ProjectItem) => void
	private databasePromise: Promise<IDBDatabase> | null = null
	private persistTimeout: number | null = null
	private isRestoring = false
	private persistenceEnabled = typeof indexedDB !== "undefined"
	private nodePaths: Map<ProjectNode, number[]> = new Map()
	private nodeIdMap: Map<ProjectNode, string> = new Map()
	private idNodeMap: Map<string, ProjectNode> = new Map()
	private nextNodeId = 0
	private itemsListContainer: TreeList<ProjectNode>
	private nodeElements: Map<ProjectNode, { header: HTMLDivElement; container?: HTMLDivElement; exitDropZone?: HTMLDivElement }> = new Map()
	private dragState: { node: ProjectNode; path: number[] } | null = null
	private rootDropZone: HTMLDivElement
	private exitDropZone: { element: HTMLDivElement; parent: ProjectFolder } | null = null
	private static readonly DATABASE_NAME = "puppycad-project"
	private static readonly STORE_NAME = "projectState"
	private static readonly STORE_KEY = "items"
	private static readonly PERSIST_DEBOUNCE_MS = 200

	private log(...args: unknown[]): void {
		if (typeof console !== "undefined") {
			console.log("[ProjectTreeView]", ...args)
		}
	}

	private describeNode(node: ProjectNode): string {
		if (isFolder(node)) {
			return `Folder(${node.name}, children=${node.children.length})`
		}
		return `${node.type}:${node.name}`
	}

	private describePath(path: number[] | null): string {
		if (!path) {
			return "<null>"
		}
		return `[${path.join(",")}]`
	}

	public constructor(args: {
		onClick?: (item: ProjectItem) => void
	}) {
		super(document.createElement("div"))
		this.onItemSelected = args.onClick
		this.modal = new Modal({
			title: "New Item",
			content: new ItemList<NewNodeType>({
				onClick: (type) => {
					this.addNode(type)
					this.modal.hide()
				},
				items: [
					{ label: "Folder", value: "folder" },
					{ label: "Schemantic", value: "schemantic" },
					{ label: "PCB", value: "pcb" },
					{ label: "Part", value: "part" },
					{ label: "Assembly", value: "assembly" },
					{ label: "Diagram", value: "diagram" }
				]
			})
		})
		this.root.style.width = "200px"
		this.root.style.overflowY = "auto"
		this.root.style.borderRight = "1px solid #ccc"
		this.root.style.padding = "10px"
		this.root.style.boxSizing = "border-box"

		const newButton = document.createElement("button")
		newButton.textContent = "New"
		newButton.onclick = this.newButtonClicked.bind(this)
		this.root.appendChild(newButton)

		const renameButton = document.createElement("button")
		renameButton.textContent = "Rename"
		renameButton.onclick = () => this.renameSelectedItem()
		this.root.appendChild(renameButton)

		const saveButton = document.createElement("button")
		saveButton.textContent = "Save"
		saveButton.onclick = () => this.saveProjectToFile()
		this.root.appendChild(saveButton)

		const serverSaveButton = document.createElement("button")
		serverSaveButton.textContent = "Save to Server"
		serverSaveButton.onclick = () => {
			void this.saveProjectToServer()
		}
		this.root.appendChild(serverSaveButton)

		this.itemsListContainer = new TreeList<ProjectNode>({
			items: [],
			onClick: (item) => this.handleNodeSelection(item),
			onRenderNode: (value, elements) => {
				this.registerNodeElements(value, elements)
			}
		})
		this.rootDropZone = this.createRootDropZone()
		this.root.appendChild(this.rootDropZone)
		this.root.appendChild(this.itemsListContainer.root)
		this.itemsListContainer.root.style.display = "none"
		this.projectList = new ProjectList(document, {
			onMove: ({ sourceId, destinationId }) => this.handleMoveRequest(sourceId, destinationId),
			canMove: ({ sourceId, destinationId }) => this.canMoveNodes(sourceId, destinationId),
			onSelect: ({ id }) => this.handleSelectionById(id)
		})
		this.projectList.root.style.marginTop = "12px"
		this.root.appendChild(this.projectList.root)
		// Allow drops anywhere within the tree container so nested targets receive drop
		this.itemsListContainer.root.addEventListener("dragover", (event) => {
			if (!this.dragState) return
			event.preventDefault()
			event.stopPropagation()
			if (event.dataTransfer) {
				event.dataTransfer.dropEffect = "move"
			}
		})
		this.itemsListContainer.root.addEventListener("drop", (event) => {
			// No-op handler to keep default browser handlers from swallowing the event
			if (!this.dragState) return
			event.preventDefault()
			event.stopPropagation()
		})
		// Allow dropping on empty space/background to move to root
		this.root.addEventListener("dragover", (event) => {
			if (!this.canDropToRoot()) return
			const target = event.target as Node | null
			// If we're not over any folder header, treat as root-area dragover
			if (!this.isOverAnyHeader(target)) {
				event.preventDefault()
				event.stopPropagation()
				if (event.dataTransfer) {
					event.dataTransfer.dropEffect = "move"
				}
			}
		})
		this.root.addEventListener("drop", (event) => {
			if (!this.canDropToRoot()) return
			const target = event.target as Node | null
			if (!this.isOverAnyHeader(target)) {
				event.preventDefault()
				event.stopPropagation()
				this.moveNodeToRoot()
			}
		})
		if (this.persistenceEnabled) {
			void this.loadFromIndexedDB()
		}
	}

	private createRootDropZone(): HTMLDivElement {
		const dropZone = document.createElement("div")
		dropZone.textContent = "Drop here to move to root"
		dropZone.style.display = "none"
		dropZone.style.padding = "8px"
		dropZone.style.margin = "8px 0"
		dropZone.style.border = "2px dashed #cbd5e1"
		dropZone.style.borderRadius = "8px"
		dropZone.style.color = "#475569"
		dropZone.style.fontSize = "12px"
		dropZone.style.textAlign = "center"
		dropZone.style.userSelect = "none"
		dropZone.addEventListener("dragenter", (event) => this.handleRootDropZoneDragEnter(event))
		dropZone.addEventListener("dragleave", (event) => this.handleRootDropZoneDragLeave(event))
		dropZone.addEventListener("dragover", (event) => this.handleRootDropZoneDragOver(event))
		dropZone.addEventListener("drop", (event) => this.handleRootDropZoneDrop(event))
		return dropZone
	}

	private renderItems() {
		this.log("renderItems:start", { totalNodes: this.items.length })
		this.nodePaths.clear()
		this.nodeElements.clear()
		this.idNodeMap.clear()
		const collapsedIds = this.projectList.getCollapsedFolderIds()
		const treeItems = this.buildTreeNodes(this.items)
		this.itemsListContainer.setItems(treeItems)
		const listEntries = this.buildProjectListEntries(this.items)
		let selectedId: string | null = null
		if (this.selectedPath !== null) {
			const selectedNode = this.getNodeByPath(this.selectedPath)
			if (selectedNode) {
				this.itemsListContainer.setSelected(selectedNode)
				selectedId = this.getNodeId(selectedNode)
			} else {
				this.selectedPath = null
				if (!this.isRestoring) {
					this.schedulePersist()
				}
			}
		}
		this.projectList.setItems(listEntries, selectedId)
		this.projectList.applyCollapsedState(collapsedIds)
		this.projectList.expandToId(selectedId)
		this.log("renderItems:complete", {
			registeredNodes: this.nodePaths.size,
			selectedPath: this.describePath(this.selectedPath)
		})
	}

	private registerNodeElements(node: ProjectNode, elements: { header: HTMLDivElement; container?: HTMLDivElement }) {
		const entry: { header: HTMLDivElement; container?: HTMLDivElement; exitDropZone?: HTMLDivElement } = {
			header: elements.header,
			container: elements.container
		}
		if (isFolder(node) && elements.container) {
			entry.exitDropZone = this.ensureExitDropZone(elements.container, node)
		}
		this.nodeElements.set(node, entry)
		const path = this.nodePaths.get(node)
		if (!path) {
			this.log("registerNodeElements:path-missing", { node: this.describeNode(node) })
			return
		}
		this.log("registerNodeElements", {
			node: this.describeNode(node),
			path: this.describePath(path),
			hasContainer: Boolean(elements.container)
		})
		const header = entry.header
		header.draggable = true
		header.addEventListener("dragstart", (event) => {
			this.handleDragStart(event, node, path.slice())
		})
		header.addEventListener("dragend", () => this.handleDragEnd())
		if (isFolder(node)) {
			const folderNode = node
			header.addEventListener("dragenter", (event) => this.handleFolderDragEnter(event, folderNode))
			header.addEventListener("dragleave", (event) => this.handleFolderDragLeave(event, folderNode))
			header.addEventListener("dragover", (event) => this.handleFolderDragOver(event, folderNode))
			header.addEventListener("drop", (event) => this.handleFolderDrop(event, folderNode))
			const container = entry.container
			if (container) {
				container.addEventListener("dragenter", (event) => this.handleFolderDragEnter(event, folderNode))
				container.addEventListener("dragover", (event) => this.handleFolderDragOver(event, folderNode))
				container.addEventListener("drop", (event) => this.handleFolderDrop(event, folderNode))
				container.addEventListener("dragleave", (event) => this.handleFolderAreaDragLeave(event, folderNode))
			}
		}
	}

	private buildTreeNodes(nodes: ProjectNode[], prefix: number[] = []): TreeNode<ProjectNode>[] {
		return nodes.map((node, index) => {
			const path = [...prefix, index]
			this.nodePaths.set(node, path)
			this.log("buildTreeNodes:register", {
				node: this.describeNode(node),
				path: this.describePath(path)
			})
			if (isFolder(node)) {
				const children = this.buildTreeNodes(node.children, path)
				return {
					label: `${node.name}/`,
					value: node,
					children: children.length > 0 ? children : undefined
				}
			}
			return {
				label: `${node.name} (${node.type})`,
				value: node
			}
		})
	}

	private buildProjectListEntries(nodes: ProjectNode[], prefix: number[] = []): ProjectListEntry[] {
		return nodes.map((node, index) => {
			const path = [...prefix, index]
			this.nodePaths.set(node, path)
			const id = this.getNodeId(node)
			if (isFolder(node)) {
				return {
					kind: "folder" as const,
					id,
					name: node.name,
					items: this.buildProjectListEntries(node.children, path)
				}
			}
			return {
				kind: "file" as const,
				id,
				name: node.name
			}
		})
	}

	private getNodeId(node: ProjectNode): string {
		let id = this.nodeIdMap.get(node)
		if (!id) {
			id = `project-node-${this.nextNodeId++}`
			this.nodeIdMap.set(node, id)
		}
		this.idNodeMap.set(id, node)
		return id
	}

	private handleSelectionById(id: string) {
		const node = this.idNodeMap.get(id)
		if (!node) {
			return
		}
		this.handleNodeSelection(node)
	}

	private canMoveNodes(sourceId: string, destinationId: string | null): boolean {
		const sourceNode = this.idNodeMap.get(sourceId)
		if (!sourceNode) {
			return false
		}
		if (!destinationId) {
			return true
		}
		const destinationNode = this.idNodeMap.get(destinationId)
		if (!destinationNode || !isFolder(destinationNode)) {
			return false
		}
		if (sourceNode === destinationNode) {
			return false
		}
		if (isFolder(sourceNode) && this.isNodeDescendant(sourceNode, destinationNode)) {
			return false
		}
		return true
	}

	private handleMoveRequest(sourceId: string, destinationId: string | null) {
		const sourceNode = this.idNodeMap.get(sourceId)
		if (!sourceNode) {
			return
		}
		if (!this.canMoveNodes(sourceId, destinationId)) {
			return
		}
		const sourcePath = this.nodePaths.get(sourceNode)
		if (!sourcePath) {
			return
		}
		const movedNode = this.removeNodeAtPath(sourcePath)
		if (!movedNode) {
			return
		}
		if (destinationId) {
			const destinationNode = this.idNodeMap.get(destinationId)
			if (!destinationNode || !isFolder(destinationNode)) {
				return
			}
			destinationNode.children.push(movedNode)
		} else {
			this.items.push(movedNode)
		}
		this.selectedPath = null
		this.renderItems()
		this.handleNodeSelection(movedNode)
		this.schedulePersist()
	}

	private isNodeDescendant(ancestor: ProjectNode, candidate: ProjectNode): boolean {
		const ancestorPath = this.nodePaths.get(ancestor)
		const candidatePath = this.nodePaths.get(candidate)
		if (!ancestorPath || !candidatePath) {
			return false
		}
		if (ancestorPath.length >= candidatePath.length) {
			return false
		}
		for (let i = 0; i < ancestorPath.length; i += 1) {
			if (ancestorPath[i] !== candidatePath[i]) {
				return false
			}
		}
		return true
	}

	private newButtonClicked() {
		this.modal.show()
	}

	private ensureExitDropZone(container: HTMLDivElement, parentNode: ProjectFolder): HTMLDivElement {
		const existing = container.querySelector<HTMLDivElement>(":scope > .project-tree-exit-drop-zone")
		if (existing) {
			return existing
		}
		const dropZone = document.createElement("div")
		dropZone.className = "project-tree-exit-drop-zone"
		dropZone.style.display = "none"
		dropZone.style.padding = "6px"
		dropZone.style.margin = "4px"
		dropZone.style.border = "2px dashed #cbd5e1"
		dropZone.style.borderRadius = "8px"
		dropZone.style.color = "#475569"
		dropZone.style.fontSize = "12px"
		dropZone.style.textAlign = "center"
		dropZone.style.userSelect = "none"
		dropZone.style.pointerEvents = "none"
		dropZone.addEventListener("dragenter", (event) => this.handleExitDropZoneDragEnter(event, parentNode))
		dropZone.addEventListener("dragleave", (event) => this.handleExitDropZoneDragLeave(event))
		dropZone.addEventListener("dragover", (event) => this.handleExitDropZoneDragOver(event, parentNode))
		dropZone.addEventListener("drop", (event) => this.handleExitDropZoneDrop(event, parentNode))
		container.insertBefore(dropZone, container.firstChild)
		return dropZone
	}

	private createDragPreview(node: ProjectNode): HTMLElement {
		const preview = document.createElement("div")
		preview.style.position = "absolute"
		preview.style.top = "-1000px"
		preview.style.left = "-1000px"
		preview.style.pointerEvents = "none"
		preview.style.padding = "4px 8px"
		preview.style.background = "rgba(30, 41, 59, 0.85)"
		preview.style.color = "#f8fafc"
		preview.style.borderRadius = "6px"
		preview.style.fontSize = "12px"
		preview.style.fontWeight = "500"
		preview.style.boxShadow = "0 8px 16px rgba(15, 23, 42, 0.3)"
		preview.textContent = isFolder(node) ? `${node.name}/` : `${node.name} (${node.type})`
		if (document.body) {
			document.body.appendChild(preview)
			setTimeout(() => {
				preview.remove()
			}, 0)
		}
		return preview
	}

	private getNodeHeader(node: ProjectNode): HTMLDivElement | undefined {
		return this.nodeElements.get(node)?.header
	}

	private getNodeChildContainer(node: ProjectFolder): HTMLDivElement | undefined {
		return this.nodeElements.get(node)?.container
	}

	private getNodeExitDropZone(node: ProjectFolder): HTMLDivElement | undefined {
		return this.nodeElements.get(node)?.exitDropZone
	}

	private handleDragStart(event: DragEvent, node: ProjectNode, path: number[]): void {
		event.stopPropagation()
		this.dragState = { node, path: path.slice() }
		this.log("handleDragStart", {
			node: this.describeNode(node),
			path: this.describePath(path)
		})
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = "move"
			event.dataTransfer.setData("text/plain", node.name)
			if (event.dataTransfer.setDragImage) {
				const preview = this.createDragPreview(node)
				event.dataTransfer.setDragImage(preview, 10, 10)
			}
		}
		this.resetRootDropZoneStyles()
		this.setRootDropZoneVisible(path.length > 1)
		this.showExitDropZone(path)
	}

	private handleDragEnd(): void {
		this.log("handleDragEnd")
		this.resetDragState()
	}

	private handleFolderDragEnter(event: DragEvent, folder: ProjectFolder): void {
		event.stopPropagation()
		if (!this.canDropOnFolder(folder)) {
			this.log("handleFolderDragEnter:blocked", {
				target: this.describeNode(folder),
				dragging: this.dragState ? this.describeNode(this.dragState.node) : null
			})
			return
		}
		this.log("handleFolderDragEnter", {
			target: this.describeNode(folder),
			dragging: this.dragState ? this.describeNode(this.dragState.node) : null
		})
		this.setFolderDropHighlight(folder, true)
	}

	private handleFolderDragLeave(event: DragEvent, folder: ProjectFolder): void {
		event.stopPropagation()
		const related = event.relatedTarget as Node | null
		const header = this.getNodeHeader(folder)
		if (related && header && header.contains(related)) {
			this.log("handleFolderDragLeave:within-header", { target: this.describeNode(folder) })
			return
		}
		this.log("handleFolderDragLeave", { target: this.describeNode(folder) })
		this.setFolderDropHighlight(folder, false)
	}

	private handleFolderAreaDragLeave(event: DragEvent, folder: ProjectFolder): void {
		event.stopPropagation()
		const related = event.relatedTarget as Node | null
		const header = this.getNodeHeader(folder)
		const container = this.getNodeChildContainer(folder)
		// If pointer moves within the folder's header or container, keep highlight
		if (related && (header?.contains(related) || container?.contains(related))) {
			this.log("handleFolderAreaDragLeave:within", { target: this.describeNode(folder) })
			return
		}
		this.log("handleFolderAreaDragLeave", { target: this.describeNode(folder) })
		this.setFolderDropHighlight(folder, false)
	}

	private handleFolderDragOver(event: DragEvent, folder: ProjectFolder): void {
		if (!this.canDropOnFolder(folder)) {
			this.log("handleFolderDragOver:blocked", {
				target: this.describeNode(folder),
				dragging: this.dragState ? this.describeNode(this.dragState.node) : null
			})
			this.setFolderDropHighlight(folder, false)
			return
		}
		this.log("handleFolderDragOver", {
			target: this.describeNode(folder),
			dragging: this.dragState ? this.describeNode(this.dragState.node) : null
		})
		event.preventDefault()
		event.stopPropagation()
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = "move"
		}
		this.setFolderDropHighlight(folder, true)
	}

	private handleFolderDrop(event: DragEvent, folder: ProjectFolder): void {
		event.preventDefault()
		event.stopPropagation()
		const dragState = this.dragState
		if (!dragState) {
			this.log("handleFolderDrop:no-drag-state", { target: this.describeNode(folder) })
			this.resetDragState()
			return
		}
		if (!this.canDropOnFolder(folder)) {
			this.log("handleFolderDrop:blocked", {
				target: this.describeNode(folder),
				dragging: this.describeNode(dragState.node)
			})
			this.resetDragState()
			return
		}
		const movedNode = this.removeNodeAtPath(dragState.path)
		if (!movedNode) {
			this.log("handleFolderDrop:remove-failed", {
				sourcePath: this.describePath(dragState.path)
			})
			this.resetDragState()
			return
		}
		this.log("handleFolderDrop:success", {
			target: this.describeNode(folder),
			moved: this.describeNode(movedNode),
			sourcePath: this.describePath(dragState.path)
		})
		this.resetDragState()
		folder.children.push(movedNode)
		this.renderItems()
		this.handleNodeSelection(movedNode)
		this.schedulePersist()
	}

	private handleRootDropZoneDragEnter(event: DragEvent): void {
		if (!this.canDropToRoot()) {
			this.log("handleRootDropZoneDragEnter:blocked")
			return
		}
		event.preventDefault()
		event.stopPropagation()
		this.log("handleRootDropZoneDragEnter", {
			dragging: this.dragState ? this.describeNode(this.dragState.node) : null
		})
		this.rootDropZone.style.backgroundColor = "#bfdbfe"
		this.rootDropZone.style.color = "#1e293b"
	}

	private handleRootDropZoneDragLeave(event: DragEvent): void {
		event.stopPropagation()
		const related = event.relatedTarget as Node | null
		if (related && this.rootDropZone.contains(related)) {
			this.log("handleRootDropZoneDragLeave:within")
			return
		}
		this.log("handleRootDropZoneDragLeave")
		this.resetRootDropZoneStyles()
	}

	private handleRootDropZoneDragOver(event: DragEvent): void {
		if (!this.canDropToRoot()) {
			this.log("handleRootDropZoneDragOver:blocked")
			return
		}
		event.preventDefault()
		event.stopPropagation()
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = "move"
		}
		this.log("handleRootDropZoneDragOver")
	}

	private handleRootDropZoneDrop(event: DragEvent): void {
		event.preventDefault()
		event.stopPropagation()
		if (!this.canDropToRoot()) {
			this.log("handleRootDropZoneDrop:blocked")
			this.resetDragState()
			return
		}
		this.log("handleRootDropZoneDrop", {
			dragging: this.dragState ? this.describeNode(this.dragState.node) : null
		})
		this.resetRootDropZoneStyles()
		this.moveNodeToRoot()
	}

	private showExitDropZone(path: number[]): void {
		this.removeExitDropZone()
		if (path.length < 2) {
			this.log("showExitDropZone:root-item", { path: this.describePath(path) })
			return
		}
		const parentPath = path.slice(0, -1)
		const parentNode = this.getNodeByPath(parentPath)
		if (!parentNode || !isFolder(parentNode)) {
			this.log("showExitDropZone:no-parent", {
				path: this.describePath(path)
			})
			return
		}
		const dropZone = this.getNodeExitDropZone(parentNode)
		if (!dropZone) {
			this.log("showExitDropZone:no-exit-zone", {
				parent: this.describeNode(parentNode)
			})
			return
		}
		dropZone.textContent = `Drop here to move out of ${parentNode.name}`
		dropZone.style.display = "block"
		dropZone.style.pointerEvents = "auto"
		dropZone.style.backgroundColor = ""
		dropZone.style.color = "#475569"
		this.exitDropZone = { element: dropZone, parent: parentNode }
		this.log("showExitDropZone", {
			parent: this.describeNode(parentNode),
			path: this.describePath(path)
		})
	}

	private removeExitDropZone(): void {
		if (!this.exitDropZone) {
			this.log("removeExitDropZone:noop")
			return
		}
		const { element } = this.exitDropZone
		element.style.display = "none"
		element.style.pointerEvents = "none"
		this.exitDropZone = null
		this.log("removeExitDropZone:removed")
	}

	private handleExitDropZoneDragEnter(event: DragEvent, parent: ProjectFolder): void {
		if (!this.canDropToParent(parent)) {
			this.log("handleExitDropZoneDragEnter:blocked", {
				parent: this.describeNode(parent)
			})
			return
		}
		event.preventDefault()
		event.stopPropagation()
		if (this.exitDropZone) {
			this.exitDropZone.element.style.backgroundColor = "#bfdbfe"
			this.exitDropZone.element.style.color = "#1e293b"
		}
		this.log("handleExitDropZoneDragEnter", {
			parent: this.describeNode(parent)
		})
	}

	private handleExitDropZoneDragLeave(event: DragEvent): void {
		event.stopPropagation()
		const related = event.relatedTarget as Node | null
		if (this.exitDropZone && related && this.exitDropZone.element.contains(related)) {
			this.log("handleExitDropZoneDragLeave:within")
			return
		}
		this.log("handleExitDropZoneDragLeave")
		this.resetExitDropZoneStyles()
	}

	private handleExitDropZoneDragOver(event: DragEvent, parent: ProjectFolder): void {
		if (!this.canDropToParent(parent)) {
			this.log("handleExitDropZoneDragOver:blocked", {
				parent: this.describeNode(parent)
			})
			return
		}
		event.preventDefault()
		event.stopPropagation()
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = "move"
		}
		if (this.exitDropZone) {
			this.exitDropZone.element.style.backgroundColor = "#bfdbfe"
			this.exitDropZone.element.style.color = "#1e293b"
		}
		this.log("handleExitDropZoneDragOver", {
			parent: this.describeNode(parent)
		})
	}

	private handleExitDropZoneDrop(event: DragEvent, parent: ProjectFolder): void {
		event.preventDefault()
		event.stopPropagation()
		if (!this.canDropToParent(parent)) {
			this.log("handleExitDropZoneDrop:blocked", {
				parent: this.describeNode(parent)
			})
			this.resetDragState()
			return
		}
		this.resetExitDropZoneStyles()
		this.moveNodeToParent()
		this.log("handleExitDropZoneDrop", {
			parent: this.describeNode(parent)
		})
	}

	private resetExitDropZoneStyles(): void {
		if (!this.exitDropZone) {
			this.log("resetExitDropZoneStyles:noop")
			return
		}
		const { element } = this.exitDropZone
		element.style.backgroundColor = ""
		element.style.color = "#475569"
		this.log("resetExitDropZoneStyles:reset")
	}

	private canDropOnFolder(node: ProjectNode): node is ProjectFolder {
		if (!this.dragState) {
			this.log("canDropOnFolder:false:no-drag")
			return false
		}
		if (!isFolder(node)) {
			this.log("canDropOnFolder:false:not-folder", { node: this.describeNode(node) })
			return false
		}
		const folderPath = this.nodePaths.get(node)
		if (!folderPath) {
			this.log("canDropOnFolder:false:no-path", { node: this.describeNode(node) })
			return false
		}
		if (this.pathsEqual(this.dragState.path, folderPath)) {
			this.log("canDropOnFolder:false:same-path", {
				node: this.describeNode(node),
				path: this.describePath(folderPath)
			})
			return false
		}
		if (this.isAncestorPath(this.dragState.path, folderPath)) {
			this.log("canDropOnFolder:false:ancestor", {
				node: this.describeNode(node),
				path: this.describePath(folderPath)
			})
			return false
		}
		this.log("canDropOnFolder:true", { node: this.describeNode(node) })
		return true
	}

	private canDropToRoot(): boolean {
		if (!this.dragState) {
			this.log("canDropToRoot:false:no-drag")
			return false
		}
		return this.dragState.path.length > 1
	}

	private canDropToParent(parent: ProjectFolder): boolean {
		if (!this.dragState) {
			this.log("canDropToParent:false:no-drag")
			return false
		}
		const parentPath = this.nodePaths.get(parent)
		if (!parentPath) {
			this.log("canDropToParent:false:no-path", { parent: this.describeNode(parent) })
			return false
		}
		if (this.dragState.path.length < 2) {
			this.log("canDropToParent:false:root-item", {
				parent: this.describeNode(parent)
			})
			return false
		}
		const currentParentPath = this.dragState.path.slice(0, -1)
		if (currentParentPath.length !== parentPath.length) {
			this.log("canDropToParent:false:length-mismatch", {
				currentParentPath: this.describePath(currentParentPath),
				targetPath: this.describePath(parentPath)
			})
			return false
		}
		for (let i = 0; i < parentPath.length; i += 1) {
			if (parentPath[i] !== currentParentPath[i]) {
				this.log("canDropToParent:false:index-mismatch", {
					currentParentPath: this.describePath(currentParentPath),
					targetPath: this.describePath(parentPath)
				})
				return false
			}
		}
		this.log("canDropToParent:true", { parent: this.describeNode(parent) })
		return true
	}

	private setFolderDropHighlight(folder: ProjectFolder, isActive: boolean): void {
		const header = this.getNodeHeader(folder)
		if (!header) {
			return
		}
		header.style.outline = isActive ? "2px dashed #2563eb" : ""
	}

	private clearDropHighlights(): void {
		for (const node of this.nodePaths.keys()) {
			if (isFolder(node)) {
				const header = this.getNodeHeader(node)
				if (header) {
					header.style.outline = ""
				}
			}
		}
		this.resetRootDropZoneStyles()
		this.resetExitDropZoneStyles()
	}

	private resetDragState(): void {
		this.dragState = null
		this.clearDropHighlights()
		this.setRootDropZoneVisible(false)
		this.removeExitDropZone()
		this.log("resetDragState")
	}

	private setRootDropZoneVisible(isVisible: boolean): void {
		this.rootDropZone.style.display = isVisible ? "block" : "none"
	}

	private resetRootDropZoneStyles(): void {
		this.rootDropZone.style.backgroundColor = ""
		this.rootDropZone.style.color = "#475569"
	}

	// Returns true if the given target node is inside any header element (for any node)
	private isOverAnyHeader(target: Node | null): boolean {
		if (!target) return false
		for (const node of this.nodePaths.keys()) {
			const header = this.getNodeHeader(node)
			if (header?.contains(target as Node)) {
				this.log("isOverAnyHeader:true", { node: this.describeNode(node) })
				return true
			}
		}
		return false
	}

	private isAncestorPath(ancestor: number[], descendant: number[]): boolean {
		if (ancestor.length >= descendant.length) {
			return false
		}
		for (let i = 0; i < ancestor.length; i += 1) {
			if (ancestor[i] !== descendant[i]) {
				return false
			}
		}
		return true
	}

	private removeNodeAtPath(path: number[]): ProjectNode | null {
		if (path.length === 0) {
			return null
		}
		let container: ProjectNode[] = this.items
		for (let i = 0; i < path.length - 1; i += 1) {
			const indexValue = path[i]
			if (indexValue === undefined) {
				return null
			}
			const index = indexValue
			const node = container[index]
			if (!node || !isFolder(node)) {
				return null
			}
			container = node.children
		}
		const lastIndexValue = path[path.length - 1]
		if (lastIndexValue === undefined) {
			return null
		}
		const index = lastIndexValue
		if (index < 0 || index >= container.length) {
			return null
		}
		const [removed] = container.splice(index, 1)
		return removed ?? null
	}

	private moveNodeToRoot(): void {
		const dragState = this.dragState
		if (!dragState) {
			this.log("moveNodeToRoot:no-drag-state")
			return
		}
		const movedNode = this.removeNodeAtPath(dragState.path)
		if (!movedNode) {
			this.log("moveNodeToRoot:remove-failed", { path: this.describePath(dragState.path) })
			this.resetDragState()
			return
		}
		this.log("moveNodeToRoot:success", {
			node: this.describeNode(movedNode),
			sourcePath: this.describePath(dragState.path)
		})
		this.resetDragState()
		this.items.push(movedNode)
		this.renderItems()
		this.handleNodeSelection(movedNode)
		this.schedulePersist()
	}

	private moveNodeToParent(): void {
		const dragState = this.dragState
		if (!dragState) {
			this.log("moveNodeToParent:no-drag-state")
			return
		}
		if (dragState.path.length < 2) {
			this.log("moveNodeToParent:already-root", { path: this.describePath(dragState.path) })
			this.resetDragState()
			return
		}
		const parentPath = dragState.path.slice(0, -1)
		const containerPath = parentPath.slice(0, -1)
		const movedNode = this.removeNodeAtPath(dragState.path)
		if (!movedNode) {
			this.log("moveNodeToParent:remove-failed", { path: this.describePath(dragState.path) })
			this.resetDragState()
			return
		}
		const container = this.getContainerByPath(containerPath)
		if (!container) {
			this.log("moveNodeToParent:container-missing", { containerPath: this.describePath(containerPath) })
			this.resetDragState()
			return
		}
		this.log("moveNodeToParent:success", {
			node: this.describeNode(movedNode),
			sourcePath: this.describePath(dragState.path),
			newContainerPath: this.describePath(containerPath)
		})
		this.resetDragState()
		container.push(movedNode)
		this.renderItems()
		this.handleNodeSelection(movedNode)
		this.schedulePersist()
	}

	private getContainerByPath(path: number[]): ProjectNode[] | null {
		if (path.length === 0) {
			return this.items
		}
		const parentNode = this.getNodeByPath(path)
		if (!parentNode || !isFolder(parentNode)) {
			return null
		}
		return parentNode.children
	}

	private addNode(type: NewNodeType) {
		if (type === "folder") {
			this.addFolder()
		} else {
			this.addItem(type)
		}
	}

	private addItem(type: ProjectFileType) {
		const container = this.getContainerForNewNode()
		const item = this.createProjectItem(type, undefined, container)
		container.push(item)
		this.renderItems()
		this.handleNodeSelection(item)
	}

	private addFolder() {
		const container = this.getContainerForNewNode()
		const folder = this.createFolder(undefined, container)
		container.push(folder)
		this.renderItems()
		this.handleNodeSelection(folder)
	}

	private renameSelectedItem() {
		if (!this.selectedPath) {
			return
		}
		const currentItem = this.getNodeByPath(this.selectedPath)
		if (!currentItem) {
			return
		}
		const siblings = this.getSiblingsForPath(this.selectedPath)
		if (!siblings) {
			return
		}
		const promptFn = typeof window !== "undefined" ? window.prompt : null
		if (!promptFn) {
			return
		}
		const newName = promptFn("Enter a new name", currentItem.name)
		if (!newName) {
			return
		}
		const trimmed = newName.trim()
		if (!trimmed || trimmed === currentItem.name) {
			return
		}
		if (this.isNameTaken(trimmed, siblings, currentItem)) {
			if (typeof window !== "undefined" && typeof window.alert === "function") {
				window.alert("An item with that name already exists.")
			}
			return
		}
		currentItem.name = trimmed
		this.renderItems()
		this.schedulePersist()
	}

	private handleNodeSelection(node: ProjectNode) {
		const path = this.nodePaths.get(node)
		if (!path) {
			this.log("handleNodeSelection:path-missing", { node: this.describeNode(node) })
			return
		}
		const selectionChanged = !this.pathsEqual(this.selectedPath, path)
		this.selectedPath = path.slice()
		this.log("handleNodeSelection", {
			node: this.describeNode(node),
			path: this.describePath(path),
			selectionChanged
		})
		this.itemsListContainer.setSelected(node)
		this.projectList.selectById(this.getNodeId(node))
		if (isProjectItem(node)) {
			this.onItemSelected?.(node)
		}
		if (selectionChanged) {
			this.schedulePersist()
		}
	}

	private createProjectItem(type: ProjectFileType, name?: string, existingNodes: ProjectNode[] = this.items, schemanticState?: SchemanticEditorState, partState?: PartEditorState): ProjectItem {
		const resolvedName = this.resolveItemName(type, name, existingNodes)
		switch (type) {
			case "schemantic": {
				const editor = new SchemanticEditor({
					initialState: schemanticState,
					onStateChange: () => this.schedulePersist()
				})
				return {
					type,
					name: resolvedName,
					editor,
					getState: () => editor.getState()
				}
			}
			case "pcb":
				return { type, name: resolvedName, editor: new PCBEditor() }
			case "part": {
				const editor = new PartEditor({
					initialState: partState,
					onStateChange: () => this.schedulePersist()
				})
				return {
					type,
					name: resolvedName,
					editor,
					getState: () => editor.getState()
				}
			}
			case "assembly":
				return { type, name: resolvedName, editor: new AssemblyEditor() }
			case "diagram":
				return { type, name: resolvedName, editor: createDiagramEditor() }
		}
		throw new Error(`Unsupported project item type: ${type}`)
	}

	private createFolder(name?: string, existingNodes: ProjectNode[] = this.items): ProjectFolder {
		return {
			kind: "folder",
			name: this.resolveFolderName(name, existingNodes),
			children: []
		}
	}

	private resolveItemName(type: ProjectFileType, desiredName: string | undefined, siblings: ProjectNode[]): string {
		return this.resolveName(desiredName, siblings, () => {
			const base = `${type.charAt(0).toUpperCase()}${type.slice(1)}`
			return this.generateUniqueName(base, siblings)
		})
	}

	private resolveFolderName(desiredName: string | undefined, siblings: ProjectNode[]): string {
		return this.resolveName(desiredName, siblings, () => this.generateUniqueName("Folder", siblings))
	}

	private resolveName(desiredName: string | undefined, siblings: ProjectNode[], generator: () => string): string {
		const trimmed = desiredName?.trim() ?? ""
		if (trimmed && !siblings.some((node) => node.name === trimmed)) {
			return trimmed
		}
		return generator()
	}

	private generateUniqueName(base: string, siblings: ProjectNode[]): string {
		let index = 1
		let candidate = `${base} ${index}`
		while (siblings.some((node) => node.name === candidate)) {
			index += 1
			candidate = `${base} ${index}`
		}
		return candidate
	}

	private isNameTaken(name: string, siblings: ProjectNode[], currentNode: ProjectNode | null): boolean {
		return siblings.some((node) => node !== currentNode && node.name === name)
	}

	private getNodeByPath(path: number[]): ProjectNode | null {
		if (path.length === 0) {
			return null
		}
		let nodes = this.items
		let node: ProjectNode | undefined
		for (let i = 0; i < path.length; i += 1) {
			const indexValue = path[i]
			if (indexValue === undefined) {
				return null
			}
			const index = indexValue as number
			if (index < 0 || index >= nodes.length) {
				return null
			}
			const candidate = nodes[index]
			node = candidate
			if (!node) {
				return null
			}
			if (i < path.length - 1) {
				if (!isFolder(node)) {
					return null
				}
				nodes = node.children
			}
		}
		return node ?? null
	}

	private getSiblingsForPath(path: number[]): ProjectNode[] | null {
		if (path.length === 0) {
			return this.items
		}
		if (path.length === 1) {
			return this.items
		}
		const parentPath = path.slice(0, -1)
		const parentNode = this.getNodeByPath(parentPath)
		if (!parentNode || !isFolder(parentNode)) {
			return null
		}
		return parentNode.children
	}

	private getContainerForNewNode(): ProjectNode[] {
		if (!this.selectedPath) {
			return this.items
		}
		const selectedNode = this.getNodeByPath(this.selectedPath)
		if (selectedNode && isFolder(selectedNode)) {
			return selectedNode.children
		}
		if (this.selectedPath.length === 1) {
			return this.items
		}
		const parentPath = this.selectedPath.slice(0, -1)
		const parentNode = this.getNodeByPath(parentPath)
		if (parentNode && isFolder(parentNode)) {
			return parentNode.children
		}
		return this.items
	}

	private pathsEqual(left: number[] | null, right: number[]): boolean {
		if (!left) {
			return false
		}
		if (left.length !== right.length) {
			return false
		}
		for (let i = 0; i < left.length; i += 1) {
			if (left[i] !== right[i]) {
				return false
			}
		}
		return true
	}

	private schedulePersist() {
		if (this.isRestoring || !this.persistenceEnabled) {
			this.log("schedulePersist:skipped", {
				isRestoring: this.isRestoring,
				persistenceEnabled: this.persistenceEnabled
			})
			return
		}
		if (this.persistTimeout !== null) {
			this.log("schedulePersist:clear-existing-timeout")
			window.clearTimeout(this.persistTimeout)
		}
		this.persistTimeout = window.setTimeout(() => {
			this.log("schedulePersist:timeout-fired")
			this.persistTimeout = null
			void this.saveToIndexedDB()
		}, ProjectTreeView.PERSIST_DEBOUNCE_MS)
		this.log("schedulePersist:scheduled", { delayMs: ProjectTreeView.PERSIST_DEBOUNCE_MS })
	}

	private async saveToIndexedDB() {
		if (!this.persistenceEnabled) {
			this.log("saveToIndexedDB:skipped:persistence-disabled")
			return
		}
		try {
			this.log("saveToIndexedDB:start")
			const db = await this.getDatabase()
			const transaction = db.transaction(ProjectTreeView.STORE_NAME, "readwrite")
			const store = transaction.objectStore(ProjectTreeView.STORE_NAME)
			const state = this.buildProjectFile()
			store.put(state, ProjectTreeView.STORE_KEY)
			await new Promise<void>((resolve, reject) => {
				transaction.oncomplete = () => resolve()
				transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"))
				transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"))
			})
			this.log("saveToIndexedDB:complete")
		} catch (error) {
			this.handlePersistenceError("Failed to save project items", error)
		}
	}

	private async loadFromIndexedDB() {
		if (!this.persistenceEnabled) {
			this.log("loadFromIndexedDB:skipped:persistence-disabled")
			return
		}
		try {
			this.log("loadFromIndexedDB:start")
			const db = await this.getDatabase()
			const transaction = db.transaction(ProjectTreeView.STORE_NAME, "readonly")
			const store = transaction.objectStore(ProjectTreeView.STORE_NAME)
			const request = store.get(ProjectTreeView.STORE_KEY)
			const result = await this.promisifyRequest<ProjectFile | undefined>(request)
			await new Promise<void>((resolve, reject) => {
				transaction.oncomplete = () => resolve()
				transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"))
				transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"))
			})
			const normalized = normalizeProjectFile(result)
			if (!normalized) {
				this.log("loadFromIndexedDB:no-data")
				return
			}
			this.restoreFromProjectFile(normalized)
			this.log("loadFromIndexedDB:complete")
		} catch (error) {
			this.handlePersistenceError("Failed to load project items", error)
		}
	}

	private promisifyRequest<T>(request: IDBRequest<T>): Promise<T> {
		return new Promise<T>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result)
			request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"))
		})
	}

	private getDatabase(): Promise<IDBDatabase> {
		if (!this.persistenceEnabled) {
			return Promise.reject(new Error("IndexedDB persistence disabled"))
		}
		if (!this.databasePromise) {
			this.databasePromise = new Promise((resolve, reject) => {
				const request = indexedDB.open(ProjectTreeView.DATABASE_NAME, 1)
				request.onerror = () => reject(request.error ?? new Error("Failed to open IndexedDB"))
				request.onsuccess = () => resolve(request.result)
				request.onupgradeneeded = () => {
					const db = request.result
					if (!db.objectStoreNames.contains(ProjectTreeView.STORE_NAME)) {
						db.createObjectStore(ProjectTreeView.STORE_NAME)
					}
				}
			})
		}
		return this.databasePromise
	}

	private buildProjectFile(): ProjectFile {
		const items = this.buildProjectFileEntries(this.items)
		return createProjectFile({
			items,
			selectedPath: this.selectedPath ? this.selectedPath.slice() : null
		})
	}

	private buildProjectFileEntries(nodes: ProjectNode[]): ProjectFileEntry[] {
		return nodes.map((node) => {
			if (isFolder(node)) {
				return {
					kind: "folder",
					name: node.name,
					items: this.buildProjectFileEntries(node.children)
				}
			}
			if (node.type === "schemantic") {
				return {
					type: node.type,
					name: node.name,
					data: node.getState()
				}
			}
			if (node.type === "part") {
				return {
					type: node.type,
					name: node.name,
					data: node.getState()
				}
			}
			return { type: node.type, name: node.name }
		})
	}

	private restoreFromProjectFile(projectFile: ProjectFile) {
		this.isRestoring = true
		try {
			this.items = this.createNodesFromEntries(projectFile.items)
			this.selectedPath = projectFile.selectedPath ? projectFile.selectedPath.slice() : null
			this.renderItems()
			if (this.selectedPath) {
				const selectedNode = this.getNodeByPath(this.selectedPath)
				if (selectedNode) {
					this.itemsListContainer.setSelected(selectedNode)
					if (isProjectItem(selectedNode)) {
						this.onItemSelected?.(selectedNode)
					}
				} else {
					this.selectedPath = null
				}
			}
		} finally {
			this.isRestoring = false
		}
	}

	private createNodesFromEntries(entries: ProjectFileEntry[]): ProjectNode[] {
		const nodes: ProjectNode[] = []
		for (const entry of entries) {
			if (this.isFolderEntry(entry)) {
				const folder = this.createFolder(entry.name, nodes)
				folder.children = this.createNodesFromEntries(entry.items)
				nodes.push(folder)
				continue
			}
			const schemanticState = entry.type === "schemantic" ? entry.data : undefined
			const partState = entry.type === "part" ? entry.data : undefined
			nodes.push(this.createProjectItem(entry.type, entry.name, nodes, schemanticState, partState))
		}
		return nodes
	}

	private isFolderEntry(entry: ProjectFileEntry): entry is ProjectFileFolder {
		return (entry as ProjectFileFolder).kind === "folder"
	}

	private async saveProjectToFile() {
		try {
			const projectFile = this.buildProjectFile()
			const json = serializeProjectFile(projectFile)
			const blob = new Blob([json], { type: PROJECT_FILE_MIME_TYPE })
			const url = URL.createObjectURL(blob)
			const anchor = document.createElement("a")
			const timestamp = new Date().toISOString().replace(/[:.]/g, "-")
			anchor.href = url
			anchor.download = `puppycad-project-${timestamp}.json`
			anchor.style.display = "none"
			document.body?.appendChild(anchor)
			anchor.click()
			anchor.remove()
			URL.revokeObjectURL(url)
		} catch (error) {
			console.error("Failed to export project file", error)
		}
	}

	private async saveProjectToServer() {
		try {
			const projectFile = this.buildProjectFile()
			const response = await fetch("/api/projects", {
				method: "POST",
				headers: {
					"Content-Type": PROJECT_FILE_MIME_TYPE
				},
				body: serializeProjectFile(projectFile)
			})

			if (!response.ok) {
				let errorDetail = ""
				try {
					errorDetail = await response.text()
				} catch {
					// ignore response body parsing issues
				}
				const reason = errorDetail.trim() ? `: ${errorDetail}` : ""
				throw new Error(`Server responded with ${response.status}${reason}`)
			}

			let message = "Project saved on server."
			try {
				const result = (await response.json()) as { fileName?: string } | null
				if (result?.fileName) {
					message = `Project saved on server as ${result.fileName}.`
				}
			} catch {
				// ignore JSON parse issues, best-effort message only
			}

			console.log(message)
			if (typeof window !== "undefined" && typeof window.alert === "function") {
				window.alert(message)
			}
		} catch (error) {
			console.error("Failed to save project to server", error)
			if (typeof window !== "undefined" && typeof window.alert === "function") {
				const description = error instanceof Error ? error.message : "Unknown error"
				window.alert(`Failed to save project to server: ${description}`)
			}
		}
	}

	private handlePersistenceError(message: string, error: unknown) {
		console.error(message, error)
		if (!this.persistenceEnabled) {
			return
		}
		this.persistenceEnabled = false
		this.databasePromise = null
		if (this.persistTimeout !== null) {
			window.clearTimeout(this.persistTimeout)
			this.persistTimeout = null
		}
	}
}

export class ProjectView extends UiComponent<HTMLDivElement> {
	private treeView: ProjectTreeView
	private content: HTMLDivElement
	public constructor() {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "row"
		this.treeView = new ProjectTreeView({
			onClick: (item) => {
				this.content.innerHTML = ""
				this.content.appendChild(item.editor.root)
			}
		})
		this.root.appendChild(this.treeView.root)
		this.content = document.createElement("div")
		this.content.style.flexGrow = "1"
		this.root.appendChild(this.content)
	}
}
