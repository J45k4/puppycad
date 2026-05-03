import type { PartAction } from "./part-actions"
import { applyPartAction } from "./part-actions"
import type { PartProjectItemData, Project, ProjectDocument, ProjectDocumentType, ProjectNode } from "./contract"
import { normalizeProjectFile } from "./project-file"
import { createPartRuntimeState, createPartRuntimeStateFromFeatures, materializePartFeatures, serializePCadState } from "./pcad/part-state"
import type { PartFeature } from "./schema"

export type ProjectCommand =
	| {
			type: "createItem"
			id: string
			documentType: ProjectDocumentType
			parentId?: string | null
			name?: string
			index?: number
	  }
	| {
			type: "createFolder"
			id: string
			parentId?: string | null
			name?: string
			index?: number
	  }
	| {
			type: "deleteNode"
			nodeId: string
	  }
	| {
			type: "renameNode"
			nodeId: string
			name: string
	  }
	| {
			type: "moveNode"
			nodeId: string
			parentId?: string | null
			index?: number
	  }
	| {
			type: "setNodeVisibility"
			nodeId: string
			visible: boolean
	  }

export type CadCommand =
	| PartAction
	| {
			type: "renameNode"
			nodeId: string
			name: string
	  }
	| {
			type: "deleteNodeCascade"
			nodeId: string
	  }
	| {
			type: "deleteChamfer"
			chamferId: string
	  }

export type SyncedProjectCommand = ProjectCommand | { type: "cad"; partId: string; command: CadCommand }

export class ProjectCommandError extends Error {
	public readonly code: string

	public constructor(code: string, message: string) {
		super(message)
		this.name = "ProjectCommandError"
		this.code = code
	}
}

export function applySyncedProjectCommands(project: Project, commands: readonly SyncedProjectCommand[]): Project {
	const nextProject = cloneProject(project)
	for (const command of commands) {
		applySyncedProjectCommand(nextProject, command)
	}
	return nextProject
}

function applySyncedProjectCommand(project: Project, command: SyncedProjectCommand): void {
	if (!command || typeof command !== "object") {
		throw new ProjectCommandError("invalid_command", "Command must be an object.")
	}
	if (command.type === "cad") {
		applyCadProjectCommand(project, command.partId, command.command)
		return
	}
	applyProjectCommand(project, command)
}

function applyProjectCommand(project: Project, command: ProjectCommand): void {
	switch (command.type) {
		case "createItem":
			createProjectItem(project, command)
			return
		case "createFolder":
			createProjectFolder(project, command)
			return
		case "deleteNode":
			deleteProjectNode(project, command.nodeId)
			return
		case "renameNode":
			renameProjectNode(project, command.nodeId, command.name)
			return
		case "moveNode":
			moveProjectNode(project, command.nodeId, command.parentId ?? null, command.index)
			return
		case "setNodeVisibility":
			setProjectNodeVisibility(project, command.nodeId, command.visible)
			return
	}
}

function createProjectItem(project: Project, command: Extract<ProjectCommand, { type: "createItem" }>): void {
	const id = normalizeCommandId(command.id, "item id")
	if (findNodeById(project.items, id)) {
		throw new ProjectCommandError("duplicate_node_id", `Project node "${id}" already exists.`)
	}
	if (!isProjectDocumentType(command.documentType)) {
		throw new ProjectCommandError("invalid_document_type", "Unsupported project item type.")
	}
	const siblings = resolveContainer(project, command.parentId ?? null)
	const node: ProjectDocument = {
		id,
		type: command.documentType,
		name: normalizeNodeName(command.name, `${capitalize(command.documentType)} ${countDocumentType(project.items, command.documentType) + 1}`),
		...(command.documentType === "part" ? { data: { features: [] } } : {})
	} as ProjectDocument
	insertNode(siblings, node, command.index)
}

function createProjectFolder(project: Project, command: Extract<ProjectCommand, { type: "createFolder" }>): void {
	const id = normalizeCommandId(command.id, "folder id")
	if (findNodeById(project.items, id)) {
		throw new ProjectCommandError("duplicate_node_id", `Project node "${id}" already exists.`)
	}
	const siblings = resolveContainer(project, command.parentId ?? null)
	insertNode(
		siblings,
		{
			id,
			kind: "folder",
			name: normalizeNodeName(command.name, `Folder ${countFolders(project.items) + 1}`),
			items: []
		},
		command.index
	)
}

function deleteProjectNode(project: Project, nodeId: string): void {
	const located = findNodeWithParent(project.items, normalizeCommandId(nodeId, "node id"))
	if (!located) {
		throw new ProjectCommandError("missing_node", `Project node "${nodeId}" does not exist.`)
	}
	located.siblings.splice(located.index, 1)
	project.selectedPath = null
}

