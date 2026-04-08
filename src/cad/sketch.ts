import type { AlignedRectangle, CenteredRectangle, CenterPointCircle, CornerRectangle, MidpointLine, Profile, SketchFeature } from "../contract"
import type { Point2D } from "../types"
import { solveSketchConstraints } from "./constraints"

const DEFAULT_CIRCLE_SEGMENTS = 32
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

export interface SketchProfileEngineOptions {
	circleSegments?: number
	tolerance?: number
}

type SketchProfileInput = Pick<SketchFeature, "id" | "type" | "target" | "entities" | "profiles" | "constraints" | "dimensions">

export function getProfilesFromSketch(sketch: SketchProfileInput, options: SketchProfileEngineOptions = {}): Profile[] {
	if (sketch.profiles.length > 0) {
		return cloneProfiles(sketch.profiles)
	}

	const { sketch: solvedSketch } = solveSketchConstraints(sketch)
	const tolerance = options.tolerance ?? DEFAULT_TOLERANCE
	const circleSegments = normalizeCircleSegments(options.circleSegments)
	const directLoops: Point2D[][] = []
	const segments: Segment[] = []

	for (const entity of solvedSketch.entities) {
		switch (entity.type) {
			case "line":
				segments.push({
					start: clonePoint(entity.p0),
					end: clonePoint(entity.p1)
				})
				break
			case "midpointLine":
				segments.push(midpointLineToSegment(entity))
				break
			case "centeredRectangle":
				directLoops.push(centeredRectangleToLoop(entity))
				break
			case "cornerRectangle":
				directLoops.push(cornerRectangleToLoop(entity))
				break
			case "alignedRectangle":
				directLoops.push(alignedRectangleToLoop(entity))
				break
			case "centerPointCircle":
				directLoops.push(circleToLoop(entity, circleSegments))
				break
			default:
				assertNever(entity)
		}
	}

	const allLoops = [...directLoops, ...traceClosedLoops(segments, tolerance)]
	return buildProfilesFromLoops(solvedSketch.id, allLoops, tolerance)
}

function cloneProfiles(profiles: Profile[]): Profile[] {
	return profiles.map((profile) => ({
		id: profile.id,
		vertices: profile.vertices.map(clonePoint),
		loops: profile.loops.map((loop) => [...loop])
	}))
}

function midpointLineToSegment(line: MidpointLine): Segment {
	const angle = line.angle ?? 0
	const halfLength = line.length / 2
	const dx = Math.cos(angle) * halfLength
	const dy = Math.sin(angle) * halfLength
	return {
		start: {
			x: line.midpoint.x - dx,
			y: line.midpoint.y - dy
		},
		end: {
			x: line.midpoint.x + dx,
			y: line.midpoint.y + dy
		}
	}
}

function centeredRectangleToLoop(rectangle: CenteredRectangle): Point2D[] {
	const halfWidth = rectangle.width / 2
	const halfHeight = rectangle.height / 2
	const corners = [
		{ x: -halfWidth, y: -halfHeight },
		{ x: halfWidth, y: -halfHeight },
		{ x: halfWidth, y: halfHeight },
		{ x: -halfWidth, y: halfHeight }
	]

	return applyRotationAndTranslation(corners, rectangle.rotation ?? 0, rectangle.center)
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

function alignedRectangleToLoop(rectangle: AlignedRectangle): Point2D[] {
	const dx = rectangle.p1.x - rectangle.p0.x
	const dy = rectangle.p1.y - rectangle.p0.y
	const length = Math.hypot(dx, dy)
	if (length <= DEFAULT_TOLERANCE) {
		return []
	}

	const offsetX = (-dy / length) * rectangle.height
	const offsetY = (dx / length) * rectangle.height

	return [clonePoint(rectangle.p0), clonePoint(rectangle.p1), { x: rectangle.p1.x + offsetX, y: rectangle.p1.y + offsetY }, { x: rectangle.p0.x + offsetX, y: rectangle.p0.y + offsetY }]
}

function circleToLoop(circle: CenterPointCircle, segments: number): Point2D[] {
	const points: Point2D[] = []
	for (let index = 0; index < segments; index += 1) {
		const angle = (index / segments) * Math.PI * 2
		points.push({
			x: circle.center.x + Math.cos(angle) * circle.radius,
			y: circle.center.y + Math.sin(angle) * circle.radius
		})
	}
	return points
}

function applyRotationAndTranslation(points: Point2D[], angle: number, center: Point2D): Point2D[] {
	const cosAngle = Math.cos(angle)
	const sinAngle = Math.sin(angle)

	return points.map((point) => ({
		x: center.x + point.x * cosAngle - point.y * sinAngle,
		y: center.y + point.x * sinAngle + point.y * cosAngle
	}))
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

		const startAdjacency = adjacency.get(startIndex)
		if (startAdjacency) {
			startAdjacency.push(segmentIndex)
		} else {
			adjacency.set(startIndex, [segmentIndex])
		}

		const endAdjacency = adjacency.get(endIndex)
		if (endAdjacency) {
			endAdjacency.push(segmentIndex)
		} else {
			adjacency.set(endIndex, [segmentIndex])
		}
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

function buildProfilesFromLoops(sketchId: string, loops: Point2D[][], tolerance: number): Profile[] {
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
			if (candidateAreaMagnitude <= loopAreaMagnitude) {
				continue
			}

			if (candidateAreaMagnitude >= parentArea) {
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

	const profiles: Profile[] = []
	let profileIndex = 1

	for (let loopIndex = 0; loopIndex < loopDescriptors.length; loopIndex += 1) {
		const descriptor = loopDescriptors[loopIndex]
		if (!descriptor || descriptor.depth % 2 !== 0) {
			continue
		}

		const polygons = [
			orientLoop(descriptor.points, true),
			...loopDescriptors
				.filter((candidate) => candidate.parentIndex === loopIndex && candidate.depth === descriptor.depth + 1)
				.map((candidate) => orientLoop(candidate.points, false))
		]

		profiles.push({
			id: `${sketchId}-profile-${profileIndex}`,
			...polygonsToProfile(polygons)
		})
		profileIndex += 1
	}

	return profiles
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

function polygonsToProfile(polygons: Point2D[][]): Omit<Profile, "id"> {
	const vertices: Point2D[] = []
	const loops = polygons.map((polygon) =>
		polygon.map((point) => {
			const index = vertices.length
			vertices.push(clonePoint(point))
			return index
		})
	)

	return { vertices, loops }
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

function normalizeCircleSegments(value: number | undefined): number {
	if (typeof value !== "number" || !Number.isFinite(value)) {
		return DEFAULT_CIRCLE_SEGMENTS
	}

	return Math.max(3, Math.round(value))
}

function assertNever(value: never): never {
	throw new Error(`Unsupported sketch entity: ${JSON.stringify(value)}`)
}
