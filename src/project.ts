import { AssemblyEditor } from "./assembly"
import { createDiagramEditor } from "./diagram"
import { PartEditor } from "./part"
import type { PartEditorState } from "./part"
import { PCBEditor } from "./pcb"
import { SchemanticEditor, type SchemanticEditorState } from "./schemantic"
import { PROJECT_FILE_MIME_TYPE, createProjectFile, normalizeProjectFile, serializeProjectFile } from "./project-file"
import type { ProjectFile, ProjectFileEntry, ProjectFileFolder, ProjectFileType } from "./project-file"
import { DockLayout, type DockOrientation, type DockLayoutState } from "./dock-layout"
import { ItemList, Modal, UiComponent, TreeList, showTextPromptModal, type TreeNode } from "./ui"
import { ProjectList, type ProjectListEntry } from "./project-list"

type BaseProjectItem = {
	type: ProjectFileType
	name: string
	editor: UiComponent<HTMLDivElement>
	toolbar?: UiComponent<HTMLElement>
	paneToolbar?: UiComponent<HTMLElement>
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

type SyntheticProjectEntry =
	| {
			kind: "part-sketch"
			part: PartProjectItem
			sketchIndex: number
			id: string
	  }
	| {
			kind: "part-plane"
			part: PartProjectItem
			plane: "Top" | "Front" | "Right"
			id: string
	  }

class ProjectTreeView extends UiComponent<HTMLDivElement> {
	private items: ProjectNode[] = []
	private modal: Modal
	private projectList: ProjectList
	private selectedPath: number[] | null = null
	private selectedSyntheticId: string | null = null
	private readonly onItemSelected?: (item: ProjectItem) => void
	private readonly onItemsDeleted?: (items: ProjectItem[]) => void
	private persistTimeout: number | null = null
	private isRestoring = false
	private persistenceEnabled = typeof indexedDB !== "undefined"
	private nodePaths: Map<ProjectNode, number[]> = new Map()
	private nodeIdMap: Map<ProjectNode, string> = new Map()
	private idNodeMap: Map<string, ProjectNode> = new Map()
	private readonly syntheticSelectionTargets = new Map<string, ProjectItem>()
	private readonly syntheticEntries = new Map<string, SyntheticProjectEntry>()
	private nextNodeId = 0
	private itemsListContainer: TreeList<ProjectNode>
	private nodeElements: Map<ProjectNode, { header: HTMLDivElement; container?: HTMLDivElement; exitDropZone?: HTMLDivElement }> = new Map()
	private dragState: { node: ProjectNode; path: number[] } | null = null
	private rootDropZone: HTMLDivElement
	private exitDropZone: { element: HTMLDivElement; parent: ProjectFolder } | null = null
	private readonly projectId: string
	public static readonly DATABASE_NAME = "puppycad-project"
	public static readonly STORE_NAME = "projectState"
	public static readonly STORE_KEY_PREFIX = "project:"
	public static readonly LEGACY_STORE_KEY = "items"
	private static readonly PERSIST_DEBOUNCE_MS = 200

	private log(...args: unknown[]): void {
		if (typeof console !== "undefined") {
			console.log("[ProjectTreeView]", `[${this.projectId}]`, ...args)
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

	private getStoreKey(): string {
		return `${ProjectTreeView.STORE_KEY_PREFIX}${this.projectId}`
	}

	public constructor(args: {
		onClick?: (item: ProjectItem) => void
		onItemsDeleted?: (items: ProjectItem[]) => void
		projectId?: string
	}) {
		super(document.createElement("div"))
		this.onItemSelected = args.onClick
		this.onItemsDeleted = args.onItemsDeleted
		this.projectId = args.projectId ?? "default"
		this.root.classList.add("project-tree-panel")
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

		const newButton = document.createElement("button")
		newButton.textContent = "New"
		newButton.classList.add("button", "button--primary")
		newButton.onclick = this.newButtonClicked.bind(this)
		this.root.appendChild(newButton)

		const saveButton = document.createElement("button")
		saveButton.textContent = "Save"
		saveButton.classList.add("button", "button--secondary")
		saveButton.onclick = () => this.saveProjectToFile()
		this.root.appendChild(saveButton)

		const serverSaveButton = document.createElement("button")
		serverSaveButton.textContent = "Save to Server"
		serverSaveButton.classList.add("button", "button--ghost")
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
			onSelect: ({ id }) => this.handleSelectionById(id),
			getActions: ({ id }) => {
				const node = this.idNodeMap.get(id)
				if (node) {
					return [
						{
							label: "Rename",
							onSelect: () => {
								void this.renameNode(node)
							}
						},
						{
							label: "Delete",
							onSelect: () => {
								void this.deleteNode(node)
							}
						}
					]
				}
				const syntheticEntry = this.syntheticEntries.get(id)
				if (syntheticEntry?.kind === "part-sketch") {
					return [
						{
							label: "Edit",
							onSelect: () => {
								syntheticEntry.part.editor.enterSketchMode()
								this.onItemSelected?.(syntheticEntry.part)
							}
						},
						{
							label: "Rename",
							onSelect: () => {
								void this.renamePartSketch(syntheticEntry)
							}
						},
						{
							label: "Delete",
							onSelect: () => {
								void this.deletePartSketch(syntheticEntry)
							}
						}
					]
				}
				return []
			}
		})
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
		dropZone.classList.add("project-tree-drop-zone")
		dropZone.textContent = "Drop here to move to root"
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
		this.syntheticSelectionTargets.clear()
		this.syntheticEntries.clear()
		const collapsedIds = this.projectList.getCollapsedFolderIds()
		const treeItems = this.buildTreeNodes(this.items)
		this.itemsListContainer.setItems(treeItems)
		const listEntries = this.buildProjectListEntries(this.items)
		let selectedId: string | null = null
		if (this.selectedSyntheticId) {
			const syntheticEntry = this.syntheticEntries.get(this.selectedSyntheticId)
			if (syntheticEntry) {
				const path = this.nodePaths.get(syntheticEntry.part)
				if (path) {
					this.selectedPath = path.slice()
					this.itemsListContainer.setSelected(syntheticEntry.part)
					selectedId = this.selectedSyntheticId
				} else {
					this.selectedSyntheticId = null
				}
			} else {
				this.selectedSyntheticId = null
			}
		}
		if (this.selectedPath !== null) {
			const selectedNode = this.getNodeByPath(this.selectedPath)
			if (selectedNode) {
				this.itemsListContainer.setSelected(selectedNode)
				if (selectedId === null) {
					selectedId = this.getNodeId(selectedNode)
				}
			} else {
				this.selectedPath = null
				this.selectedSyntheticId = null
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
			if (node.type === "part") {
				const state = node.getState()
				const childItems: ProjectListEntry[] = []
				const addPlaneChild = (suffix: string, name: "Top" | "Front" | "Right") => {
					const childId = `${id}:${suffix}`
					this.syntheticSelectionTargets.set(childId, node)
					this.syntheticEntries.set(childId, {
						kind: "part-plane",
						part: node,
						plane: name,
						id: childId
					})
					childItems.push({
						kind: "file" as const,
						id: childId,
						name,
						metadata: { draggable: false, synthetic: true }
					})
				}
				const addSketchChild = (sketchIndex: number, name: string) => {
					const childId = `${id}:sketch-${sketchIndex}`
					this.syntheticSelectionTargets.set(childId, node)
					this.syntheticEntries.set(childId, {
						kind: "part-sketch",
						part: node,
						sketchIndex,
						id: childId
					})
					childItems.push({
						kind: "file" as const,
						id: childId,
						name,
						metadata: { draggable: false, synthetic: true }
					})
				}
				const addSyntheticChild = (suffix: string, name: string) => {
					const childId = `${id}:${suffix}`
					this.syntheticSelectionTargets.set(childId, node)
					childItems.push({
						kind: "file" as const,
						id: childId,
						name,
						metadata: { draggable: false, synthetic: true }
					})
				}
				addPlaneChild("plane-top", "Top")
				addPlaneChild("plane-front", "Front")
				addPlaneChild("plane-right", "Right")
				if (state.sketchPoints.length > 0) {
					const sketchName = state.sketchName?.trim() || node.editor.getSketchName() || "Sketch 1"
					addSketchChild(1, sketchName)
				}
				if (state.extrudedModel) {
					addSyntheticChild("extrude-1", `Extrude 1 (${state.extrudedModel.rawHeight.toFixed(1)}u)`)
				}
				return {
					kind: "folder" as const,
					id,
					name: node.name,
					items: childItems
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
		if (node) {
			this.selectedSyntheticId = null
			this.handleNodeSelection(node)
			return
		}
		const syntheticEntry = this.syntheticEntries.get(id)
		if (syntheticEntry) {
			const path = this.nodePaths.get(syntheticEntry.part)
			if (!path) {
				return
			}
			const selectionChanged = !this.pathsEqual(this.selectedPath, path) || this.selectedSyntheticId !== id
			this.selectedPath = path.slice()
			this.selectedSyntheticId = id
			this.itemsListContainer.setSelected(syntheticEntry.part)
			if (syntheticEntry.kind === "part-sketch") {
				syntheticEntry.part.editor.enterSketchMode()
			} else {
				syntheticEntry.part.editor.selectReferencePlane(syntheticEntry.plane)
			}
			this.onItemSelected?.(syntheticEntry.part)
			if (selectionChanged) {
				this.schedulePersist()
			}
			return
		}
		const syntheticTarget = this.syntheticSelectionTargets.get(id)
		if (!syntheticTarget) {
			return
		}
		this.handleNodeSelection(syntheticTarget)
	}

	public getProjectItemByNodeId(id: string): ProjectItem | null {
		const node = this.idNodeMap.get(id)
		if (node && isProjectItem(node)) {
			return node
		}
		return this.syntheticSelectionTargets.get(id) ?? null
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
		dropZone.className = "project-tree-drop-zone project-tree-exit-drop-zone"
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
			const nodeId = this.getNodeId(node)
			event.dataTransfer.setData("text/plain", nodeId)
			event.dataTransfer.setData("application/x-puppycad-project-node", nodeId)
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
		this.rootDropZone.classList.add("project-tree-drop-zone--active")
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
		dropZone.classList.remove("project-tree-drop-zone--active")
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
		element.classList.remove("project-tree-drop-zone--active")
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
			this.exitDropZone.element.classList.add("project-tree-drop-zone--active")
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
			this.exitDropZone.element.classList.add("project-tree-drop-zone--active")
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
		element.classList.remove("project-tree-drop-zone--active")
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
		header.classList.toggle("project-folder__title--droppable", isActive)
	}

	private clearDropHighlights(): void {
		for (const node of this.nodePaths.keys()) {
			if (isFolder(node)) {
				const header = this.getNodeHeader(node)
				if (header) {
					header.classList.remove("project-folder__title--droppable")
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
		this.rootDropZone.classList.remove("project-tree-drop-zone--active")
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
		void this.renameNode(currentItem)
	}

	private async renameNode(node: ProjectNode) {
		const path = this.nodePaths.get(node)
		if (!path) {
			return
		}
		const siblings = this.getSiblingsForPath(path)
		if (!siblings) {
			return
		}
		if (typeof window === "undefined") {
			return
		}
		const newName = await showTextPromptModal({
			title: "Rename Item",
			initialValue: node.name,
			confirmText: "Save",
			cancelText: "Cancel"
		})
		if (!newName) {
			return
		}
		const trimmed = newName.trim()
		if (!trimmed || trimmed === node.name) {
			return
		}
		if (this.isNameTaken(trimmed, siblings, node)) {
			if (typeof window !== "undefined" && typeof window.alert === "function") {
				window.alert("An item with that name already exists.")
			}
			return
		}
		node.name = trimmed
		this.selectedPath = path.slice()
		this.renderItems()
		this.schedulePersist()
	}

	private async deleteNode(node: ProjectNode) {
		const path = this.nodePaths.get(node)
		if (!path) {
			return
		}
		const shouldDelete = typeof window === "undefined" || typeof window.confirm !== "function" ? true : window.confirm(`Delete ${isFolder(node) ? "folder" : "item"} "${node.name}"?`)
		if (!shouldDelete) {
			return
		}
		const removed = this.removeNodeAtPath(path)
		if (!removed) {
			return
		}
		const removedItems = this.collectProjectItems(removed)
		this.selectedPath = null
		this.renderItems()
		this.projectList.selectById(null)
		this.onItemsDeleted?.(removedItems)
		this.schedulePersist()
	}

	private collectProjectItems(node: ProjectNode): ProjectItem[] {
		if (isProjectItem(node)) {
			return [node]
		}
		const items: ProjectItem[] = []
		for (const child of node.children) {
			items.push(...this.collectProjectItems(child))
		}
		return items
	}

	private async renamePartSketch(entry: Extract<SyntheticProjectEntry, { kind: "part-sketch" }>) {
		if (typeof window === "undefined") {
			return
		}
		const currentName = entry.part.editor.getSketchName()
		const nextName = await showTextPromptModal({
			title: "Rename Sketch",
			initialValue: currentName,
			confirmText: "Save",
			cancelText: "Cancel"
		})
		if (!nextName) {
			return
		}
		const trimmed = nextName.trim()
		if (!trimmed) {
			return
		}
		entry.part.editor.setSketchName(trimmed)
		this.renderItems()
		this.handleSelectionById(entry.id)
		this.schedulePersist()
	}

	private async deletePartSketch(entry: Extract<SyntheticProjectEntry, { kind: "part-sketch" }>) {
		const confirmed = typeof window === "undefined" || typeof window.confirm !== "function" ? true : window.confirm(`Delete sketch "${entry.part.editor.getSketchName()}"?`)
		if (!confirmed) {
			return
		}
		entry.part.editor.deleteSketch()
		this.renderItems()
		this.handleNodeSelection(entry.part)
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
		this.selectedSyntheticId = null
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
					onStateChange: () => {
						this.schedulePersist()
						this.renderItems()
					}
				})
				return {
					type,
					name: resolvedName,
					editor,
					paneToolbar: editor.createPaneToolbar(),
					getState: () => editor.getState()
				}
			}
			case "assembly":
				return { type, name: resolvedName, editor: new AssemblyEditor() }
			case "diagram": {
				const editor = createDiagramEditor()
				return {
					type,
					name: resolvedName,
					editor,
					toolbar: editor.createToolbar()
				}
			}
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
			const storeKey = this.getStoreKey()
			this.log("saveToIndexedDB:start", { storeKey })
			const db = await this.getDatabase()
			const transaction = db.transaction(ProjectTreeView.STORE_NAME, "readwrite")
			const store = transaction.objectStore(ProjectTreeView.STORE_NAME)
			const state = this.buildProjectFile()
			store.put(state, storeKey)
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
			const storeKey = this.getStoreKey()
			this.log("loadFromIndexedDB:start", { storeKey })
			const db = await this.getDatabase()
			const transaction = db.transaction(ProjectTreeView.STORE_NAME, "readonly")
			const store = transaction.objectStore(ProjectTreeView.STORE_NAME)
			const request = store.get(storeKey)
			let result = await this.promisifyRequest<ProjectFile | undefined>(request)
			if (!result && this.projectId === "default") {
				this.log("loadFromIndexedDB:legacy-fallback")
				const legacyRequest = store.get(ProjectTreeView.LEGACY_STORE_KEY)
				result = await this.promisifyRequest<ProjectFile | undefined>(legacyRequest)
			}
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
		return openProjectDatabase()
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
		if (this.persistTimeout !== null) {
			window.clearTimeout(this.persistTimeout)
			this.persistTimeout = null
		}
	}
}

let sharedDatabasePromise: Promise<IDBDatabase> | null = null

async function openProjectDatabase(): Promise<IDBDatabase> {
	if (typeof indexedDB === "undefined") {
		return Promise.reject(new Error("IndexedDB is not available"))
	}
	if (!sharedDatabasePromise) {
		const databasePromise = new Promise<IDBDatabase>((resolve, reject) => {
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
		sharedDatabasePromise = databasePromise.catch((error) => {
			sharedDatabasePromise = null
			throw error
		})
	}
	if (!sharedDatabasePromise) {
		throw new Error("Failed to initialize project database")
	}
	return sharedDatabasePromise
}

export async function deleteProjectState(projectId: string): Promise<void> {
	if (typeof indexedDB === "undefined") {
		return
	}
	try {
		const db = await openProjectDatabase()
		await new Promise<void>((resolve, reject) => {
			const transaction = db.transaction(ProjectTreeView.STORE_NAME, "readwrite")
			const store = transaction.objectStore(ProjectTreeView.STORE_NAME)
			store.delete(`${ProjectTreeView.STORE_KEY_PREFIX}${projectId}`)
			if (projectId === "default") {
				store.delete(ProjectTreeView.LEGACY_STORE_KEY)
			}
			transaction.oncomplete = () => resolve()
			transaction.onerror = () => reject(transaction.error ?? new Error("IndexedDB transaction failed"))
			transaction.onabort = () => reject(transaction.error ?? new Error("IndexedDB transaction aborted"))
		})
	} catch (error) {
		console.error("Failed to delete project state", error)
	}
}

export class ProjectView extends UiComponent<HTMLDivElement> {
	private treeView: ProjectTreeView
	private content: HTMLDivElement
	private toolbarContainer: HTMLDivElement
	private dockLayout: DockLayout
	private readonly layoutStorageKey: string
	private titleElement: HTMLHeadingElement
	private projectName: string
	private readonly onBack: () => void
	private readonly onRename?: (name: string) => Promise<string | null> | string | null
	private activeToolbar: UiComponent<HTMLElement> | null = null
	private readonly paneItems: Map<string, ProjectItem | null> = new Map()
	private static readonly EMPTY_PANE_MESSAGE = "Select a file from the project tree to open it in this pane."
	private static readonly LAYOUT_STORAGE_PREFIX = "projectLayout:"

	private static getLayoutStorageKey(projectId: string): string {
		return `${ProjectView.LAYOUT_STORAGE_PREFIX}${projectId}`
	}

	public constructor(args: {
		projectId: string
		projectName: string
		onBack: () => void
		onRename?: (name: string) => Promise<string | null> | string | null
	}) {
		super(document.createElement("div"))
		this.projectName = args.projectName
		this.onBack = args.onBack
		this.onRename = args.onRename
		this.layoutStorageKey = ProjectView.getLayoutStorageKey(args.projectId)

		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.width = "100%"
		this.root.style.height = "100%"

		const header = document.createElement("div")
		header.style.display = "flex"
		header.style.alignItems = "center"
		header.style.flexWrap = "wrap"
		header.style.gap = "8px"
		header.style.padding = "12px"
		header.style.borderBottom = "1px solid #ccc"

		const backButton = document.createElement("button")
		backButton.textContent = "Back to Projects"
		backButton.onclick = () => {
			this.onBack()
		}
		header.appendChild(backButton)

		const layoutControls = document.createElement("div")
		layoutControls.style.display = "flex"
		layoutControls.style.alignItems = "center"
		layoutControls.style.gap = "8px"
		layoutControls.style.flexWrap = "wrap"
		layoutControls.style.padding = "0"
		layoutControls.style.borderBottom = "none"
		layoutControls.style.backgroundColor = "transparent"

		const layoutLabel = document.createElement("span")
		layoutLabel.textContent = "Layout"
		layoutLabel.style.fontWeight = "600"
		layoutControls.appendChild(layoutLabel)

		const splitHorizontalButton = this.createSplitButton("horizontal")
		layoutControls.appendChild(splitHorizontalButton)

		const splitVerticalButton = this.createSplitButton("vertical")
		layoutControls.appendChild(splitVerticalButton)

		this.toolbarContainer = document.createElement("div")
		this.toolbarContainer.style.display = "none"
		this.toolbarContainer.style.marginLeft = "12px"
		this.toolbarContainer.style.gap = "12px"
		this.toolbarContainer.style.alignItems = "center"
		this.toolbarContainer.style.boxSizing = "border-box"
		layoutControls.appendChild(this.toolbarContainer)
		header.appendChild(layoutControls)

		this.titleElement = document.createElement("h1")
		this.titleElement.textContent = this.projectName
		this.titleElement.style.fontSize = "1.2rem"
		this.titleElement.style.margin = "0"
		this.titleElement.style.flexGrow = "1"
		header.appendChild(this.titleElement)

		const renameButton = document.createElement("button")
		renameButton.textContent = "Rename Project"
		renameButton.onclick = () => {
			void this.requestRename()
		}
		header.appendChild(renameButton)

		this.root.appendChild(header)

		const main = document.createElement("div")
		main.style.display = "flex"
		main.style.flexDirection = "row"
		main.style.flexGrow = "1"
		main.style.minHeight = "0"

		this.treeView = new ProjectTreeView({
			projectId: args.projectId,
			onClick: (item) => {
				this.showProjectItem(item)
			},
			onItemsDeleted: (items) => {
				this.handleItemsDeleted(items)
			}
		})
		main.appendChild(this.treeView.root)

		this.content = document.createElement("div")
		this.content.style.display = "flex"
		this.content.style.flexDirection = "column"
		this.content.style.flexGrow = "1"
		this.content.style.minHeight = "0"

		this.dockLayout = new DockLayout()
		this.dockLayout.onActivePaneChange = (paneId) => {
			this.handleActivePaneChange(paneId)
		}
		this.dockLayout.onPaneClosed = (paneId, nextActivePaneId) => {
			this.handlePaneClosed(paneId, nextActivePaneId)
		}
		this.dockLayout.canAcceptExternalDrop = (event) => {
			const dataTransfer = event.dataTransfer
			if (!dataTransfer) {
				return false
			}
			const types = Array.from(dataTransfer.types ?? [])
			if (types.includes("application/x-puppycad-project-node")) {
				return true
			}
			if (types.includes("text/plain")) {
				return true
			}
			// Some environments don't expose drag types during dragover; allow hover indicator
			// and validate the payload on drop.
			return types.length === 0
		}
		this.dockLayout.onExternalDrop = ({ paneId, position, event }) => {
			const item = this.getDraggedProjectItem(event)
			if (!item) {
				return
			}
			this.openDraggedItemInPane(item, paneId, position)
		}

		const restoredLayout = this.restoreDockLayoutState()

		this.ensureExistingPanesRegistered()

		const initialPaneId = this.dockLayout.getActivePaneId()
		if (initialPaneId) {
			this.ensurePaneRegistration(initialPaneId)
			this.updateToolbarForPane(initialPaneId)
		}

		if (!restoredLayout) {
			this.persistLayoutState()
		}

		this.content.appendChild(this.dockLayout.root)
		main.appendChild(this.content)

		this.root.appendChild(main)
	}

	private showProjectItem(item: ProjectItem) {
		const activePaneId = this.dockLayout.getActivePaneId()
		if (!activePaneId) {
			return
		}
		this.openItemInPane(activePaneId, item)
		this.dockLayout.setActivePane(activePaneId)
	}

	private getDraggedProjectItem(event: DragEvent): ProjectItem | null {
		const dataTransfer = event.dataTransfer
		if (!dataTransfer) {
			return null
		}
		const nodeId = dataTransfer.getData("application/x-puppycad-project-node") || dataTransfer.getData("text/plain")
		if (!nodeId) {
			return null
		}
		return this.treeView.getProjectItemByNodeId(nodeId)
	}

	private findPaneForItem(item: ProjectItem): string | null {
		for (const [paneId, paneItem] of this.paneItems) {
			if (paneItem === item) {
				return paneId
			}
		}
		return null
	}

	private clearPaneAssignment(paneId: string): void {
		this.paneItems.set(paneId, null)
		this.dockLayout.clearPane(paneId)
		this.dockLayout.setPaneTitle(paneId, "Empty Pane")
		this.dockLayout.setPaneHeaderToolbar(paneId, null)
	}

	private openItemInPane(paneId: string, item: ProjectItem): void {
		this.ensurePaneRegistration(paneId)
		const previousPaneId = this.findPaneForItem(item)
		if (previousPaneId && previousPaneId !== paneId) {
			this.clearPaneAssignment(previousPaneId)
		}
		this.dockLayout.setPaneContent(paneId, item.editor)
		this.dockLayout.setPaneTitle(paneId, item.name)
		this.dockLayout.setPaneHeaderToolbar(paneId, item.paneToolbar ?? null)
		this.paneItems.set(paneId, item)
		this.updateToolbarForPane(this.dockLayout.getActivePaneId())
		this.persistLayoutState()
	}

	private openDraggedItemInPane(item: ProjectItem, paneId: string | null, position: "top" | "bottom" | "center" | "left" | "right"): void {
		if (!paneId) {
			const activePaneId = this.dockLayout.getActivePaneId()
			if (!activePaneId) {
				return
			}
			if (position === "center") {
				this.openItemInPane(activePaneId, item)
				this.dockLayout.setActivePane(activePaneId)
				return
			}
			if (this.tryOpenDraggedItemAsFloatingSplit(item, activePaneId, position)) {
				return
			}
			const orientation = position === "left" || position === "right" ? "horizontal" : "vertical"
			const splitPaneId = this.dockLayout.splitPane(activePaneId, orientation)
			if (!splitPaneId) {
				this.openItemInPane(activePaneId, item)
				this.dockLayout.setActivePane(activePaneId)
				return
			}
			this.ensurePaneRegistration(splitPaneId)
			this.openItemInPane(splitPaneId, item)
			this.dockLayout.movePane(splitPaneId, null, position)
			this.dockLayout.setActivePane(splitPaneId)
			return
		}

		if (position === "center") {
			this.openItemInPane(paneId, item)
			this.dockLayout.setActivePane(paneId)
			return
		}

		if (this.tryOpenDraggedItemAsFloatingSplit(item, paneId, position)) {
			return
		}

		const existing = this.paneItems.get(paneId) ?? null
		this.dockLayout.setActivePane(paneId)
		const orientation = position === "left" || position === "right" ? "horizontal" : "vertical"
		const splitPaneId = this.dockLayout.splitPane(paneId, orientation)
		if (!splitPaneId) {
			this.openItemInPane(paneId, item)
			this.dockLayout.setActivePane(paneId)
			return
		}
		this.ensurePaneRegistration(splitPaneId)

		if (position === "top" || position === "left") {
			if (existing) {
				this.openItemInPane(splitPaneId, existing)
			}
			this.openItemInPane(paneId, item)
			this.dockLayout.setActivePane(paneId)
			return
		}

		this.openItemInPane(splitPaneId, item)
		if (existing) {
			this.openItemInPane(paneId, existing)
		}
		this.dockLayout.setActivePane(splitPaneId)
	}

	private tryOpenDraggedItemAsFloatingSplit(item: ProjectItem, targetPaneId: string, position: "top" | "bottom" | "left" | "right"): boolean {
		if (!this.dockLayout.isPaneFloating(targetPaneId)) {
			return false
		}
		const targetBounds = this.getPaneBoundsRelativeToDockRoot(targetPaneId)
		this.dockLayout.setPaneFloating(targetPaneId, false)
		this.dockLayout.setActivePane(targetPaneId)
		const orientation = position === "left" || position === "right" ? "horizontal" : "vertical"
		const splitPaneId = this.dockLayout.splitPane(targetPaneId, orientation)
		if (!splitPaneId) {
			this.openItemInPane(targetPaneId, item)
			this.dockLayout.setPaneFloating(targetPaneId, true)
			this.dockLayout.setActivePane(targetPaneId)
			if (targetBounds) {
				this.setPaneFloatingBounds(targetPaneId, targetBounds)
			}
			return true
		}
		this.ensurePaneRegistration(splitPaneId)
		this.openItemInPane(splitPaneId, item)
		this.dockLayout.movePane(splitPaneId, targetPaneId, position)
		this.dockLayout.setPaneFloating(targetPaneId, true)
		this.dockLayout.setPaneFloating(splitPaneId, true)
		if (targetBounds) {
			this.setPaneFloatingBounds(targetPaneId, targetBounds)
			const floatingSplitBounds = this.computeFloatingSplitBounds(targetBounds, position)
			this.setPaneFloatingBounds(splitPaneId, floatingSplitBounds)
		}
		this.dockLayout.setActivePane(splitPaneId)
		return true
	}

	private getPaneBoundsRelativeToDockRoot(paneId: string): { left: number; top: number; width: number; height: number } | null {
		const paneElement = this.dockLayout.root.querySelector(`[data-pane-id="${paneId}"]`) as HTMLDivElement | null
		if (!paneElement) {
			return null
		}
		const rootRect = this.dockLayout.root.getBoundingClientRect()
		const paneRect = paneElement.getBoundingClientRect()
		return {
			left: paneRect.left - rootRect.left,
			top: paneRect.top - rootRect.top,
			width: paneRect.width,
			height: paneRect.height
		}
	}

	private setPaneFloatingBounds(paneId: string, bounds: { left: number; top: number; width: number; height: number }): void {
		const paneElement = this.dockLayout.root.querySelector(`[data-pane-id="${paneId}"]`) as HTMLDivElement | null
		if (!paneElement) {
			return
		}
		const rootRect = this.dockLayout.root.getBoundingClientRect()
		const minWidth = 240
		const minHeight = 160
		const width = Math.max(Math.min(bounds.width, Math.max(rootRect.width - 8, minWidth)), minWidth)
		const height = Math.max(Math.min(bounds.height, Math.max(rootRect.height - 8, minHeight)), minHeight)
		const maxLeft = Math.max(rootRect.width - width, 0)
		const maxTop = Math.max(rootRect.height - height, 0)
		const left = Math.max(0, Math.min(bounds.left, maxLeft))
		const top = Math.max(0, Math.min(bounds.top, maxTop))
		paneElement.style.left = `${Math.round(left)}px`
		paneElement.style.top = `${Math.round(top)}px`
		paneElement.style.width = `${Math.round(width)}px`
		paneElement.style.height = `${Math.round(height)}px`
	}

	private computeFloatingSplitBounds(
		targetBounds: { left: number; top: number; width: number; height: number },
		position: "top" | "bottom" | "left" | "right"
	): { left: number; top: number; width: number; height: number } {
		const gap = 16
		const width = Math.max(targetBounds.width, 320)
		const height = Math.max(targetBounds.height, 220)
		switch (position) {
			case "left":
				return {
					left: targetBounds.left - width - gap,
					top: targetBounds.top,
					width,
					height
				}
			case "right":
				return {
					left: targetBounds.left + targetBounds.width + gap,
					top: targetBounds.top,
					width,
					height
				}
			case "top":
				return {
					left: targetBounds.left,
					top: targetBounds.top - height - gap,
					width,
					height
				}
			default:
				return {
					left: targetBounds.left,
					top: targetBounds.top + targetBounds.height + gap,
					width,
					height
				}
		}
	}

	private setToolbar(toolbar: UiComponent<HTMLElement> | null) {
		if (this.activeToolbar === toolbar && this.toolbarContainer.childElementCount > 0) {
			return
		}
		this.toolbarContainer.innerHTML = ""
		this.activeToolbar = toolbar
		if (!toolbar) {
			this.toolbarContainer.style.display = "none"
			return
		}
		this.toolbarContainer.style.display = "flex"
		this.toolbarContainer.appendChild(toolbar.root)
	}

	private handleSplitRequest(orientation: DockOrientation) {
		const activePaneId = this.dockLayout.getActivePaneId()
		if (!activePaneId) {
			return
		}

		const newPaneId = this.dockLayout.splitPane(activePaneId, orientation)
		if (!newPaneId) {
			return
		}

		this.ensurePaneRegistration(newPaneId)
		this.updateToolbarForPane(newPaneId)
	}

	private handleActivePaneChange(paneId: string) {
		this.ensurePaneRegistration(paneId)
		this.updateToolbarForPane(paneId)
		this.persistLayoutState()
	}

	private handlePaneClosed(paneId: string, nextActivePaneId: string | null) {
		this.paneItems.delete(paneId)

		if (nextActivePaneId) {
			this.ensurePaneRegistration(nextActivePaneId)
			this.updateToolbarForPane(nextActivePaneId)
		} else {
			this.updateToolbarForPane(null)
		}

		this.persistLayoutState()
	}

	private handleItemsDeleted(items: ProjectItem[]) {
		if (items.length === 0) {
			return
		}
		const deletedSet = new Set(items)
		let didClearAnyPane = false
		for (const [paneId, paneItem] of this.paneItems) {
			if (!paneItem || !deletedSet.has(paneItem)) {
				continue
			}
			this.clearPaneAssignment(paneId)
			didClearAnyPane = true
		}
		if (!didClearAnyPane) {
			return
		}
		this.updateToolbarForPane(this.dockLayout.getActivePaneId())
		this.persistLayoutState()
	}

	private ensurePaneRegistration(paneId: string) {
		if (!this.paneItems.has(paneId)) {
			this.paneItems.set(paneId, null)
			this.dockLayout.setPaneTitle(paneId, "Empty Pane")
			this.dockLayout.setPanePlaceholder(paneId, ProjectView.EMPTY_PANE_MESSAGE)
		}
	}

	private ensureExistingPanesRegistered() {
		for (const paneId of this.dockLayout.getPaneIds()) {
			this.ensurePaneRegistration(paneId)
		}
	}

	private persistLayoutState() {
		if (typeof window === "undefined") {
			return
		}

		try {
			const serialized = JSON.stringify(this.dockLayout.getState())
			window.localStorage?.setItem(this.layoutStorageKey, serialized)
		} catch (error) {
			console.error("Failed to persist dock layout state", error)
		}
	}

	private restoreDockLayoutState(): boolean {
		if (typeof window === "undefined") {
			return false
		}

		try {
			const stored = window.localStorage?.getItem(this.layoutStorageKey)
			if (!stored) {
				return false
			}

			const parsed = JSON.parse(stored) as DockLayoutState
			if (!parsed || typeof parsed !== "object" || !("root" in parsed)) {
				return false
			}

			this.dockLayout.restoreState(parsed)
			return true
		} catch (error) {
			console.error("Failed to restore dock layout state", error)
			try {
				window.localStorage?.removeItem(this.layoutStorageKey)
			} catch (cleanupError) {
				console.error("Failed to clear invalid dock layout state", cleanupError)
			}
			return false
		}
	}

	private createSplitButton(orientation: DockOrientation): HTMLButtonElement {
		const button = document.createElement("button")
		const label = orientation === "horizontal" ? "Split horizontally" : "Split vertically"
		const splitDirection: DockOrientation = orientation === "horizontal" ? "vertical" : "horizontal"
		const defaultBackground = "#ffffff"
		const hoverBackground = "#f1f5f9"
		const activeBackground = "#e2e8f0"
		const defaultBorder = "#cbd5f5"
		const hoverBorder = "#94a3b8"

		button.type = "button"
		button.title = label
		button.setAttribute("aria-label", label)
		button.style.display = "flex"
		button.style.alignItems = "center"
		button.style.justifyContent = "center"
		button.style.width = "32px"
		button.style.height = "32px"
		button.style.border = `1px solid ${defaultBorder}`
		button.style.borderRadius = "6px"
		button.style.backgroundColor = defaultBackground
		button.style.cursor = "pointer"
		button.style.padding = "4px"
		button.style.transition = "background-color 120ms ease, border-color 120ms ease"

		button.appendChild(this.createSplitIcon(orientation))

		button.addEventListener("mouseenter", () => {
			button.style.backgroundColor = hoverBackground
			button.style.borderColor = hoverBorder
		})
		button.addEventListener("mouseleave", () => {
			button.style.backgroundColor = defaultBackground
			button.style.borderColor = defaultBorder
		})
		button.addEventListener("mousedown", () => {
			button.style.backgroundColor = activeBackground
		})
		button.addEventListener("mouseup", () => {
			button.style.backgroundColor = hoverBackground
		})
		button.addEventListener("blur", () => {
			button.style.backgroundColor = defaultBackground
			button.style.borderColor = defaultBorder
		})

		button.onclick = () => {
			this.handleSplitRequest(splitDirection)
		}

		return button
	}

	private createSplitIcon(orientation: DockOrientation): SVGSVGElement {
		const svgNamespace = "http://www.w3.org/2000/svg"
		const svg = document.createElementNS(svgNamespace, "svg")
		svg.setAttribute("viewBox", "0 0 20 20")
		svg.setAttribute("width", "20")
		svg.setAttribute("height", "20")

		const frame = document.createElementNS(svgNamespace, "rect")
		frame.setAttribute("x", "3")
		frame.setAttribute("y", "3")
		frame.setAttribute("width", "14")
		frame.setAttribute("height", "14")
		frame.setAttribute("rx", "2")
		frame.setAttribute("ry", "2")
		frame.setAttribute("fill", "none")
		frame.setAttribute("stroke", "#475569")
		frame.setAttribute("stroke-width", "1.5")
		svg.appendChild(frame)

		const divider = document.createElementNS(svgNamespace, "line")
		if (orientation === "horizontal") {
			divider.setAttribute("x1", "3")
			divider.setAttribute("y1", "10")
			divider.setAttribute("x2", "17")
			divider.setAttribute("y2", "10")
		} else {
			divider.setAttribute("x1", "10")
			divider.setAttribute("y1", "3")
			divider.setAttribute("x2", "10")
			divider.setAttribute("y2", "17")
		}
		divider.setAttribute("stroke", "#475569")
		divider.setAttribute("stroke-width", "1.5")
		divider.setAttribute("stroke-linecap", "round")
		svg.appendChild(divider)

		return svg
	}

	private updateToolbarForPane(paneId: string | null) {
		if (!paneId) {
			this.setToolbar(null)
			return
		}

		const item = this.paneItems.get(paneId) ?? null
		this.setToolbar(item?.toolbar ?? null)
	}

	public setProjectName(name: string) {
		this.projectName = name
		this.titleElement.textContent = name
	}

	private async requestRename() {
		if (typeof window === "undefined") {
			return
		}
		const input = await showTextPromptModal({
			title: "Rename Project",
			initialValue: this.projectName,
			confirmText: "Save",
			cancelText: "Cancel"
		})
		if (input === null || input === undefined) {
			return
		}
		const trimmed = input.trim()
		if (!trimmed) {
			return
		}
		try {
			const result = this.onRename ? await this.onRename(trimmed) : trimmed
			const trimmedResult = result?.trim()
			if (trimmedResult) {
				this.setProjectName(trimmedResult)
			}
		} catch (error) {
			console.error("Failed to rename project", error)
		}
	}
}
