import { UiComponent } from "./ui"
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
		this.root.style.display = "flex"
		this.root.style.flexDirection = "column"
		this.root.style.gap = "16px"
		this.root.style.padding = "24px"
		this.root.style.boxSizing = "border-box"
		this.root.style.flexGrow = "1"

		const header = document.createElement("div")
		header.style.display = "flex"
		header.style.alignItems = "center"
		header.style.justifyContent = "space-between"

		const title = document.createElement("h1")
		title.textContent = "Projects"
		title.style.margin = "0"
		header.appendChild(title)

		const createButton = document.createElement("button")
		createButton.textContent = "New Project"
		createButton.onclick = () => {
			const project = createProject()
			this.refresh()
			this.onOpenProject(project)
		}
		header.appendChild(createButton)

		this.root.appendChild(header)

		this.list = document.createElement("div")
		this.list.style.display = "flex"
		this.list.style.flexDirection = "column"
		this.list.style.gap = "12px"
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
			empty.style.color = "#555"
			this.list.appendChild(empty)
			return
		}
		for (const project of this.projects) {
			this.list.appendChild(this.createProjectItem(project))
		}
	}

	private createProjectItem(project: ProjectMetadata): HTMLElement {
		const container = document.createElement("div")
		container.style.display = "flex"
		container.style.alignItems = "center"
		container.style.padding = "12px"
		container.style.border = "1px solid #ddd"
		container.style.borderRadius = "8px"
		container.style.gap = "12px"

		const info = document.createElement("div")
		info.style.display = "flex"
		info.style.flexDirection = "column"
		info.style.flexGrow = "1"

		const name = document.createElement("span")
		name.textContent = project.name
		name.style.fontWeight = "bold"
		info.appendChild(name)

		const detail = document.createElement("span")
		const updated = new Date(project.updatedAt)
		detail.textContent = `Last opened: ${updated.toLocaleString()}`
		detail.style.fontSize = "0.85rem"
		detail.style.color = "#666"
		info.appendChild(detail)

		container.appendChild(info)

		const openButton = document.createElement("button")
		openButton.textContent = "Open"
		openButton.onclick = () => {
			this.onOpenProject(project)
		}
		container.appendChild(openButton)

		const renameButton = document.createElement("button")
		renameButton.textContent = "Rename"
		renameButton.onclick = () => {
			const input = typeof window !== "undefined" ? window.prompt("Project name", project.name) : null
			if (input === null || input === undefined) {
				return
			}
			const updated = renameProject(project.id, input)
			if (updated) {
				this.refresh()
			}
		}
		container.appendChild(renameButton)

		const deleteButton = document.createElement("button")
		deleteButton.textContent = "Delete"
		deleteButton.onclick = async () => {
			const confirmed = typeof window === "undefined" ? true : window.confirm(`Delete project "${project.name}"?`)
			if (!confirmed) {
				return
			}
			await deleteProject(project.id)
			this.refresh()
		}
		container.appendChild(deleteButton)

		return container
	}
}
