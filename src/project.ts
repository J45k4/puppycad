import { AssemblyEditor } from "./assembly"
import { DiagramEditor } from "./diagram"
import { PartEditor } from "./part"
import { PCBEditor } from "./pcb"
import { SchemanticEditor } from "./schemantic"
import { ItemList, Modal, UiComponent, TreeList } from "./ui"

export type ProjectFileType = "schemantic" | "pcb" | "part" | "assembly" | "diagram"

export type ProjectItem = {
	type: ProjectFileType
	editor: UiComponent<HTMLDivElement>
}

type PersistedProjectItem = {
	type: ProjectFileType
}

type PersistedProjectState = {
	items: PersistedProjectItem[]
	selectedIndex?: number | null
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

		this.itemsListContainer = new TreeList<ProjectItem>({
			items: [],
			onClick: (item) => this.handleItemSelection(item)
		})
		this.root.appendChild(this.itemsListContainer.root)
		void this.loadFromIndexedDB()
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
				return { type, editor: new DiagramEditor() }
		}
		throw new Error(`Unsupported project item type: ${type}`)
	}

	private schedulePersist() {
		if (this.isRestoring) {
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
		try {
			const db = await this.getDatabase()
			const transaction = db.transaction(ProjectTreeView.STORE_NAME, "readwrite")
			const store = transaction.objectStore(ProjectTreeView.STORE_NAME)
			const state: PersistedProjectState = {
				items: this.items.map((item) => ({ type: item.type })),
				selectedIndex: this.selectedIndex
			}
			store.put(state, ProjectTreeView.STORE_KEY)
			await new Promise<void>((resolve, reject) => {
				transaction.oncomplete = () => resolve()
				transaction.onerror = () => reject(transaction.error)
				transaction.onabort = () => reject(transaction.error)
			})
		} catch (error) {
			console.error("Failed to save project items", error)
		}
	}

	private async loadFromIndexedDB() {
		try {
			const db = await this.getDatabase()
			const transaction = db.transaction(ProjectTreeView.STORE_NAME, "readonly")
			const store = transaction.objectStore(ProjectTreeView.STORE_NAME)
			const request = store.get(ProjectTreeView.STORE_KEY)
			const result = await this.promisifyRequest<PersistedProjectState | undefined>(request)
			await new Promise<void>((resolve, reject) => {
				transaction.oncomplete = () => resolve()
				transaction.onerror = () => reject(transaction.error)
				transaction.onabort = () => reject(transaction.error)
			})
			if (!result) {
				return
			}
			this.isRestoring = true
			this.items = result.items.map((item) => this.createProjectItem(item.type))
			this.selectedIndex = result.selectedIndex ?? null
			this.renderItems()
			if (this.selectedIndex !== null) {
				const selectedItem = this.items[this.selectedIndex]
				if (selectedItem) {
					this.handleItemSelection(selectedItem)
				} else {
					this.selectedIndex = null
				}
			}
		} catch (error) {
			console.error("Failed to load project items", error)
		} finally {
			this.isRestoring = false
		}
	}

	private promisifyRequest<T>(request: IDBRequest<T>): Promise<T> {
		return new Promise<T>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result)
			request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"))
		})
	}

	private getDatabase(): Promise<IDBDatabase> {
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
