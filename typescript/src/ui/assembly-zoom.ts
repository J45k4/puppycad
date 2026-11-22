import { type Box3, type OrthographicCamera, type Vector2, Vector3 } from "three"

// Safety bounds span ten orders of magnitude around the model's fitted view.
export function assemblyZoom(zoom: number, deltaPixels: number): number {
	if (!Number.isFinite(deltaPixels)) return zoom
	return Math.exp(Math.max(Math.log(1e-5), Math.min(Math.log(1e5), Math.log(zoom) - deltaPixels * 0.001)))
}

/** Shift the camera target so the world point under the cursor keeps its screen position. */
export function anchorAssemblyZoom(camera: OrthographicCamera, target: Vector3, pointer: Vector2, ratio: number): void {
	camera.updateMatrixWorld()
	const fraction = 1 - 1 / ratio
	target.addScaledVector(new Vector3().setFromMatrixColumn(camera.matrixWorld, 0), pointer.x * (camera.right - camera.left) * 0.5 * fraction)
	target.addScaledVector(new Vector3().setFromMatrixColumn(camera.matrixWorld, 1), pointer.y * (camera.top - camera.bottom) * 0.5 * fraction)
}

/** Fit projected bounds with padding, including narrow viewports and rotated assemblies. */
export function assemblyFitHalfHeight(camera: OrthographicCamera, bounds: Box3, aspect: number): number {
	camera.updateMatrixWorld()
	const center = bounds.getCenter(new Vector3())
	const right = new Vector3().setFromMatrixColumn(camera.matrixWorld, 0)
	const up = new Vector3().setFromMatrixColumn(camera.matrixWorld, 1)
	let extent = 0
	for (const x of [bounds.min.x, bounds.max.x])
		for (const y of [bounds.min.y, bounds.max.y])
			for (const z of [bounds.min.z, bounds.max.z]) {
				const offset = new Vector3(x, y, z).sub(center)
				extent = Math.max(extent, Math.abs(offset.dot(up)), Math.abs(offset.dot(right)) / Math.max(aspect, 0.01))
			}
	return Math.max(extent * 1.15, 1e-6)
}
