export const PROJECT_FILE_VERSION = 1 as const

export const PROJECT_FILE_TYPES = ["schemantic", "pcb", "part", "assembly", "diagram"] as const

export type ProjectFileType = (typeof PROJECT_FILE_TYPES)[number]

export type SchemanticProjectComponentData = {
	type?: string
}

export type SchemanticProjectComponent = {
	id: number
	x: number
	y: number
	width: number
	height: number
	data?: SchemanticProjectComponentData
}

export type SchemanticProjectConnectionEndpoint = {
	componentId: number
	edge: "left" | "right" | "top" | "bottom"
	ratio: number
}

export type SchemanticProjectConnection = {
	from: SchemanticProjectConnectionEndpoint
	to: SchemanticProjectConnectionEndpoint
}

export type SchemanticProjectItemData = {
	components: SchemanticProjectComponent[]
	connections: SchemanticProjectConnection[]
}

export type PartProjectPoint = { x: number; y: number }

export type PartProjectExtrudedModel = {
	base: PartProjectPoint[]
	height: number
	scale: number
	rawHeight: number
}

export type PartProjectPreviewRotation = {
	yaw: number
	pitch: number
}

export type PartProjectItemData = {
	sketchPoints: PartProjectPoint[]
	isSketchClosed: boolean
	extrudedModel?: PartProjectExtrudedModel
	height: number
	previewRotation: PartProjectPreviewRotation
}

export const PART_PROJECT_DEFAULT_HEIGHT = 30

export const PART_PROJECT_DEFAULT_ROTATION: PartProjectPreviewRotation = {
	yaw: Math.PI / 4,
	pitch: Math.PI / 5
}

export type ProjectFileItem =
	| {
			type: "schemantic"
			name: string
			data?: SchemanticProjectItemData
	  }
	| {
			type: "part"
			name: string
			data?: PartProjectItemData
	  }
	| {
			type: Exclude<ProjectFileType, "schemantic" | "part">
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
		items: args.items.map((item) => {
			if (item.type === "schemantic") {
				return {
					type: item.type,
					name: item.name,
					data: cloneSchemanticProjectItemData(item.data)
				}
			}
			if (item.type === "part") {
				return {
					type: item.type,
					name: item.name,
					data: clonePartProjectItemData(item.data)
				}
			}
			return {
				type: item.type,
				name: item.name
			}
		}),
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
		if (!isProjectFileType(type)) {
			continue
		}

		const rawName = (rawItem as { name?: unknown }).name
		let name = typeof rawName === "string" ? rawName.trim() : ""
		if (!name) {
			name = generateDefaultName(type, usedNames)
		}
		if (usedNames.has(name)) {
			name = generateDefaultName(type, usedNames)
		}
		usedNames.add(name)

		if (type === "schemantic") {
			const data = normalizeSchemanticProjectItemData((rawItem as { data?: unknown }).data)
			items.push({
				type,
				name,
				data: data.components.length === 0 && data.connections.length === 0 ? undefined : data
			})
			continue
		}

		if (type === "part") {
			const data = normalizePartProjectItemData((rawItem as { data?: unknown }).data)
			items.push({
				type,
				name,
				data
			})
			continue
		}

		items.push({ type, name })
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
		connections: data.connections.map((connection) => ({
			from: { ...connection.from },
			to: { ...connection.to }
		}))
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
		connections.push({ from, to })
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
	return {
		sketchPoints: data.sketchPoints.map((point) => ({ x: point.x, y: point.y })),
		isSketchClosed: data.isSketchClosed,
		extrudedModel:
			data.extrudedModel === undefined
				? undefined
				: {
						base: data.extrudedModel.base.map((point) => ({ x: point.x, y: point.y })),
						height: data.extrudedModel.height,
						scale: data.extrudedModel.scale,
						rawHeight: data.extrudedModel.rawHeight
					},
		height: data.height,
		previewRotation: {
			yaw: data.previewRotation.yaw,
			pitch: data.previewRotation.pitch
		}
	}
}

function normalizePartProjectItemData(input: unknown): PartProjectItemData {
	const defaults = createDefaultPartProjectItemData()
	if (!input || typeof input !== "object") {
		return defaults
	}

	const value = input as Partial<{
		sketchPoints: unknown
		isSketchClosed: unknown
		extrudedModel: unknown
		height: unknown
		previewRotation: unknown
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

	const heightValue = typeof value.height === "number" && Number.isFinite(value.height) ? value.height : defaults.height

	const previewRotationValue = value.previewRotation
	const previewRotation: PartProjectPreviewRotation = {
		yaw:
			previewRotationValue && typeof previewRotationValue === "object"
				? extractFiniteNumber((previewRotationValue as { yaw?: unknown }).yaw, defaults.previewRotation.yaw)
				: defaults.previewRotation.yaw,
		pitch:
			previewRotationValue && typeof previewRotationValue === "object"
				? extractFiniteNumber((previewRotationValue as { pitch?: unknown }).pitch, defaults.previewRotation.pitch)
				: defaults.previewRotation.pitch
	}

	const extrudedModel = normalizePartProjectExtrudedModel(value.extrudedModel)

	return {
		sketchPoints,
		isSketchClosed,
		extrudedModel,
		height: heightValue,
		previewRotation
	}
}

function normalizePartProjectExtrudedModel(input: unknown): PartProjectExtrudedModel | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const value = input as Partial<PartProjectExtrudedModel>
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
	const height = typeof value.height === "number" && Number.isFinite(value.height) ? value.height : null
	const scale = typeof value.scale === "number" && Number.isFinite(value.scale) ? value.scale : null
	const rawHeight = typeof value.rawHeight === "number" && Number.isFinite(value.rawHeight) ? value.rawHeight : null
	if (height === null || scale === null || rawHeight === null) {
		return undefined
	}
	return {
		base,
		height,
		scale,
		rawHeight
	}
}

function normalizePartProjectPoint(input: unknown): PartProjectPoint | null {
	if (!input || typeof input !== "object") {
		return null
	}
	const xValue = (input as { x?: unknown }).x
	const yValue = (input as { y?: unknown }).y
	const x = typeof xValue === "number" && Number.isFinite(xValue) ? xValue : null
	const y = typeof yValue === "number" && Number.isFinite(yValue) ? yValue : null
	if (x === null || y === null) {
		return null
	}
	return { x, y }
}

function extractFiniteNumber(value: unknown, fallback: number): number {
	return typeof value === "number" && Number.isFinite(value) ? value : fallback
}

function createDefaultPartProjectItemData(): PartProjectItemData {
	return {
		sketchPoints: [],
		isSketchClosed: false,
		extrudedModel: undefined,
		height: PART_PROJECT_DEFAULT_HEIGHT,
		previewRotation: {
			yaw: PART_PROJECT_DEFAULT_ROTATION.yaw,
			pitch: PART_PROJECT_DEFAULT_ROTATION.pitch
		}
	}
}
