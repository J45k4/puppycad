import { expect, it } from "bun:test"
import { orderOffsetChain, offsetLineChain } from "../src/sketch-offset-chain"
import { solveSketch, normalizeSketchRelations, remapSketchRelations, type SketchRelation } from "../src/sketch-solver"
import { editSketchCurve } from "../src/sketch-edit"
import type { Line, Sketch } from "../src/schema"
import { requireValue } from "../src/required"
const line = (id: string, x0: number, y0: number, x1: number, y1: number): Line => ({ id, type: "line", p0: { x: x0, y: y0 }, p1: { x: x1, y: y1 } })
const square = () => [line("bottom", 0, 0, 20, 0), line("right", 20, 0, 20, 20), line("top", 20, 20, 0, 20), line("left", 0, 20, 0, 0)]
const targets = ["a", "b", "c", "d"]
it("orders reversed and shuffled edges into an outward closed offset with exact corners", () => {
	const input = square()
	const first = requireValue(input[0])
	input[0] = { ...first, p0: first.p1, p1: first.p0 }
	input.reverse()
	const definition = orderOffsetChain(input)
	const result = offsetLineChain(input, definition, 3, targets)
	expect(definition.closed).toBe(true)
	expect(Math.min(...result.map((l) => l.p0.x))).toBeCloseTo(-3, 8)
	expect(Math.max(...result.map((l) => l.p0.y))).toBeCloseTo(23, 8)
	result.forEach((l, i) => expect(l.p1).toEqual(requireValue(result[(i + 1) % 4]).p0))
	expect(square()).toContainEqual(line("bottom", 0, 0, 20, 0))
})
it("joins open chains, preserves free ends, and supports collinear segments", () => {
	const input = [line("x", 0, 0, 10, 0), line("y", 10, 0, 20, 0), line("z", 20, 0, 20, 10)]
	const definition = orderOffsetChain(input)
	const result = offsetLineChain(input, definition, 2, ["a", "b", "c"])
	expect(result).toMatchObject([
		{ p0: { x: 0, y: 2 }, p1: { x: 10, y: 2 } },
		{ p0: { x: 10, y: 2 }, p1: { x: 18, y: 2 } },
		{ p0: { x: 18, y: 2 }, p1: { x: 18, y: 10 } }
	])
})
it("rejects disconnected selections, branches and collapsed insets", () => {
	expect(() => orderOffsetChain([line("a", 0, 0, 1, 0), line("b", 2, 0, 3, 0)])).toThrow("connected")
	expect(() => orderOffsetChain([line("a", 0, 0, 1, 0), line("b", 1, 0, 2, 0), line("c", 1, 0, 1, 1)])).toThrow("branch")
	const input = square()
	expect(() => offsetLineChain(input, orderOffsetChain(input), -10, targets)).toThrow("collapses")
	expect(() => offsetLineChain(input, orderOffsetChain(input), -12, targets)).toThrow("collapses")
})
it("uses one persisted driving distance for every edge and invalidates the group on trim", async () => {
	const input = square()
	const definition = orderOffsetChain(input)
	const copies = offsetLineChain(input, definition, 3, targets)
	const relation: SketchRelation = { id: "offset", type: "offsetChain", ...definition, targets, value: 6 }
	const fixed: SketchRelation[] = input.flatMap((l) => (["p0", "p1"] as const).map((point) => ({ id: `${l.id}-${point}`, type: "fixed", anchor: { entityId: l.id, point }, position: l[point] })))
	const result = solveSketch([...input, ...copies], [...fixed, relation])
	expect(result.status).toBe("fully-constrained")
	expect(structuredClone(result.entities.find((e) => e.id === "a"))).toMatchObject({
		p0: { x: expect.closeTo(-6, 5), y: expect.closeTo(-6, 5) },
		p1: { x: expect.closeTo(26, 5), y: expect.closeTo(-6, 5) }
	})
	const normalized = normalizeSketchRelations(JSON.parse(JSON.stringify([relation])))
	expect(
		remapSketchRelations(
			normalized,
			new Map([
				["bottom", "new-bottom"],
				["a", "new-a"]
			])
		)?.[0]
	).toMatchObject({ sources: expect.arrayContaining([{ entityId: "new-bottom", reversed: false }]), targets: ["new-a", "b", "c", "d"] })
	const sketch: Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: result.entities,
		relations: [...fixed, relation],
		dimensions: [],
		loops: [],
		vertices: [],
		profiles: []
	}
	const edited = editSketchCurve(sketch, "a", { x: 10, y: -6 }, "Split")
	expect(edited.removedRelations).toContain("offset")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations).toContainEqual(relation)
	expect(saved.profiles.length).toBeGreaterThan(0)
})

it("rejects crossing offsets without changing the input geometry", () => {
	const input = [line("a", 0, 0, 10, 10), line("b", 10, 10, 0, 10), line("c", 0, 10, 10, 0)]
	const original = structuredClone(input)
	expect(() => offsetLineChain(input, orderOffsetChain(input), 0, ["x", "y", "z"])).toThrow("intersect")
	expect(input).toEqual(original)
})
