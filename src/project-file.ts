import type {
	PartProjectExtrudedModel,
	PartProjectItemData,
	PartProjectPoint,
	PartProjectPreviewRotation,
	PartProjectQuaternion,
	PartProjectVector3,
	Project,
	ProjectItem,
	ProjectFileFolder,
	ProjectFileType,
	SchemanticProjectComponent,
	SchemanticProjectComponentData,
	SchemanticProjectConnection,
	SchemanticProjectConnectionEndpoint,
	SchemanticProjectItemData
} from "./puppycad-types"

export type {
	PartProjectExtrudedModel,
	Part,
	PartProjectItemData,
	PartProjectPoint,
	PartProjectPreviewRotation,
	PartProjectQuaternion,
	PartProjectReferencePlaneVisibility,
	PartProjectVector3,
	Project as ProjectFile,
	Project,
	ProjectItem as ProjectFileEntry,
	ProjectFileFolder,
	ProjectFileType,
	SchemanticProjectComponent,
	SchemanticProjectComponentData,
	SchemanticProjectConnection,
	SchemanticProjectConnectionEndpoint,
	SchemanticProjectItemData,
	PuppyCadProject
} from "./puppycad-types"

export const PROJECT_FILE_VERSION = 2 as const

export const PROJECT_FILE_TYPES = ["schemantic", "pcb", "part", "assembly", "diagram"] as const

export const PART_PROJECT_DEFAULT_HEIGHT = 30
export const PART_PROJECT_DEFAULT_PREVIEW_DISTANCE = 44

export const PART_PROJECT_DEFAULT_ROTATION: PartProjectPreviewRotation = {
	yaw: Math.PI / 4,
	pitch: Math.PI / 5
}

export const PROJECT_FILE_MIME_TYPE = "application/json"

export function createProjectFile(args: {
	items: ProjectItem[]
	selectedPath: number[] | null
}): Project {
	return {
		version: PROJECT_FILE_VERSION,
		items: args.items.map(cloneProjectFileEntry),
		selectedPath: cloneSelectedPath(args.selectedPath)
	}
}

export function serializeProjectFile(file: Project): string {
	return JSON.stringify(file, null, 2)
}

export function normalizeProjectFile(input: unknown): Project | null {
	if (!input || typeof input !== "object") {
		return null
	}

	const value = input as Partial<{
		version: unknown
		items: unknown
		selectedIndex: unknown
		selectedPath: unknown
	}>

	const rawVersion = typeof value.version === "number" ? value.version : 1
	if (rawVersion !== 1 && rawVersion !== PROJECT_FILE_VERSION) {
		return null
	}

	const items = normalizeProjectFileEntries(value.items)

	let selectedPath: number[] | null = null
	if (rawVersion === 1) {
		const selectedInput = value.selectedIndex
		if (typeof selectedInput === "number" && Number.isInteger(selectedInput)) {
			selectedPath = validateSelectedPath([selectedInput], items)
		}
	} else if (Array.isArray(value.selectedPath)) {
		const candidate = value.selectedPath.filter((index) => typeof index === "number" && Number.isInteger(index))
		if (candidate.length === value.selectedPath.length) {
			selectedPath = validateSelectedPath(candidate as number[], items)
		}
	}

	return {
		version: PROJECT_FILE_VERSION,
		items,
		selectedPath
	}
}

