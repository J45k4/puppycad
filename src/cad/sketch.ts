import type { CornerRectangle, Loop, Profile, Sketch } from "../schema"
import type { Point2D } from "../types"

const DEFAULT_TOLERANCE = 1e-6

type Segment = {
	start: Point2D
	end: Point2D
}

type IndexedSegment = {
	startIndex: number
	endIndex: number
}

type LoopDescriptor = {
	points: Point2D[]
	area: number
	parentIndex: number | null
	depth: number
}

export interface MaterializeSketchOptions {
	tolerance?: number
}

export function materializeSketch(sketch: Sketch, options: MaterializeSketchOptions = {}): Sketch {
	const tolerance = options.tolerance ?? DEFAULT_TOLERANCE
	const directLoops: Point2D[][] = []
	const segments: Segment[] = []

	for (const entity of sketch.entities) {
		switch (entity.type) {
			case "line":
				segments.push({
					start: clonePoint(entity.p0),
					end: clonePoint(entity.p1)
				})
				break
			case "cornerRectangle":
				directLoops.push(cornerRectangleToLoop(entity))
				break
			default:
				assertNever(entity)
		}
	}

	const allLoops = [...directLoops, ...traceClosedLoops(segments, tolerance)]
	const topology = buildTopologyFromLoops(sketch.id, allLoops, tolerance)

	return {
		...sketch,
		vertices: topology.vertices,
		loops: topology.loops,
		profiles: topology.profiles
	}
}

export function resolveLoopPoints(sketch: Pick<Sketch, "vertices" | "loops">, loopId: string): Point2D[] {
	const loop = sketch.loops.find((entry) => entry.id === loopId)
	if (!loop) {
		throw new Error(`Loop "${loopId}" does not exist on this sketch.`)
	}

	const points = loop.vertexIndices.map((vertexIndex) => {
		const point = sketch.vertices[vertexIndex]
		if (!point) {
			throw new Error(`Loop "${loopId}" references missing vertex ${vertexIndex}.`)
		}
		return clonePoint(point)
	})

	if (points.length < 3) {
		throw new Error(`Loop "${loopId}" does not contain a valid polygon.`)
	}

	return points
}

export function resolveProfileLoops(sketch: Pick<Sketch, "vertices" | "loops" | "profiles">, profileId: string): Point2D[][] {
	const profile = sketch.profiles.find((entry) => entry.id === profileId)
	if (!profile) {
		throw new Error(`Profile "${profileId}" does not exist on this sketch.`)
	}

	return [resolveLoopPoints(sketch, profile.outerLoopId), ...profile.holeLoopIds.map((loopId) => resolveLoopPoints(sketch, loopId))]
}

function cornerRectangleToLoop(rectangle: CornerRectangle): Point2D[] {
	const minX = Math.min(rectangle.p0.x, rectangle.p1.x)
	const maxX = Math.max(rectangle.p0.x, rectangle.p1.x)
	const minY = Math.min(rectangle.p0.y, rectangle.p1.y)
	const maxY = Math.max(rectangle.p0.y, rectangle.p1.y)
	return [
		{ x: minX, y: minY },
		{ x: maxX, y: minY },
		{ x: maxX, y: maxY },
		{ x: minX, y: maxY }
	]
}

function traceClosedLoops(segments: Segment[], tolerance: number): Point2D[][] {
	if (segments.length === 0) {
		return []
	}

	const indexedPoints: Point2D[] = []
	const indexedSegments: IndexedSegment[] = []
	const pointIndexByKey = new Map<string, number>()
	const adjacency = new Map<number, number[]>()

	for (const segment of segments) {
		if (pointsEqual(segment.start, segment.end, tolerance)) {
			continue
		}

		const startIndex = indexPoint(segment.start, indexedPoints, pointIndexByKey, tolerance)
		const endIndex = indexPoint(segment.end, indexedPoints, pointIndexByKey, tolerance)
		const segmentIndex = indexedSegments.length

		indexedSegments.push({ startIndex, endIndex })
		pushAdjacency(adjacency, startIndex, segmentIndex)
		pushAdjacency(adjacency, endIndex, segmentIndex)
	}

	const processedSegments = new Set<number>()
	const loops: Point2D[][] = []

	for (let segmentIndex = 0; segmentIndex < indexedSegments.length; segmentIndex += 1) {
		if (processedSegments.has(segmentIndex)) {
			continue
		}

		const component = collectSegmentComponent(segmentIndex, indexedSegments, adjacency)
		for (const componentSegmentIndex of component.segmentIndexes) {
			processedSegments.add(componentSegmentIndex)
		}

		const isClosedComponent = component.vertexIndexes.every((vertexIndex) => (adjacency.get(vertexIndex)?.length ?? 0) === 2)
		if (!isClosedComponent) {
			continue
		}

		const loop = traceLoop(component.segmentIndexes, indexedSegments, indexedPoints, adjacency)
		if (loop.length >= 3) {
			loops.push(loop)
		}
	}

	return loops
}

