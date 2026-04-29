import { extrudeSolidFeature, resolveSketchTargetFrame } from "./cad/extrude"
import { materializeSketch } from "./cad/sketch"
import type { EdgeReference, FaceReference, PartDocument, PartFeature, Sketch, SketchDimension, SketchEntity, SketchPlane, SolidChamfer, SolidExtrude } from "./schema"

type PartSketchEntity = Extract<SketchEntity, { type: "line" | "cornerRectangle" }>

export type PartAction =
	| {
			type: "createSketch"
			sketchId: string
			name: string
			target: { type: "plane"; plane: SketchPlane } | { type: "face"; face: FaceReference }
	  }
	| {
			type: "renameSketch"
			sketchId: string
			name: string
	  }
	| {
			type: "addSketchEntity"
			sketchId: string
			entity: PartSketchEntity
	  }
	| {
			type: "undoSketchEntity"
			sketchId: string
	  }
	| {
			type: "resetSketch"
			sketchId: string
	  }
	| {
			type: "finishSketch"
			sketchId: string
	  }
	| {
			type: "createExtrude"
			extrudeId: string
			name: string
			sketchId: string
			profileId: string
			depth: number
	  }
	| {
			type: "setExtrudeDepth"
			extrudeId: string
			depth: number
	  }
	| {
			type: "createChamfer"
			chamferId: string
			name: string
			target: {
				edge: EdgeReference
			}
			d1: number
			d2?: number
	  }
	| {
			type: "setChamferDistances"
			chamferId: string
			d1: number
			d2?: number
	  }
	| {
			type: "setSketchDimension"
			sketchId: string
			dimension: SketchDimension
	  }
	| {
			type: "deleteSketch"
			sketchId: string
	  }
	| {
			type: "deleteExtrude"
			extrudeId: string
	  }

export function applyPartAction(state: PartDocument, action: PartAction): PartDocument {
	switch (action.type) {
		case "createSketch":
			return createSketch(state, action)
		case "renameSketch":
			return renameSketch(state, action)
		case "addSketchEntity":
			return updateSketch(state, action.sketchId, (sketch) => materializePartSketch({ ...sketch, entities: [...sketch.entities, action.entity] }), {
				requireDirty: true
			})
		case "undoSketchEntity":
			return updateSketch(state, action.sketchId, (sketch) => materializePartSketch({ ...sketch, entities: sketch.entities.slice(0, -1) }), {
				requireDirty: true,
				isApplicable: (sketch) => sketch.entities.length > 0
			})
		case "resetSketch":
			return updateSketch(state, action.sketchId, (sketch) => materializePartSketch({ ...sketch, entities: [] }), {
				requireDirty: true,
				isApplicable: (sketch) => sketch.entities.length > 0
			})
		case "finishSketch":
			return updateSketch(state, action.sketchId, (sketch) => materializePartSketch({ ...sketch, dirty: false }), {
				requireDirty: true,
				isApplicable: (sketch) => materializeSketch(sketch).profiles.length === 1
			})
		case "createExtrude":
			return createExtrude(state, action)
		case "setExtrudeDepth":
			return setExtrudeDepth(state, action)
		case "createChamfer":
			return createChamfer(state, action)
		case "setChamferDistances":
			return setChamferDistances(state, action)
		case "setSketchDimension":
			return setSketchDimension(state, action)
		case "deleteSketch":
			return deleteFeatureCascade(state, action.sketchId, "sketch")
		case "deleteExtrude":
			return deleteFeatureCascade(state, action.extrudeId, "extrude")
	}
}

function createSketch(state: PartDocument, action: Extract<PartAction, { type: "createSketch" }>): PartDocument {
	const nextName = action.name.trim()
	if (!nextName || hasFeatureId(state.features, action.sketchId) || state.features.some((feature) => feature.type === "sketch" && feature.dirty)) {
		return state
	}
	if (!canResolveSketchTarget(state, action.target)) {
		return state
	}

	const nextSketch = materializeSketch({
		type: "sketch",
		id: action.sketchId,
		name: nextName,
		dirty: true,
		target: action.target,
		entities: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	})

	return withFeatures(state, [...state.features, normalizeSketchDimensions(nextSketch)])
}