function normalizeProjectFileEntries(input: unknown): ProjectItem[] {
	const itemsInput = Array.isArray(input) ? input : []
	const usedNames = new Set<string>()
	const items: ProjectItem[] = []

	for (const rawItem of itemsInput) {
		if (!rawItem || typeof rawItem !== "object") {
			continue
		}

		if (isFolderEntry(rawItem)) {
			const folderName = normalizeEntryName(rawItem, usedNames, generateDefaultFolderName)
			const visible = normalizeVisibleFlag(rawItem)
			const children = normalizeProjectFileEntries((rawItem as { items?: unknown }).items)
			items.push({
				kind: "folder",
				name: folderName,
				items: children,
				...(visible === undefined ? {} : { visible })
			})
			continue
		}

		const type = (rawItem as { type?: unknown }).type
		if (!isProjectFileType(type)) {
			continue
		}

		const name = normalizeEntryName(rawItem, usedNames, (names) => generateDefaultName(type, names))

		if (type === "schemantic") {
			const data = normalizeSchemanticProjectItemData((rawItem as { data?: unknown }).data)
			const visible = normalizeVisibleFlag(rawItem)
			items.push({
				type,
				name,
				data: data.components.length === 0 && data.connections.length === 0 ? undefined : data,
				...(visible === undefined ? {} : { visible })
			})
			continue
		}

		if (type === "part") {
			const data = normalizePartProjectItemData((rawItem as { data?: unknown }).data)
			const visible = normalizeVisibleFlag(rawItem)
			items.push({
				type,
				name,
				data,
				...(visible === undefined ? {} : { visible })
			})
			continue
		}

		const visible = normalizeVisibleFlag(rawItem)
		items.push({
			type,
			name,
			...(visible === undefined ? {} : { visible })
		})
	}

	return items
}

function normalizeEntryName(rawItem: unknown, usedNames: Set<string>, generateDefault: (names: Set<string>) => string): string {
	const rawName = (rawItem as { name?: unknown }).name
	let name = typeof rawName === "string" ? rawName.trim() : ""
	if (!name) {
		name = generateDefault(usedNames)
	}
	if (usedNames.has(name)) {
		name = generateDefault(usedNames)
	}
	usedNames.add(name)
	return name
}

function isFolderEntry(rawItem: unknown): rawItem is { kind?: unknown; items?: unknown } {
	if (!rawItem || typeof rawItem !== "object") {
		return false
	}
	const kind = (rawItem as { kind?: unknown }).kind
	if (kind === "folder") {
		return true
	}
	const type = (rawItem as { type?: unknown }).type
	return type === "folder"
}

function validateSelectedPath(path: number[] | null, items: ProjectItem[]): number[] | null {
	if (!path || path.length === 0) {
		return null
	}

	const result: number[] = []
	let currentItems: ProjectItem[] = items

	for (let i = 0; i < path.length; i += 1) {
		const index = path[i]
		if (typeof index !== "number" || !Number.isInteger(index)) {
			return null
		}
		if (index < 0 || index >= currentItems.length) {
			return null
		}

		result.push(index)
		const entry = currentItems[index]
		if (!entry) {
			return null
		}
		if (i < path.length - 1) {
			if (!isFolderProjectEntry(entry)) {
				return null
			}
			currentItems = entry.items
		}
	}

	return result
}

function isFolderProjectEntry(entry: ProjectItem): entry is ProjectFileFolder {
	return (entry as ProjectFileFolder).kind === "folder"
}

function cloneProjectFileEntry(entry: ProjectItem): ProjectItem {
	if (isFolderProjectEntry(entry)) {
		return {
			kind: "folder",
			name: entry.name,
			items: entry.items.map(cloneProjectFileEntry),
			...(entry.visible === undefined ? {} : { visible: entry.visible })
		}
	}

	if (entry.type === "schemantic") {
		return {
			type: entry.type,
			name: entry.name,
			data: cloneSchemanticProjectItemData(entry.data),
			...(entry.visible === undefined ? {} : { visible: entry.visible })
		}
	}

	if (entry.type === "part") {
		return {
			type: entry.type,
			name: entry.name,
			data: clonePartProjectItemData(entry.data),
			...(entry.visible === undefined ? {} : { visible: entry.visible })
		}
	}

	return {
		type: entry.type,
		name: entry.name,
		...(entry.visible === undefined ? {} : { visible: entry.visible })
	}
}

function cloneSelectedPath(path: number[] | null): number[] | null {
	if (!path) {
		return null
	}
	return path.slice()
}

