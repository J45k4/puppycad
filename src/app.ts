import { ProjectView } from "./project"
import { ProjectsView, touchProject, renameProject, type ProjectMetadata, getLastOpenedProjectId, listProjects, setLastOpenedProjectId, clearLastOpenedProjectId } from "./projects"
import { Container, type UiComponent } from "./ui"

type ViewHistoryState =
	| { view: "projects" }
	| {
			view: "project"
			projectId: string
	  }

type NavigationOptions = {
	replace?: boolean
	skipHistory?: boolean
}

function parseProjectIdFromPath(pathname: string): string | null {
	const match = pathname.match(/^\/project\/([^/]+)\/?$/)
	if (!match) {
		return null
	}
	const rawId = match[1]
	if (!rawId) {
		return null
	}
	try {
		return decodeURIComponent(rawId)
	} catch (error) {
		console.error("Failed to decode project id from path", error)
		return null
	}
}

function updateHistoryState(state: ViewHistoryState, path: string, options?: { replace?: boolean; skip?: boolean }): void {
	if (options?.skip) {
		return
	}
	if (typeof history === "undefined") {
		return
	}
	const method = options?.replace ? history.replaceState : history.pushState
	try {
		method.call(history, state, "", path)
	} catch (error) {
		console.error("Failed to update history state", error)
	}
}

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
			openProject(project)
		}
	})

	const showProjects = (options: NavigationOptions = {}) => {
		setActiveView(projectsView)
		updateHistoryState({ view: "projects" }, "/", { replace: options.replace, skip: options.skipHistory })
	}

	const openProject = (project: ProjectMetadata, options: NavigationOptions = {}) => {
		let currentProject = project
		const touchedProject = touchProject(project.id)
		if (touchedProject) {
			currentProject = touchedProject
		}
		setLastOpenedProjectId(project.id)
		updateHistoryState({ view: "project", projectId: project.id }, `/project/${encodeURIComponent(project.id)}`, { replace: options.replace, skip: options.skipHistory })
		const projectView = new ProjectView({
			projectId: currentProject.id,
			projectName: currentProject.name,
			onBack: () => {
				projectsView.refresh()
				showProjects()
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
		projectsView.refresh()
		setActiveView(projectView)
	}

	const getProjectById = (id: string): ProjectMetadata | undefined => {
		return listProjects().find((candidate) => candidate.id === id)
	}

	window.addEventListener("popstate", (event) => {
		const state = event.state as ViewHistoryState | null
		if (state?.view === "project") {
			const project = getProjectById(state.projectId)
			if (project) {
				openProject(project, { skipHistory: true, replace: true })
				return
			}
			clearLastOpenedProjectId()
		}
		projectsView.refresh()
		showProjects({ skipHistory: true, replace: true })
	})

	projectsView.refresh()
	const initialProjectId = parseProjectIdFromPath(window.location.pathname)
	if (initialProjectId) {
		const project = getProjectById(initialProjectId)
		if (project) {
			openProject(project, { replace: true })
			return
		}
		clearLastOpenedProjectId()
		showProjects({ replace: true })
		return
	}
	const lastOpenedId = getLastOpenedProjectId()
	if (lastOpenedId) {
		const project = getProjectById(lastOpenedId)
		if (project) {
			openProject(project, { replace: true })
			return
		}
		clearLastOpenedProjectId()
	}
	showProjects({ replace: true })
}
