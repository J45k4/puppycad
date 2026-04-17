import type {
	PartProjectItemData,
	PartProjectPreviewRotation,
	Project,
	ProjectDocumentType,
	ProjectFolder,
	ProjectNode,
	SchemanticProjectComponent,
	SchemanticProjectComponentData,
	SchemanticProjectConnection,
	SchemanticProjectConnectionEndpoint,
	SchemanticProjectItemData
} from "./contract"
import { materializeSketch } from "./cad/sketch"
import type { FaceReference, PartFeature, Sketch, SketchEntity, SketchPlane, Solid, SolidEdge, SolidExtrude, SolidFace, SolidVertex } from "./schema"
import type { Point2D, Quaternion, Vector3D } from "./types"

export const PROJECT_FILE_VERSION = 3 as const

export const PROJECT_FILE_TYPES = ["schemantic", "pcb", "part", "assembly", "diagram"] as const

export const PART_PROJECT_DEFAULT_HEIGHT = 30
export const PART_PROJECT_DEFAULT_PREVIEW_DISTANCE = 44

export const PART_PROJECT_DEFAULT_ROTATION: PartProjectPreviewRotation = {
	yaw: Math.PI / 4,
	pitch: Math.PI / 5
}

export const PROJECT_FILE_MIME_TYPE = "application/json"