function renameProjectNode(project: Project, nodeId: string, name: string): void {
	const node = findNodeById(project.items, normalizeCommandId(nodeId, "node id"))
	if (!node) {
		throw new ProjectCommandError("missing_node", `Project node "${nodeId}" does not exist.`)
	}
	node.name = normalizeNodeName(name, node.name)
}

function moveProjectNode(project: Project, nodeId: string, parentId: string | null, index: number | undefined): void {
	const id = normalizeCommandId(nodeId, "node id")
	const located = findNodeWithParent(project.items, id)
	if (!located) {
		throw new ProjectCommandError("missing_node", `Project node "${nodeId}" does not exist.`)
	}
	if (parentId && isDescendantOf(project.items, parentId, id)) {
		throw new ProjectCommandError("invalid_move", "Cannot move a folder into itself or its descendant.")
	}
	const [node] = located.siblings.splice(located.index, 1)
	if (!node) {
		throw new ProjectCommandError("missing_node", `Project node "${nodeId}" does not exist.`)
	}
	const destination = resolveContainer(project, parentId)
	insertNode(destination, node, index)
	project.selectedPath = null
}

function setProjectNodeVisibility(project: Project, nodeId: string, visible: boolean): void {
	const node = findNodeById(project.items, normalizeCommandId(nodeId, "node id"))
	if (!node) {
		throw new ProjectCommandError("missing_node", `Project node "${nodeId}" does not exist.`)
	}
	if (typeof visible !== "boolean") {
		throw new ProjectCommandError("invalid_visibility", "Visibility must be a boolean.")
	}
	node.visible = visible
}

function applyCadProjectCommand(project: Project, partId: string, command: CadCommand): void {
	const node = findNodeById(project.items, normalizeCommandId(partId, "part id"))
	if (!node || !("type" in node) || node.type !== "part") {
		throw new ProjectCommandError("missing_part", `Part "${partId}" does not exist.`)
	}
	node.data = applyCadCommand(node.data ?? { features: [] }, command)
}

export function applyCadCommand(document: PartProjectItemData, command: CadCommand): PartProjectItemData {
	const runtime = createPartRuntimeState(document)
	const baseDocument: PartProjectItemData = {
		...document,
		features: materializePartFeatures(runtime.cad, runtime.tree)
	}
	const nextDocument = applyCadCommandToFeatures(baseDocument, command)
	if (nextDocument === baseDocument) {
		throw new ProjectCommandError("invalid_cad_command", "CAD command did not apply.")
	}
	const nextRuntime = createPartRuntimeStateFromFeatures(nextDocument.features)
	return {
		...nextDocument,
		cad: serializePCadState(nextRuntime.cad),
		tree: nextRuntime.tree
	}
}

function applyCadCommandToFeatures(document: PartProjectItemData, command: CadCommand): PartProjectItemData {
	if (!command || typeof command !== "object") {
		throw new ProjectCommandError("invalid_cad_command", "CAD command must be an object.")
	}
	if (isPartAction(command)) {
		return applyPartAction(document, command)
	}
	switch (command.type) {
		case "renameNode":
			return renamePartFeature(document, command.nodeId, command.name)
		case "deleteNodeCascade":
			return deletePartFeatureCascade(document, command.nodeId)
		case "deleteChamfer":
			return deleteChamferFeature(document, command.chamferId)
		default:
			throw new ProjectCommandError("invalid_cad_command", "Unsupported CAD command.")
	}
}

function renamePartFeature(document: PartProjectItemData, nodeId: string, name: string): PartProjectItemData {
	const trimmed = normalizeNodeName(name, "")
	if (!trimmed) {
		return document
	}
	const feature = document.features.find((candidate) => candidate.id === nodeId)
	if (!feature || feature.name === trimmed) {
		return document
	}
	if (feature.type === "sketch") {
		return applyPartAction(document, { type: "renameSketch", sketchId: nodeId, name: trimmed })
	}
	return replaceFeatures(document, (candidate) => (candidate.id === nodeId ? { ...candidate, name: trimmed } : candidate))
}

function deletePartFeatureCascade(document: PartProjectItemData, nodeId: string): PartProjectItemData {
	const feature = document.features.find((candidate) => candidate.id === nodeId)
	if (!feature) {
		return document
	}
	if (feature.type === "sketch") {
		return applyPartAction(document, { type: "deleteSketch", sketchId: nodeId })
	}
	if (feature.type === "extrude") {
		return applyPartAction(document, { type: "deleteExtrude", extrudeId: nodeId })
	}
	if (feature.type === "chamfer") {
		return deleteChamferFeature(document, nodeId)
	}
	return document
}

