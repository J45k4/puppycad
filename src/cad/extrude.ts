import type { FaceReference, PartDocument, ProfileReference, SketchPlane, SketchTarget, Solid, SolidEdge, SolidExtrude, SolidFace, SolidVertex } from "../schema"
import { materializeSketch, resolveProfileLoops } from "./sketch"
import type { Point2D, Vector3D } from "../types"

export interface ExtrudeFeatureOptions {
	solidId?: string
	startOffset?: number
}

export interface SketchFrame3D {
	origin: Vector3D
	xAxis: Vector3D
	yAxis: Vector3D
	normal: Vector3D
}

export interface ExtrudedFaceDescriptor {
	faceId: string
	label: string
	frame: SketchFrame3D
}

export interface ExtrudedSolid {
	solid: Solid
	frame: SketchFrame3D
	depth: number
	profileLoops: Point2D[][]
}

type NormalizedLoop = {
	points: Point2D[]
}

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
	const frame = resolveSketchTargetFrame(part, materializedSketch.target)

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
			bottomVertexIds.set(pointKey, addVertex(projectPoint(point, frame, startOffset)))
			topVertexIds.set(pointKey, addVertex(projectPoint(point, frame, endOffset)))
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
		frame,
		depth: feature.depth,
		profileLoops: normalizedLoops.map((loop) => loop.points.map(clonePoint))
	}
}

export function resolveSketchTargetFrame(part: Pick<PartDocument, "features">, target: SketchTarget): SketchFrame3D {
	if (target.type === "plane") {
		return getPlaneSketchFrame(target.plane)
	}
	return resolveFaceReferenceFrame(part, target.face)
}

export function getPlaneSketchFrame(plane: SketchPlane): SketchFrame3D {
	switch (plane) {
		case "YZ":
			return {
				origin: { x: 0, y: 0, z: 0 },
				xAxis: { x: 0, y: 1, z: 0 },
				yAxis: { x: 0, y: 0, z: 1 },
				normal: { x: 1, y: 0, z: 0 }
			}
		case "XZ":
			return {
				origin: { x: 0, y: 0, z: 0 },
				xAxis: { x: 1, y: 0, z: 0 },
				yAxis: { x: 0, y: 0, z: 1 },
				normal: { x: 0, y: 1, z: 0 }
			}
		default:
			return {
				origin: { x: 0, y: 0, z: 0 },
				xAxis: { x: 1, y: 0, z: 0 },
				yAxis: { x: 0, y: 1, z: 0 },
				normal: { x: 0, y: 0, z: 1 }
			}
	}
}

export function getExtrudedFaceDescriptors(extrusion: ExtrudedSolid): ExtrudedFaceDescriptor[] {
	const descriptors: ExtrudedFaceDescriptor[] = []
	const totalSideFaces = extrusion.profileLoops.reduce((count, loop) => count + loop.length, 0)
	const bottomFace = extrusion.solid.faces[totalSideFaces]
	const topFace = extrusion.solid.faces[totalSideFaces + 1]
	let sideFaceIndex = 0
	let faceIndex = 0

	for (const loop of extrusion.profileLoops) {
		for (let pointIndex = 0; pointIndex < loop.length; pointIndex += 1) {
			const face = extrusion.solid.faces[faceIndex]
			const start = loop[pointIndex]
			const end = loop[(pointIndex + 1) % loop.length]
			if (face && start && end) {
				const worldStart = projectPoint(start, extrusion.frame, 0)
				const worldEnd = projectPoint(end, extrusion.frame, 0)
				const edgeDirection = normalizeVector(subtractVector(worldEnd, worldStart))
				const extrusionDirection = normalizeVector(extrusion.frame.normal)
				descriptors.push({
					faceId: face.id,
					label: `Side Face ${sideFaceIndex + 1}`,
					frame: {
						origin: worldStart,
						xAxis: edgeDirection,
						yAxis: extrusionDirection,
						normal: normalizeVector(crossVector(edgeDirection, extrusionDirection))
					}
				})
				sideFaceIndex += 1
			}
			faceIndex += 1
		}
	}

	if (bottomFace) {
		descriptors.push({
			faceId: bottomFace.id,
			label: "Bottom Face",
			frame: {
				origin: cloneVector3D(extrusion.frame.origin),
				xAxis: cloneVector3D(extrusion.frame.xAxis),
				yAxis: cloneVector3D(extrusion.frame.yAxis),
				normal: negateVector(extrusion.frame.normal)
			}
		})
	}

	if (topFace) {
		descriptors.push({
			faceId: topFace.id,
			label: "Top Face",
			frame: {
				origin: addScaledVector(extrusion.frame.origin, extrusion.frame.normal, extrusion.depth),
				xAxis: cloneVector3D(extrusion.frame.xAxis),
				yAxis: cloneVector3D(extrusion.frame.yAxis),
				normal: cloneVector3D(extrusion.frame.normal)
			}
		})
	}

	return descriptors
}