function renameSketch(state: PartDocument, action: Extract<PartAction, { type: "renameSketch" }>): PartDocument {
	const nextName = action.name.trim()
	if (!nextName) {
		return state
	}

	return updateSketch(
		state,
		action.sketchId,
		(sketch) => ({
			...sketch,
			name: nextName
		}),
		{
			isApplicable: (sketch) => sketch.name !== nextName
		}
	)
}

function createExtrude(state: PartDocument, action: Extract<PartAction, { type: "createExtrude" }>): PartDocument {
	const nextName = action.name.trim()
	if (!nextName || hasFeatureId(state.features, action.extrudeId) || !Number.isFinite(action.depth) || action.depth <= 0) {
		return state
	}

	const sketchIndex = state.features.findIndex((feature) => feature.type === "sketch" && feature.id === action.sketchId)
	if (sketchIndex < 0) {
		return state
	}

	const sketch = state.features[sketchIndex]
	if (!sketch || sketch.type !== "sketch") {
		return state
	}

	const materializedSketch = materializePartSketch(sketch)
	if (materializedSketch.dirty || materializedSketch.profiles.length !== 1 || materializedSketch.profiles[0]?.id !== action.profileId) {
		return state
	}

	const nextExtrude: SolidExtrude = {
		type: "extrude",
		id: action.extrudeId,
		name: nextName,
		target: {
			type: "profileRef",
			sketchId: materializedSketch.id,
			profileId: action.profileId
		},
		depth: action.depth
	}

	const nextFeatures = state.features.slice()
	nextFeatures[sketchIndex] = materializedSketch
	nextFeatures.push(nextExtrude)
	return withFeatures(state, nextFeatures)
}

function setExtrudeDepth(state: PartDocument, action: Extract<PartAction, { type: "setExtrudeDepth" }>): PartDocument {
	if (!Number.isFinite(action.depth) || action.depth <= 0) {
		return state
	}

	const extrudeIndex = state.features.findIndex((feature) => feature.type === "extrude" && feature.id === action.extrudeId)
	if (extrudeIndex < 0) {
		return state
	}

	const extrude = state.features[extrudeIndex]
	if (!extrude || extrude.type !== "extrude" || extrude.depth === action.depth) {
		return state
	}

	const nextFeatures = state.features.slice()
	nextFeatures[extrudeIndex] = {
		...extrude,
		depth: action.depth
	}
	return withFeatures(state, nextFeatures)
}

function createChamfer(state: PartDocument, action: Extract<PartAction, { type: "createChamfer" }>): PartDocument {
	const nextName = action.name.trim()
	if (
		!nextName ||
		hasFeatureId(state.features, action.chamferId) ||
		!isPositiveFiniteNumber(action.d1) ||
		(action.d2 !== undefined && !isPositiveFiniteNumber(action.d2)) ||
		!canResolveEdgeReference(state, action.target.edge) ||
		findChamferForEdge(state.features, action.target.edge)
	) {
		return state
	}

	const nextChamfer: SolidChamfer = {
		type: "chamfer",
		id: action.chamferId,
		name: nextName,
		target: {
			edge: cloneEdgeReference(action.target.edge)
		},
		d1: action.d1,
		...(action.d2 === undefined ? {} : { d2: action.d2 })
	}

	return withFeatures(state, [...state.features, nextChamfer])
}

function setChamferDistances(state: PartDocument, action: Extract<PartAction, { type: "setChamferDistances" }>): PartDocument {
	if (!isPositiveFiniteNumber(action.d1) || (action.d2 !== undefined && !isPositiveFiniteNumber(action.d2))) {
		return state
	}

	const chamferIndex = state.features.findIndex((feature) => feature.type === "chamfer" && feature.id === action.chamferId)
	if (chamferIndex < 0) {
		return state
	}

	const chamfer = state.features[chamferIndex]
	if (!chamfer || chamfer.type !== "chamfer" || (chamfer.d1 === action.d1 && chamfer.d2 === action.d2) || !canResolveEdgeReference(state, chamfer.target.edge)) {
		return state
	}

	const nextFeatures = state.features.slice()
	const nextChamfer: SolidChamfer = {
		type: "chamfer",
		id: chamfer.id,
		...(chamfer.name === undefined ? {} : { name: chamfer.name }),
		target: chamfer.target,
		d1: action.d1,
		...(action.d2 === undefined ? {} : { d2: action.d2 })
	}
	nextFeatures[chamferIndex] = nextChamfer
	return withFeatures(state, nextFeatures)
}