function isProjectFileType(value: unknown): value is ProjectFileType {
	if (typeof value !== "string") {
		return false
	}
	return (PROJECT_FILE_TYPES as readonly string[]).includes(value)
}

function generateDefaultName(type: ProjectFileType, usedNames: Set<string>): string {
	const base = `${type.charAt(0).toUpperCase()}${type.slice(1)}`
	return generateUniqueName(base, usedNames)
}

function generateDefaultFolderName(usedNames: Set<string>): string {
	return generateUniqueName("Folder", usedNames)
}

function generateUniqueName(base: string, usedNames: Set<string>): string {
	let suffix = 1
	let candidate = `${base} ${suffix}`
	while (usedNames.has(candidate)) {
		suffix += 1
		candidate = `${base} ${suffix}`
	}
	return candidate
}

function cloneSchemanticProjectItemData(data: SchemanticProjectItemData | undefined): SchemanticProjectItemData | undefined {
	if (!data) {
		return undefined
	}
	return {
		components: data.components.map((component) => {
			const typeValue = component.data?.type
			const normalizedType = typeValue === undefined ? undefined : typeof typeValue === "string" ? typeValue : String(typeValue)
			return {
				id: component.id,
				x: component.x,
				y: component.y,
				width: component.width,
				height: component.height,
				data: normalizedType === undefined ? undefined : { type: normalizedType }
			}
		}),
		connections: data.connections.map((connection) => {
			const style = connection.style === "dashed" ? "dashed" : connection.style === "solid" ? "solid" : undefined
			return style ? { from: { ...connection.from }, to: { ...connection.to }, style } : { from: { ...connection.from }, to: { ...connection.to } }
		})
	}
}

function normalizeSchemanticProjectItemData(input: unknown): SchemanticProjectItemData {
	if (!input || typeof input !== "object") {
		return { components: [], connections: [] }
	}

	const value = input as Partial<{
		components: unknown
		connections: unknown
	}>

	const componentsInput = Array.isArray(value.components) ? value.components : []
	const components: SchemanticProjectComponent[] = []
	const componentIds = new Set<number>()

	for (const raw of componentsInput) {
		if (!raw || typeof raw !== "object") {
			continue
		}
		const candidate = raw as Partial<SchemanticProjectComponent>
		const id = typeof candidate.id === "number" && Number.isInteger(candidate.id) ? candidate.id : null
		const x = typeof candidate.x === "number" && Number.isFinite(candidate.x) ? candidate.x : null
		const y = typeof candidate.y === "number" && Number.isFinite(candidate.y) ? candidate.y : null
		const width = typeof candidate.width === "number" && Number.isFinite(candidate.width) && candidate.width >= 0 ? candidate.width : null
		const height = typeof candidate.height === "number" && Number.isFinite(candidate.height) && candidate.height >= 0 ? candidate.height : null
		if (id === null || componentIds.has(id) || x === null || y === null || width === null || height === null) {
			continue
		}
		const dataType = candidate.data && typeof candidate.data === "object" ? ((candidate.data as SchemanticProjectComponentData).type ?? undefined) : undefined
		const normalizedData =
			dataType === undefined
				? undefined
				: {
						type: typeof dataType === "string" ? dataType : String(dataType)
					}
		components.push({
			id,
			x,
			y,
			width,
			height,
			data: normalizedData
		})
		componentIds.add(id)
	}

	const connectionsInput = Array.isArray(value.connections) ? value.connections : []
	const connections: SchemanticProjectConnection[] = []

	for (const raw of connectionsInput) {
		if (!raw || typeof raw !== "object") {
			continue
		}
		const candidate = raw as Partial<SchemanticProjectConnection>
		const from = normalizeConnectionEndpoint(candidate.from, componentIds)
		const to = normalizeConnectionEndpoint(candidate.to, componentIds)
		if (!from || !to) {
			continue
		}
		const style = candidate.style === "dashed" ? "dashed" : candidate.style === "solid" ? "solid" : undefined
		const connection: SchemanticProjectConnection = { from, to }
		if (style) {
			connection.style = style
		}
		connections.push(connection)
	}

	return { components, connections }
}

