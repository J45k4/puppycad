import { expect, it } from "bun:test"
import { BoxGeometry, Mesh, MeshBasicMaterial, OrthographicCamera, PerspectiveCamera, Raycaster, Vector2, Vector3 } from "three"
import { orbitAssemblyTarget, pickOrbitPivot } from "../src/ui/orbit-pivot"

it("uses the nearest visible surface and falls back along the cursor ray for both camera types", () => {
	for (const camera of [new PerspectiveCamera(45, 1, 0.1, 1000), new OrthographicCamera(-10, 10, 10, -10, 0.1, 1000)]) {
		camera.position.set(0, 0, 20)
		camera.lookAt(0, 0, 0)
		const mesh = new Mesh(new BoxGeometry(2, 2, 2), new MeshBasicMaterial())
		expect(pickOrbitPivot(camera, new Vector2(), [mesh], 20).z).toBeCloseTo(1)
		const pointer = new Vector2(0.8, -0.7)
		const pivot = pickOrbitPivot(camera, pointer, [mesh], 20)
		const raycaster = new Raycaster()
		raycaster.setFromCamera(pointer, camera)
		expect(pivot.distanceTo(raycaster.ray.origin)).toBeCloseTo(20)
		expect(raycaster.ray.distanceToPoint(pivot)).toBeLessThan(1e-8)
		const screen = pivot.clone().project(camera)
		expect(screen.x).toBeCloseTo(pointer.x)
		expect(screen.y).toBeCloseTo(pointer.y)
		mesh.visible = false
		expect(pickOrbitPivot(camera, new Vector2(), [mesh], 20).z).toBeCloseTo(0)
	}
})

it("keeps an off-center pivot at the same screen position throughout assembly rotation", () => {
	const camera = new OrthographicCamera(-20, 20, 20, -20, 0.1, 1000)
	camera.up.set(0, 0, 1)
	const target = new Vector3(2, 3, 4)
	const update = (yaw: number, pitch: number) => {
		camera.position.copy(target).add(new Vector3(Math.cos(yaw) * Math.cos(pitch), Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch)).multiplyScalar(60))
		camera.lookAt(target)
		camera.updateMatrixWorld()
	}
	update(-0.7, 0.6)
	const pointer = new Vector2(0.7, -0.4)
	const pivot = pickOrbitPivot(camera, pointer, [], 60)
	orbitAssemblyTarget(target, pivot, -0.7, 0.6, 0.3, 1)
	update(0.3, 1)
	const projected = pivot.clone().project(camera)
	expect(projected.x).toBeCloseTo(pointer.x)
	expect(projected.y).toBeCloseTo(pointer.y)
})
