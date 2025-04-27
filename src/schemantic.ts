import { EditorCanvas } from "./canvas"
import { UiComponent } from "./ui"

class ComponentList extends UiComponent<HTMLDivElement> {
    public constructor() {
        super(document.createElement("div"));
        this.root.style.width = "200px";
        this.root.style.display = "flex";
        this.root.style.flexDirection = "column";
        this.root.style.gap = "8px";
        const items = [
            { type: "resistor", label: "Resistor" },
            { type: "capacitor", label: "Capacitor" },
            { type: "ic", label: "IC" }
        ];
        items.forEach(item => {
            const el = document.createElement("div");
            el.textContent = item.label;
            el.draggable = true;
            el.addEventListener("dragstart", event => {
                event.dataTransfer!.setData("component", item.type);
            });
            this.root.appendChild(el);
        });
    }
}

export class SchemanticEditor extends UiComponent<HTMLDivElement> {
	private editor: EditorCanvas

	public constructor() {
		super(document.createElement("div"))
		this.root.style.display = "flex"
		this.root.style.flexDirection = "row"

		this.editor = new EditorCanvas()
		this.root.appendChild(this.editor.root)

		const componentList = new ComponentList()
		this.root.appendChild(componentList.root)
	}
}