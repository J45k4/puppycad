import { ProjectView } from "./project"
import { Container } from "./ui"

window.onload = () => {
	const body = document.querySelector("body")!
	// body.style.display = "flex"
	// body.style.flexDirection = "column"

	const container = new Container(body)

	const projectView = new ProjectView()

	container.add(projectView)

	// const schemanticEditor = new SchemanticEditor()
	// container.add(schemanticEditor)

	// // Create layout container with canvas and sidebar
	// const container = document.createElement("div")
	// container.style.display = "flex"
	// document.body.appendChild(container)

	// // Project view panel
	// const projectView = document.createElement("div")
	// projectView.style.width = "200px"
	// projectView.style.marginRight = "10px"
	// projectView.style.display = "flex"
	// projectView.style.flexDirection = "column"
	// projectView.style.gap = "8px"
	// const projects = [
	//     { id: "schematic1", label: "Schematic Design" },
	//     { id: "mechanical1", label: "Mechanical Design" }
	// ]
	// projects.forEach(proj => {
	//     const btn = document.createElement("button")
	//     btn.textContent = proj.label
	//     btn.addEventListener("click", () => {
	//         console.log("Load project", proj.id)
	//         // TODO: implement project loading logic
	//     })
	//     projectView.appendChild(btn)
	// })
	// container.appendChild(projectView)

	// // Canvas area
	// const canvas = document.createElement("canvas")
	// canvas.width = 800
	// canvas.height = 600
	// container.appendChild(canvas)

	// // Sidebar component list
	// const sidebar = document.createElement("div")
	// sidebar.style.width = "150px"
	// sidebar.style.marginLeft = "10px"
	// sidebar.style.display = "flex"
	// sidebar.style.flexDirection = "column"
	// sidebar.style.gap = "8px"
	// const available = [{ type: "resistor", label: "Resistor" }]
	// available.forEach(item => {
	// 	const el = document.createElement("div")
	// 	el.textContent = item.label
	// 	el.draggable = true
	// 	el.dataset.compType = item.type
	// 	el.style.padding = "4px"
	// 	el.style.border = "1px solid black"
	// 	el.style.cursor = "grab"
	// 	el.addEventListener("dragstart", ev => {
	// 		ev.dataTransfer.setData("component", item.type)
	// 	})
	// 	sidebar.appendChild(el)
	// })
	// container.appendChild(sidebar)

	// schemanticEditor(canvas)
}
