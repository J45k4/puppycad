import { AssemblyEditor } from "./assembly";
import { DiagramEditor } from "./diagram";
import { PartEditor } from "./part";
import { PCBEditor } from "./pcb";
import { SchemanticEditor } from "./schemantic";
import { ItemList, Modal, UiComponent, TreeList } from "./ui";

export type ProjectFileType = "schemantic" | "pcb" | "part" | "assembly" | "diagram"

export type ProjectItem = {
	type: ProjectFileType
	editor: UiComponent<HTMLDivElement>
}

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
                                                        break
                                                }
                                                case "pcb": {
                                                        this.items.push({
                                                                type: "pcb",
                                                                editor: new PCBEditor()
                                                        })
                                                        break
                                                }
                                                case "part": {
                                                        this.items.push({
                                                                type: "part",
                                                                editor: new PartEditor()
                                                        })
                                                        break
                                                }
                                                case "assembly": {
                                                        this.items.push({
                                                                type: "assembly",
                                                                editor: new AssemblyEditor()
                                                        })
                                                        break
                                                }
                                                case "diagram": {
                                                        this.items.push({
                                                                type: "diagram",
                                                                editor: new DiagramEditor()
                                                        })
                                                        break
                                                }
                                        }
					this.renderItems()
				},
				items: [
                                        { label: "Schemantic", value: "schemantic" },
                                        { label: "PCB", value: "pcb" },
                                        { label: "Part", value: "part" },
                                        { label: "Assembly", value: "assembly" },
                                        { label: "Diagram", value: "diagram" },
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
				this.content.appendChild(item.editor.root)
			}
		})
		this.root.appendChild(this.treeView.root)
		this.content = document.createElement("div")
		this.content.style.flexGrow = "1"
		this.root.appendChild(this.content)
	}
}