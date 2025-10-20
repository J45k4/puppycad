import { UiComponent, showTextPromptModal } from "./ui"
import { deleteProjectState } from "./project"

export type ProjectMetadata = {
	id: string
	name: string
	createdAt: number
	updatedAt: number
}

const PROJECTS_STORAGE_KEY = "puppycad.projects"
const LAST_OPENED_PROJECT_STORAGE_KEY = "puppycad.projects.lastOpenedId"
const FALLBACK_PROJECT_NAME = "Untitled Project"
const DEFAULT_PROJECT_PREFIX = "Project"

const inMemoryStorage: { value: string | null } = { value: null }
const inMemoryLastOpened: { value: string | null } = { value: null }

function hasLocalStorage(): boolean {
	return typeof localStorage !== "undefined"
}

function readStorage(): string | null {
	if (hasLocalStorage()) {
		try {
			return localStorage.getItem(PROJECTS_STORAGE_KEY)
		} catch (error) {
			console.error("Failed to read projects from localStorage", error)
			return null
		}
	}
	return inMemoryStorage.value
}

function writeStorage(value: string | null): void {
	if (hasLocalStorage()) {
		try {
			if (value === null) {
				localStorage.removeItem(PROJECTS_STORAGE_KEY)
			} else {
				localStorage.setItem(PROJECTS_STORAGE_KEY, value)
			}
		} catch (error) {
			console.error("Failed to write projects to localStorage", error)
		}
		return
	}
	inMemoryStorage.value = value
}

function readLastOpenedId(): string | null {
	if (hasLocalStorage()) {
		try {
			return localStorage.getItem(LAST_OPENED_PROJECT_STORAGE_KEY)
		} catch (error) {
			console.error("Failed to read last opened project", error)
			return null
		}
	}
	return inMemoryLastOpened.value
}

function writeLastOpenedId(value: string | null): void {
	if (hasLocalStorage()) {
		try {
			if (value === null) {
				localStorage.removeItem(LAST_OPENED_PROJECT_STORAGE_KEY)
			} else {
				localStorage.setItem(LAST_OPENED_PROJECT_STORAGE_KEY, value)
			}
		} catch (error) {
			console.error("Failed to write last opened project", error)
		}
		return
	}
	inMemoryLastOpened.value = value
}

function normalizeProjectRecord(input: unknown): ProjectMetadata | null {
	if (!input || typeof input !== "object") {
		return null
	}
	const candidate = input as Partial<ProjectMetadata>
	if (typeof candidate.id !== "string" || !candidate.id) {
		return null
	}
	if (typeof candidate.name !== "string" || typeof candidate.createdAt !== "number" || typeof candidate.updatedAt !== "number") {
		return null
	}
	return {
		id: candidate.id,
		name: candidate.name,
		createdAt: candidate.createdAt,
		updatedAt: candidate.updatedAt
	}
}

function loadProjects(): ProjectMetadata[] {
	const raw = readStorage()
	if (!raw) {
		return []
	}
	try {
		const parsed = JSON.parse(raw)
		if (!Array.isArray(parsed)) {
			return []
		}
		const projects: ProjectMetadata[] = []
		for (const entry of parsed) {
			const normalized = normalizeProjectRecord(entry)
			if (normalized) {
				projects.push(normalized)
			}
		}
		return projects
	} catch (error) {
		console.error("Failed to parse projects", error)
		return []
	}
}

function saveProjects(projects: ProjectMetadata[]): void {
	if (projects.length === 0) {
		writeStorage(null)
		return
	}
	writeStorage(JSON.stringify(projects))
}

function normalizeProjectName(input: string | undefined, existing: ProjectMetadata[]): string {
	const trimmed = (input ?? "").trim()
	if (trimmed) {
		return trimmed
	}
	return generateDefaultProjectName(existing)
}

function generateDefaultProjectName(existing: ProjectMetadata[]): string {
	const names = new Set(existing.map((project) => project.name))
	let index = existing.length + 1
	let candidate = `${DEFAULT_PROJECT_PREFIX} ${index}`
	while (names.has(candidate)) {
		index += 1
		candidate = `${DEFAULT_PROJECT_PREFIX} ${index}`
	}
	return candidate
}

function generateId(): string {
	if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
		return crypto.randomUUID()
	}
	const randomPart = Math.random().toString(36).slice(2, 10)
	const timestamp = Date.now().toString(36)
	return `project-${timestamp}-${randomPart}`
}

export function listProjects(): ProjectMetadata[] {
	const projects = loadProjects()
	return [...projects].sort((left, right) => right.updatedAt - left.updatedAt)
}

export function getLastOpenedProjectId(): string | null {
	return readLastOpenedId()
}

export function setLastOpenedProjectId(id: string): void {
	writeLastOpenedId(id)
}

export function clearLastOpenedProjectId(): void {
	writeLastOpenedId(null)
}

export function createProject(name?: string): ProjectMetadata {
	const projects = loadProjects()
	const finalName = normalizeProjectName(name, projects)
	const now = Date.now()
	const project: ProjectMetadata = {
		id: generateId(),
		name: finalName,
		createdAt: now,
		updatedAt: now
	}
	projects.push(project)
	saveProjects(projects)
	return project
}

