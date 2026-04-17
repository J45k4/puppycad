import type { PartDocument, ProfileReference, SketchPlane, Solid, SolidEdge, SolidExtrude, SolidFace, SolidVertex } from "../schema"
import { materializeSketch, resolveProfileLoops } from "./sketch"
import type { Point2D, Vector3D } from "../types"

export interface ExtrudeFeatureOptions {
	solidId?: string
	startOffset?: number
}

export interface ExtrudedSolid {
	solid: Solid
	plane: SketchPlane
	depth: number
	profileLoops: Point2D[][]
}

type NormalizedLoop = {
	points: Point2D[]
}

const DEFAULT_PLANE: SketchPlane = "XY"

export function extrudeSolidFeature(part: Pick<PartDocument, "features">, feature: SolidExtrude, options: ExtrudeFeatureOptions = {}): ExtrudedSolid {
	if (!Number.isFinite(feature.depth) || feature.depth <= 0) {
		throw new Error("Extrude depth must be a finite number greater than zero.")
	}

	const sketch = resolveSketchForProfile(part, feature.target)
	const materializedSketch = materializeSketch(sketch)
	const profileLoops = resolveProfileLoops(materializedSketch, feature.target.profileId)
	const normalizedLoops = normalizeLoops(profileLoops)
	if (normalizedLoops.length === 0) {
		throw new Error(`Profile "${feature.target.profileId}" does not contain any valid closed loops.`)
	}

	const solidId = options.solidId ?? `${feature.id}-solid`
	const startOffset = options.startOffset ?? 0
	const endOffset = startOffset + feature.depth
	const plane = materializedSketch.target.plane ?? DEFAULT_PLANE

	const vertices: SolidVertex[] = []
	const edges: SolidEdge[] = []
	const faces: SolidFace[] = []
	const bottomVertexIds = new Map<string, string>()
	const topVertexIds = new Map<string, string>()

	const addVertex = (position: Vector3D): string => {
		const id = `${solidId}-vertex-${vertices.length + 1}`
		vertices.push({ id, position })
		return id
	}

	const addEdge = (vertexIds: string[]): string => {
		const id = `${solidId}-edge-${edges.length + 1}`
		edges.push({ id, vertexIds })
		return id
	}

	const addFace = (edgeIds: string[]): string => {
		const id = `${solidId}-face-${faces.length + 1}`
		faces.push({ id, edgeIds })
		return id
	}

	for (let loopIndex = 0; loopIndex < normalizedLoops.length; loopIndex += 1) {
		const loop = normalizedLoops[loopIndex]
		if (!loop) {
			continue
		}
		for (let pointIndex = 0; pointIndex < loop.points.length; pointIndex += 1) {
			const point = loop.points[pointIndex]
			if (!point) {
				continue
			}
			const pointKey = `${loopIndex}:${pointIndex}`
			bottomVertexIds.set(pointKey, addVertex(projectPoint(point, plane, startOffset)))
			topVertexIds.set(pointKey, addVertex(projectPoint(point, plane, endOffset)))
		}
	}

	const bottomFaceEdges: string[] = []
	const topFaceEdges: string[] = []

	for (let loopIndex = 0; loopIndex < normalizedLoops.length; loopIndex += 1) {
		const loop = normalizedLoops[loopIndex]
		if (!loop) {
			continue
		}

		for (let pointIndex = 0; pointIndex < loop.points.length; pointIndex += 1) {
			const nextIndex = (pointIndex + 1) % loop.points.length
			const bottomStartId = getRequiredId(bottomVertexIds, `${loopIndex}:${pointIndex}`, "bottom vertex")
			const bottomEndId = getRequiredId(bottomVertexIds, `${loopIndex}:${nextIndex}`, "bottom vertex")
			const topStartId = getRequiredId(topVertexIds, `${loopIndex}:${pointIndex}`, "top vertex")
			const topEndId = getRequiredId(topVertexIds, `${loopIndex}:${nextIndex}`, "top vertex")

			const bottomEdgeId = addEdge([bottomStartId, bottomEndId])
			const topEdgeId = addEdge([topStartId, topEndId])
			const sideStartEdgeId = addEdge([bottomStartId, topStartId])
			const sideEndEdgeId = addEdge([bottomEndId, topEndId])

			bottomFaceEdges.push(bottomEdgeId)
			topFaceEdges.push(topEdgeId)
			addFace([bottomEdgeId, sideEndEdgeId, topEdgeId, sideStartEdgeId])
		}
	}

	addFace(bottomFaceEdges)
	addFace(topFaceEdges)

	return {
		solid: {
			id: solidId,
			featureId: feature.id,
			vertices,
			edges,
			faces
		},
		plane,
		depth: feature.depth,
		profileLoops: normalizedLoops.map((loop) => loop.points.map(clonePoint))
	}
}

function resolveSketchForProfile(part: Pick<PartDocument, "features">, reference: ProfileReference) {
	const sketch = part.features.find((feature) => feature.type === "sketch" && feature.id === reference.sketchId)
	if (!sketch || sketch.type !== "sketch") {
		throw new Error(`Sketch "${reference.sketchId}" does not exist.`)
	}
	return sketch
}

function normalizeLoops(profileLoops: Point2D[][]): NormalizedLoop[] {
	return profileLoops.map((loop) => normalizeLoop(loop)).filter((loop): loop is NormalizedLoop => loop !== null)
}

function normalizeLoop(loop: Point2D[]): NormalizedLoop | null {
	const points: Point2D[] = []
	for (const point of loop) {
		const previous = points[points.length - 1]
		if (!previous || previous.x !== point.x || previous.y !== point.y) {
			points.push(clonePoint(point))
		}
	}

	if (points.length >= 2) {
		const first = points[0]
		const last = points[points.length - 1]
		if (first && last && first.x === last.x && first.y === last.y) {
			points.pop()
		}
	}

	return points.length >= 3 ? { points } : null
}

function projectPoint(point: Point2D, plane: SketchPlane, extrusionOffset: number): Vector3D {
	switch (plane) {
		case "YZ":
			return { x: extrusionOffset, y: point.x, z: point.y }
		case "XZ":
			return { x: point.x, y: extrusionOffset, z: point.y }
		default:
			return { x: point.x, y: point.y, z: extrusionOffset }
	}
}

function getRequiredId(store: Map<string, string>, key: string, label: string): string {
	const id = store.get(key)
	if (!id) {
		throw new Error(`Missing ${label} for ${key}.`)
	}
	return id
}

function clonePoint(point: Point2D): Point2D {
	return { x: point.x, y: point.y }
}