function deleteChamferFeature(document: PartProjectItemData, chamferId: string): PartProjectItemData {
	if (!document.features.some((feature) => feature.type === "chamfer" && feature.id === chamferId)) {
		return document
	}
	return {
		...document,
		features: document.features.filter((feature) => feature.id !== chamferId)
	}
}

function replaceFeatures(document: PartProjectItemData, replacer: (feature: PartFeature) => PartFeature): PartProjectItemData {
	let changed = false
	const features = document.features.map((feature) => {
		const nextFeature = replacer(feature)
		if (nextFeature !== feature) {
			changed = true
		}
		return nextFeature
	})
	return changed ? { ...document, features } : document
}

function isPartAction(command: CadCommand): command is PartAction {
	return (
		command.type === "createSketch" ||
		command.type === "renameSketch" ||
		command.type === "addSketchEntity" ||
		command.type === "undoSketchEntity" ||
		command.type === "resetSketch" ||
		command.type === "finishSketch" ||
		command.type === "createExtrude" ||
		command.type === "setExtrudeDepth" ||
		command.type === "createChamfer" ||
		command.type === "setChamferDistances" ||
		command.type === "setSketchDimension" ||
		command.type === "deleteSketch" ||
		command.type === "deleteExtrude"
	)
}

function resolveContainer(project: Project, parentId: string | null): ProjectNode[] {
	if (!parentId) {
		return project.items
	}
	const parent = findNodeById(project.items, normalizeCommandId(parentId, "parent id"))
	if (!parent || !("kind" in parent) || parent.kind !== "folder") {
		throw new ProjectCommandError("missing_parent", `Folder "${parentId}" does not exist.`)
	}
	return parent.items
}

function insertNode(siblings: ProjectNode[], node: ProjectNode, index: number | undefined): void {
	const insertionIndex = typeof index === "number" && Number.isInteger(index) && index >= 0 ? Math.min(index, siblings.length) : siblings.length
	siblings.splice(insertionIndex, 0, node)
}

function findNodeById(nodes: ProjectNode[], id: string): ProjectNode | null {
	for (const node of nodes) {
		if (node.id === id) {
			return node
		}
		if ("kind" in node && node.kind === "folder") {
			const child = findNodeById(node.items, id)
			if (child) {
				return child
			}
		}
	}
	return null
}

function findNodeWithParent(nodes: ProjectNode[], id: string): { node: ProjectNode; siblings: ProjectNode[]; index: number } | null {
	for (let index = 0; index < nodes.length; index += 1) {
		const node = nodes[index]
		if (!node) {
			continue
		}
		if (node.id === id) {
			return { node, siblings: nodes, index }
		}
		if ("kind" in node && node.kind === "folder") {
			const child = findNodeWithParent(node.items, id)
			if (child) {
				return child
			}
		}
	}
	return null
}

function isDescendantOf(nodes: ProjectNode[], candidateId: string, ancestorId: string): boolean {
	const ancestor = findNodeById(nodes, ancestorId)
	if (!ancestor || !("kind" in ancestor) || ancestor.kind !== "folder") {
		return false
	}
	return !!findNodeById(ancestor.items, candidateId)
}

function countDocumentType(nodes: ProjectNode[], type: ProjectDocumentType): number {
	let count = 0
	for (const node of nodes) {
		if ("type" in node && node.type === type) {
			count += 1
		}
		if ("kind" in node && node.kind === "folder") {
			count += countDocumentType(node.items, type)
		}
	}
	return count
}

function countFolders(nodes: ProjectNode[]): number {
	let count = 0
	for (const node of nodes) {
		if ("kind" in node && node.kind === "folder") {
			count += 1 + countFolders(node.items)
		}
	}
	return count
}

function normalizeCommandId(value: unknown, label: string): string {
	if (typeof value !== "string" || !value.trim()) {
		throw new ProjectCommandError("invalid_id", `Missing ${label}.`)
	}
	return value.trim()
}

function normalizeNodeName(value: unknown, fallback: string): string {
	return typeof value === "string" && value.trim() ? value.trim() : fallback
}

function isProjectDocumentType(value: unknown): value is ProjectDocumentType {
	return value === "schemantic" || value === "pcb" || value === "part" || value === "assembly" || value === "diagram"
}

function capitalize(value: string): string {
	return value.charAt(0).toUpperCase() + value.slice(1)
}

function cloneProject(project: Project): Project {
	const normalized = normalizeProjectFile(JSON.parse(JSON.stringify(project)))
	if (!normalized) {
		throw new ProjectCommandError("invalid_project", "Project cannot be normalized.")
	}
	return normalized
}