export function renameProject(id: string, name: string): ProjectMetadata | null {
	const projects = loadProjects()
	const index = projects.findIndex((project) => project.id === id)
	if (index === -1) {
		return null
	}
	const existing = projects[index]
	if (!existing) {
		return null
	}
	const trimmed = name.trim()
	const finalName = trimmed || FALLBACK_PROJECT_NAME
	const now = Date.now()
	const updatedProject: ProjectMetadata = { ...existing, name: finalName, updatedAt: now }
	projects[index] = updatedProject
	saveProjects(projects)
	return updatedProject
}

export function touchProject(id: string): ProjectMetadata | null {
	const projects = loadProjects()
	const index = projects.findIndex((project) => project.id === id)
	if (index === -1) {
		return null
	}
	const existing = projects[index]
	if (!existing) {
		return null
	}
	const now = Date.now()
	const updatedProject: ProjectMetadata = { ...existing, updatedAt: now }
	projects[index] = updatedProject
	saveProjects(projects)
	return updatedProject
}

export async function deleteProject(id: string): Promise<boolean> {
	const projects = loadProjects()
	const index = projects.findIndex((project) => project.id === id)
	if (index === -1) {
		return false
	}
	projects.splice(index, 1)
	saveProjects(projects)
	if (readLastOpenedId() === id) {
		writeLastOpenedId(null)
	}
	await deleteProjectState(id)
	return true
}

export class ProjectsView extends UiComponent<HTMLDivElement> {
	private projects: ProjectMetadata[] = []
	private list: HTMLDivElement
	private readonly onOpenProject: (project: ProjectMetadata) => void

	constructor(args: { onOpenProject: (project: ProjectMetadata) => void }) {
		super(document.createElement("div"))
		this.onOpenProject = args.onOpenProject
		this.root.classList.add("view", "projects-view")

		const header = document.createElement("div")
		header.classList.add("view-header")

		const title = document.createElement("h1")
		title.textContent = "Projects"
		title.classList.add("view-title")
		header.appendChild(title)

		const createButton = document.createElement("button")
		createButton.textContent = "New Project"
		createButton.type = "button"
		createButton.classList.add("button", "button--primary")
		createButton.onclick = () => {
			const project = createProject()
			this.refresh()
			this.onOpenProject(project)
		}
		header.appendChild(createButton)

		this.root.appendChild(header)

		this.list = document.createElement("div")
		this.list.classList.add("projects-list")
		this.root.appendChild(this.list)

		this.refresh()
	}

	public refresh() {
		this.projects = listProjects()
		this.render()
	}

	private render() {
		this.list.innerHTML = ""
		if (this.projects.length === 0) {
			const empty = document.createElement("div")
			empty.textContent = "Create your first project to get started."
			empty.classList.add("empty-state")
			this.list.appendChild(empty)
			return
		}
		for (const project of this.projects) {
			this.list.appendChild(this.createProjectItem(project))
		}
	}

	private createProjectItem(project: ProjectMetadata): HTMLElement {
		const container = document.createElement("div")
		container.classList.add("projects-item")

		const info = document.createElement("div")
		info.classList.add("projects-item__info")

		const name = document.createElement("span")
		name.textContent = project.name
		name.classList.add("projects-item__name")
		info.appendChild(name)

		const detail = document.createElement("span")
		const updated = new Date(project.updatedAt)
		detail.textContent = `Last opened: ${updated.toLocaleString()}`
		detail.classList.add("projects-item__meta")
		info.appendChild(detail)

		container.appendChild(info)

		const actions = document.createElement("div")
		actions.classList.add("projects-item__actions")

		const openButton = document.createElement("button")
		openButton.textContent = "Open"
		openButton.type = "button"
		openButton.classList.add("button", "button--primary", "button--sm")
		openButton.onclick = () => {
			this.onOpenProject(project)
		}
		actions.appendChild(openButton)

		const renameButton = document.createElement("button")
		renameButton.textContent = "Rename"
		renameButton.type = "button"
		renameButton.classList.add("button", "button--ghost", "button--sm")
		renameButton.onclick = async () => {
			if (typeof window === "undefined") {
				return
			}
			const input = await showTextPromptModal({
				title: "Rename Project",
				initialValue: project.name,
				confirmText: "Save",
				cancelText: "Cancel"
			})
			if (input === null || input === undefined) {
				return
			}
			const updated = renameProject(project.id, input)
			if (updated) {
				this.refresh()
			}
		}
		actions.appendChild(renameButton)

		const deleteButton = document.createElement("button")
		deleteButton.textContent = "Delete"
		deleteButton.type = "button"
		deleteButton.classList.add("button", "button--danger", "button--sm")
		deleteButton.onclick = async () => {
			const confirmed = typeof window === "undefined" ? true : window.confirm(`Delete project "${project.name}"?`)
			if (!confirmed) {
				return
			}
			await deleteProject(project.id)
			this.refresh()
		}
		actions.appendChild(deleteButton)

		container.appendChild(actions)

		return container
	}
}
