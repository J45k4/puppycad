import { Box3, Triangle, Vector3 } from "three"
import { requireValue } from "../../src/required"
export type TriangleMesh = { positions: number[]; triangles: number[] }
/** Decode the embedded glTF fixture without browser or GLTFLoader dependencies. */
export function readReferenceMeshes(json: string): { mesh: TriangleMesh; translation: Vector3 }[] {
	const gltf = JSON.parse(json)
	const buffers = gltf.buffers.map((b: { uri: string }) => Buffer.from(b.uri.split(",")[1] ?? "", "base64"))
	const accessor = (id: number) => {
		const a = gltf.accessors[id]
		const view = gltf.bufferViews[a.bufferView]
		const buffer = buffers[view.buffer] as Buffer
		const size = a.componentType === 5126 || a.componentType === 5125 ? 4 : 2
		const width = a.type === "VEC3" ? 3 : 1
		const start = (view.byteOffset ?? 0) + (a.byteOffset ?? 0)
		const stride = view.byteStride ?? size * width
		return Array.from({ length: a.count * width }, (_, i) => {
			const offset = start + Math.floor(i / width) * stride + (i % width) * size
			return a.componentType === 5126 ? buffer.readFloatLE(offset) : a.componentType === 5125 ? buffer.readUInt32LE(offset) : buffer.readUInt16LE(offset)
		})
	}
	return gltf.meshes.map((source: { primitives: { attributes: { POSITION: number }; indices: number }[] }, index: number) => {
		const positions: number[] = []
		const triangles: number[] = []
		for (const primitive of source.primitives) {
			const offset = positions.length / 3
			triangles.push(...accessor(primitive.indices).map((i) => i + offset))
			positions.push(...accessor(primitive.attributes.POSITION).map((v) => v * 1000))
		}
		const nodeIndex = gltf.nodes.findIndex((node: { mesh?: number }) => node.mesh === index)
		const parent = gltf.nodes.find((node: { children?: number[] }) => node.children?.includes(nodeIndex))
		const matrix = parent.matrix as number[]
		return { mesh: { positions, triangles }, translation: new Vector3(requireValue(matrix[12]) * 1000, requireValue(matrix[13]) * 1000, requireValue(matrix[14]) * 1000) }
	})
}
type Node = { bounds: Box3; left?: Node; right?: Node; indices?: number[] }
export class SurfaceIndex {
	private readonly root: Node
	private readonly triangles: Triangle[]
	constructor(mesh: TriangleMesh) {
		const vertex = (i: number) => new Vector3(requireValue(mesh.positions[i * 3]), requireValue(mesh.positions[i * 3 + 1]), requireValue(mesh.positions[i * 3 + 2]))
		this.triangles = []
		for (let i = 0; i < mesh.triangles.length; i += 3)
			this.triangles.push(new Triangle(vertex(requireValue(mesh.triangles[i])), vertex(requireValue(mesh.triangles[i + 1])), vertex(requireValue(mesh.triangles[i + 2]))))
		const boxes = this.triangles.map((t) => new Box3().setFromPoints([t.a, t.b, t.c]))
		const centers = boxes.map((b) => b.getCenter(new Vector3()))
		const build = (indices: number[]): Node => {
			const bounds = new Box3()
			for (const i of indices) bounds.union(requireValue(boxes[i]))
			if (indices.length <= 12) return { bounds, indices }
			const size = bounds.getSize(new Vector3())
			const axis = size.x >= size.y && size.x >= size.z ? "x" : size.y >= size.z ? "y" : "z"
			indices.sort((a, b) => requireValue(centers[a])[axis] - requireValue(centers[b])[axis])
			const middle = Math.floor(indices.length / 2)
			return { bounds, left: build(indices.slice(0, middle)), right: build(indices.slice(middle)) }
		}
		this.root = build(this.triangles.map((_, i) => i))
	}
	distance(point: Vector3): number {
		let best = Number.POSITIVE_INFINITY
		const closest = new Vector3()
		const visit = (node: Node) => {
			if (node.bounds.distanceToPoint(point) > best) return
			if (node.indices) {
				for (const i of node.indices) {
					requireValue(this.triangles[i]).closestPointToPoint(point, closest)
					best = Math.min(best, closest.distanceTo(point))
				}
				return
			}
			const left = requireValue(node.left)
			const right = requireValue(node.right)
			if (left.bounds.distanceToPoint(point) < right.bounds.distanceToPoint(point)) {
				visit(left)
				visit(right)
			} else {
				visit(right)
				visit(left)
			}
		}
		visit(this.root)
		return best
	}
}
export function surfaceSamples(mesh: TriangleMesh, count = 2000): Vector3[] {
	const points: Vector3[] = []
	const unique = new Set<string>()
	for (let i = 0; i < mesh.positions.length; i += 3) {
		const p = new Vector3(mesh.positions[i], mesh.positions[i + 1], mesh.positions[i + 2])
		const key = p
			.toArray()
			.map((n) => n.toFixed(5))
			.join(",")
		if (!unique.has(key)) {
			unique.add(key)
			points.push(p)
		}
	}
	const v = (index: number) => new Vector3(mesh.positions[index * 3], mesh.positions[index * 3 + 1], mesh.positions[index * 3 + 2])
	const triangles: Triangle[] = []
	const cumulative: number[] = []
	let total = 0
	for (let i = 0; i < mesh.triangles.length; i += 3) {
		const t = new Triangle(v(requireValue(mesh.triangles[i])), v(requireValue(mesh.triangles[i + 1])), v(requireValue(mesh.triangles[i + 2])))
		triangles.push(t)
		total += t.getArea()
		cumulative.push(total)
	}
	let state = 5731
	const random = () => {
		state = (Math.imul(state, 1664525) + 1013904223) >>> 0
		return state / 4294967296
	}
	for (let i = 0; i < count; i++) {
		const target = random() * total
		let lo = 0
		let hi = cumulative.length - 1
		while (lo < hi) {
			const mid = (lo + hi) >>> 1
			if (requireValue(cumulative[mid]) < target) lo = mid + 1
			else hi = mid
		}
		const t = requireValue(triangles[lo])
		const r = Math.sqrt(random())
		const s = random()
		points.push(
			t.a
				.clone()
				.multiplyScalar(1 - r)
				.addScaledVector(t.b, r * (1 - s))
				.addScaledVector(t.c, r * s)
		)
	}
	return points
}
export function compareSurfaces(a: TriangleMesh, b: TriangleMesh) {
	const directed = (source: TriangleMesh, target: TriangleMesh) => {
		const index = new SurfaceIndex(target)
		const samples = surfaceSamples(source)
		const distances = samples.map((p) => index.distance(p)).sort((x, y) => x - y)
		return {
			samples: samples.length,
			max: requireValue(distances.at(-1)),
			p99: requireValue(distances[Math.floor(distances.length * 0.99)]),
			mean: distances.reduce((sum, d) => sum + d, 0) / distances.length
		}
	}
	return { aToB: directed(a, b), bToA: directed(b, a) }
}
