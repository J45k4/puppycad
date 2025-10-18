import { ProjectView } from "./project"
import { Container } from "./ui"

window.onload = () => {
	const body = document.body
	if (!body) {
		throw new Error("document.body is not available")
	}
	const container = new Container(body)

	const projectView = new ProjectView()

	container.add(projectView)
}
