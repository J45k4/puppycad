export const PROJECT_FILE_VERSION = 1 as const

export const PROJECT_FILE_TYPES = ["schemantic", "pcb", "part", "assembly", "diagram"] as const

export type ProjectFileType = (typeof PROJECT_FILE_TYPES)[number]

export type ProjectFileItem = {
	type: ProjectFileType
	name: string
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
		items: args.items.map((item) => ({
			type: item.type,
			name: item.name
		})),
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
	const usedNames = new Set<string>()

	for (const rawItem of itemsInput) {
		if (!rawItem || typeof rawItem !== "object") {
			continue
		}
		const type = (rawItem as { type?: unknown }).type
		if (isProjectFileType(type)) {
			const rawName = (rawItem as { name?: unknown }).name
			let name = typeof rawName === "string" ? rawName.trim() : ""
			if (!name) {
				name = generateDefaultName(type, usedNames)
			}
			if (usedNames.has(name)) {
				name = generateDefaultName(type, usedNames)
			}
			usedNames.add(name)
			items.push({ type, name })
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

function generateDefaultName(type: ProjectFileType, usedNames: Set<string>): string {
	const base = `${type.charAt(0).toUpperCase()}${type.slice(1)}`
	let suffix = 1
	let candidate = `${base} ${suffix}`
	while (usedNames.has(candidate)) {
		suffix += 1
		candidate = `${base} ${suffix}`
	}
	return candidate
}
