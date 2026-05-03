import type { ChamferNode, ExtrudeNode, PartFeature, PCadGraphNode, PCadState, SketchDimension, SketchEntity, Solid, SolidEdge, SolidFace, SolidVertex } from "../schema"
import { getNodeDependencies } from "../pcad/runtime"
import type { Point2D, Vector3D } from "../types"
import { EditorCanvas, type CanvasComponent, type Connection } from "./canvas"
import { UiComponent } from "./ui"

export type PCadGeneratedState = {
	features: readonly PartFeature[]
	solids: readonly Solid[]
}

export type PCadGeneratedSelection =
	| {
			type: "sketch"
			graphId: string
			sketchId: string
	  }
	| {
			type: "sketchEntity"
			graphId: string
			sketchId: string
			entityId: string
	  }
	| {
			type: "sketchDimension"
			graphId: string
			sketchId: string
			entityId: string
			dimensionType: SketchDimension["type"]
	  }
	| {
			type: "solidFace"
			graphId: string
			extrudeId: string
			faceId: string
	  }
	| {
			type: "solidEdge"
			graphId: string
			extrudeId: string
			edgeId: string
	  }
	| {
			type: "solidVertex"
			graphId: string
			extrudeId: string
			vertexId: string
	  }

type GeneratedNodeType =
	| "sketchEntity"
	| "sketchDimension"
	| "generatedSketch"
	| "generatedSketchVertex"
	| "generatedSketchLoop"
	| "generatedSketchProfile"
	| "generatedSolid"
	| "generatedSolidVertex"
	| "generatedSolidEdge"
	| "generatedSolidFace"

type GeneratedGraphNode = {
	id: string
	type: GeneratedNodeType
	sourceId: string
	dependencies: readonly string[]
	label: string
	detail: string
	rows: readonly { label: string; value: string }[]
	selection?: PCadGeneratedSelection
}

type RenderGraphNode =
	| {
			id: string
			kind: "pcad"
			node: PCadGraphNode
			dependencies: readonly string[]
	  }
	| {
			id: string
			kind: "generated"
			node: GeneratedGraphNode
			dependencies: readonly string[]
	  }

type PCadNodeComponentData = {
	graphId: string
	nodeId?: string
	nodeType: PCadGraphNode["type"] | GeneratedNodeType
	label: string
	detail: string
	editable: boolean
	generated: boolean
}

type PCadNodeEditorOptions = {
	state: PCadState
	generatedState?: PCadGeneratedState
	selectedNodeId?: string | null
	showAllGenerated?: boolean
	onSelectNode?: (nodeId: string | null) => void
	onSelectGenerated?: (selection: PCadGeneratedSelection) => void
	onRenameNode?: (nodeId: string, name: string) => void
	onSetExtrudeDepth?: (nodeId: string, depth: number) => void
	onSetChamferDistances?: (nodeId: string, d1: number, d2?: number) => void
	onDeleteNode?: (nodeId: string) => void
}

const NODE_WIDTH = 180
const NODE_HEIGHT = 86
const COLUMN_GAP = 250
const ROW_GAP = 122
const NODE_START_X = 48
const NODE_START_Y = 44

export class PCadNodeEditor extends UiComponent<HTMLDivElement> {
	private readonly editor: EditorCanvas<PCadNodeComponentData>
	private readonly inspector: HTMLDivElement
	private readonly showAllGeneratedInput: HTMLInputElement
	private readonly options: Omit<PCadNodeEditorOptions, "state" | "generatedState" | "selectedNodeId" | "showAllGenerated">
	private readonly nodePositions = new Map<string, { x: number; y: number }>()
	private graphIdByComponentId = new Map<number, string>()
	private componentIdByGraphId = new Map<string, number>()
	private generatedNodeById = new Map<string, GeneratedGraphNode>()
	private state: PCadState
	private generatedState?: PCadGeneratedState
	private selectedGraphId: string | null
	private showAllGenerated: boolean
	private suppressSelectionChange = false

