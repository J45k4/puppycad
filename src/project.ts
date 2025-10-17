import { AssemblyEditor } from "./assembly"
import { createDiagramEditor } from "./diagram"
import { PartEditor } from "./part"
import type { PartEditorState } from "./part"
import { PCBEditor } from "./pcb"
import { SchemanticEditor, type SchemanticEditorState } from "./schemantic"
import { PROJECT_FILE_MIME_TYPE, createProjectFile, normalizeProjectFile, serializeProjectFile } from "./project-file"
import type { ProjectFile, ProjectFileEntry, ProjectFileFolder, ProjectFileType } from "./project-file"
import { ItemList, Modal, UiComponent, TreeList, type TreeNode } from "./ui"

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
	private itemsListContainer: TreeList<ProjectNode>
	private selectedPath: number[] | null = null
	private readonly onItemSelected?: (item: ProjectItem) => void
	private databasePromise: Promise<IDBDatabase> | null = null
	private persistTimeout: number | null = null
	private isRestoring = false
	private persistenceEnabled = typeof indexedDB !== "undefined"
	private nodePaths: Map<ProjectNode, number[]> = new Map()
	private static readonly DATABASE_NAME = "puppycad-project"
	private static readonly STORE_NAME = "projectState"
	private static readonly STORE_KEY = "items"
	private static readonly PERSIST_DEBOUNCE_MS = 200

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
			onClick: (item) => this.handleNodeSelection(item)
		})
		this.root.appendChild(this.itemsListContainer.root)
		if (this.persistenceEnabled) {
			void this.loadFromIndexedDB()
		}
	}

	private renderItems() {
		this.nodePaths.clear()
		const treeItems = this.buildTreeNodes(this.items)
		this.itemsListContainer.setItems(treeItems)
		if (this.selectedPath !== null) {
			const selectedNode = this.getNodeByPath(this.selectedPath)
			if (selectedNode) {
				this.itemsListContainer.setSelected(selectedNode)
			} else {
				this.selectedPath = null
				if (!this.isRestoring) {
					this.schedulePersist()
				}
			}
		}
	}

	private buildTreeNodes(nodes: ProjectNode[], prefix: number[] = []): TreeNode<ProjectNode>[] {
		return nodes.map((node, index) => {
			const path = [...prefix, index]
			this.nodePaths.set(node, path)
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

	private newButtonClicked() {
		this.modal.show()
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
			return
		}
		const selectionChanged = !this.pathsEqual(this.selectedPath, path)
		this.selectedPath = path.slice()
		this.itemsListContainer.setSelected(node)
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
			return
		}
		if (this.persistTimeout !== null) {
			window.clearTimeout(this.persistTimeout)
		}
		this.persistTimeout = window.setTimeout(() => {
			this.persistTimeout = null
			void this.saveToIndexedDB()
		}, ProjectTreeView.PERSIST_DEBOUNCE_MS)
	}

	private async saveToIndexedDB() {
		if (!this.persistenceEnabled) {
			return
		}
		try {
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
		} catch (error) {
			this.handlePersistenceError("Failed to save project items", error)
		}
	}

	private async loadFromIndexedDB() {
		if (!this.persistenceEnabled) {
			return
		}
		try {
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
				return
			}
			this.restoreFromProjectFile(normalized)
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
