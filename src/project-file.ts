export const PROJECT_FILE_VERSION = 1 as const

export const PROJECT_FILE_TYPES = ["schemantic", "pcb", "part", "assembly", "diagram"] as const

export type ProjectFileType = (typeof PROJECT_FILE_TYPES)[number]

export type ProjectFileItem = {
	type: ProjectFileType
}

export type ProjectFile = {
	version: typeof PROJECT_FILE_VERSION
	items: ProjectFileItem[]
	selectedIndex: number | null
}

export const PROJECT_FILE_MIME_TYPE = "application/json"

export function createProjectFile(args: {
	items: ProjectFileItem[]
	selectedIndex: number | null
}): ProjectFile {
	return {
		version: PROJECT_FILE_VERSION,
		items: args.items,
		selectedIndex: args.selectedIndex ?? null
	}
}

export function serializeProjectFile(file: ProjectFile): string {
	return JSON.stringify(file, null, 2)
}

export function normalizeProjectFile(input: unknown): ProjectFile | null {
	if (!input || typeof input !== "object") {
		return null
	}

	const value = input as Partial<{
		version: unknown
		items: unknown
		selectedIndex: unknown
	}>

	const version = typeof value.version === "number" ? value.version : PROJECT_FILE_VERSION
	if (version !== PROJECT_FILE_VERSION) {
		return null
	}

	const itemsInput = Array.isArray(value.items) ? value.items : []
	const items: ProjectFileItem[] = []

	for (const rawItem of itemsInput) {
		if (!rawItem || typeof rawItem !== "object") {
			continue
		}
		const type = (rawItem as { type?: unknown }).type
		if (isProjectFileType(type)) {
			items.push({ type })
		}
	}

	let selectedIndex: number | null = null
	const selectedInput = value.selectedIndex
	if (typeof selectedInput === "number" && Number.isInteger(selectedInput)) {
		selectedIndex = selectedInput
	}
	if (selectedIndex !== null) {
		if (selectedIndex < 0 || selectedIndex >= items.length) {
			selectedIndex = null
		}
	}

	return {
		version: PROJECT_FILE_VERSION,
		items,
		selectedIndex
	}
}

function isProjectFileType(value: unknown): value is ProjectFileType {
	if (typeof value !== "string") {
		return false
	}
	return (PROJECT_FILE_TYPES as readonly string[]).includes(value)
}
