import { type Camera, type Object3D, Matrix4, Quaternion, Raycaster, type Vector2, Vector3 } from "three"

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

/** Rotate the camera target about an off-center pivot without centering it on screen. */
export function orbitAssemblyTarget(target: Vector3, pivot: Vector3, yaw: number, pitch: number, nextYaw: number, nextPitch: number): void {
	const orientation = (y: number, p: number) =>
		new Quaternion().setFromRotationMatrix(new Matrix4().lookAt(new Vector3(Math.cos(y) * Math.cos(p), Math.sin(y) * Math.cos(p), Math.sin(p)), new Vector3(), new Vector3(0, 0, 1)))
	const rotation = orientation(nextYaw, nextPitch).multiply(orientation(yaw, pitch).invert())
	target.sub(pivot).applyQuaternion(rotation).add(pivot)
}