function normalizeConnectionEndpoint(endpoint: SchemanticProjectConnectionEndpoint | undefined, componentIds: Set<number>): SchemanticProjectConnectionEndpoint | null {
	if (!endpoint || typeof endpoint !== "object") {
		return null
	}
	const componentId = typeof endpoint.componentId === "number" && Number.isInteger(endpoint.componentId) ? endpoint.componentId : null
	if (componentId === null || !componentIds.has(componentId)) {
		return null
	}
	const edge = endpoint.edge
	if (edge !== "left" && edge !== "right" && edge !== "top" && edge !== "bottom") {
		return null
	}
	const ratio = typeof endpoint.ratio === "number" && Number.isFinite(endpoint.ratio) ? Math.min(Math.max(endpoint.ratio, 0), 1) : 0.5
	return { componentId, edge, ratio }
}

function clonePartProjectItemData(data: PartProjectItemData | undefined): PartProjectItemData | undefined {
	if (!data) {
		return undefined
	}
	const sketchName = typeof data.sketchName === "string" ? data.sketchName.trim() : ""
	return {
		sketchPoints: data.sketchPoints.map((point) => ({ x: point.x, y: point.y })),
		sketchName: sketchName || undefined,
		isSketchClosed: data.isSketchClosed,
		extrudedModels: data.extrudedModels.map((model) => ({
			base: model.base.map((point) => ({ x: point.x, y: point.y })),
			height: model.height,
			scale: model.scale,
			rawHeight: model.rawHeight,
			origin: model.origin ? { x: model.origin.x, y: model.origin.y, z: model.origin.z } : undefined,
			rotation: model.rotation ? { x: model.rotation.x, y: model.rotation.y, z: model.rotation.z, w: model.rotation.w } : undefined,
			startOffset: model.startOffset
		})),
		height: data.height,
		variables: data.variables ? { ...data.variables } : undefined
	}
}

function normalizePartProjectItemData(input: unknown): PartProjectItemData {
	const defaults = createDefaultPartProjectItemData()
	if (!input || typeof input !== "object") {
		return defaults
	}

	const value = input as Partial<{
		sketchPoints: unknown
		sketchName: unknown
		isSketchClosed: unknown
		extrudedModels: unknown
		extrudedModel: unknown
		height: unknown
		variables: unknown
	}>

	const sketchPointsInput = Array.isArray(value.sketchPoints) ? value.sketchPoints : []
	const sketchPoints: PartProjectPoint[] = []
	for (const raw of sketchPointsInput) {
		const point = normalizePartProjectPoint(raw)
		if (point) {
			sketchPoints.push(point)
		}
	}

	const isSketchClosed = typeof value.isSketchClosed === "boolean" ? value.isSketchClosed : defaults.isSketchClosed
	const sketchName = typeof value.sketchName === "string" && value.sketchName.trim().length > 0 ? value.sketchName.trim() : undefined

	const heightValue = typeof value.height === "number" && Number.isFinite(value.height) ? value.height : defaults.height

	const extrudedModelsInput = Array.isArray(value.extrudedModels) ? value.extrudedModels : []
	const extrudedModels = extrudedModelsInput.map((entry) => normalizePartProjectExtrudedModel(entry)).filter((entry): entry is PartProjectExtrudedModel => entry !== undefined)
	if (extrudedModels.length === 0) {
		const legacyExtrudedModel = normalizePartProjectExtrudedModel(value.extrudedModel)
		if (legacyExtrudedModel) {
			extrudedModels.push(legacyExtrudedModel)
		}
	}

	const variables = normalizeVariables(value.variables)

	return {
		sketchPoints,
		sketchName,
		isSketchClosed,
		extrudedModels,
		height: heightValue,
		variables
	}
}

