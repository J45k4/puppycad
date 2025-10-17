import { EditorCanvas } from "./canvas"
import type { CanvasComponent } from "./canvas"
import { UiComponent } from "./ui"

type SchematicComponentData = {
	type: string
}

class ComponentList extends UiComponent<HTMLDivElement> {
	public constructor() {
		super(document.createElement("div"))
		this.root.style.width = "200px"
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.gap = "8px"
		const items = [
			{ type: "resistor", label: "Resistor" },
			{ type: "capacitor", label: "Capacitor" },
			{ type: "ic", label: "IC" }
		]
		for (const item of items) {
			const el = document.createElement("div")
			el.textContent = item.label
			el.draggable = true
			el.addEventListener("dragstart", (event) => {
				const dataTransfer = event.dataTransfer
				if (!dataTransfer) {
					return
				}
				dataTransfer.setData("component", item.type)
			})
			this.root.appendChild(el)
		}
	}
}

export class SchemanticEditor extends UiComponent<HTMLDivElement> {
	private editor: EditorCanvas<SchematicComponentData>

	public constructor() {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "row"

		this.editor = new EditorCanvas<SchematicComponentData>({
			initialComponents: [],
			getComponentLabel: (component) => component.data?.type?.toUpperCase() ?? `C${component.id}`,
			createComponent: (type, position, helpers) => {
				const size = this.getComponentSize(type)
				if (!size) {
					return null
				}
				const id = helpers.createId()
				const x = position.x - size.width / 2
				const y = position.y - size.height / 2
				const component: CanvasComponent<SchematicComponentData> = {
					id,
					x,
					y,
					width: size.width,
					height: size.height,
					data: { type }
				}
				return component
			}
		})
		this.root.appendChild(this.editor.root)

		const componentList = new ComponentList()
		this.root.appendChild(componentList.root)
	}

	private getComponentSize(type: string): { width: number; height: number } | null {
		switch (type) {
			case "resistor":
				return { width: 100, height: 40 }
			case "capacitor":
				return { width: 80, height: 60 }
			case "ic":
				return { width: 120, height: 80 }
			default:
				console.warn(`Unknown schemantic component: ${type}`)
				return null
		}
	}
}