function resolveSketchForProfile(part: Pick<PartDocument, "features">, reference: ProfileReference) {
	const sketch = part.features.find((feature) => feature.type === "sketch" && feature.id === reference.sketchId)
	if (!sketch || sketch.type !== "sketch") {
		throw new Error(`Sketch "${reference.sketchId}" does not exist.`)
	}
	return sketch
}

function resolveFaceReferenceFrame(part: Pick<PartDocument, "features">, reference: FaceReference): SketchFrame3D {
	const extrude = part.features.find((feature) => feature.type === "extrude" && feature.id === reference.extrudeId)
	if (!extrude || extrude.type !== "extrude") {
		throw new Error(`Extrude "${reference.extrudeId}" does not exist.`)
	}
	const extrusion = extrudeSolidFeature(part, extrude)
	const face = getExtrudedFaceDescriptors(extrusion).find((entry) => entry.faceId === reference.faceId)
	if (!face) {
		throw new Error(`Face "${reference.faceId}" does not exist on extrude "${reference.extrudeId}".`)
	}
	return cloneSketchFrame(face.frame)
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

function projectPoint(point: Point2D, frame: SketchFrame3D, extrusionOffset: number): Vector3D {
	return addScaledVector(addScaledVector(addScaledVector(frame.origin, frame.xAxis, point.x), frame.yAxis, point.y), frame.normal, extrusionOffset)
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

function cloneVector3D(vector: Vector3D): Vector3D {
	return { x: vector.x, y: vector.y, z: vector.z }
}

function cloneSketchFrame(frame: SketchFrame3D): SketchFrame3D {
	return {
		origin: cloneVector3D(frame.origin),
		xAxis: cloneVector3D(frame.xAxis),
		yAxis: cloneVector3D(frame.yAxis),
		normal: cloneVector3D(frame.normal)
	}
}

function addScaledVector(origin: Vector3D, axis: Vector3D, scalar: number): Vector3D {
	return {
		x: origin.x + axis.x * scalar,
		y: origin.y + axis.y * scalar,
		z: origin.z + axis.z * scalar
	}
}

function subtractVector(a: Vector3D, b: Vector3D): Vector3D {
	return {
		x: a.x - b.x,
		y: a.y - b.y,
		z: a.z - b.z
	}
}

function crossVector(a: Vector3D, b: Vector3D): Vector3D {
	return {
		x: a.y * b.z - a.z * b.y,
		y: a.z * b.x - a.x * b.z,
		z: a.x * b.y - a.y * b.x
	}
}

function negateVector(vector: Vector3D): Vector3D {
	return {
		x: -vector.x,
		y: -vector.y,
		z: -vector.z
	}
}

function normalizeVector(vector: Vector3D): Vector3D {
	const length = Math.hypot(vector.x, vector.y, vector.z)
	if (length <= 1e-9) {
		throw new Error("Cannot normalize a zero-length vector.")
	}
	return {
		x: vector.x / length,
		y: vector.y / length,
		z: vector.z / length
	}
}
