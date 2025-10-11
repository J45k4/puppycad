import { Profile, LinePath, Solid, Vec3 } from "./puppycad.ts"

it("box part", () => {
	const part = new Profile([
		[0, 0, 0],
		[10, 0, 0],
		[10, 10, 0],
		[0, 10, 0],
		[0, 0, 0]
	])
	const rect = new Solid(part)
	const upPath = new LinePath(
		new Vec3(0, 0, 0),
		new Vec3(0, 0, 1) // Move 1 unit upward
	)
	rect.sweep(upPath)
})