function pushAdjacency(adjacency: Map<number, number[]>, vertexIndex: number, segmentIndex: number): void {
	const existing = adjacency.get(vertexIndex)
	if (existing) {
		existing.push(segmentIndex)
		return
	}
	adjacency.set(vertexIndex, [segmentIndex])
}

function indexPoint(point: Point2D, indexedPoints: Point2D[], pointIndexByKey: Map<string, number>, tolerance: number): number {
	const key = pointKey(point, tolerance)
	const existingIndex = pointIndexByKey.get(key)
	if (existingIndex !== undefined) {
		return existingIndex
	}

	const nextIndex = indexedPoints.length
	indexedPoints.push(clonePoint(point))
	pointIndexByKey.set(key, nextIndex)
	return nextIndex
}

function collectSegmentComponent(seedSegmentIndex: number, indexedSegments: IndexedSegment[], adjacency: Map<number, number[]>): { segmentIndexes: number[]; vertexIndexes: number[] } {
	const segmentIndexes: number[] = []
	const vertexIndexes = new Set<number>()
	const pendingSegments = [seedSegmentIndex]
	const visitedSegments = new Set<number>()

	while (pendingSegments.length > 0) {
		const currentSegmentIndex = pendingSegments.pop()
		if (currentSegmentIndex === undefined || visitedSegments.has(currentSegmentIndex)) {
			continue
		}

		visitedSegments.add(currentSegmentIndex)
		segmentIndexes.push(currentSegmentIndex)

		const segment = indexedSegments[currentSegmentIndex]
		if (!segment) {
			continue
		}

		vertexIndexes.add(segment.startIndex)
		vertexIndexes.add(segment.endIndex)

		for (const vertexIndex of [segment.startIndex, segment.endIndex]) {
			const neighboringSegments = adjacency.get(vertexIndex) ?? []
			for (const neighboringSegmentIndex of neighboringSegments) {
				if (!visitedSegments.has(neighboringSegmentIndex)) {
					pendingSegments.push(neighboringSegmentIndex)
				}
			}
		}
	}

	return {
		segmentIndexes,
		vertexIndexes: [...vertexIndexes]
	}
}

function traceLoop(componentSegmentIndexes: number[], indexedSegments: IndexedSegment[], indexedPoints: Point2D[], adjacency: Map<number, number[]>): Point2D[] {
	const [firstSegmentIndex] = componentSegmentIndexes
	if (firstSegmentIndex === undefined) {
		return []
	}

	const firstSegment = indexedSegments[firstSegmentIndex]
	if (!firstSegment) {
		return []
	}

	const startPoint = indexedPoints[firstSegment.startIndex]
	const endPoint = indexedPoints[firstSegment.endIndex]
	if (!startPoint || !endPoint) {
		return []
	}

	let currentVertexIndex = firstSegment.endIndex
	let previousSegmentIndex = firstSegmentIndex
	const usedSegments = new Set<number>([firstSegmentIndex])
	const orderedPoints: Point2D[] = [clonePoint(startPoint), clonePoint(endPoint)]

	while (currentVertexIndex !== firstSegment.startIndex) {
		const nextSegmentIndex = (adjacency.get(currentVertexIndex) ?? []).find((candidateSegmentIndex) => candidateSegmentIndex !== previousSegmentIndex)
		if (nextSegmentIndex === undefined || usedSegments.has(nextSegmentIndex)) {
			return []
		}

		usedSegments.add(nextSegmentIndex)
		const nextSegment = indexedSegments[nextSegmentIndex]
		if (!nextSegment) {
			return []
		}

		const nextVertexIndex = nextSegment.startIndex === currentVertexIndex ? nextSegment.endIndex : nextSegment.startIndex

		if (nextVertexIndex !== firstSegment.startIndex) {
			const nextPoint = indexedPoints[nextVertexIndex]
			if (!nextPoint) {
				return []
			}

			orderedPoints.push(clonePoint(nextPoint))
		}

		previousSegmentIndex = nextSegmentIndex
		currentVertexIndex = nextVertexIndex
	}

	return usedSegments.size === componentSegmentIndexes.length ? orderedPoints : []
}

