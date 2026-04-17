import { resolveSketchTargetFrame } from "./cad/extrude"
import { materializeSketch } from "./cad/sketch"
import type { FaceReference, PartDocument, PartFeature, Sketch, SketchEntity, SketchPlane, SolidExtrude } from "./schema"

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
			return updateSketch(state, action.sketchId, (sketch) => materializeSketch({ ...sketch, entities: [...sketch.entities, action.entity] }), {
				requireDirty: true
			})
		case "undoSketchEntity":
			return updateSketch(state, action.sketchId, (sketch) => materializeSketch({ ...sketch, entities: sketch.entities.slice(0, -1) }), {
				requireDirty: true,
				isApplicable: (sketch) => sketch.entities.length > 0
			})
		case "resetSketch":
			return updateSketch(state, action.sketchId, (sketch) => materializeSketch({ ...sketch, entities: [] }), {
				requireDirty: true,
				isApplicable: (sketch) => sketch.entities.length > 0
			})
		case "finishSketch":
			return updateSketch(state, action.sketchId, (sketch) => materializeSketch({ ...sketch, dirty: false }), {
				requireDirty: true,
				isApplicable: (sketch) => materializeSketch(sketch).profiles.length === 1
			})
		case "createExtrude":
			return createExtrude(state, action)
		case "setExtrudeDepth":
			return setExtrudeDepth(state, action)
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
		vertices: [],
		loops: [],
		profiles: []
	})

	return withFeatures(state, [...state.features, nextSketch])
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

	const materializedSketch = materializeSketch(sketch)
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

	const nextSketch = updater(sketch)
	if (nextSketch === sketch) {
		return state
	}

	const nextFeatures = state.features.slice()
	nextFeatures[sketchIndex] = nextSketch
	return withFeatures(state, nextFeatures)
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

function hasFeatureId(features: PartFeature[], featureId: string): boolean {
	return features.some((feature) => feature.id === featureId)
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