function setSketchDimension(state: PartDocument, action: Extract<PartAction, { type: "setSketchDimension" }>): PartDocument {
	if (!Number.isFinite(action.dimension.value) || action.dimension.value <= 0) {
		return state
	}

	return updateSketch(
		state,
		action.sketchId,
		(sketch) => {
			const entityIndex = sketch.entities.findIndex((entity) => entity.id === action.dimension.entityId)
			if (entityIndex < 0) {
				return sketch
			}

			const entity = sketch.entities[entityIndex]
			if (!entity || !canApplyDimensionToEntity(entity, action.dimension)) {
				return sketch
			}

			const nextEntities = sketch.entities.slice()
			nextEntities[entityIndex] = applyDimensionToEntity(entity, action.dimension)
			return materializePartSketch({
				...sketch,
				entities: nextEntities,
				dimensions: upsertSketchDimension(sketch.dimensions, action.dimension)
			})
		},
		{
			isApplicable: (sketch) => {
				const entity = sketch.entities.find((candidate) => candidate.id === action.dimension.entityId)
				return !!entity && canApplyDimensionToEntity(entity, action.dimension)
			}
		}
	)
}

function deleteFeatureCascade(state: PartDocument, featureId: string, type: PartFeature["type"]): PartDocument {
	const feature = state.features.find((candidate) => candidate.type === type && candidate.id === featureId)
	if (!feature) {
		return state
	}

	const removedIds = collectDependentFeatureIds(state.features, [feature.id])
	return withFeatures(
		state,
		state.features.filter((candidate) => !removedIds.has(candidate.id))
	)
}

function updateSketch(
	state: PartDocument,
	sketchId: string,
	updater: (sketch: Sketch) => Sketch,
	options: {
		requireDirty?: boolean
		isApplicable?: (sketch: Sketch) => boolean
	} = {}
): PartDocument {
	const sketchIndex = state.features.findIndex((feature) => feature.type === "sketch" && feature.id === sketchId)
	if (sketchIndex < 0) {
		return state
	}

	const sketch = state.features[sketchIndex]
	if (!sketch || sketch.type !== "sketch" || (options.requireDirty && !sketch.dirty) || (options.isApplicable && !options.isApplicable(sketch))) {
		return state
	}

	const nextSketch = normalizeSketchDimensions(updater(sketch))
	if (nextSketch === sketch) {
		return state
	}

	const nextFeatures = state.features.slice()
	nextFeatures[sketchIndex] = nextSketch
	return withFeatures(state, nextFeatures)
}

function materializePartSketch(sketch: Sketch): Sketch {
	return normalizeSketchDimensions(materializeSketch(sketch))
}

function normalizeSketchDimensions(sketch: Sketch): Sketch {
	const dimensions = sketch.dimensions.filter((dimension) => {
		const entity = sketch.entities.find((candidate) => candidate.id === dimension.entityId)
		return !!entity && canApplyDimensionToEntity(entity, dimension) && Number.isFinite(dimension.value) && dimension.value > 0
	})
	return dimensions.length === sketch.dimensions.length ? sketch : { ...sketch, dimensions }
}

function canApplyDimensionToEntity(entity: SketchEntity, dimension: SketchDimension): boolean {
	if (entity.type === "line") {
		return dimension.type === "lineLength"
	}
	return dimension.type === "rectangleWidth" || dimension.type === "rectangleHeight"
}

