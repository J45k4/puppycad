import { AssemblyEditor } from "./assembly"
import { createDiagramEditor } from "./diagram"
import { PartEditor } from "./part"
import { PCBEditor } from "./pcb"
import { SchemanticEditor } from "./schemantic"
import { PROJECT_FILE_MIME_TYPE, createProjectFile, normalizeProjectFile, serializeProjectFile } from "./project-file"
import type { ProjectFile, ProjectFileItem, ProjectFileType } from "./project-file"
import { ItemList, Modal, UiComponent, TreeList } from "./ui"

export type ProjectItem = {
	type: ProjectFileType
	editor: UiComponent<HTMLDivElement>
}

class ProjectTreeView extends UiComponent<HTMLDivElement> {
	private items: ProjectItem[] = []
	private modal: Modal
	private itemsListContainer: TreeList<ProjectItem>
	private selectedIndex: number | null = null
	private readonly onItemSelected?: (item: ProjectItem) => void
	private databasePromise: Promise<IDBDatabase> | null = null
	private persistTimeout: number | null = null
	private isRestoring = false
	private persistenceEnabled = typeof indexedDB !== "undefined"
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
			content: new ItemList<ProjectFileType>({
				onClick: (type) => {
					console.log("createNew", type)
					this.addItem(type)
					this.modal.hide()
				},
				items: [
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

		const saveButton = document.createElement("button")
		saveButton.textContent = "Save"
		saveButton.onclick = () => this.saveProjectToFile()
		this.root.appendChild(saveButton)

		this.itemsListContainer = new TreeList<ProjectItem>({
			items: [],
			onClick: (item) => this.handleItemSelection(item)
		})
		this.root.appendChild(this.itemsListContainer.root)
		if (this.persistenceEnabled) {
			void this.loadFromIndexedDB()
		}
	}

	private renderItems() {
		this.itemsListContainer.setItems(
			this.items.map((item) => ({
				label: item.type,
				value: item
			}))
		)
		if (this.selectedIndex !== null) {
			const selectedItem = this.items[this.selectedIndex]
			if (selectedItem) {
				this.itemsListContainer.setSelected(selectedItem)
			} else {
				this.selectedIndex = null
				this.schedulePersist()
			}
		}
	}

	private newButtonClicked() {
		this.modal.show()
	}

	private addItem(type: ProjectFileType) {
		const item = this.createProjectItem(type)
		this.items.push(item)
		this.renderItems()
		this.handleItemSelection(item)
	}

	private handleItemSelection(item: ProjectItem) {
		const index = this.items.indexOf(item)
		if (index === -1) {
			return
		}
		const selectionChanged = this.selectedIndex !== index
		this.selectedIndex = index
		this.itemsListContainer.setSelected(item)
		this.onItemSelected?.(item)
		if (selectionChanged) {
			this.schedulePersist()
		}
	}

	private createProjectItem(type: ProjectFileType): ProjectItem {
		switch (type) {
			case "schemantic":
				return { type, editor: new SchemanticEditor() }
			case "pcb":
				return { type, editor: new PCBEditor() }
			case "part":
				return { type, editor: new PartEditor() }
			case "assembly":
				return { type, editor: new AssemblyEditor() }
			case "diagram":
				return { type, editor: createDiagramEditor() }
		}
		throw new Error(`Unsupported project item type: ${type}`)
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
		const items: ProjectFileItem[] = this.items.map((item) => ({ type: item.type }))
		return createProjectFile({
			items,
			selectedIndex: this.selectedIndex
		})
	}

	private restoreFromProjectFile(projectFile: ProjectFile) {
		this.isRestoring = true
		try {
			this.items = projectFile.items.map((item) => this.createProjectItem(item.type))
			this.selectedIndex = projectFile.selectedIndex
			if (this.selectedIndex !== null) {
				if (this.selectedIndex < 0 || this.selectedIndex >= this.items.length) {
					this.selectedIndex = null
				}
			}
			this.renderItems()
			if (this.selectedIndex !== null) {
				const selectedItem = this.items[this.selectedIndex]
				if (selectedItem) {
					this.handleItemSelection(selectedItem)
				} else {
					this.selectedIndex = null
				}
			}
		} finally {
			this.isRestoring = false
		}
	}

	private saveProjectToFile() {
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
				console.log("selected item", item)
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