function buildTopologyFromLoops(sketchId: string, loops: Point2D[][], tolerance: number): Pick<Sketch, "vertices" | "loops" | "profiles"> {
	const loopDescriptors: LoopDescriptor[] = []

	for (const loop of loops) {
		const normalizedLoop = normalizeLoop(loop)
		if (!normalizedLoop) {
			continue
		}

		const area = signedArea(normalizedLoop)
		if (Math.abs(area) <= tolerance) {
			continue
		}

		loopDescriptors.push({
			points: normalizedLoop,
			area,
			parentIndex: null,
			depth: 0
		})
	}

	for (let loopIndex = 0; loopIndex < loopDescriptors.length; loopIndex += 1) {
		const samplePoint = loopDescriptors[loopIndex]?.points[0]
		if (!samplePoint) {
			continue
		}

		let parentIndex: number | null = null
		let parentArea = Number.POSITIVE_INFINITY

		for (let candidateIndex = 0; candidateIndex < loopDescriptors.length; candidateIndex += 1) {
			if (candidateIndex === loopIndex) {
				continue
			}

			const candidateLoop = loopDescriptors[candidateIndex]
			if (!candidateLoop) {
				continue
			}

			const candidateAreaMagnitude = Math.abs(candidateLoop.area)
			const loopAreaMagnitude = Math.abs(loopDescriptors[loopIndex]?.area ?? 0)
			if (candidateAreaMagnitude <= loopAreaMagnitude || candidateAreaMagnitude >= parentArea) {
				continue
			}

			if (pointInPolygon(samplePoint, candidateLoop.points, tolerance)) {
				parentIndex = candidateIndex
				parentArea = candidateAreaMagnitude
			}
		}

		const descriptor = loopDescriptors[loopIndex]
		if (descriptor) {
			descriptor.parentIndex = parentIndex
		}
	}

	for (let loopIndex = 0; loopIndex < loopDescriptors.length; loopIndex += 1) {
		let depth = 0
		let parentIndex = loopDescriptors[loopIndex]?.parentIndex ?? null

		while (parentIndex !== null) {
			depth += 1
			parentIndex = loopDescriptors[parentIndex]?.parentIndex ?? null
		}

		const descriptor = loopDescriptors[loopIndex]
		if (descriptor) {
			descriptor.depth = depth
		}
	}

	const vertices: Point2D[] = []
	const builtLoops: Loop[] = []
	const descriptorLoopIds = new Map<number, string>()

	const addLoop = (points: Point2D[]): string => {
		const id = `${sketchId}-loop-${builtLoops.length + 1}`
		const vertexIndices = points.map((point) => {
			const index = vertices.length
			vertices.push(clonePoint(point))
			return index
		})
		builtLoops.push({ id, vertexIndices })
		return id
	}

	const profiles: Profile[] = []
	let profileIndex = 1

	for (let loopIndex = 0; loopIndex < loopDescriptors.length; loopIndex += 1) {
		const descriptor = loopDescriptors[loopIndex]
		if (!descriptor || descriptor.depth % 2 !== 0) {
			continue
		}

		const outerPoints = orientLoop(descriptor.points, true)
		const outerLoopId = descriptorLoopIds.get(loopIndex) ?? addLoop(outerPoints)
		descriptorLoopIds.set(loopIndex, outerLoopId)

		const holeLoopIds = loopDescriptors
			.map((candidate, candidateIndex) => ({ candidate, candidateIndex }))
			.filter(({ candidate }) => candidate && candidate.parentIndex === loopIndex && candidate.depth === descriptor.depth + 1)
			.map(({ candidate, candidateIndex }) => {
				const holePoints = orientLoop(candidate.points, false)
				const holeLoopId = descriptorLoopIds.get(candidateIndex) ?? addLoop(holePoints)
				descriptorLoopIds.set(candidateIndex, holeLoopId)
				return holeLoopId
			})

		profiles.push({
			id: `${sketchId}-profile-${profileIndex}`,
			outerLoopId,
			holeLoopIds
		})
		profileIndex += 1
	}

	return { vertices, loops: builtLoops, profiles }
}