export function createProjectFile(args: {
	items: ProjectNode[]
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
	if (rawVersion !== 1 && rawVersion !== 2 && rawVersion !== PROJECT_FILE_VERSION) {
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

function normalizeProjectFileEntries(input: unknown): ProjectNode[] {
	const itemsInput = Array.isArray(input) ? input : []
	const usedNames = new Set<string>()
	const items: ProjectNode[] = []

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
		if (!isProjectDocumentType(type)) {
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

function validateSelectedPath(path: number[] | null, items: ProjectNode[]): number[] | null {
	if (!path || path.length === 0) {
		return null
	}

	const result: number[] = []
	let currentItems: ProjectNode[] = items

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

function isFolderProjectEntry(entry: ProjectNode): entry is ProjectFolder {
	return (entry as ProjectFolder).kind === "folder"
}

function cloneProjectFileEntry(entry: ProjectNode): ProjectNode {
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

function isProjectDocumentType(value: unknown): value is ProjectDocumentType {
	if (typeof value !== "string") {
		return false
	}
	return (PROJECT_FILE_TYPES as readonly string[]).includes(value)
}

function generateDefaultName(type: ProjectDocumentType, usedNames: Set<string>): string {
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
	return structuredClone(data) as PartProjectItemData
}

function normalizePartProjectItemData(input: unknown): PartProjectItemData {
	const defaults = createDefaultPartProjectItemData()
	if (!input || typeof input !== "object") {
		return defaults
	}

	const value = input as { features?: unknown; solids?: unknown }
	if (Array.isArray(value.features) || Array.isArray(value.solids)) {
		return normalizeSchemaPartProjectItemData(value)
	}

	return normalizeLegacyPartProjectItemData(input)
}

type LegacyPartProjectItemData = Partial<{
	sketchPoints: unknown
	sketchName: unknown
	isSketchClosed: unknown
	extrudedModels: unknown
	extrudedModel: unknown
}>

type LegacyExtrudedModel = {
	base: Point2D[]
	height: number
	scale: number
	rawHeight: number
	origin?: Vector3D
	rotation?: Quaternion
	startOffset?: number
}

function normalizeSchemaPartProjectItemData(input: { features?: unknown; solids?: unknown; migrationWarnings?: unknown }): PartProjectItemData {
	const featuresInput = Array.isArray(input.features) ? input.features : []
	const features = featuresInput.map((feature, index) => normalizePartFeature(feature, index)).filter((feature): feature is PartFeature => feature !== undefined)
	const solids = normalizeSolids(input.solids)
	const migrationWarnings = normalizeMigrationWarnings(input.migrationWarnings)

	return {
		features,
		...(solids.length > 0 ? { solids } : {}),
		...(migrationWarnings.length > 0 ? { migrationWarnings } : {})
	}
}

function normalizePartFeature(input: unknown, index: number): PartFeature | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}

	const type = (input as { type?: unknown }).type
	if (type === "sketch") {
		const value = input as Partial<Sketch>
		const id = typeof value.id === "string" && value.id.trim() ? value.id.trim() : `part-sketch-${index + 1}`
		const name = typeof value.name === "string" && value.name.trim() ? value.name.trim() : undefined
		const target = normalizeSketchTarget(value.target)
		if (!target) {
			return undefined
		}
		const entities = normalizeSketchEntities(value.entities)
		return materializeSketch({
			type: "sketch",
			id,
			name,
			dirty: value.dirty === true,
			target,
			entities,
			vertices: [],
			loops: [],
			profiles: []
		})
	}

	if (type === "extrude") {
		const value = input as Partial<SolidExtrude>
		const id = typeof value.id === "string" && value.id.trim() ? value.id.trim() : `part-extrude-${index + 1}`
		const depth = extractFiniteNumber(value.depth, PART_PROJECT_DEFAULT_HEIGHT)
		if (!value.target || typeof value.target !== "object") {
			return undefined
		}
		const sketchId = typeof value.target.sketchId === "string" && value.target.sketchId.trim() ? value.target.sketchId.trim() : null
		const profileId = typeof value.target.profileId === "string" && value.target.profileId.trim() ? value.target.profileId.trim() : null
		if (!sketchId || !profileId) {
			return undefined
		}
		const name = typeof value.name === "string" && value.name.trim() ? value.name.trim() : undefined
		return {
			type: "extrude",
			id,
			name,
			target: {
				type: "profileRef",
				sketchId,
				profileId
			},
			depth
		}
	}

	return undefined
}

function normalizeSolids(input: unknown): Solid[] {
	if (!Array.isArray(input)) {
		return []
	}
	return input.map((solid, index) => normalizeSolid(solid, index)).filter((solid): solid is Solid => solid !== undefined)
}

function normalizeSolid(input: unknown, index: number): Solid | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const value = input as Partial<Solid>
	const id = typeof value.id === "string" && value.id.trim() ? value.id.trim() : `part-solid-${index + 1}`
	const featureId = typeof value.featureId === "string" && value.featureId.trim() ? value.featureId.trim() : null
	if (!featureId) {
		return undefined
	}
	const vertices = normalizeSolidVertices(value.vertices)
	const edges = normalizeSolidEdges(value.edges)
	const faces = normalizeSolidFaces(value.faces)
	return {
		id,
		featureId,
		vertices,
		edges,
		faces
	}
}

function normalizeSolidVertices(input: unknown): SolidVertex[] {
	if (!Array.isArray(input)) {
		return []
	}
	return input.map((vertex, index) => normalizeSolidVertex(vertex, index)).filter((vertex): vertex is SolidVertex => vertex !== undefined)
}

function normalizeSolidVertex(input: unknown, index: number): SolidVertex | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const value = input as Partial<SolidVertex>
	const id = typeof value.id === "string" && value.id.trim() ? value.id.trim() : `solid-vertex-${index + 1}`
	const position = normalizeVector3D(value.position)
	if (!position) {
		return undefined
	}
	return {
		id,
		position
	}
}

function normalizeSolidEdges(input: unknown): SolidEdge[] {
	if (!Array.isArray(input)) {
		return []
	}
	return input.map((edge, index) => normalizeSolidEdge(edge, index)).filter((edge): edge is SolidEdge => edge !== undefined)
}

function normalizeSolidEdge(input: unknown, index: number): SolidEdge | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const value = input as Partial<SolidEdge>
	const id = typeof value.id === "string" && value.id.trim() ? value.id.trim() : `solid-edge-${index + 1}`
	const vertexIds = normalizeIdList(value.vertexIds)
	if (vertexIds.length === 0) {
		return undefined
	}
	return {
		id,
		vertexIds
	}
}

function normalizeSolidFaces(input: unknown): SolidFace[] {
	if (!Array.isArray(input)) {
		return []
	}
	return input.map((face, index) => normalizeSolidFace(face, index)).filter((face): face is SolidFace => face !== undefined)
}

function normalizeSolidFace(input: unknown, index: number): SolidFace | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const value = input as Partial<SolidFace>
	const id = typeof value.id === "string" && value.id.trim() ? value.id.trim() : `solid-face-${index + 1}`
	const edgeIds = normalizeIdList(value.edgeIds)
	if (edgeIds.length === 0) {
		return undefined
	}
	return {
		id,
		edgeIds
	}
}

function normalizeSketchEntities(input: unknown): SketchEntity[] {
	if (!Array.isArray(input)) {
		return []
	}

	const entities: SketchEntity[] = []
	for (let index = 0; index < input.length; index += 1) {
		const entity = input[index]
		if (!entity || typeof entity !== "object") {
			continue
		}

		const id = typeof entity.id === "string" && entity.id.trim() ? entity.id.trim() : `entity-${index + 1}`
		if (entity.type === "line") {
			const p0 = normalizePoint2D(entity.p0)
			const p1 = normalizePoint2D(entity.p1)
			if (!p0 || !p1) {
				continue
			}
			entities.push({ id, type: "line", p0, p1 })
			continue
		}

		if (entity.type === "cornerRectangle") {
			const p0 = normalizePoint2D(entity.p0)
			const p1 = normalizePoint2D(entity.p1)
			if (!p0 || !p1) {
				continue
			}
			entities.push({ id, type: "cornerRectangle", p0, p1 })
		}
	}

	return entities
}

function normalizeLegacyPartProjectItemData(input: unknown): PartProjectItemData {
	const defaults = createDefaultPartProjectItemData()
	if (!input || typeof input !== "object") {
		return defaults
	}

	const value = input as LegacyPartProjectItemData
	const features: PartFeature[] = []
	const warnings: string[] = []
	let featureIndex = 1

	const sketchPoints = normalizeLegacySketchPoints(value.sketchPoints)
	const sketchName = typeof value.sketchName === "string" && value.sketchName.trim() ? value.sketchName.trim() : "Sketch 1"
	const isSketchClosed = value.isSketchClosed === true

	if (sketchPoints.length >= 2) {
		const sketch = materializeSketch({
			type: "sketch",
			id: `legacy-sketch-${featureIndex}`,
			name: sketchName,
			dirty: !isSketchClosed,
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: buildLegacySketchEntities(sketchPoints, isSketchClosed, `legacy-sketch-${featureIndex}`),
			vertices: [],
			loops: [],
			profiles: []
		})
		features.push(sketch)
		featureIndex += 1
	}

	const extrudedModelsInput = Array.isArray(value.extrudedModels) ? value.extrudedModels : []
	const normalizedLegacyExtrusions = extrudedModelsInput.map((entry) => normalizeLegacyExtrudedModel(entry)).filter((entry): entry is LegacyExtrudedModel => entry !== undefined)

	if (normalizedLegacyExtrusions.length === 0) {
		const legacyExtrudedModel = normalizeLegacyExtrudedModel(value.extrudedModel)
		if (legacyExtrudedModel) {
			normalizedLegacyExtrusions.push(legacyExtrudedModel)
		}
	}

	for (let index = 0; index < normalizedLegacyExtrusions.length; index += 1) {
		const entry = normalizedLegacyExtrusions[index]
		if (!entry) {
			continue
		}
		const converted = convertLegacyExtrudedModel(entry, index + 1, featureIndex)
		if (!converted) {
			warnings.push(`Skipped unsupported legacy extrusion ${index + 1}.`)
			continue
		}
		features.push(converted.sketch, converted.extrude)
		featureIndex += 2
	}

	return {
		features,
		...(warnings.length > 0 ? { migrationWarnings: warnings } : {})
	}
}

function normalizeLegacySketchPoints(input: unknown): Point2D[] {
	if (!Array.isArray(input)) {
		return []
	}
	const sketchPoints: Point2D[] = []
	for (const raw of input) {
		const point = normalizePoint2D(raw)
		if (point) {
			sketchPoints.push(point)
		}
	}
	return sketchPoints
}

function buildLegacySketchEntities(points: Point2D[], isClosed: boolean, idPrefix: string): SketchEntity[] {
	const entities: SketchEntity[] = []
	for (let index = 0; index < points.length - 1; index += 1) {
		const start = points[index]
		const end = points[index + 1]
		if (!start || !end) {
			continue
		}
		entities.push({
			id: `${idPrefix}-line-${entities.length + 1}`,
			type: "line",
			p0: clonePoint2D(start),
			p1: clonePoint2D(end)
		})
	}
	if (isClosed && points.length >= 3) {
		const start = points[points.length - 1]
		const end = points[0]
		if (start && end && (start.x !== end.x || start.y !== end.y)) {
			entities.push({
				id: `${idPrefix}-line-${entities.length + 1}`,
				type: "line",
				p0: clonePoint2D(start),
				p1: clonePoint2D(end)
			})
		}
	}
	return entities
}

function normalizeLegacyExtrudedModel(input: unknown): LegacyExtrudedModel | undefined {
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
	const base: Point2D[] = []
	for (const raw of baseInput) {
		const point = normalizePoint2D(raw)
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
	const origin = normalizeVector3D(value.origin)
	const rotation = normalizeQuaternion(value.rotation)
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

function convertLegacyExtrudedModel(input: LegacyExtrudedModel, index: number, featureIndex: number): { sketch: Sketch; extrude: SolidExtrude } | null {
	const plane = resolveLegacySketchPlane(input.rotation)
	if (!plane) {
		return null
	}
	if (input.origin && (Math.abs(input.origin.x) > 1e-4 || Math.abs(input.origin.y) > 1e-4 || Math.abs(input.origin.z) > 1e-4)) {
		return null
	}

	const reconstructedPoints = input.base.map((point) => ({
		x: point.x * input.scale,
		y: -point.y * input.scale
	}))
	if (reconstructedPoints.length < 3) {
		return null
	}

	const sketchId = `legacy-extrude-sketch-${featureIndex}`
	const sketch = materializeSketch({
		type: "sketch",
		id: sketchId,
		name: `Legacy Sketch ${index}`,
		dirty: false,
		target: {
			type: "plane",
			plane
		},
		entities: buildLegacySketchEntities(reconstructedPoints, true, sketchId),
		vertices: [],
		loops: [],
		profiles: []
	})
	const profileId = sketch.profiles[0]?.id
	if (!profileId) {
		return null
	}

	return {
		sketch,
		extrude: {
			type: "extrude",
			id: `legacy-extrude-${featureIndex + 1}`,
			name: `Extrude ${index}`,
			target: {
				type: "profileRef",
				sketchId,
				profileId
			},
			depth: input.rawHeight
		}
	}
}

function resolveLegacySketchPlane(rotation: Quaternion | undefined): SketchPlane | null {
	if (!rotation) {
		return "XY"
	}

	const normalized = normalizeQuaternion(rotation)
	if (!normalized) {
		return null
	}

	const canonicalRotations: Array<{ plane: SketchPlane; quaternion: Quaternion }> = [
		{ plane: "XY", quaternion: { x: 0, y: 0, z: 0, w: 1 } },
		{ plane: "XZ", quaternion: { x: -Math.SQRT1_2, y: 0, z: 0, w: Math.SQRT1_2 } },
		{ plane: "YZ", quaternion: { x: 0, y: Math.SQRT1_2, z: 0, w: Math.SQRT1_2 } }
	]

	for (const candidate of canonicalRotations) {
		const dotProduct = normalized.x * candidate.quaternion.x + normalized.y * candidate.quaternion.y + normalized.z * candidate.quaternion.z + normalized.w * candidate.quaternion.w
		if (Math.abs(dotProduct) >= 0.999) {
			return candidate.plane
		}
	}

	return null
}

function normalizePoint2D(input: unknown): Point2D | null {
	if (!input || typeof input !== "object") {
		return null
	}
	const candidate = input as Partial<Point2D>
	const x = typeof candidate.x === "number" && Number.isFinite(candidate.x) ? candidate.x : null
	const y = typeof candidate.y === "number" && Number.isFinite(candidate.y) ? candidate.y : null
	if (x === null || y === null) {
		return null
	}
	return { x, y }
}

function normalizeVector3D(input: unknown): Vector3D | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const candidate = input as Partial<Vector3D>
	const x = typeof candidate.x === "number" && Number.isFinite(candidate.x) ? candidate.x : null
	const y = typeof candidate.y === "number" && Number.isFinite(candidate.y) ? candidate.y : null
	const z = typeof candidate.z === "number" && Number.isFinite(candidate.z) ? candidate.z : null
	if (x === null || y === null || z === null) {
		return undefined
	}
	return { x, y, z }
}

function normalizeQuaternion(input: unknown): Quaternion | undefined {
	if (!input || typeof input !== "object") {
		return undefined
	}
	const candidate = input as Partial<Quaternion>
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
		features: []
	}
}

function normalizeSketchPlane(input: unknown): SketchPlane | null {
	return input === "XY" || input === "YZ" || input === "XZ" ? input : null
}

function normalizeSketchTarget(input: unknown): Sketch["target"] | null {
	if (!input || typeof input !== "object") {
		return null
	}
	const value = input as Partial<Sketch["target"]>
	if (value.type === "plane") {
		const plane = normalizeSketchPlane(value.plane)
		return plane
			? {
					type: "plane",
					plane
				}
			: null
	}
	if (value.type === "face") {
		const face = normalizeFaceReference(value.face)
		return face
			? {
					type: "face",
					face
				}
			: null
	}
	return null
}

function normalizeFaceReference(input: unknown): FaceReference | null {
	if (!input || typeof input !== "object") {
		return null
	}
	const value = input as Partial<FaceReference>
	if (value.type !== "extrudeFace") {
		return null
	}
	const extrudeId = typeof value.extrudeId === "string" && value.extrudeId.trim() ? value.extrudeId.trim() : null
	const faceId = typeof value.faceId === "string" && value.faceId.trim() ? value.faceId.trim() : null
	if (!extrudeId || !faceId) {
		return null
	}
	return {
		type: "extrudeFace",
		extrudeId,
		faceId
	}
}

function normalizeMigrationWarnings(input: unknown): string[] {
	if (!Array.isArray(input)) {
		return []
	}
	return input.filter((warning): warning is string => typeof warning === "string" && warning.trim().length > 0).map((warning) => warning.trim())
}

function normalizeIdList(input: unknown): string[] {
	if (!Array.isArray(input)) {
		return []
	}
	return input.filter((id): id is string => typeof id === "string" && id.trim().length > 0).map((id) => id.trim())
}

function clonePoint2D(point: Point2D): Point2D {
	return { x: point.x, y: point.y }
}

function normalizeVisibleFlag(rawItem: unknown): boolean | undefined {
	if (!rawItem || typeof rawItem !== "object") {
		return undefined
	}
	const candidate = rawItem as { visible?: unknown }
	return typeof candidate.visible === "boolean" ? candidate.visible : undefined
}