	public constructor(options: PCadNodeEditorOptions) {
		super(document.createElement("div"))
		this.options = {
			onSelectNode: options.onSelectNode,
			onSelectGenerated: options.onSelectGenerated,
			onRenameNode: options.onRenameNode,
			onSetExtrudeDepth: options.onSetExtrudeDepth,
			onSetChamferDistances: options.onSetChamferDistances,
			onDeleteNode: options.onDeleteNode
		}
		this.state = options.state
		this.generatedState = options.generatedState
		this.selectedGraphId = options.selectedNodeId ?? null
		this.showAllGenerated = options.showAllGenerated ?? false

		this.root.className = "pcad-node-editor"
		this.root.style.display = "flex"
		this.root.style.gap = "12px"
		this.root.style.width = "100%"
		this.root.style.height = "100%"
		this.root.style.minWidth = "0"
		this.root.style.minHeight = "0"
		this.root.style.overflow = "hidden"

		const graphPane = document.createElement("div")
		graphPane.style.flex = "1 1 auto"
		graphPane.style.minWidth = "0"
		graphPane.style.minHeight = "0"
		graphPane.style.display = "flex"
		graphPane.style.flexDirection = "column"
		graphPane.style.gap = "8px"
		this.root.appendChild(graphPane)

		const graphToolbar = document.createElement("div")
		graphToolbar.style.display = "flex"
		graphToolbar.style.justifyContent = "flex-end"
		graphToolbar.style.alignItems = "center"
		graphToolbar.style.flex = "0 0 auto"
		graphToolbar.style.minWidth = "0"
		graphPane.appendChild(graphToolbar)

		const showAllGeneratedLabel = document.createElement("label")
		showAllGeneratedLabel.style.display = "inline-flex"
		showAllGeneratedLabel.style.alignItems = "center"
		showAllGeneratedLabel.style.gap = "6px"
		showAllGeneratedLabel.style.fontSize = "12px"
		showAllGeneratedLabel.style.fontWeight = "700"
		showAllGeneratedLabel.style.color = "#334155"
		this.showAllGeneratedInput = document.createElement("input")
		this.showAllGeneratedInput.type = "checkbox"
		this.showAllGeneratedInput.checked = this.showAllGenerated
		this.showAllGeneratedInput.addEventListener("change", () => {
			this.showAllGenerated = this.showAllGeneratedInput.checked
			this.applyGraph(this.buildCurrentGraph())
		})
		showAllGeneratedLabel.append(this.showAllGeneratedInput, document.createTextNode("Show all generated"))
		graphToolbar.appendChild(showAllGeneratedLabel)

		this.inspector = document.createElement("div")
		this.inspector.className = "pcad-node-inspector"
		this.inspector.style.flex = "0 0 280px"
		this.inspector.style.minWidth = "240px"
		this.inspector.style.maxWidth = "320px"
		this.inspector.style.background = "#ffffff"
		this.inspector.style.border = "1px solid #cbd5e1"
		this.inspector.style.borderRadius = "8px"
		this.inspector.style.padding = "12px"
		this.inspector.style.boxSizing = "border-box"
		this.inspector.style.overflowY = "auto"
		this.root.appendChild(this.inspector)

		const graph = this.buildCurrentGraph()
		this.editor = new EditorCanvas<PCadNodeComponentData>({
			initialComponents: graph.components,
			initialConnections: graph.connections,
			allowConnectionCreation: false,
			allowDeletion: false,
			gridSpacing: 80,
			getComponentLabel: (component) => component.data?.label ?? `Node ${component.id}`,
			renderComponent: (ctx, component, renderState) => renderPCadNodeComponent(ctx, component, renderState.selected),
			renderConnection: (ctx, _connection, renderState) => renderPCadDependencyConnection(ctx, renderState),
			onComponentsChange: (components) => this.rememberComponentPositions(components),
			onSelectionChange: (ids) => this.handleSelectionChange(ids)
		})
		this.editor.canvasElement.tabIndex = -1
		this.editor.root.style.flex = "1 1 auto"
		this.editor.root.style.minWidth = "0"
		this.editor.root.style.minHeight = "0"
		graphPane.appendChild(this.editor.root)

		this.applyGraph(graph)
	}

	public update(state: PCadState, selectedNodeId?: string | null, generatedState?: PCadGeneratedState): void {
		this.state = state
		this.generatedState = generatedState
		this.selectedGraphId = selectedNodeId ?? null
		this.applyGraph(this.buildCurrentGraph())
	}

	public getCanvasForTesting(): EditorCanvas<PCadNodeComponentData> {
		return this.editor
	}

	private buildCurrentGraph(): { components: CanvasComponent<PCadNodeComponentData>[]; connections: Connection[]; generatedNodes: GeneratedGraphNode[] } {
		return buildGraph(this.state, this.nodePositions, this.generatedState, {
			selectedGraphId: this.selectedGraphId,
			showAllGenerated: this.showAllGenerated
		})
	}

	private applyGraph(graph: { components: CanvasComponent<PCadNodeComponentData>[]; connections: Connection[]; generatedNodes: GeneratedGraphNode[] }): void {
		this.graphIdByComponentId = new Map(graph.components.map((component) => [component.id, component.data?.graphId ?? ""]))
		this.componentIdByGraphId = new Map(graph.components.map((component) => [component.data?.graphId ?? "", component.id]))
		this.generatedNodeById = new Map(graph.generatedNodes.map((node) => [node.id, node]))
		this.editor.setComponents(graph.components)
		this.editor.setConnections(graph.connections)
		this.renderInspector()
		const selectedComponentId = this.selectedGraphId ? this.componentIdByGraphId.get(this.selectedGraphId) : undefined
		this.suppressSelectionChange = true
		if (selectedComponentId) {
			this.editor.setSelection([selectedComponentId])
		} else {
			this.editor.clearSelection()
		}
		this.suppressSelectionChange = false
		this.editor.redraw()
	}

	private rememberComponentPositions(components: CanvasComponent<PCadNodeComponentData>[]): void {
		for (const component of components) {
			const graphId = component.data?.graphId
			if (graphId) {
				this.nodePositions.set(graphId, { x: component.x, y: component.y })
			}
		}
	}

	private handleSelectionChange(ids: number[]): void {
		if (this.suppressSelectionChange) {
			return
		}
		const graphId = ids.length > 0 ? (this.graphIdByComponentId.get(ids[0] ?? -1) ?? null) : null
		this.selectedGraphId = graphId || null
		this.renderInspector()
		if (!this.selectedGraphId || this.state.nodes.has(this.selectedGraphId)) {
			this.options.onSelectNode?.(this.selectedGraphId)
			return
		}
		const generatedSelection = this.generatedNodeById.get(this.selectedGraphId)?.selection
		if (generatedSelection) {
			this.options.onSelectGenerated?.(generatedSelection)
		}
	}