function normalizeLoop(loop: Point2D[]): Point2D[] | null {
	if (loop.length < 3) {
		return null
	}

	const normalized: Point2D[] = []
	for (const point of loop) {
		const previousPoint = normalized[normalized.length - 1]
		if (!previousPoint || previousPoint.x !== point.x || previousPoint.y !== point.y) {
			normalized.push(clonePoint(point))
		}
	}

	if (normalized.length >= 2) {
		const firstPoint = normalized[0]
		const lastPoint = normalized[normalized.length - 1]
		if (firstPoint && lastPoint && firstPoint.x === lastPoint.x && firstPoint.y === lastPoint.y) {
			normalized.pop()
		}
	}

	return normalized.length >= 3 ? normalized : null
}

function orientLoop(loop: Point2D[], ccw: boolean): Point2D[] {
	const area = signedArea(loop)
	const shouldReverse = ccw ? area < 0 : area > 0
	return shouldReverse ? [...loop].reverse() : loop.map(clonePoint)
}

function pointInPolygon(point: Point2D, polygon: Point2D[], tolerance: number): boolean {
	let inside = false

	for (let index = 0, previousIndex = polygon.length - 1; index < polygon.length; previousIndex = index, index += 1) {
		const currentPoint = polygon[index]
		const previousPoint = polygon[previousIndex]
		if (!currentPoint || !previousPoint) {
			continue
		}

		if (pointOnSegment(point, previousPoint, currentPoint, tolerance)) {
			return true
		}

		const intersects =
			currentPoint.y > point.y !== previousPoint.y > point.y &&
			point.x < ((previousPoint.x - currentPoint.x) * (point.y - currentPoint.y)) / (previousPoint.y - currentPoint.y) + currentPoint.x
		if (intersects) {
			inside = !inside
		}
	}

	return inside
}

function pointOnSegment(point: Point2D, start: Point2D, end: Point2D, tolerance: number): boolean {
	const crossProduct = (point.y - start.y) * (end.x - start.x) - (point.x - start.x) * (end.y - start.y)
	if (Math.abs(crossProduct) > tolerance) {
		return false
	}

	const dotProduct = (point.x - start.x) * (end.x - start.x) + (point.y - start.y) * (end.y - start.y)
	if (dotProduct < -tolerance) {
		return false
	}

	const squaredLength = (end.x - start.x) ** 2 + (end.y - start.y) ** 2
	return dotProduct <= squaredLength + tolerance
}

function signedArea(loop: Point2D[]): number {
	let area = 0
	for (let index = 0; index < loop.length; index += 1) {
		const currentPoint = loop[index]
		const nextPoint = loop[(index + 1) % loop.length]
		if (!currentPoint || !nextPoint) {
			continue
		}
		area += currentPoint.x * nextPoint.y - nextPoint.x * currentPoint.y
	}
	return area / 2
}

function pointsEqual(a: Point2D, b: Point2D, tolerance: number): boolean {
	return Math.abs(a.x - b.x) <= tolerance && Math.abs(a.y - b.y) <= tolerance
}

function pointKey(point: Point2D, tolerance: number): string {
	const factor = 1 / tolerance
	return `${Math.round(point.x * factor)}:${Math.round(point.y * factor)}`
}

function clonePoint(point: Point2D): Point2D {
	return { x: point.x, y: point.y }
}

function assertNever(value: never): never {
	throw new Error(`Unsupported sketch entity: ${JSON.stringify(value)}`)
}
