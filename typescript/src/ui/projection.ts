import { type Box3, type OrthographicCamera, type PerspectiveCamera, Vector3 } from "three"
export type Projection = "perspective" | "orthographic"
export function projectionControl(value: Projection, change: (value: Projection) => void): HTMLSelectElement {
	const select = document.createElement("select")
	select.setAttribute("aria-label", "Projection")
	select.style.cssText = "width:160px;flex:0 0 auto;padding:7px 10px;border:1px solid #b8c5d2;border-radius:6px;background:white;color:#263544"
	for (const mode of ["perspective", "orthographic"] as const) {
		const option = document.createElement("option")
		option.value = mode
		option.textContent = mode === "perspective" ? "Perspective" : "Orthographic"
		select.append(option)
	}
	select.value = value
	select.onchange = () => change(select.value as Projection)
	return select
}
/** Match the visible size at the target plane without changing camera orientation. */
export function matchOrthographic(camera: PerspectiveCamera, orthographic: OrthographicCamera, distance: number): void {
	const half = (Math.max(1e-6, distance) * Math.tan((camera.fov * Math.PI) / 360)) / camera.zoom
	orthographic.left = -half * camera.aspect
	orthographic.right = half * camera.aspect
	orthographic.top = half
	orthographic.bottom = -half
	orthographic.position.copy(camera.position)
	orthographic.quaternion.copy(camera.quaternion)
	orthographic.near = camera.near
	orthographic.far = camera.far
	orthographic.updateProjectionMatrix()
	orthographic.updateMatrixWorld()
}

/** Keep the complete object in front of the orthographic camera without altering magnification. */
export function fitOrthographicDepth(camera: OrthographicCamera, bounds: Box3): void {
	if (bounds.isEmpty()) return
	camera.updateMatrixWorld()
	let nearest = Number.POSITIVE_INFINITY
	let farthest = Number.NEGATIVE_INFINITY
	for (const x of [bounds.min.x, bounds.max.x])
		for (const y of [bounds.min.y, bounds.max.y])
			for (const z of [bounds.min.z, bounds.max.z]) {
				const depth = -new Vector3(x, y, z).applyMatrix4(camera.matrixWorldInverse).z
				nearest = Math.min(nearest, depth)
				farthest = Math.max(farthest, depth)
			}
	const margin = Math.max(0.01, (farthest - nearest) * 0.05)
	const retreat = Math.max(0, margin * 2 - nearest)
	camera.position.addScaledVector(new Vector3(0, 0, 1).applyQuaternion(camera.quaternion), retreat)
	camera.near = margin
	camera.far = Math.max(margin * 3, farthest + retreat + margin)
	camera.updateMatrixWorld()
	camera.updateProjectionMatrix()
}
