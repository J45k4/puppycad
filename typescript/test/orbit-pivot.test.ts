import { expect, it } from "bun:test"
import { BoxGeometry, Quaternion, Mesh, MeshBasicMaterial, OrthographicCamera, PerspectiveCamera, Raycaster, Vector2, Vector3 } from "three"
import { orbitAssemblyTarget, pickOrbitPivot, screenRotation } from "../src/ui/orbit-pivot"

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
	camera.position.set(40, -30, 45)
	camera.lookAt(target)
	const orientation = camera.quaternion.clone()
	const update = () => {
		camera.position.set(0, 0, 60).applyQuaternion(orientation).add(target)
		camera.quaternion.copy(orientation)
		camera.updateMatrixWorld()
	}
	update()
	const pointer = new Vector2(0.7, -0.4)
	const pivot = pickOrbitPivot(camera, pointer, [], 60)
	for (let i = 0; i < 100; i++) orbitAssemblyTarget(target, pivot, orientation, 0.03, 0.05)
	update()
	const projected = pivot.clone().project(camera)
	expect(projected.x).toBeCloseTo(pointer.x)
	expect(projected.y).toBeCloseTo(pointer.y)
})

it("rotates relative to the current view even upside down and preserves a full revolution", () => {
	const orientation = screenRotation(0, Math.PI)
	const original = orientation.clone()
	const target = new Vector3()
	orbitAssemblyTarget(target, target.clone(), orientation, 0.3, 0)
	const expected = original.clone().multiply(new Quaternion().setFromAxisAngle(new Vector3(0, 1, 0), -0.3))
	expect(orientation.angleTo(expected)).toBeLessThan(1e-7)
	orbitAssemblyTarget(target, target.clone(), orientation, -0.3, 0)
	expect(orientation.angleTo(original)).toBeLessThan(1e-7)
	for (let i = 0; i < 100; i++) orbitAssemblyTarget(target, target.clone(), orientation, 0, (2 * Math.PI) / 100)
	expect(orientation.angleTo(original)).toBeLessThan(1e-7)
})
