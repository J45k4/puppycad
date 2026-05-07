import { Modal, UiComponent, showTextPromptModal } from "./ui"
import type { Project } from "../contract"
import { normalizeProjectFile } from "../project-file"
import { deleteProjectState, saveProjectState } from "./project"

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
const VIEWABLE_IMAGE_EXTENSIONS = new Set(["avif", "bmp", "gif", "jpeg", "jpg", "png", "svg", "webp"])

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

export function ensureProject(id: string, name?: string): ProjectMetadata {
	const projects = loadProjects()
	const existing = projects.find((project) => project.id === id)
	if (existing) {
		return existing
	}
	const now = Date.now()
	const project: ProjectMetadata = {
		id,
		name: normalizeProjectName(name, projects),
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

export function getProjectNameFromFileName(fileName: string): string {
	const trimmed = fileName.trim()
	if (!trimmed) {
		return FALLBACK_PROJECT_NAME
	}
	const withoutPath = trimmed.split(/[/\\]/).pop() ?? trimmed
	const withoutExtension = withoutPath.replace(/\.(pcad|json)$/i, "").trim()
	return withoutExtension || FALLBACK_PROJECT_NAME
}

export async function readProjectFile(file: { name: string; text: () => Promise<string> }): Promise<Project> {
	let parsed: unknown
	try {
		parsed = JSON.parse(await file.text())
	} catch {
		throw new Error("Project files must contain valid JSON.")
	}
	const project = normalizeProjectFile(parsed)
	if (!project) {
		throw new Error("Invalid PuppyCAD project file.")
	}
	return project
}

export function isViewableImageFile(file: { name: string; type?: string }): boolean {
	const mimeType = file.type?.trim().toLowerCase()
	if (mimeType?.startsWith("image/")) {
		return true
	}
	const extension = file.name.trim().split(".").pop()?.toLowerCase()
	return Boolean(extension && VIEWABLE_IMAGE_EXTENSIONS.has(extension))
}

async function createProjectFromFile(file: File): Promise<ProjectMetadata> {
	const projectFile = await readProjectFile(file)
	const project = createProject(getProjectNameFromFileName(file.name))
	try {
		await saveProjectState(project.id, projectFile)
	} catch (error) {
		await deleteProject(project.id).catch(() => undefined)
		throw error
	}
	return project
}

export class ProjectsView extends UiComponent<HTMLDivElement> {
	private projects: ProjectMetadata[] = []
	private list: HTMLDivElement
	private readonly onOpenProject: (project: ProjectMetadata) => void
	private fileInput: HTMLInputElement

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

		const openFileButton = document.createElement("button")
		openFileButton.textContent = "Open File"
		openFileButton.type = "button"
		openFileButton.classList.add("button", "button--secondary")
		openFileButton.onclick = () => {
			this.fileInput.click()
		}
		header.appendChild(openFileButton)

		this.fileInput = document.createElement("input")
		this.fileInput.type = "file"
		this.fileInput.accept = ".pcad,.json,application/json,image/*"
		this.fileInput.style.display = "none"
		this.fileInput.addEventListener("change", () => {
			const file = this.fileInput.files?.[0]
			this.fileInput.value = ""
			if (!file) {
				return
			}
			void this.openProjectFile(file)
		})
		header.appendChild(this.fileInput)

		this.root.appendChild(header)

		this.list = document.createElement("div")
		this.list.classList.add("projects-list")
		this.root.appendChild(this.list)

		this.refresh()
	}

	private async openProjectFile(file: File): Promise<void> {
		if (isViewableImageFile(file)) {
			this.showImageFilePreview(file)
			return
		}
		try {
			const project = await createProjectFromFile(file)
			this.refresh()
			this.onOpenProject(project)
		} catch (error) {
			const message = error instanceof Error ? error.message : "Unable to open project file."
			console.error("Failed to open project file", error)
			if (typeof window !== "undefined" && typeof window.alert === "function") {
				window.alert(message)
			}
		}
	}

	private showImageFilePreview(file: File): void {
		const url = URL.createObjectURL(file)
		const content = document.createElement("div")
		content.style.display = "flex"
		content.style.alignItems = "center"
		content.style.justifyContent = "center"
		content.style.width = "min(80vw, 960px)"
		content.style.maxHeight = "75vh"
		content.style.overflow = "auto"
		content.style.backgroundColor = "#0f172a"
		content.style.borderRadius = "8px"
		content.style.padding = "12px"

		const image = document.createElement("img")
		image.src = url
		image.alt = file.name
		image.style.display = "block"
		image.style.maxWidth = "100%"
		image.style.maxHeight = "70vh"
		image.style.objectFit = "contain"
		content.appendChild(image)

		const modal = new Modal({ title: file.name, content })
		modal.addAction({
			label: "Close",
			type: "secondary",
			onClick: () => modal.close()
		})
		modal.onClose(() => {
			URL.revokeObjectURL(url)
		})
		modal.open()
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
		container.tabIndex = 0
		container.setAttribute("role", "button")
		container.setAttribute("aria-label", `Open project ${project.name}`)

		container.addEventListener("click", (event) => {
			const target = event.target as HTMLElement | null
			if (target?.closest("button")) {
				return
			}
			this.onOpenProject(project)
		})

		container.addEventListener("keydown", (event) => {
			const target = event.target as HTMLElement | null
			if (target?.closest("button")) {
				return
			}
			if (event.key !== "Enter" && event.key !== " " && event.key !== "Spacebar" && event.key !== "Space") {
				return
			}
			event.preventDefault()
			this.onOpenProject(project)
		})

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

		const renameButton = document.createElement("button")
		renameButton.textContent = "Rename"
		renameButton.type = "button"
		renameButton.classList.add("button", "button--ghost", "button--sm")
		renameButton.onclick = async (event) => {
			event.stopPropagation()
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
		deleteButton.onclick = async (event) => {
			event.stopPropagation()
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
