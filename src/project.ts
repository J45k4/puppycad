import { SchemanticEditor } from "./schemantic";
import { ItemList, Modal, UiComponent, TreeList } from "./ui";

export type ProjectFileType = "schemantic" | "pcb" | "part" | "assembly"

export type SchemanticItem = {
	type: "schemantic"
	editor: SchemanticEditor
}

export type ProjectItem = SchemanticItem



class ProjectTreeView extends UiComponent<HTMLDivElement> {
	private items: ProjectItem[] = []
	private modal: Modal
	private itemsListContainer: TreeList<ProjectItem>

	public constructor(args: {
		onClick?: (item: ProjectItem) => void
	}) {
		super(document.createElement("div"))
		this.modal = new Modal({
			title: "New Item",
			content: new ItemList<ProjectFileType>({
				onClick: (type) => {
					console.log("createNew", type)
					switch (type) {
						case "schemantic": {
							this.items.push({
								type: "schemantic",
								editor: new SchemanticEditor()
							})
							this.renderItems()
							break
						}
						case "pcb": {
							break
						}
						case "part": {
							break
						}
						case "assembly": {
							break
						}
					}
				},
				items: [
					{ label: "Schemantic", value: "schemantic" },
					{ label: "PCB", value: "pcb" },
					{ label: "Mechanical", value: "part" },
					{ label: "Assembly", value: "assembly" },
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
			onClick: (item) => {
				args.onClick?.(item)
				this.itemsListContainer.setSelected(item)
			}
		});
		this.root.appendChild(this.itemsListContainer.root);
	}

	private renderItems() {
		// Rebuild tree list
		this.itemsListContainer.root.remove();
		this.itemsListContainer.setItems(this.items.map(item => {
			return {
				label: item.type,
				value: item,
				onClick: () => {
					// this.items.forEach(i => i.editor.hide());
					// item.editor.show();
				}
			}
		}));
		this.root.appendChild(this.itemsListContainer.root);
	}

	private newButtonClicked() {
		this.modal.show()
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
				if (item.type === "schemantic") {
					console.log("add schemantic editor")
					this.content.appendChild(item.editor.root)
				}
			}
		})
		this.root.appendChild(this.treeView.root)
		this.content = document.createElement("div")
		this.content.style.flexGrow = "1"
		this.root.appendChild(this.content)
	}
}