	private renderInspector(): void {
		this.inspector.innerHTML = ""
		const node = this.selectedGraphId ? this.state.nodes.get(this.selectedGraphId) : undefined
		const generatedNode = this.selectedGraphId ? this.generatedNodeById.get(this.selectedGraphId) : undefined
		if (!node) {
			if (generatedNode) {
				this.renderGeneratedInspector(generatedNode)
				return
			}
			this.inspector.appendChild(createInspectorTitle("Node"))
			this.inspector.appendChild(createHelpText("Select a graph node to inspect its PCAD data."))
			return
		}

		this.inspector.appendChild(createInspectorTitle(getNodeLabel(node)))
		this.inspector.appendChild(createReadonlyRow("Type", node.type))
		this.inspector.appendChild(createReadonlyRow("ID", node.id))
		for (const dependencyId of getNodeDependencies(node)) {
			this.inspector.appendChild(createReadonlyRow("Depends on", dependencyId))
		}

		if (isNamedEditableNode(node)) {
			this.inspector.appendChild(
				createTextField("Name", node.name ?? "", (value) => {
					const trimmed = value.trim()
					if (trimmed) {
						this.options.onRenameNode?.(node.id, trimmed)
					}
				})
			)
		}

		if (node.type === "extrude") {
			this.renderExtrudeInspector(node)
		} else if (node.type === "chamfer") {
			this.renderChamferInspector(node)
		} else if (node.type === "sketch") {
			this.inspector.appendChild(createReadonlyRow("Entities", String(node.entities.length)))
			this.inspector.appendChild(createReadonlyRow("Dimensions", String(node.dimensions.length)))
		} else if (node.type === "referencePlane") {
			this.inspector.appendChild(createReadonlyRow("Plane", node.plane))
		} else if (node.type === "face") {
			this.inspector.appendChild(createReadonlyRow("Face", node.faceId))
		} else if (node.type === "edge") {
			this.inspector.appendChild(createReadonlyRow("Edge", node.edgeId))
		}

		if (isDeletableNode(node)) {
			const deleteButton = document.createElement("button")
			deleteButton.type = "button"
			deleteButton.textContent = "Delete Node"
			deleteButton.className = "button button--danger"
			deleteButton.style.width = "100%"
			deleteButton.style.marginTop = "12px"
			deleteButton.addEventListener("click", () => this.options.onDeleteNode?.(node.id))
			this.inspector.appendChild(deleteButton)
		}
	}

	private renderExtrudeInspector(node: ExtrudeNode): void {
		this.inspector.appendChild(createReadonlyRow("Sketch", node.sketchId))
		this.inspector.appendChild(createReadonlyRow("Profile", node.profileId))
		this.inspector.appendChild(createReadonlyRow("Operation", node.operation))
		this.inspector.appendChild(
			createNumberField("Depth", node.depth, (value) => {
				if (Number.isFinite(value) && value > 0) {
					this.options.onSetExtrudeDepth?.(node.id, value)
				}
			})
		)
	}

	private renderChamferInspector(node: ChamferNode): void {
		this.inspector.appendChild(createReadonlyRow("Edge node", node.edgeId))
		const submit = () => {
			const d1Input = this.inspector.querySelector<HTMLInputElement>('[data-pcad-field="chamfer-d1"]')
			const d2Input = this.inspector.querySelector<HTMLInputElement>('[data-pcad-field="chamfer-d2"]')
			const d1 = Number.parseFloat(d1Input?.value ?? "")
			const d2Value = d2Input?.value.trim() ?? ""
			const d2 = d2Value ? Number.parseFloat(d2Value) : undefined
			if (Number.isFinite(d1) && d1 > 0 && (d2 === undefined || (Number.isFinite(d2) && d2 > 0))) {
				this.options.onSetChamferDistances?.(node.id, d1, d2)
			}
		}
		this.inspector.appendChild(createNumberField("D1", node.d1, submit, "chamfer-d1"))
		this.inspector.appendChild(createNumberField("D2", node.d2, submit, "chamfer-d2"))
	}

	private renderGeneratedInspector(node: GeneratedGraphNode): void {
		this.inspector.appendChild(createInspectorTitle(node.label))
		this.inspector.appendChild(createReadonlyRow("Type", node.type))
		this.inspector.appendChild(createReadonlyRow("Source", node.sourceId))
		this.inspector.appendChild(createReadonlyRow("Mode", getReadOnlyNodeMode(node.type)))
		for (const row of node.rows) {
			this.inspector.appendChild(createReadonlyRow(row.label, row.value))
		}
	}
}

