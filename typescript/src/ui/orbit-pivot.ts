import { type Camera, type Object3D, Quaternion, Raycaster, type Vector2, Vector3 } from "three"

/** Pick once at drag start; a miss stays on the cursor ray at the default view distance. */
export function pickOrbitPivot(camera: Camera, pointer: Vector2, objects: Object3D[], distance: number): Vector3 {
	camera.updateMatrixWorld()
	for (const object of objects) object.updateWorldMatrix(true, true)
	const raycaster = new Raycaster()
	raycaster.setFromCamera(pointer, camera)
	const hit = raycaster.intersectObjects(objects, true).find((hit) => {
		for (let object: Object3D | null = hit.object; object; object = object.parent) if (!object.visible) return false
		return true
	})
	return hit?.point.clone() ?? raycaster.ray.at(distance, new Vector3())
}

/** Incremental object rotation in screen axes; angles are in radians. */
export function screenRotation(horizontal: number, vertical: number): Quaternion {
	const axis = new Vector3(vertical, horizontal, 0)
	const angle = axis.length()
	return angle === 0 ? new Quaternion() : new Quaternion().setFromAxisAngle(axis.multiplyScalar(1 / angle), angle)
}

/** Orbit in the camera's current axes, retaining roll and the pivot's screen position. */
export function orbitAssemblyTarget(target: Vector3, pivot: Vector3, orientation: Quaternion, horizontal: number, vertical: number): void {
	const previous = orientation.clone()
	orientation.multiply(screenRotation(horizontal, vertical).invert()).normalize()
	const rotation = orientation.clone().multiply(previous.invert())
	target.sub(pivot).applyQuaternion(rotation).add(pivot)
}
