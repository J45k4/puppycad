import type { ExtrudeEdgeSelector, ExtrudeFaceSelector, FeatureId, Profile, Solid, SolidEdge, SolidFace, SolidVertex } from "../contract"
import type { Vector3D } from "../types"

export interface ExtrudeProfileOptions {
	featureId: FeatureId
	distance: number
	direction?: "positive" | "negative"
	plane?: "XY" | "YZ" | "XZ"
	solidId?: string
	startOffset?: number
}

export interface ExtrudedSolid {
	solid: Solid
	faces: {
		top: string
		bottom: string
		sides: string[]
	}
	edges: {
		top: string[]
		bottom: string[]
		sides: string[]
		topLoops: string[][]
		bottomLoops: string[][]
	}
	vertices: {
		top: string[]
		bottom: string[]
		topLoops: string[][]
		bottomLoops: string[][]
	}
}

type NormalizedProfileLoop = {
	vertexIndexes: number[]
}

const DEFAULT_PLANE = "XY"
const DEFAULT_DIRECTION = "positive"

export function extrudeProfile(profile: Profile, options: ExtrudeProfileOptions): ExtrudedSolid {
	if (!Number.isFinite(options.distance) || options.distance <= 0) {
		throw new Error("Extrude distance must be a finite number greater than zero.")
	}

	const plane = options.plane ?? DEFAULT_PLANE
	const direction = options.direction ?? DEFAULT_DIRECTION
	const startOffset = options.startOffset ?? 0
	const signedDistance = direction === "negative" ? -options.distance : options.distance
	const endOffset = startOffset + signedDistance
	const solidId = options.solidId ?? `${options.featureId}-solid`
	const loops = normalizeProfileLoops(profile)

	if (loops.length === 0) {
		throw new Error(`Profile "${profile.id}" does not contain any valid closed loops.`)
	}

	const vertices: SolidVertex[] = []
	const edges: SolidEdge[] = []
	const faces: SolidFace[] = []
	const bottomVertexIds = new Map<number, string>()
	const topVertexIds = new Map<number, string>()
	const sideEdgeIds = new Map<number, string>()
	const referencedVertexIndexes: number[] = []

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

	for (const loop of loops) {
		for (const vertexIndex of loop.vertexIndexes) {
			if (bottomVertexIds.has(vertexIndex)) {
				continue
			}

			const point = profile.vertices[vertexIndex]
			if (!point) {
				continue
			}

			referencedVertexIndexes.push(vertexIndex)
			bottomVertexIds.set(vertexIndex, addVertex(projectPoint(point, plane, startOffset)))
			topVertexIds.set(vertexIndex, addVertex(projectPoint(point, plane, endOffset)))
		}
	}

	for (const vertexIndex of referencedVertexIndexes) {
		const bottomVertexId = bottomVertexIds.get(vertexIndex)
		const topVertexId = topVertexIds.get(vertexIndex)
		if (!bottomVertexId || !topVertexId) {
			continue
		}

		sideEdgeIds.set(vertexIndex, addEdge([bottomVertexId, topVertexId]))
	}

	const bottomLoopVertexIds = loops.map((loop) => loop.vertexIndexes.map((vertexIndex) => getRequiredId(bottomVertexIds, vertexIndex, "bottom vertex")))
	const topLoopVertexIds = loops.map((loop) => loop.vertexIndexes.map((vertexIndex) => getRequiredId(topVertexIds, vertexIndex, "top vertex")))
	const flatBottomVertexIds = bottomLoopVertexIds.flat()
	const flatTopVertexIds = topLoopVertexIds.flat()
	const bottomLoopEdgeIds: string[][] = []
	const topLoopEdgeIds: string[][] = []
	const sideFaceIds: string[] = []

	for (const loop of loops) {
		const loopBottomEdges: string[] = []
		const loopTopEdges: string[] = []

		for (let index = 0; index < loop.vertexIndexes.length; index += 1) {
			const startVertexIndex = loop.vertexIndexes[index]
			const endVertexIndex = loop.vertexIndexes[(index + 1) % loop.vertexIndexes.length]

			if (startVertexIndex === undefined || endVertexIndex === undefined) {
				continue
			}

			const bottomStartId = getRequiredId(bottomVertexIds, startVertexIndex, "bottom vertex")
			const bottomEndId = getRequiredId(bottomVertexIds, endVertexIndex, "bottom vertex")
			const topStartId = getRequiredId(topVertexIds, startVertexIndex, "top vertex")
			const topEndId = getRequiredId(topVertexIds, endVertexIndex, "top vertex")
			const bottomEdgeId = addEdge([bottomStartId, bottomEndId])
			const topEdgeId = addEdge([topStartId, topEndId])

			loopBottomEdges.push(bottomEdgeId)
			loopTopEdges.push(topEdgeId)

			const sideEndEdgeId = getRequiredId(sideEdgeIds, endVertexIndex, "side edge")
			const sideStartEdgeId = getRequiredId(sideEdgeIds, startVertexIndex, "side edge")

			sideFaceIds.push(addFace([bottomEdgeId, sideEndEdgeId, topEdgeId, sideStartEdgeId]))
		}

		bottomLoopEdgeIds.push(loopBottomEdges)
		topLoopEdgeIds.push(loopTopEdges)
	}

	const bottomFaceId = addFace(bottomLoopEdgeIds.flat())
	const topFaceId = addFace(topLoopEdgeIds.flat())

	return {
		solid: {
			id: solidId,
			featureId: options.featureId,
			vertices,
			edges,
			faces
		},
		faces: {
			top: topFaceId,
			bottom: bottomFaceId,
			sides: sideFaceIds
		},
		edges: {
			top: topLoopEdgeIds.flat(),
			bottom: bottomLoopEdgeIds.flat(),
			sides: referencedVertexIndexes.map((vertexIndex) => getRequiredId(sideEdgeIds, vertexIndex, "side edge")),
			topLoops: topLoopEdgeIds,
			bottomLoops: bottomLoopEdgeIds
		},
		vertices: {
			top: flatTopVertexIds,
			bottom: flatBottomVertexIds,
			topLoops: topLoopVertexIds,
			bottomLoops: bottomLoopVertexIds
		}
	}
}

