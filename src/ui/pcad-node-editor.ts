import type { ChamferNode, ExtrudeNode, PCadGraphNode, PCadState } from "../schema"
import { getNodeDependencies } from "../pcad/runtime"
import { EditorCanvas, type CanvasComponent, type Connection } from "./canvas"
import { UiComponent } from "./ui"

type PCadNodeComponentData = {
	nodeId: string
	nodeType: PCadGraphNode["type"]
	label: string
	detail: string
	editable: boolean
}

type PCadNodeEditorOptions = {
	state: PCadState
	selectedNodeId?: string | null
	onSelectNode?: (nodeId: string | null) => void
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
	private readonly options: Omit<PCadNodeEditorOptions, "state" | "selectedNodeId">
	private readonly nodePositions = new Map<string, { x: number; y: number }>()
	private nodeIdByComponentId = new Map<number, string>()
	private componentIdByNodeId = new Map<string, number>()
	private state: PCadState
	private selectedNodeId: string | null
	private suppressSelectionChange = false

	public constructor(options: PCadNodeEditorOptions) {
		super(document.createElement("div"))
		this.options = {
			onSelectNode: options.onSelectNode,
			onRenameNode: options.onRenameNode,
			onSetExtrudeDepth: options.onSetExtrudeDepth,
			onSetChamferDistances: options.onSetChamferDistances,
			onDeleteNode: options.onDeleteNode
		}
		this.state = options.state
		this.selectedNodeId = options.selectedNodeId ?? null

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
		this.root.appendChild(graphPane)

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

		const graph = buildGraph(this.state, this.nodePositions)
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

	public update(state: PCadState, selectedNodeId?: string | null): void {
		this.state = state
		this.selectedNodeId = selectedNodeId ?? null
		this.applyGraph(buildGraph(this.state, this.nodePositions))
	}

	public getCanvasForTesting(): EditorCanvas<PCadNodeComponentData> {
		return this.editor
	}

	private applyGraph(graph: { components: CanvasComponent<PCadNodeComponentData>[]; connections: Connection[] }): void {
		this.nodeIdByComponentId = new Map(graph.components.map((component) => [component.id, component.data?.nodeId ?? ""]))
		this.componentIdByNodeId = new Map(graph.components.map((component) => [component.data?.nodeId ?? "", component.id]))
		this.editor.setComponents(graph.components)
		this.editor.setConnections(graph.connections)
		this.renderInspector()
		const selectedComponentId = this.selectedNodeId ? this.componentIdByNodeId.get(this.selectedNodeId) : undefined
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
			const nodeId = component.data?.nodeId
			if (nodeId) {
				this.nodePositions.set(nodeId, { x: component.x, y: component.y })
			}
		}
	}

	private handleSelectionChange(ids: number[]): void {
		if (this.suppressSelectionChange) {
			return
		}
		const nodeId = ids.length > 0 ? (this.nodeIdByComponentId.get(ids[0] ?? -1) ?? null) : null
		this.selectedNodeId = nodeId || null
		this.renderInspector()
		this.options.onSelectNode?.(this.selectedNodeId)
	}

	private renderInspector(): void {
		this.inspector.innerHTML = ""
		const node = this.selectedNodeId ? this.state.nodes.get(this.selectedNodeId) : undefined
		if (!node) {
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
}

function buildGraph(state: PCadState, positions: ReadonlyMap<string, { x: number; y: number }>): { components: CanvasComponent<PCadNodeComponentData>[]; connections: Connection[] } {
	const nodes = getSortedNodes(state)
	const depthByNodeId = computeDepths(state, nodes)
	const rowsByDepth = new Map<number, PCadGraphNode[]>()
	for (const node of nodes) {
		const depth = depthByNodeId.get(node.id) ?? 0
		rowsByDepth.set(depth, [...(rowsByDepth.get(depth) ?? []), node])
	}

	const componentIdByNodeId = new Map<string, number>()
	const components: CanvasComponent<PCadNodeComponentData>[] = []
	for (let index = 0; index < nodes.length; index += 1) {
		const node = nodes[index]
		if (!node) {
			continue
		}
		componentIdByNodeId.set(node.id, index + 1)
	}
	for (const node of nodes) {
		const componentId = componentIdByNodeId.get(node.id)
		if (!componentId) {
			continue
		}
		const depth = depthByNodeId.get(node.id) ?? 0
		const row = rowsByDepth.get(depth)?.findIndex((candidate) => candidate.id === node.id) ?? 0
		const savedPosition = positions.get(node.id)
		components.push({
			id: componentId,
			x: savedPosition?.x ?? NODE_START_X + depth * COLUMN_GAP,
			y: savedPosition?.y ?? NODE_START_Y + row * ROW_GAP,
			width: NODE_WIDTH,
			height: NODE_HEIGHT,
			data: {
				nodeId: node.id,
				nodeType: node.type,
				label: getNodeLabel(node),
				detail: getNodeDetail(node),
				editable: isNamedEditableNode(node)
			}
		})
	}

	const connections: Connection[] = []
	for (const node of nodes) {
		const toId = componentIdByNodeId.get(node.id)
		if (!toId) {
			continue
		}
		for (const dependencyId of getNodeDependencies(node)) {
			const fromId = componentIdByNodeId.get(dependencyId)
			if (!fromId) {
				continue
			}
			connections.push({
				from: { componentId: fromId, edge: "right", ratio: 0.5 },
				to: { componentId: toId, edge: "left", ratio: 0.5 }
			})
		}
	}

	return { components, connections }
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

function computeDepths(state: PCadState, nodes: readonly PCadGraphNode[]): Map<string, number> {
	const visiting = new Set<string>()
	const depthById = new Map<string, number>()
	const visit = (nodeId: string): number => {
		const cached = depthById.get(nodeId)
		if (cached !== undefined) {
			return cached
		}
		if (visiting.has(nodeId)) {
			return 0
		}
		visiting.add(nodeId)
		const node = state.nodes.get(nodeId)
		const dependencies = node ? getNodeDependencies(node).filter((dependencyId) => state.nodes.has(dependencyId)) : []
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
	ctx.fillStyle = "#ffffff"
	ctx.strokeStyle = selected ? "#2563eb" : palette.border
	ctx.lineWidth = selected ? 3 : 1.5
	ctx.fillRect(component.x, component.y, component.width, component.height)
	ctx.strokeRect(component.x, component.y, component.width, component.height)

	ctx.fillStyle = palette.fill
	ctx.fillRect(component.x, component.y, component.width, 24)
	ctx.fillStyle = palette.text
	ctx.font = "700 11px Inter, Arial, sans-serif"
	ctx.textAlign = "left"
	ctx.textBaseline = "middle"
	ctx.fillText((data?.nodeType ?? "node").toUpperCase(), component.x + 10, component.y + 12)

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

function getNodePalette(type?: PCadGraphNode["type"]): { fill: string; border: string; text: string } {
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
