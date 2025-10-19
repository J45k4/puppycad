import { EditorCanvas } from "./canvas"
import type { CanvasComponent, EditorCanvasOptions } from "./canvas"
import { SelectGroup, UiComponent } from "./ui"
import type { SchemanticProjectItemData } from "./project-file"

type SchematicComponentData = {
	type?: string
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

export type SchemanticEditorState = SchemanticProjectItemData

type EditorCanvasLike = Pick<EditorCanvas<SchematicComponentData>, "root" | "getComponents" | "getConnections" | "setGridSpacing" | "getGridSpacing">

type SchemanticEditorOptions = {
	initialState?: SchemanticEditorState
	onStateChange?: () => void
	createEditorCanvas?: (options: EditorCanvasOptions<SchematicComponentData>) => EditorCanvasLike
}

export class SchemanticEditor extends UiComponent<HTMLDivElement> {
	private editor: EditorCanvasLike
	private readonly onStateChange?: () => void

	public constructor(options?: SchemanticEditorOptions) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "row"

		this.onStateChange = options?.onStateChange

		const createEditorCanvas = options?.createEditorCanvas ?? ((editorOptions: EditorCanvasOptions<SchematicComponentData>) => new EditorCanvas(editorOptions))

		this.editor = createEditorCanvas({
			initialComponents: options?.initialState?.components ?? [],
			initialConnections: options?.initialState?.connections ?? [],
			gridSpacing: 80,
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
			},
			onComponentsChange: () => this.handleStateChange(),
			onConnectionsChange: () => this.handleStateChange()
		})
		this.root.appendChild(this.editor.root)

		const componentList = new ComponentList()
		this.root.appendChild(componentList.root)
	}

	public createToolbar(): UiComponent<HTMLElement> {
		return new SchemanticToolbar({ editor: this })
	}

	public getState(): SchemanticEditorState {
		return {
			components: this.editor.getComponents(),
			connections: this.editor.getConnections()
		}
	}

	public setGridSpacing(spacing: number): void {
		this.editor.setGridSpacing(spacing)
	}

	public getGridSpacing(): number {
		return this.editor.getGridSpacing()
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

	private handleStateChange() {
		this.onStateChange?.()
	}
}

type SchemanticToolbarOptions = {
	editor: SchemanticEditor
}

class SchemanticToolbar extends UiComponent<HTMLDivElement> {
	public constructor({ editor }: SchemanticToolbarOptions) {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.gap = "16px"
		this.root.style.alignItems = "center"

		const gridSpacingControl = new SelectGroup({
			label: "Grid Spacing",
			value: String(editor.getGridSpacing()),
			options: [
				{ value: "40", text: "Compact (40px)" },
				{ value: "80", text: "Comfortable (80px)" },
				{ value: "120", text: "Spacious (120px)" }
			]
		})
		gridSpacingControl.onChange = (value) => {
			const spacing = Number.parseInt(value, 10)
			if (!Number.isNaN(spacing)) {
				editor.setGridSpacing(spacing)
			}
		}

		this.root.appendChild(gridSpacingControl.root)
	}
}
