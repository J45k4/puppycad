import type { PCadState, SketchConstraint, SketchConstraintNode, SketchDimension } from "../schema"

export function isSketchConstraintNode(node: { type: string } | null | undefined): node is SketchConstraintNode {
	return node?.type === "sketchConstraint"
}

export function isSketchDimensionConstraint(constraint: SketchConstraint): constraint is Extract<SketchConstraint, { type: SketchDimension["type"] }> {
	return constraint.type === "lineLength" || constraint.type === "rectangleWidth" || constraint.type === "rectangleHeight"
}

export function sketchDimensionToConstraintNode(sketchId: string, dimension: SketchDimension, id = dimension.id): SketchConstraintNode {
	return {
		id,
		type: "sketchConstraint",
		sketchId,
		constraint: {
			type: dimension.type,
			entityId: dimension.entityId,
			value: dimension.value
		}
	}
}

export function sketchConstraintNodeToDimension(node: SketchConstraintNode): SketchDimension | null {
	if (!isSketchDimensionConstraint(node.constraint)) {
		return null
	}
	return {
		id: node.id,
		type: node.constraint.type,
		entityId: node.constraint.entityId,
		value: node.constraint.value
	} as SketchDimension
}

export function getSketchConstraintNodes(state: PCadState, sketchId: string): SketchConstraintNode[] {
	return [...state.nodes.values()].filter((node): node is SketchConstraintNode => isSketchConstraintNode(node) && node.sketchId === sketchId)
}

export function getSketchDimensionConstraintNodes(state: PCadState, sketchId: string): SketchConstraintNode[] {
	return getSketchConstraintNodes(state, sketchId).filter((node) => isSketchDimensionConstraint(node.constraint))
}

export function getSketchConstraintNodesForEntity(state: PCadState, sketchId: string, entityId: string): SketchConstraintNode[] {
	return getSketchConstraintNodes(state, sketchId).filter((node) => getSketchConstraintEntityIds(node.constraint).includes(entityId))
}

export function getSketchDimensions(state: PCadState, sketchId: string, legacyDimensions: readonly SketchDimension[] = []): SketchDimension[] {
	const dimensions = [...legacyDimensions.map((dimension) => ({ ...dimension }))]
	for (const constraintNode of getSketchDimensionConstraintNodes(state, sketchId)) {
		const dimension = sketchConstraintNodeToDimension(constraintNode)
		if (!dimension) {
			continue
		}
		const existingIndex = dimensions.findIndex((item) => item.entityId === dimension.entityId && item.type === dimension.type)
		if (existingIndex >= 0) {
			dimensions[existingIndex] = dimension
		} else {
			dimensions.push(dimension)
		}
	}
	return dimensions
}

export function findSketchDimensionConstraintNode(state: PCadState, sketchId: string, dimension: Pick<SketchDimension, "type" | "entityId">): SketchConstraintNode | undefined {
	return getSketchDimensionConstraintNodes(state, sketchId).find(
		(node) => isSketchDimensionConstraint(node.constraint) && node.constraint.type === dimension.type && node.constraint.entityId === dimension.entityId
	)
}

export function getSketchConstraintEntityIds(constraint: SketchConstraint): string[] {
	switch (constraint.type) {
		case "lineLength":
		case "rectangleWidth":
		case "rectangleHeight":
		case "horizontal":
		case "vertical":
			return [constraint.entityId]
		case "coincident":
			return [constraint.a.entityId, constraint.b.entityId]
	}
}
