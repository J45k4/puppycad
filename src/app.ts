import { ProjectView } from "./project"
import { ProjectsView, touchProject, renameProject, type ProjectMetadata, getLastOpenedProjectId, listProjects, setLastOpenedProjectId, clearLastOpenedProjectId } from "./projects"
import { Container, type UiComponent } from "./ui"

function swapView(container: HTMLElement, view: UiComponent<HTMLElement>) {
	container.innerHTML = ""
	container.appendChild(view.root)
}

window.onload = () => {
	const body = document.body
	if (!body) {
		throw new Error("document.body is not available")
	}

	body.style.margin = "0"
	body.style.height = "100vh"
	body.style.display = "flex"
	body.style.flexDirection = "column"

	const container = new Container(body)
	const root = document.createElement("div")
	root.style.display = "flex"
	root.style.flexDirection = "column"
	root.style.flexGrow = "1"
	root.style.minHeight = "0"
	container.root.appendChild(root)

	const setActiveView = (view: UiComponent<HTMLElement>) => {
		swapView(root, view)
	}

	const projectsView = new ProjectsView({
		onOpenProject: (project) => {
			const updated = touchProject(project.id) ?? project
			projectsView.refresh()
			openProject(updated)
		}
	})

	const openProject = (project: ProjectMetadata) => {
		let currentProject = project
		setLastOpenedProjectId(project.id)
		const projectView = new ProjectView({
			projectId: currentProject.id,
			projectName: currentProject.name,
			onBack: () => {
				projectsView.refresh()
				setActiveView(projectsView)
			},
			onRename: async (name) => {
				const updated = renameProject(currentProject.id, name)
				if (updated) {
					currentProject = updated
					projectsView.refresh()
					return updated.name
				}
				return null
			}
		})
		setActiveView(projectView)
	}

	projectsView.refresh()
	const lastOpenedId = getLastOpenedProjectId()
	if (lastOpenedId) {
		const project = listProjects().find((candidate) => candidate.id === lastOpenedId)
		if (project) {
			openProject(project)
			return
		}
		clearLastOpenedProjectId()
	}
	setActiveView(projectsView)
}
