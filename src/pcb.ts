import { UiComponent } from "./ui";

export class PCBEditor extends UiComponent<HTMLDivElement> {
	public constructor() {
		super(document.createElement("div"))
		this.root.style.width = "100%"
		this.root.style.height = "100%"
		this.root.style.backgroundColor = "#f0f0f0"
		this.root.style.border = "1px solid #ccc"
		this.root.style.position = "relative"
		this.root.innerHTML = "<h2>PCB Editor</h2>"
	}
}