export function resolveExtrudeFaceId(extrusion: ExtrudedSolid, selector: ExtrudeFaceSelector): string | null {
	if (selector.type === "cap") {
		return selector.side === "top" ? extrusion.faces.top : extrusion.faces.bottom
	}

	return extrusion.faces.sides[selector.index] ?? null
}

export function resolveExtrudeEdgeId(extrusion: ExtrudedSolid, selector: ExtrudeEdgeSelector): string | null {
	if (selector.type === "capLoop") {
		const edges = selector.side === "top" ? extrusion.edges.top : extrusion.edges.bottom
		return edges[selector.index] ?? null
	}

	return extrusion.edges.sides[selector.index] ?? null
}

function normalizeProfileLoops(profile: Profile): NormalizedProfileLoop[] {
	const loops: NormalizedProfileLoop[] = []

	for (const loop of profile.loops) {
		const normalizedIndexes: number[] = []

		for (const vertexIndex of loop) {
			if (!Number.isInteger(vertexIndex) || vertexIndex < 0 || vertexIndex >= profile.vertices.length) {
				continue
			}

			const previousIndex = normalizedIndexes[normalizedIndexes.length - 1]
			if (previousIndex !== vertexIndex) {
				normalizedIndexes.push(vertexIndex)
			}
		}

		if (normalizedIndexes.length >= 2 && normalizedIndexes[0] === normalizedIndexes[normalizedIndexes.length - 1]) {
			normalizedIndexes.pop()
		}

		if (normalizedIndexes.length >= 3) {
			loops.push({ vertexIndexes: normalizedIndexes })
		}
	}

	return loops
}

function projectPoint(point: Profile["vertices"][number], plane: ExtrudeProfileOptions["plane"], extrusionOffset: number): Vector3D {
	switch (plane) {
		case "YZ":
			return { x: extrusionOffset, y: point.x, z: point.y }
		case "XZ":
			return { x: point.x, y: extrusionOffset, z: point.y }
		default:
			return { x: point.x, y: point.y, z: extrusionOffset }
	}
}

function getRequiredId(store: Map<number, string>, index: number, label: string): string {
	const id = store.get(index)
	if (!id) {
		throw new Error(`Missing ${label} for profile vertex ${index}.`)
	}

	return id
}