function buildGraph(
	state: PCadState,
	positions: ReadonlyMap<string, { x: number; y: number }>,
	generatedState?: PCadGeneratedState,
	options: { selectedGraphId?: string | null; showAllGenerated?: boolean } = {}
): { components: CanvasComponent<PCadNodeComponentData>[]; connections: Connection[]; generatedNodes: GeneratedGraphNode[] } {
	const pcadNodes = getSortedNodes(state)
	const generatedNodes = filterGeneratedNodes(buildGeneratedNodes(state, generatedState), state, options)
	const graphNodes = [
		...pcadNodes.map(
			(node): RenderGraphNode => ({
				id: node.id,
				kind: "pcad",
				node,
				dependencies: getNodeDependencies(node)
			})
		),
		...generatedNodes.map(
			(node): RenderGraphNode => ({
				id: node.id,
				kind: "generated",
				node,
				dependencies: node.dependencies
			})
		)
	]
	const depthByNodeId = computeDepths(graphNodes)
	const rowsByDepth = new Map<number, RenderGraphNode[]>()
	for (const graphNode of graphNodes) {
		const depth = depthByNodeId.get(graphNode.id) ?? 0
		rowsByDepth.set(depth, [...(rowsByDepth.get(depth) ?? []), graphNode])
	}

	const componentIdByGraphId = new Map<string, number>()
	const components: CanvasComponent<PCadNodeComponentData>[] = []
	for (let index = 0; index < graphNodes.length; index += 1) {
		const graphNode = graphNodes[index]
		if (!graphNode) {
			continue
		}
		componentIdByGraphId.set(graphNode.id, index + 1)
	}
	for (const graphNode of graphNodes) {
		const componentId = componentIdByGraphId.get(graphNode.id)
		if (!componentId) {
			continue
		}
		const depth = depthByNodeId.get(graphNode.id) ?? 0
		const row = rowsByDepth.get(depth)?.findIndex((candidate) => candidate.id === graphNode.id) ?? 0
		const savedPosition = positions.get(graphNode.id)
		components.push({
			id: componentId,
			x: savedPosition?.x ?? NODE_START_X + depth * COLUMN_GAP,
			y: savedPosition?.y ?? NODE_START_Y + row * ROW_GAP,
			width: NODE_WIDTH,
			height: NODE_HEIGHT,
			data: {
				graphId: graphNode.id,
				...(graphNode.kind === "pcad" ? { nodeId: graphNode.node.id } : {}),
				nodeType: graphNode.node.type,
				label: graphNode.kind === "pcad" ? getNodeLabel(graphNode.node) : graphNode.node.label,
				detail: graphNode.kind === "pcad" ? getNodeDetail(graphNode.node) : graphNode.node.detail,
				editable: graphNode.kind === "pcad" ? isNamedEditableNode(graphNode.node) : false,
				generated: graphNode.kind === "generated"
			}
		})
	}

	const connections: Connection[] = []
	for (const graphNode of graphNodes) {
		const toId = componentIdByGraphId.get(graphNode.id)
		if (!toId) {
			continue
		}
		for (const dependencyId of graphNode.dependencies) {
			const fromId = componentIdByGraphId.get(dependencyId)
			if (!fromId) {
				continue
			}
			connections.push({
				from: { componentId: fromId, edge: "right", ratio: 0.5 },
				to: { componentId: toId, edge: "left", ratio: 0.5 }
			})
		}
	}

	return { components, connections, generatedNodes }
}