function applyDimensionToEntity(entity: SketchEntity, dimension: SketchDimension): SketchEntity {
	if (entity.type === "line" && dimension.type === "lineLength") {
		const dx = entity.p1.x - entity.p0.x
		const dy = entity.p1.y - entity.p0.y
		const length = Math.hypot(dx, dy)
		if (length <= 1e-9) {
			return {
				...entity,
				p1: {
					x: entity.p0.x + dimension.value,
					y: entity.p0.y
				}
			}
		}
		const scale = dimension.value / length
		return {
			...entity,
			p1: {
				x: entity.p0.x + dx * scale,
				y: entity.p0.y + dy * scale
			}
		}
	}

	if (entity.type === "cornerRectangle" && dimension.type === "rectangleWidth") {
		const sign = entity.p1.x === entity.p0.x ? 1 : Math.sign(entity.p1.x - entity.p0.x)
		return {
			...entity,
			p1: {
				x: entity.p0.x + sign * dimension.value,
				y: entity.p1.y
			}
		}
	}

	if (entity.type === "cornerRectangle" && dimension.type === "rectangleHeight") {
		const sign = entity.p1.y === entity.p0.y ? 1 : Math.sign(entity.p1.y - entity.p0.y)
		return {
			...entity,
			p1: {
				x: entity.p1.x,
				y: entity.p0.y + sign * dimension.value
			}
		}
	}

	return entity
}

function upsertSketchDimension(dimensions: SketchDimension[], nextDimension: SketchDimension): SketchDimension[] {
	const nextDimensions = dimensions.filter((dimension) => !(dimension.entityId === nextDimension.entityId && dimension.type === nextDimension.type))
	nextDimensions.push(nextDimension)
	return nextDimensions
}

function collectDependentFeatureIds(features: PartFeature[], seedFeatureIds: Iterable<string>): Set<string> {
	const removedIds = new Set(seedFeatureIds)
	let changed = true
	while (changed) {
		changed = false
		for (const feature of features) {
			if (removedIds.has(feature.id)) {
				continue
			}
			if (feature.type === "extrude" && removedIds.has(feature.target.sketchId)) {
				removedIds.add(feature.id)
				changed = true
				continue
			}
			if (feature.type === "sketch" && feature.target.type === "face" && removedIds.has(feature.target.face.extrudeId)) {
				removedIds.add(feature.id)
				changed = true
				continue
			}
			if (feature.type === "chamfer" && removedIds.has(feature.target.edge.extrudeId)) {
				removedIds.add(feature.id)
				changed = true
			}
		}
	}
	return removedIds
}

function canResolveSketchTarget(state: Pick<PartDocument, "features">, target: Sketch["target"]): boolean {
	try {
		resolveSketchTargetFrame(state, target)
		return true
	} catch (_error) {
		return false
	}
}

function canResolveEdgeReference(state: Pick<PartDocument, "features">, reference: EdgeReference): boolean {
	const extrude = state.features.find((feature) => feature.type === "extrude" && feature.id === reference.extrudeId)
	if (!extrude || extrude.type !== "extrude") {
		return false
	}
	try {
		return extrudeSolidFeature(state, extrude).solid.edges.some((edge) => edge.id === reference.edgeId)
	} catch (_error) {
		return false
	}
}

function hasFeatureId(features: PartFeature[], featureId: string): boolean {
	return features.some((feature) => feature.id === featureId)
}

function findChamferForEdge(features: PartFeature[], reference: EdgeReference): SolidChamfer | null {
	const chamfer = features.find((feature) => feature.type === "chamfer" && edgeReferencesEqual(feature.target.edge, reference))
	return chamfer?.type === "chamfer" ? chamfer : null
}

function edgeReferencesEqual(a: EdgeReference, b: EdgeReference): boolean {
	return a.extrudeId === b.extrudeId && a.edgeId === b.edgeId
}

function cloneEdgeReference(reference: EdgeReference): EdgeReference {
	return {
		type: "extrudeEdge",
		extrudeId: reference.extrudeId,
		edgeId: reference.edgeId
	}
}

function isPositiveFiniteNumber(value: number): boolean {
	return Number.isFinite(value) && value > 0
}

function withFeatures(state: PartDocument, features: PartFeature[]): PartDocument {
	if (features === state.features) {
		return state
	}

	return {
		features,
		...(state.migrationWarnings ? { migrationWarnings: state.migrationWarnings } : {})
	}
}
