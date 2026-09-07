import { matchOrthographic, fitOrthographicDepth } from "../src/ui/projection"
import { expect, it } from "bun:test"
import { Box3, BoxGeometry, Group, Mesh, OrthographicCamera, PerspectiveCamera, Vector2, Vector3 } from "three"
import { assemblyZoom, anchorAssemblyZoom, assemblyFitHalfHeight } from "../src/ui/assembly-zoom"
import { partContentBounds, partFitDistance } from "../src/ui/part-fit"
it("keeps the cursor's world anchor fixed when zooming beyond the old limits", () => {
	const camera = new OrthographicCamera(-100, 100, 50, -50, 0.1, 1000)
	const target = new Vector3()
	camera.position.set(200, -200, 200)
	camera.lookAt(target)
	camera.updateMatrixWorld()
	const pointer = new Vector2(0.6, -0.4)
	const point = new Vector3(pointer.x, pointer.y, 0).unproject(camera)
	const zoom = assemblyZoom(1, -4000)
	expect(zoom).toBeGreaterThan(10)
	anchorAssemblyZoom(camera, target, pointer, zoom)
	camera.position.add(target)
	camera.lookAt(target)
	camera.left /= zoom
	camera.right /= zoom
	camera.top /= zoom
	camera.bottom /= zoom
	camera.updateProjectionMatrix()
	camera.updateMatrixWorld()
	const projected = point.project(camera)
	expect(projected.x).toBeCloseTo(pointer.x, 10)
	expect(projected.y).toBeCloseTo(pointer.y, 10)
	expect(assemblyZoom(1, 4000)).toBeLessThan(0.2)
	expect(Number.isFinite(assemblyZoom(1, -1e20))).toBe(true)
})
it("fits assembly bounds inside portrait and landscape viewports", () => {
	const camera = new OrthographicCamera()
	camera.position.set(10, -10, 10)
	camera.lookAt(0, 0, 0)
	const bounds = new Box3(new Vector3(-10, -20, -30), new Vector3(10, 20, 30))
	for (const aspect of [0.3, 2]) {
		const half = assemblyFitHalfHeight(camera, bounds, aspect)
		camera.left = -half * aspect
		camera.right = half * aspect
		camera.top = half
		camera.bottom = -half
		camera.updateProjectionMatrix()
		camera.updateMatrixWorld()
		for (const x of [-10, 10])
			for (const y of [-20, 20])
				for (const z of [-30, 30]) {
					const p = new Vector3(x, y, z).project(camera)
					expect(Math.abs(p.x)).toBeLessThan(1)
					expect(Math.abs(p.y)).toBeLessThan(1)
				}
	}
})
it("fits parts using local bounds after arbitrary pan and orbit", () => {
	const root = new Group()
	const content = new Group()
	const solids = new Group()
	root.add(content)
	content.add(solids)
	const mesh = new Mesh(new BoxGeometry(20, 40, 60))
	mesh.position.set(100, 0, 30)
	solids.add(mesh)
	const initial = partContentBounds(content, solids)
	root.position.set(400, -300, 20)
	root.rotation.set(1, 2, 0)
	content.position.set(-200, 20, -10)
	const after = partContentBounds(content, solids)
	expect(after.min.distanceTo(initial.min)).toBeLessThan(1e-10)
	expect(after.max.distanceTo(initial.max)).toBeLessThan(1e-10)
	expect(partFitDistance(after, 45, 0.3)).toBeGreaterThan(partFitDistance(after, 45, 2))
	mesh.geometry.dispose()
})

it("switches projection without changing target-plane scale or orientation", () => {
	const camera = new PerspectiveCamera(45, 1.6, 0.01, 10000)
	camera.position.set(0, 0, 100)
	camera.zoom = 2
	camera.updateProjectionMatrix()
	camera.updateMatrixWorld()
	const ortho = new OrthographicCamera()
	matchOrthographic(camera, ortho, 100)
	const point = new Vector3(10, 5, 0)
	expect(point.clone().project(camera).distanceTo(point.clone().project(ortho))).toBeGreaterThan(0) // Depth mapping differs.
	const a = point.clone().project(camera)
	const b = point.clone().project(ortho)
	expect(a.x).toBeCloseTo(b.x, 10)
	expect(a.y).toBeCloseTo(b.y, 10)
	expect(ortho.position.equals(camera.position)).toBe(true)
	expect(ortho.quaternion.equals(camera.quaternion)).toBe(true)
	const near = new Vector3(10, 5, 50)
	expect(near.clone().project(camera).x).toBeGreaterThan(a.x)
	expect(near.clone().project(ortho).x).toBeCloseTo(b.x, 10)
})

it("orthographic close-up keeps front and back geometry inside the depth range", () => {
	const camera = new PerspectiveCamera(45, 1, 0.01, 10000)
	const ortho = new OrthographicCamera()
	const bounds = new Box3(new Vector3(-100, -100, -40), new Vector3(100, 100, 60))
	for (const distance of [300, 10, 0.5]) {
		camera.position.set(0, 0, distance)
		matchOrthographic(camera, ortho, distance)
		const before = new Vector3(1, 2, 60).project(ortho)
		fitOrthographicDepth(ortho, bounds)
		const after = new Vector3(1, 2, 60).project(ortho)
		expect(after.x).toBeCloseTo(before.x, 10)
		expect(after.y).toBeCloseTo(before.y, 10)
		for (const z of [-40, 60]) expect(Math.abs(new Vector3(0, 0, z).project(ortho).z)).toBeLessThan(1)
		expect(ortho.position.z).toBeGreaterThan(bounds.max.z)
	}
})