function normalizePartProjectExtrudedModel(input: unknown): PartProjectExtrudedModel | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}

	const value = input as Partial<{
		base: unknown
		height: unknown
		scale: unknown
		rawHeight: unknown
		origin: unknown
		rotation: unknown
		startOffset: unknown
	}>

	const baseInput = Array.isArray(value.base) ? value.base : []
	const base: PartProjectPoint[] = []
	for (const raw of baseInput) {
		const point = normalizePartProjectPoint(raw)
		if (point) {
			base.push(point)
		}
	}

	if (base.length < 3) {
		return undefined
	}

	const height = extractFiniteNumber(value.height, PART_PROJECT_DEFAULT_HEIGHT)
	const scale = extractFiniteNumber(value.scale, 1)
	const rawHeight = extractFiniteNumber(value.rawHeight, PART_PROJECT_DEFAULT_HEIGHT)
	const origin = normalizePartProjectVector3(value.origin)
	const rotation = normalizePartProjectQuaternion(value.rotation)
	const startOffset = typeof value.startOffset === "number" && Number.isFinite(value.startOffset) ? value.startOffset : undefined

	return {
		base,
		height,
		scale,
		rawHeight,
		origin,
		rotation,
		startOffset
	}
}

function normalizePartProjectPoint(input: unknown): PartProjectPoint | null {
	if (!input || typeof input !== "object") {
		return null
	}
	const candidate = input as Partial<PartProjectPoint>
	const x = typeof candidate.x === "number" && Number.isFinite(candidate.x) ? candidate.x : null
	const y = typeof candidate.y === "number" && Number.isFinite(candidate.y) ? candidate.y : null
	if (x === null || y === null) {
		return null
	}
	return { x, y }
}

function normalizePartProjectVector3(input: unknown): PartProjectVector3 | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const candidate = input as Partial<PartProjectVector3>
	const x = typeof candidate.x === "number" && Number.isFinite(candidate.x) ? candidate.x : null
	const y = typeof candidate.y === "number" && Number.isFinite(candidate.y) ? candidate.y : null
	const z = typeof candidate.z === "number" && Number.isFinite(candidate.z) ? candidate.z : null
	if (x === null || y === null || z === null) {
		return undefined
	}
	return { x, y, z }
}

function normalizePartProjectQuaternion(input: unknown): PartProjectQuaternion | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const candidate = input as Partial<PartProjectQuaternion>
	const x = typeof candidate.x === "number" && Number.isFinite(candidate.x) ? candidate.x : null
	const y = typeof candidate.y === "number" && Number.isFinite(candidate.y) ? candidate.y : null
	const z = typeof candidate.z === "number" && Number.isFinite(candidate.z) ? candidate.z : null
	const w = typeof candidate.w === "number" && Number.isFinite(candidate.w) ? candidate.w : null
	if (x === null || y === null || z === null || w === null) {
		return undefined
	}
	return { x, y, z, w }
}

function extractFiniteNumber(value: unknown, defaultValue: number): number {
	return typeof value === "number" && Number.isFinite(value) ? value : defaultValue
}

function createDefaultPartProjectItemData(): PartProjectItemData {
	return {
		sketchPoints: [],
		isSketchClosed: false,
		extrudedModels: [],
		height: PART_PROJECT_DEFAULT_HEIGHT
	}
}

function normalizeVariables(input: unknown): PartProjectItemData["variables"] {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const entries = Object.entries(input)
	const variables: NonNullable<PartProjectItemData["variables"]> = {}
	for (const [key, value] of entries) {
		const name = key.trim()
		if (!name) {
			continue
		}
		if (typeof value === "number") {
			if (Number.isFinite(value)) {
				variables[name] = value
			}
			continue
		}
		if (typeof value === "string" || typeof value === "boolean") {
			variables[name] = value
		}
	}
	return Object.keys(variables).length > 0 ? variables : undefined
}

function normalizeVisibleFlag(rawItem: unknown): boolean | undefined {
	if (!rawItem || typeof rawItem !== "object") {
		return undefined
	}
	const candidate = rawItem as { visible?: unknown }
	return typeof candidate.visible === "boolean" ? candidate.visible : undefined
}