function getSortedNodes(state: PCadState): PCadGraphNode[] {
	const rootOrder = new Map(state.rootNodeIds.map((id, index) => [id, index]))
	return [...state.nodes.values()].sort((a, b) => {
		const typeOrder = getTypeOrder(a.type) - getTypeOrder(b.type)
		if (typeOrder !== 0) {
			return typeOrder
		}
		const rootDelta = (rootOrder.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (rootOrder.get(b.id) ?? Number.MAX_SAFE_INTEGER)
		if (rootDelta !== 0) {
			return rootDelta
		}
		return a.id.localeCompare(b.id)
	})
}

function computeDepths(nodes: readonly RenderGraphNode[]): Map<string, number> {
	const visiting = new Set<string>()
	const depthById = new Map<string, number>()
	const nodeById = new Map(nodes.map((node) => [node.id, node]))
	const visit = (nodeId: string): number => {
		const cached = depthById.get(nodeId)
		if (cached !== undefined) {
			return cached
		}
		if (visiting.has(nodeId)) {
			return 0
		}
		visiting.add(nodeId)
		const node = nodeById.get(nodeId)
		const dependencies = node ? node.dependencies.filter((dependencyId) => nodeById.has(dependencyId)) : []
		const depth = dependencies.length === 0 ? 0 : Math.max(...dependencies.map(visit)) + 1
		visiting.delete(nodeId)
		depthById.set(nodeId, depth)
		return depth
	}
	for (const node of nodes) {
		visit(node.id)
	}
	return depthById
}

function filterGeneratedNodes(nodes: readonly GeneratedGraphNode[], state: PCadState, options: { selectedGraphId?: string | null; showAllGenerated?: boolean }): GeneratedGraphNode[] {
	if (options.showAllGenerated) {
		return [...nodes]
	}

	const nodeById = new Map(nodes.map((node) => [node.id, node]))
	const visibleIds = new Set<string>()
	const addVisibleWithDependencies = (nodeId: string): void => {
		if (visibleIds.has(nodeId)) {
			return
		}
		const node = nodeById.get(nodeId)
		if (!node) {
			return
		}
		visibleIds.add(nodeId)
		for (const dependencyId of node.dependencies) {
			addVisibleWithDependencies(dependencyId)
		}
	}

	const selectedGraphId = options.selectedGraphId ?? null
	const selectedNode = selectedGraphId ? state.nodes.get(selectedGraphId) : undefined
	const selectedSourceId = selectedNode && (selectedNode.type === "sketch" || selectedNode.type === "extrude") ? selectedNode.id : undefined
	if (selectedGraphId && nodeById.has(selectedGraphId)) {
		addVisibleWithDependencies(selectedGraphId)
	}
	if (selectedSourceId) {
		for (const node of nodes) {
			if (isGeneratedGeometryNode(node) && node.sourceId === selectedSourceId) {
				addVisibleWithDependencies(node.id)
			}
		}
	}

	for (const node of state.nodes.values()) {
		if (node.type === "extrude") {
			addVisibleWithDependencies(getGeneratedSketchProfileNodeId(node.sketchId, node.profileId))
			continue
		}
		if (node.type === "face") {
			for (const generatedNode of nodes) {
				if (
					generatedNode.type === "generatedSolidFace" &&
					generatedNode.selection?.type === "solidFace" &&
					generatedNode.selection.extrudeId === node.sourceId &&
					generatedNode.selection.faceId === node.faceId
				) {
					addVisibleWithDependencies(generatedNode.id)
				}
			}
			continue
		}
		if (node.type === "edge") {
			for (const generatedNode of nodes) {
				if (
					generatedNode.type === "generatedSolidEdge" &&
					generatedNode.selection?.type === "solidEdge" &&
					generatedNode.selection.extrudeId === node.sourceId &&
					generatedNode.selection.edgeId === node.edgeId
				) {
					addVisibleWithDependencies(generatedNode.id)
				}
			}
		}
	}

	return nodes.filter((node) => !isGeneratedGeometryNode(node) || visibleIds.has(node.id))
}

function isGeneratedGeometryNode(node: GeneratedGraphNode): boolean {
	return (
		node.type === "generatedSketch" ||
		node.type === "generatedSketchVertex" ||
		node.type === "generatedSketchLoop" ||
		node.type === "generatedSketchProfile" ||
		node.type === "generatedSolid" ||
		node.type === "generatedSolidVertex" ||
		node.type === "generatedSolidEdge" ||
		node.type === "generatedSolidFace"
	)
}

function buildGeneratedNodes(state: PCadState, generatedState?: PCadGeneratedState): GeneratedGraphNode[] {
	const nodes = buildSketchDataNodes(state)
	if (!generatedState) {
		return nodes
	}
	for (const feature of generatedState.features) {
		if (feature.type !== "sketch" || !state.nodes.has(feature.id)) {
			continue
		}
		const sketchStateNodeId = getGeneratedSketchNodeId(feature.id)
		nodes.push({
			id: sketchStateNodeId,
			type: "generatedSketch",
			sourceId: feature.id,
			dependencies: [feature.id],
			label: "Generated sketch",
			detail: `${feature.vertices.length} vertices · ${feature.loops.length} loops · ${feature.profiles.length} profiles`,
			rows: [
				{ label: "Vertices", value: String(feature.vertices.length) },
				{ label: "Loops", value: String(feature.loops.length) },
				{ label: "Profiles", value: String(feature.profiles.length) },
				{ label: "Dirty", value: feature.dirty ? "yes" : "no" }
			],
			selection: {
				type: "sketch",
				graphId: sketchStateNodeId,
				sketchId: feature.id
			}
		})
		feature.vertices.forEach((vertex, index) => {
			const graphId = getGeneratedSketchVertexNodeId(feature.id, index)
			nodes.push({
				id: graphId,
				type: "generatedSketchVertex",
				sourceId: feature.id,
				dependencies: [sketchStateNodeId],
				label: `Vertex ${index + 1}`,
				detail: formatPoint2D(vertex),
				rows: [
					{ label: "Index", value: String(index) },
					{ label: "Position", value: formatPoint2D(vertex) }
				],
				selection: {
					type: "sketch",
					graphId,
					sketchId: feature.id
				}
			})
		})
		for (const loop of feature.loops) {
			const loopId = getGeneratedSketchLoopNodeId(feature.id, loop.id)
			const vertexDependencies = loop.vertexIndices.map((vertexIndex) => getGeneratedSketchVertexNodeId(feature.id, vertexIndex)).filter((id) => nodes.some((node) => node.id === id))
			nodes.push({
				id: loopId,
				type: "generatedSketchLoop",
				sourceId: feature.id,
				dependencies: vertexDependencies.length > 0 ? vertexDependencies : [sketchStateNodeId],
				label: loop.id,
				detail: `${loop.vertexIndices.length} vertices`,
				rows: [
					{ label: "Loop ID", value: loop.id },
					{ label: "Vertex indexes", value: loop.vertexIndices.join(", ") || "none" }
				],
				selection: {
					type: "sketch",
					graphId: loopId,
					sketchId: feature.id
				}
			})
		}
		for (const profile of feature.profiles) {
			const graphId = getGeneratedSketchProfileNodeId(feature.id, profile.id)
			const loopDependencies = [profile.outerLoopId, ...profile.holeLoopIds]
				.map((loopId) => getGeneratedSketchLoopNodeId(feature.id, loopId))
				.filter((id) => nodes.some((node) => node.id === id))
			nodes.push({
				id: graphId,
				type: "generatedSketchProfile",
				sourceId: feature.id,
				dependencies: loopDependencies.length > 0 ? loopDependencies : [sketchStateNodeId],
				label: profile.id,
				detail: `${profile.holeLoopIds.length} holes`,
				rows: [
					{ label: "Profile ID", value: profile.id },
					{ label: "Outer loop", value: profile.outerLoopId },
					{ label: "Hole loops", value: profile.holeLoopIds.join(", ") || "none" }
				],
				selection: {
					type: "sketch",
					graphId,
					sketchId: feature.id
				}
			})
		}
	}
	for (const solid of generatedState.solids) {
		if (!state.nodes.has(solid.featureId)) {
			continue
		}
		const solidStateNodeId = getGeneratedSolidNodeId(solid.id)
		nodes.push({
			id: solidStateNodeId,
			type: "generatedSolid",
			sourceId: solid.featureId,
			dependencies: [solid.featureId],
			label: "Generated solid",
			detail: `${solid.vertices.length} vertices · ${solid.edges.length} edges · ${solid.faces.length} faces`,
			rows: [
				{ label: "Solid ID", value: solid.id },
				{ label: "Feature", value: solid.featureId },
				{ label: "Vertices", value: String(solid.vertices.length) },
				{ label: "Edges", value: String(solid.edges.length) },
				{ label: "Faces", value: String(solid.faces.length) }
			]
		})
		for (const vertex of solid.vertices) {
			const graphId = getGeneratedSolidVertexNodeId(solid.id, vertex.id)
			nodes.push({
				id: graphId,
				type: "generatedSolidVertex",
				sourceId: solid.featureId,
				dependencies: [solidStateNodeId],
				label: getShortId(vertex.id),
				detail: formatVector3D(vertex.position),
				rows: solidVertexRows(vertex),
				selection: {
					type: "solidVertex",
					graphId,
					extrudeId: solid.featureId,
					vertexId: vertex.id
				}
			})
		}
		for (const edge of solid.edges) {
			const graphId = getGeneratedSolidEdgeNodeId(solid.id, edge.id)
			const vertexDependencies = edge.vertexIds.map((vertexId) => getGeneratedSolidVertexNodeId(solid.id, vertexId)).filter((id) => nodes.some((node) => node.id === id))
			nodes.push({
				id: graphId,
				type: "generatedSolidEdge",
				sourceId: solid.featureId,
				dependencies: vertexDependencies.length > 0 ? vertexDependencies : [solidStateNodeId],
				label: getShortId(edge.id),
				detail: `${edge.vertexIds.length} vertices`,
				rows: solidEdgeRows(edge),
				selection: {
					type: "solidEdge",
					graphId,
					extrudeId: solid.featureId,
					edgeId: edge.id
				}
			})
		}
		for (const face of solid.faces) {
			const graphId = getGeneratedSolidFaceNodeId(solid.id, face.id)
			const edgeDependencies = face.edgeIds.map((edgeId) => getGeneratedSolidEdgeNodeId(solid.id, edgeId)).filter((id) => nodes.some((node) => node.id === id))
			nodes.push({
				id: graphId,
				type: "generatedSolidFace",
				sourceId: solid.featureId,
				dependencies: edgeDependencies.length > 0 ? edgeDependencies : [solidStateNodeId],
				label: getShortId(face.id),
				detail: `${face.edgeIds.length} edges`,
				rows: solidFaceRows(face),
				selection: {
					type: "solidFace",
					graphId,
					extrudeId: solid.featureId,
					faceId: face.id
				}
			})
		}
	}
	return nodes
}

function buildSketchDataNodes(state: PCadState): GeneratedGraphNode[] {
	const nodes: GeneratedGraphNode[] = []
	for (const node of state.nodes.values()) {
		if (node.type !== "sketch") {
			continue
		}
		for (const entity of node.entities) {
			const graphId = getSketchEntityNodeId(node.id, entity.id)
			nodes.push({
				id: graphId,
				type: "sketchEntity",
				sourceId: node.id,
				dependencies: [node.id],
				label: getSketchEntityLabel(entity),
				detail: getSketchEntityDetail(entity),
				rows: sketchEntityRows(entity),
				selection: {
					type: "sketchEntity",
					graphId,
					sketchId: node.id,
					entityId: entity.id
				}
			})
		}
		for (const dimension of node.dimensions) {
			const entityNodeId = getSketchEntityNodeId(node.id, dimension.entityId)
			const graphId = getSketchDimensionNodeId(node.id, dimension.id)
			nodes.push({
				id: graphId,
				type: "sketchDimension",
				sourceId: node.id,
				dependencies: nodes.some((candidate) => candidate.id === entityNodeId) ? [entityNodeId] : [node.id],
				label: getSketchDimensionLabel(dimension),
				detail: formatNumber(dimension.value),
				rows: sketchDimensionRows(dimension),
				selection: {
					type: "sketchDimension",
					graphId,
					sketchId: node.id,
					entityId: dimension.entityId,
					dimensionType: dimension.type
				}
			})
		}
	}
	return nodes
}

function getSketchEntityNodeId(sketchId: string, entityId: string): string {
	return `data:${sketchId}:entity:${entityId}`
}

function getSketchDimensionNodeId(sketchId: string, dimensionId: string): string {
	return `data:${sketchId}:dimension:${dimensionId}`
}

function getGeneratedSketchNodeId(sketchId: string): string {
	return `generated:${sketchId}:sketch`
}

function getGeneratedSketchVertexNodeId(sketchId: string, vertexIndex: number): string {
	return `generated:${sketchId}:sketch-vertex:${vertexIndex}`
}

function getGeneratedSketchLoopNodeId(sketchId: string, loopId: string): string {
	return `generated:${sketchId}:sketch-loop:${loopId}`
}

function getGeneratedSketchProfileNodeId(sketchId: string, profileId: string): string {
	return `generated:${sketchId}:sketch-profile:${profileId}`
}

function getGeneratedSolidNodeId(solidId: string): string {
	return `generated:${solidId}:solid`
}

function getGeneratedSolidVertexNodeId(solidId: string, vertexId: string): string {
	return `generated:${solidId}:solid-vertex:${vertexId}`
}

function getGeneratedSolidEdgeNodeId(solidId: string, edgeId: string): string {
	return `generated:${solidId}:solid-edge:${edgeId}`
}

function getGeneratedSolidFaceNodeId(solidId: string, faceId: string): string {
	return `generated:${solidId}:solid-face:${faceId}`
}

function getSketchEntityLabel(entity: SketchEntity): string {
	switch (entity.type) {
		case "line":
			return entity.id || "Line"
		case "cornerRectangle":
			return entity.id || "Rectangle"
	}
}

function getSketchEntityDetail(entity: SketchEntity): string {
	switch (entity.type) {
		case "line":
			return `${formatPoint2D(entity.p0)} -> ${formatPoint2D(entity.p1)}`
		case "cornerRectangle":
			return `${formatPoint2D(entity.p0)} -> ${formatPoint2D(entity.p1)}`
	}
}

function getSketchDimensionLabel(dimension: SketchDimension): string {
	switch (dimension.type) {
		case "lineLength":
			return "Line length"
		case "rectangleWidth":
			return "Rectangle width"
		case "rectangleHeight":
			return "Rectangle height"
	}
}

function sketchEntityRows(entity: SketchEntity): readonly { label: string; value: string }[] {
	switch (entity.type) {
		case "line":
			return [
				{ label: "Entity ID", value: entity.id },
				{ label: "Kind", value: "line" },
				{ label: "P0", value: formatPoint2D(entity.p0) },
				{ label: "P1", value: formatPoint2D(entity.p1) }
			]
		case "cornerRectangle":
			return [
				{ label: "Entity ID", value: entity.id },
				{ label: "Kind", value: "cornerRectangle" },
				{ label: "P0", value: formatPoint2D(entity.p0) },
				{ label: "P1", value: formatPoint2D(entity.p1) }
			]
	}
}

function sketchDimensionRows(dimension: SketchDimension): readonly { label: string; value: string }[] {
	return [
		{ label: "Dimension ID", value: dimension.id },
		{ label: "Kind", value: dimension.type },
		{ label: "Entity", value: dimension.entityId },
		{ label: "Value", value: formatNumber(dimension.value) }
	]
}

function solidVertexRows(vertex: SolidVertex): readonly { label: string; value: string }[] {
	return [
		{ label: "Vertex ID", value: vertex.id },
		{ label: "Position", value: formatVector3D(vertex.position) }
	]
}

function solidEdgeRows(edge: SolidEdge): readonly { label: string; value: string }[] {
	return [
		{ label: "Edge ID", value: edge.id },
		{ label: "Vertices", value: edge.vertexIds.join(", ") || "none" }
	]
}

function solidFaceRows(face: SolidFace): readonly { label: string; value: string }[] {
	return [
		{ label: "Face ID", value: face.id },
		{ label: "Edges", value: face.edgeIds.join(", ") || "none" }
	]
}

function getTypeOrder(type: PCadGraphNode["type"]): number {
	switch (type) {
		case "referencePlane":
			return 0
		case "sketch":
			return 1
		case "extrude":
			return 2
		case "face":
			return 3
		case "edge":
			return 4
		case "chamfer":
			return 5
	}
}

function getNodeLabel(node: PCadGraphNode): string {
	return node.name?.trim() || node.id
}

function getNodeDetail(node: PCadGraphNode): string {
	switch (node.type) {
		case "referencePlane":
			return node.plane
		case "sketch":
			return `${node.entities.length} entities`
		case "extrude":
			return `Depth ${formatNumber(node.depth)}`
		case "face":
			return node.faceId
		case "edge":
			return node.edgeId
		case "chamfer":
			return `D1 ${formatNumber(node.d1)}`
	}
}

function isNamedEditableNode(node: PCadGraphNode): node is Extract<PCadGraphNode, { type: "sketch" | "extrude" | "chamfer" }> {
	return node.type === "sketch" || node.type === "extrude" || node.type === "chamfer"
}

function isDeletableNode(node: PCadGraphNode): boolean {
	return node.type === "sketch" || node.type === "extrude" || node.type === "chamfer"
}

function renderPCadNodeComponent(ctx: CanvasRenderingContext2D, component: CanvasComponent<PCadNodeComponentData>, selected: boolean): void {
	const data = component.data
	const palette = getNodePalette(data?.nodeType)
	ctx.save()
	if (selected) {
		const outlineWidth = 4
		ctx.fillStyle = "#2563eb"
		ctx.fillRect(component.x - outlineWidth, component.y - outlineWidth, component.width + outlineWidth * 2, component.height + outlineWidth * 2)
	}

	ctx.fillStyle = "#ffffff"
	ctx.fillRect(component.x, component.y, component.width, component.height)
	if (!selected) {
		ctx.strokeStyle = palette.border
		ctx.lineWidth = 1.5
		ctx.strokeRect(component.x, component.y, component.width, component.height)
	}

	ctx.fillStyle = palette.fill
	ctx.fillRect(component.x, component.y, component.width, 24)
	ctx.fillStyle = palette.text
	ctx.font = "700 11px Inter, Arial, sans-serif"
	ctx.textAlign = "left"
	ctx.textBaseline = "middle"
	ctx.fillText(getNodeTypeCaption(data), component.x + 10, component.y + 12)

	ctx.fillStyle = "#0f172a"
	ctx.font = "600 14px Inter, Arial, sans-serif"
	ctx.fillText(truncateText(ctx, data?.label ?? `Node ${component.id}`, component.width - 20), component.x + 10, component.y + 46)
	ctx.fillStyle = "#475569"
	ctx.font = "12px Inter, Arial, sans-serif"
	ctx.fillText(truncateText(ctx, data?.detail ?? "", component.width - 20), component.x + 10, component.y + 66)
	ctx.restore()
}

function renderPCadDependencyConnection(ctx: CanvasRenderingContext2D, state: { selected: boolean; from: { x: number; y: number }; to: { x: number; y: number } }): void {
	ctx.strokeStyle = state.selected ? "#2563eb" : "#64748b"
	ctx.lineWidth = state.selected ? 3 : 2
	ctx.setLineDash([])
	const midX = (state.from.x + state.to.x) / 2
	ctx.beginPath()
	ctx.moveTo(state.from.x, state.from.y)
	ctx.bezierCurveTo(midX, state.from.y, midX, state.to.y, state.to.x, state.to.y)
	ctx.stroke()
}

function getNodeTypeCaption(data?: PCadNodeComponentData): string {
	if (!data) {
		return "NODE"
	}
	switch (data.nodeType) {
		case "sketchEntity":
			return "SKETCH ENTITY"
		case "sketchDimension":
			return "DIMENSION"
		case "generatedSketch":
			return "SKETCH STATE"
		case "generatedSketchVertex":
			return "SKETCH VERTEX"
		case "generatedSketchLoop":
			return "SKETCH LOOP"
		case "generatedSketchProfile":
			return "SKETCH PROFILE"
		case "generatedSolid":
			return "SOLID STATE"
		case "generatedSolidVertex":
			return "SOLID VERTEX"
		case "generatedSolidEdge":
			return "SOLID EDGE"
		case "generatedSolidFace":
			return "SOLID FACE"
		default:
			return data.nodeType.toUpperCase()
	}
}

function getReadOnlyNodeMode(type: GeneratedNodeType): string {
	return type === "sketchEntity" || type === "sketchDimension" ? "pcad data" : "generated"
}

function getNodePalette(type?: PCadGraphNode["type"] | GeneratedNodeType): { fill: string; border: string; text: string } {
	switch (type) {
		case "referencePlane":
			return { fill: "#e0f2fe", border: "#38bdf8", text: "#075985" }
		case "sketch":
			return { fill: "#dcfce7", border: "#22c55e", text: "#166534" }
		case "extrude":
			return { fill: "#dbeafe", border: "#3b82f6", text: "#1d4ed8" }
		case "face":
			return { fill: "#f1f5f9", border: "#94a3b8", text: "#334155" }
		case "edge":
			return { fill: "#f8fafc", border: "#94a3b8", text: "#334155" }
		case "chamfer":
			return { fill: "#ffedd5", border: "#f97316", text: "#9a3412" }
		case "sketchEntity":
			return { fill: "#ecfccb", border: "#84cc16", text: "#3f6212" }
		case "sketchDimension":
			return { fill: "#fef9c3", border: "#eab308", text: "#854d0e" }
		case "generatedSketch":
			return { fill: "#ccfbf1", border: "#14b8a6", text: "#0f766e" }
		case "generatedSketchVertex":
			return { fill: "#d1fae5", border: "#10b981", text: "#047857" }
		case "generatedSketchLoop":
			return { fill: "#cffafe", border: "#06b6d4", text: "#0e7490" }
		case "generatedSketchProfile":
			return { fill: "#ede9fe", border: "#8b5cf6", text: "#5b21b6" }
		case "generatedSolid":
			return { fill: "#fef3c7", border: "#f59e0b", text: "#92400e" }
		case "generatedSolidVertex":
			return { fill: "#fef3c7", border: "#d97706", text: "#92400e" }
		case "generatedSolidEdge":
			return { fill: "#ffedd5", border: "#f97316", text: "#9a3412" }
		case "generatedSolidFace":
			return { fill: "#fee2e2", border: "#ef4444", text: "#991b1b" }
		default:
			return { fill: "#f8fafc", border: "#cbd5e1", text: "#334155" }
	}
}

function createInspectorTitle(text: string): HTMLHeadingElement {
	const title = document.createElement("h3")
	title.textContent = text
	title.style.margin = "0 0 12px"
	title.style.fontSize = "16px"
	return title
}

function createHelpText(text: string): HTMLParagraphElement {
	const element = document.createElement("p")
	element.textContent = text
	element.style.margin = "0"
	element.style.color = "#475569"
	element.style.fontSize = "13px"
	element.style.lineHeight = "1.5"
	return element
}

function createReadonlyRow(label: string, value: string): HTMLDivElement {
	const row = document.createElement("div")
	row.style.display = "grid"
	row.style.gap = "3px"
	row.style.marginBottom = "10px"
	const labelElement = document.createElement("span")
	labelElement.textContent = label
	labelElement.style.fontSize = "11px"
	labelElement.style.fontWeight = "700"
	labelElement.style.color = "#64748b"
	labelElement.style.textTransform = "uppercase"
	const valueElement = document.createElement("span")
	valueElement.textContent = value
	valueElement.style.fontSize = "13px"
	valueElement.style.color = "#0f172a"
	valueElement.style.overflowWrap = "anywhere"
	row.append(labelElement, valueElement)
	return row
}

function createTextField(label: string, value: string, onChange: (value: string) => void): HTMLLabelElement {
	const field = createFieldShell(label)
	const input = document.createElement("input")
	input.type = "text"
	input.value = value
	input.addEventListener("change", () => onChange(input.value))
	field.appendChild(input)
	return field
}

function createNumberField(label: string, value: number | undefined, onChange: (value: number) => void, fieldId?: string): HTMLLabelElement {
	const field = createFieldShell(label)
	const input = document.createElement("input")
	input.type = "number"
	input.step = "0.1"
	input.min = "0"
	input.value = value === undefined ? "" : String(value)
	if (fieldId) {
		input.dataset.pcadField = fieldId
	}
	input.addEventListener("change", () => onChange(Number.parseFloat(input.value)))
	field.appendChild(input)
	return field
}

function createFieldShell(label: string): HTMLLabelElement {
	const field = document.createElement("label")
	field.style.display = "grid"
	field.style.gap = "5px"
	field.style.marginBottom = "10px"
	field.style.fontSize = "12px"
	field.style.fontWeight = "700"
	field.style.color = "#334155"
	field.appendChild(document.createTextNode(label))
	return field
}

function truncateText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string {
	if (ctx.measureText(text).width <= maxWidth) {
		return text
	}
	let next = text
	while (next.length > 1 && ctx.measureText(`${next}...`).width > maxWidth) {
		next = next.slice(0, -1)
	}
	return `${next}...`
}

function formatNumber(value: number): string {
	return Number.isInteger(value) ? String(value) : value.toFixed(2)
}

function formatPoint2D(point: Point2D): string {
	return `(${formatNumber(point.x)}, ${formatNumber(point.y)})`
}

function formatVector3D(vector: Vector3D): string {
	return `(${formatNumber(vector.x)}, ${formatNumber(vector.y)}, ${formatNumber(vector.z)})`
}

function getShortId(id: string): string {
	const parts = id.split("-")
	return parts.length > 1 ? parts.slice(-2).join("-") : id
}
