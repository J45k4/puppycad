import type { CornerRectangle, Line, PCadState, SketchEntity, SketchEntityNode } from "../schema"

export function isSketchEntityNode(node: { type: string } | null | undefined): node is SketchEntityNode {
	return node?.type === "sketchLine" || node?.type === "sketchCornerRectangle"
}

export function sketchEntityToNode(sketchId: string, entity: SketchEntity): SketchEntityNode {
	switch (entity.type) {
		case "line":
			return {
				id: entity.id,
				type: "sketchLine",
				sketchId,
				p0: clonePoint(entity.p0),
				p1: clonePoint(entity.p1)
			}
		case "cornerRectangle":
			return {
				id: entity.id,
				type: "sketchCornerRectangle",
				sketchId,
				p0: clonePoint(entity.p0),
				p1: clonePoint(entity.p1)
			}
	}
}

export function sketchEntityNodeToEntity(node: SketchEntityNode): SketchEntity {
	switch (node.type) {
		case "sketchLine":
			return {
				id: node.id,
				type: "line",
				p0: clonePoint(node.p0),
				p1: clonePoint(node.p1)
			} satisfies Line
		case "sketchCornerRectangle":
			return {
				id: node.id,
				type: "cornerRectangle",
				p0: clonePoint(node.p0),
				p1: clonePoint(node.p1)
			} satisfies CornerRectangle
	}
}

export function getSketchEntityNodes(state: PCadState, sketchId: string): SketchEntityNode[] {
	return [...state.nodes.values()].filter((node): node is SketchEntityNode => isSketchEntityNode(node) && node.sketchId === sketchId)
}

export function getSketchEntities(state: PCadState, sketchId: string): SketchEntity[] {
	return getSketchEntityNodes(state, sketchId).map(sketchEntityNodeToEntity)
}

function clonePoint<T extends { x: number; y: number }>(point: T): T {
	return { ...point }
}
