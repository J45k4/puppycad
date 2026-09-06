import { describe, expect, it } from "bun:test"
import modeling from "@jscad/modeling"
import { Vector3 } from "three"
import { PartBuilder, rectangle, v2 } from "../src/sdk"
import { evaluateSolid } from "../src/solid-model"
import { exportPartStl } from "../src/part-mesh"
import { extrudeSolidFeature } from "../src/cad/extrude"
import { normalizeProjectFile, createProjectFile } from "../src/project-file"
import { requireValue } from "../src/required"

function verifyMesh(builder: PartBuilder) {
	const stl = exportPartStl(builder.document)
	const vertices = [...stl.matchAll(/vertex ([^\n]+)/g)].map((m) => new Vector3(...(requireValue(m[1]).split(" ").map(Number) as [number, number, number])))
	const edges = new Map<string, number>()
	let volume = 0
	const key = (v: Vector3) => [v.x, v.y, v.z].map((n) => Math.round(n * 1e5)).join(",")
	for (let i = 0; i < vertices.length; i += 3) {
		const a = requireValue(vertices[i])
		const b = requireValue(vertices[i + 1])
		const c = requireValue(vertices[i + 2])
		expect(b.clone().sub(a).cross(c.clone().sub(a)).length()).toBeGreaterThan(1e-9)
		volume += a.dot(b.clone().cross(c)) / 6
		for (const [p, q] of [
			[a, b],
			[b, c],
			[c, a]
		]) {
			const id = [key(requireValue(p)), key(requireValue(q))].sort().join("/")
			edges.set(id, (edges.get(id) ?? 0) + 1)
		}
	}
	expect([...edges.values()].every((n) => n === 2)).toBe(true)
	expect(volume).toBeGreaterThan(0)
	return volume
}
describe("solid modeling", () => {
	it("fuses coplanar touching and overlapping additions without internal faces", () => {
		const b = new PartBuilder()
		b.extrude("a", { outline: rectangle(v2(0, 0), 10, 10), depth: 10 })
		b.extrude("b", { outline: rectangle(v2(5, 0), 10, 10), depth: 10 })
		expect(verifyMesh(b)).toBeCloseTo(1500, 2)
	})
	it("cuts a blind socket while preserving the floor", () => {
		const b = new PartBuilder()
		b.extrude("base", { outline: rectangle(v2(0, 0), 20, 20), depth: 10 })
		b.extrude("socket", { outline: rectangle(v2(0, 0), 6, 6), depth: 6, translation: { x: 0, y: 0, z: 4 }, operation: "cut" })
		expect(verifyMesh(b)).toBeCloseTo(4000 - 216, 2)
	})
	it("intersects solids and rejects empty results atomically", () => {
		const b = new PartBuilder()
		b.extrude("a", { outline: rectangle(v2(0, 0), 10, 10), depth: 10 })
		b.extrude("b", { outline: rectangle(v2(5, 0), 10, 10), depth: 10, operation: "intersect" })
		expect(verifyMesh(b)).toBeCloseTo(500, 2)
		const before = structuredClone(b.document)
		expect(() => b.extrude("outside", { outline: rectangle(v2(100, 0), 10, 10), depth: 10, operation: "intersect" })).toThrow("empty")
		expect(b.document).toEqual(before)
	})
	it("makes a tapered extrusion with the analytical frustum volume", () => {
		const b = new PartBuilder()
		b.extrude("taper", { outline: rectangle(v2(0, 0), 10, 10), depth: 12, topScale: 2 })
		expect(verifyMesh(b)).toBeCloseTo(2800, 2)
	})
	it("revolves full and partial profiles with closed end caps", () => {
		for (const angle of [180, 360]) {
			const b = new PartBuilder()
			b.revolve("ring", { outline: [v2(5, 0), v2(10, 0), v2(10, 8), v2(5, 8)], angle, segments: 96 })
			expect(verifyMesh(b) / ((Math.PI * 75 * 8 * angle) / 360)).toBeCloseTo(1, 2)
		}
	})
	it("rounds extrusion profile corners to the requested radius", () => {
		const b = new PartBuilder()
		b.extrude("body", { outline: rectangle(v2(0, 0), 20, 20), depth: 5 })
		b.fillet("body", 2, undefined, 32)
		expect(verifyMesh(b)).toBeCloseTo((400 - 4 * (4 - Math.PI)) * 5, 0)
		expect(() => b.fillet("body", 100)).toThrow()
	})
	it("chamfers profiles and fillets a straight solid edge", () => {
		const b = new PartBuilder()
		b.extrude("body", { outline: rectangle(v2(0, 0), 20, 20), depth: 10 })
		b.chamfer("body", 2)
		expect(verifyMesh(b)).toBeCloseTo(3920, 2)
		const c = new PartBuilder()
		c.extrude("body", { outline: rectangle(v2(0, 0), 20, 20), depth: 10 })
		const feature = c.document.features.find((f) => f.type === "extrude")
		if (!feature || feature.type !== "extrude") throw new Error("extrude")
		const edge = requireValue(extrudeSolidFeature(c.document, feature).solid.edges[1])
		c.filletEdge("body", edge.id, 2, 32)
		expect(verifyMesh(c)).toBeCloseTo(4000 - 20 * (4 - Math.PI), 0)
	})
	it("exports legacy chamfers through the same evaluated boundary", () => {
		const b = new PartBuilder()
		b.extrude("body", { outline: rectangle(v2(0, 0), 20, 20), depth: 10 })
		const feature = b.document.features.find((f) => f.type === "extrude")
		if (!feature || feature.type !== "extrude") throw new Error("extrude")
		const edge = requireValue(extrudeSolidFeature(b.document, feature).solid.edges[1])
		b.document.features.push({ type: "chamfer", id: "bevel", target: { edge: { type: "extrudeEdge", extrudeId: "body", edgeId: edge.id } }, d1: 2, d2: 3 })
		expect(verifyMesh(b)).toBeCloseTo(3940, 2)
	})
	it("preserves solid recipes through project serialization", () => {
		const b = new PartBuilder()
		b.revolve("cup", { outline: [v2(0, 0), v2(20, 0), v2(30, 30), v2(27, 30), v2(17, 3), v2(0, 3)] })
		b.fillet("cup", 0.5, [1, 2, 3, 4])
		const project = createProjectFile({ items: [{ id: "cup", type: "part", name: "Cup", data: b.document }], selectedPath: null })
		const saved = requireValue(normalizeProjectFile(JSON.parse(JSON.stringify(project)))).items[0]
		if (!saved || "kind" in saved || saved.type !== "part") throw new Error("part")
		expect(saved.data?.solidSteps).toEqual(b.document.solidSteps)
		expect(modeling.measurements.measureVolume(evaluateSolid(requireValue(saved.data)))).toBeGreaterThan(0)
		verifyMesh(b)
	})
})

it("inherits translation for face-attached additions", () => {
	const b = new PartBuilder()
	const top = b.extrude("base", { outline: rectangle(v2(0, 0), 10, 10), depth: 4, translation: { x: 25, y: 0, z: 20 } })
	b.extrude("top", { outline: rectangle(v2(0, 0), 10, 10), depth: 6, on: top })
	expect(verifyMesh(b)).toBeCloseTo(1000, 2)
	const bounds = modeling.measurements.measureBoundingBox(evaluateSolid(b.document))
	expect(bounds[0][2]).toBe(20)
	expect(bounds[1][2]).toBe(30)
})
