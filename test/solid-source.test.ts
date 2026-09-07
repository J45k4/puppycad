import { createPartGeometries } from "../src/part-mesh"
import { expect, it } from "bun:test"
import { EdgesGeometry, Vector3 } from "three"
import { PartBuilder, circle, rectangle, v2 } from "../src/sdk"
import { pickSolidSketchSource, sourceRimSegments } from "../src/ui/solid-source"
it("picks the cutting sketch at an intermediate-depth hole rim", () => {
	const builder = new PartBuilder()
	builder.extrude("plate", { outline: rectangle(v2(0, 0), 30, 30), depth: 5 })
	builder.extrude("socket", { outline: circle(v2(0, 0), 5, 64), depth: 5.2, translation: { x: 0, y: 0, z: -0.1 }, operation: "cut" })
	const source = pickSolidSketchSource(builder.document, new Vector3(5, 0, 5), 0.01)
	expect(source?.stepId).toBe("socket")
	expect(source?.sketchId).toBe("socket/sketch")
	expect(source?.entityId).toStartWith("socket/sketch/0/")
	expect(source?.loopIndex).toBe(0)
	expect(source?.border.every((p) => Math.abs(p.z - 5) < 1e-8)).toBe(true)
	expect(pickSolidSketchSource(builder.document, new Vector3(0, 0, 5), 0.01)).toBeNull()
	expect(pickSolidSketchSource(builder.document, new Vector3(5, 0, 15), 0.01)).toBeNull()
})
it("distinguishes a built-in hole from its extrusion outline", () => {
	const builder = new PartBuilder()
	builder.extrude("ring", { outline: circle(v2(0, 0), 10, 32), holes: [circle(v2(0, 0), 5, 32)], depth: 4 })
	expect(pickSolidSketchSource(builder.document, new Vector3(5, 0, 4), 0.01)?.loopIndex).toBe(1)
	expect(pickSolidSketchSource(builder.document, new Vector3(10, 0, 4), 0.01)?.loopIndex).toBe(0)
})

it("highlights the evaluated rim instead of a slice through the hole wall", () => {
	const builder = new PartBuilder()
	builder.extrude("plate", { outline: rectangle(v2(0, 0), 30, 30), depth: 5 })
	builder.extrude("socket", { outline: circle(v2(0, 0), 5, 64), depth: 5.2, translation: { x: 0, y: 0, z: -0.1 }, operation: "cut" })
	const source = pickSolidSketchSource(builder.document, new Vector3(5, 0, 4.7), 0.01)
	if (!source) throw Error("Missing source")
	const geometry = createPartGeometries(builder.document)[0]
	if (!geometry) throw Error("Missing mesh")
	const edges = new EdgesGeometry(geometry, 15)
	const positions = edges.getAttribute("position")
	const points = Array.from({ length: positions.count }, (_, i) => new Vector3().fromBufferAttribute(positions, i))
	const rim = sourceRimSegments(source, points)
	expect(rim.length).toBeGreaterThanOrEqual(128)
	expect(rim.every((p) => Math.abs(p.z - 5) < 1e-6)).toBe(true)
	expect(rim.every((p) => Math.abs(Math.hypot(p.x, p.y) - 5) < 1e-5)).toBe(true)
	edges.dispose()
	geometry.dispose()
})
