import { Euler, Matrix4, Quaternion, Vector3 } from "three"
import type { Assembly, AssemblyConnector, AssemblyMateReference, Transform3D } from "./contract"
import { requireValue } from "./required"
export function transformMatrix(transform?: Transform3D): Matrix4 {
	const translation = transform?.translation ?? { x: 0, y: 0, z: 0 }
	const rotation = transform?.rotation ?? { x: 0, y: 0, z: 0 }
	const scale = transform?.scale ?? { x: 1, y: 1, z: 1 }
	for (const vector of [translation, rotation, scale]) if (![vector.x, vector.y, vector.z].every(Number.isFinite)) throw new Error("Transforms must contain finite numbers.")
	if (scale.x <= 0 || scale.y <= 0 || scale.z <= 0) throw new Error("Scale must be positive.")
	const radians = Math.PI / 180
	return new Matrix4().compose(
		new Vector3(translation.x, translation.y, translation.z),
		new Quaternion().setFromEuler(new Euler(rotation.x * radians, rotation.y * radians, rotation.z * radians)),
		new Vector3(scale.x, scale.y, scale.z)
	)
}
function connectorMatrix(connector: AssemblyConnector): Matrix4 {
	return transformMatrix({ translation: connector.position, rotation: connector.rotation })
}
function matrixTransform(matrix: Matrix4): Transform3D {
	const position = new Vector3()
	const quaternion = new Quaternion()
	const scale = new Vector3()
	matrix.decompose(position, quaternion, scale)
	const euler = new Euler().setFromQuaternion(quaternion)
	return {
		translation: { x: position.x, y: position.y, z: position.z },
		rotation: { x: (euler.x * 180) / Math.PI, y: (euler.y * 180) / Math.PI, z: (euler.z * 180) / Math.PI },
		scale: { x: scale.x, y: scale.y, z: scale.z }
	}
}

/** Solve rigid connector constraints, including consistent closed loops, without mutating input. */
export function solveFixedAssembly(input: Assembly, tolerance = 1e-6): Assembly {
	const assembly = structuredClone(input)
	const instances = new Map(assembly.instances.map((i) => [i.id, i]))
	const fixed = (assembly.mates ?? []).filter((m) => m.type === "fasten")
	const connector = (ref: AssemblyMateReference) => {
		const c = assembly.connectors?.find((c) => c.id === ref.connectorId && c.instanceId === ref.instanceId)
		if (!c || (c.instanceId !== null && !instances.has(c.instanceId))) throw new Error(`Unknown connector: ${ref.connectorId}`)
		return connectorMatrix(c)
	}
	type Link = { to: string | null; delta: Matrix4; mateId: string }
	const graph = new Map<string | null, Link[]>()
	const add = (from: string | null, link: Link) => graph.set(from, [...(graph.get(from) ?? []), link])
	for (const mate of fixed) {
		if (mate.a.instanceId === mate.b.instanceId) throw new Error(`Mate ${mate.id} must connect distinct instances.`)
		const delta = connector(mate.a).multiply(connector(mate.b).invert())
		add(mate.a.instanceId, { to: mate.b.instanceId, delta, mateId: mate.id })
		add(mate.b.instanceId, { to: mate.a.instanceId, delta: delta.clone().invert(), mateId: mate.id })
	}
	for (const id of graph.keys()) {
		const scale = id === null ? undefined : instances.get(id)?.transform?.scale
		if (scale && [scale.x, scale.y, scale.z].some((s) => Math.abs(s - 1) > tolerance)) throw new Error("Fixed connections require unscaled instances.")
	}
	const poses = new Map<string | null, Matrix4>()
	const solveFrom = (root: string | null) => {
		if (poses.has(root)) return
		poses.set(root, root === null ? new Matrix4() : transformMatrix(instances.get(root)?.transform))
		const queue: (string | null)[] = [root]
		for (let index = 0; index < queue.length; index++) {
			const from = queue[index] ?? null
			for (const link of graph.get(from) ?? []) {
				const expected = requireValue(poses.get(from)).clone().multiply(link.delta)
				const existing = poses.get(link.to)
				if (existing) {
					if (existing.elements.some((v, i) => Math.abs(v - requireValue(expected.elements[i])) > tolerance))
						throw new Error(`Conflicting fixed constraints at mate "${link.mateId}".`)
				} else {
					poses.set(link.to, expected)
					queue.push(link.to)
				}
			}
		}
	}
	// World is grounded. Otherwise retain the pose of a parent/root in each component.
	if (graph.has(null)) solveFrom(null)
	const children = new Set(fixed.map((m) => m.b.instanceId))
	for (const instance of assembly.instances) if (!children.has(instance.id) && graph.has(instance.id)) solveFrom(instance.id)
	for (const instance of assembly.instances) if (graph.has(instance.id)) solveFrom(instance.id)
	for (const instance of assembly.instances) {
		const pose = poses.get(instance.id)
		if (pose) instance.transform = matrixTransform(pose)
	}
	return assembly
}
