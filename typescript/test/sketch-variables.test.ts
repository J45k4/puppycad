import { expect, it } from "bun:test"
import { evaluateSketchVariables, type SketchVariable } from "../src/sketch-variables"
import { parseSketchValue } from "../src/sketch-value"
import { requireValue } from "../src/required"

it("evaluates forward dependencies with compatible units and updates derived values", () => {
	const definitions: SketchVariable[] = [
		{ name: "diameter", kind: "length", expression: "#radius * 2" },
		{ name: "radius", kind: "length", expression: "#stock + 5 mm" },
		{ name: "stock", kind: "length", expression: "1 in" },
		{ name: "turn", kind: "angle", expression: "pi rad / 2" }
	]
	const before = structuredClone(definitions)
	const values = evaluateSketchVariables(definitions)
	expect(values.get("diameter")).toEqual({ kind: "length", value: 60.8 })
	expect(values.get("turn")).toEqual({ kind: "angle", value: 90 })
	expect(parseSketchValue("#diameter / 2 + 1 cm", "length", (name) => requireValue(values.get(name)))).toBe(40.4)
	expect(definitions).toEqual(before)
	requireValue(definitions[2]).expression = "2 in"
	expect(evaluateSketchVariables(definitions).get("diameter")?.value).toBeCloseTo(111.6, 10)
	expect(values.get("diameter")?.value).toBe(60.8)
})

it("keeps variable names case-sensitive and supports names that coincide with object properties", () => {
	const variables: SketchVariable[] = ["Size", "size", "constructor", "__proto__", "pitch_2"].map((name, i) => ({ name, kind: "scalar", expression: String(i + 1) }))
	const values = evaluateSketchVariables(variables)
	expect(parseSketchValue("#Size + #size + #constructor + #__proto__ + #pitch_2", "scalar", (name) => requireValue(values.get(name)))).toBe(15)
	expect(() => evaluateSketchVariables([{ name: "a", kind: "scalar", expression: "#A" }])).toThrow("Unknown sketch variable: #A")
})

it("rejects missing references, cycles, duplicate names, unit mismatches and unsafe syntax", () => {
	const pair = (a: string, b: string): SketchVariable[] => [
		{ name: "a", kind: "length", expression: a },
		{ name: "b", kind: "length", expression: b }
	]
	for (const definitions of [pair("#missing", "1 mm"), pair("#b", "#a"), pair("#a", "1 mm"), pair("#b + 1 deg", "1 mm"), pair("globalThis.process.exit()", "1 mm")])
		expect(() => evaluateSketchVariables(definitions)).toThrow()
	expect(() => evaluateSketchVariables(pair("#b", "#a"))).toThrow("#a → #b → #a")
	expect(() =>
		evaluateSketchVariables([
			{ name: "a", kind: "scalar", expression: "1" },
			{ name: "a", kind: "scalar", expression: "2" }
		])
	).toThrow("Duplicate")
	expect(() => evaluateSketchVariables([{ name: "2bad", kind: "scalar", expression: "1" }])).toThrow("Variable names")
	expect(() => parseSketchValue("#a", "length")).toThrow("Unknown sketch variable")
})

it("bounds dependency depth and variable count while allowing shared dependency graphs", () => {
	const chain: SketchVariable[] = Array.from({ length: 65 }, (_, i) => ({ name: `v${i}`, kind: "scalar", expression: i === 64 ? "1" : `#v${i + 1}` }))
	expect(() => evaluateSketchVariables(chain)).toThrow("64 levels")
	expect(() => evaluateSketchVariables([...chain].reverse())).toThrow("64 levels")
	expect(evaluateSketchVariables(chain.slice(1)).get("v1")?.value).toBe(1)
	const shared: SketchVariable[] = Array.from({ length: 256 }, (_, i) => ({ name: `v${i}`, kind: "scalar", expression: i === 255 ? "2" : "#v255 + #v255" }))
	expect(evaluateSketchVariables(shared).get("v0")?.value).toBe(4)
	expect(() => evaluateSketchVariables([...shared, { name: "overflow", kind: "scalar", expression: "1" }])).toThrow("256")
})

it("persists native sketch variables through feature and PCad project representations", async () => {
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const { createProjectFile, normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
	const source: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: [],
		variables: [
			{ name: "diameter", kind: "length", expression: "#radius * 2" },
			{ name: "radius", kind: "length", expression: "1 in" }
		]
	}
	const original = structuredClone(source)
	for (const native of [false, true]) {
		const runtime = createPartRuntimeState({ features: [source] })
		const file = createProjectFile({
			items: [{ id: "p", type: "part", name: "Variables", data: { features: [source], ...(native ? { cad: serializePCadState(runtime.cad), tree: runtime.tree } : {}) } }],
			selectedPath: null
		})
		const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = restored.items[0] as import("../src/contract").ProjectPartDocument
		const state = createPartRuntimeState(requireValue(part.data))
		const saved = materializePartFeatures(state.cad, state.tree)[0]
		if (saved?.type !== "sketch") throw Error("Missing sketch")
		expect(saved.variables).toEqual(source.variables)
		expect(evaluateSketchVariables(requireValue(saved.variables)).get("diameter")?.value).toBe(50.8)
		requireValue(saved.variables?.[1]).expression = "2 in"
		expect(evaluateSketchVariables(requireValue(saved.variables)).get("diameter")?.value).toBe(101.6)
	}
	expect(source).toEqual(original)
})

it("rejects malformed serialized variables and copies accepted definitions", async () => {
	const { normalizeSketchVariables } = await import("../src/sketch-variables")
	for (const input of [null, {}, [null], [{ name: "x", kind: "area", expression: "2" }], [{ name: "x", kind: "length", expression: "#missing" }]])
		expect(() => normalizeSketchVariables(input)).toThrow()
	expect(normalizeSketchVariables(undefined)).toBeUndefined()
	expect(normalizeSketchVariables([])).toEqual([])
	const original = [{ name: "x", kind: "length", expression: "10 mm", ignored: true }]
	const result = normalizeSketchVariables(original)
	expect(result).toEqual([{ name: "x", kind: "length", expression: "10 mm" }])
	requireValue(result?.[0]).expression = "20 mm"
	expect(original[0]?.expression).toBe("10 mm")
})

it("drives native geometry from dependent variables and preserves reevaluation through PCad reload", async () => {
	const { materializeSketch } = await import("../src/cad/sketch")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const source: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 64 }],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: [],
		variables: [
			{ name: "radius", kind: "length", expression: "#stock / 2" },
			{ name: "stock", kind: "length", expression: "2 cm" }
		],
		relations: [{ id: "diameter", type: "diameter", entityId: "circle", value: 10, expression: "#radius * 2" }]
	}
	const before = structuredClone(source)
	const first = materializeSketch(source)
	const firstCircle = first.entities[0]
	if (firstCircle?.type !== "circle") throw Error("Missing circle")
	expect(firstCircle.radius).toBeCloseTo(10, 6)
	expect(first.relations?.[0]).toMatchObject({ value: 20, expression: "#radius * 2" })
	requireValue(first.variables?.[1]).expression = "3 cm"
	const state = createPartRuntimeState({ features: [first] })
	const reloaded = createPartRuntimeState({ features: [], cad: serializePCadState(state.cad), tree: state.tree })
	const saved = materializePartFeatures(reloaded.cad, reloaded.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	const savedCircle = saved.entities[0]
	if (savedCircle?.type !== "circle") throw Error("Missing circle")
	expect(savedCircle.radius).toBeCloseTo(15, 6)
	expect(saved.relations?.[0]).toMatchObject({ value: 30, expression: "#radius * 2" })
	const { createProjectFile, normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
	for (const native of [false, true]) {
		const current = createPartRuntimeState({ features: [saved] })
		const file = createProjectFile({
			items: [{ id: "part", type: "part", name: "Driven circle", data: { features: [saved], ...(native ? { cad: serializePCadState(current.cad), tree: current.tree } : {}) } }],
			selectedPath: null
		})
		const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = restored.items[0] as import("../src/contract").ProjectPartDocument
		const runtime = createPartRuntimeState(requireValue(part.data))
		const feature = materializePartFeatures(runtime.cad, runtime.tree)[0]
		if (feature?.type !== "sketch") throw Error("Missing reloaded sketch")
		expect(feature.variables).toEqual(saved.variables)
		expect(feature.relations).toEqual(saved.relations)
		requireValue(feature.variables?.[1]).expression = "4 cm"
		const updated = materializeSketch(feature)
		const circle = updated.entities[0]
		if (circle?.type !== "circle") throw Error("Missing reloaded circle")
		expect(circle.radius).toBeCloseTo(20, 6)
		expect(updated.relations?.[0]).toMatchObject({ value: 40, expression: "#radius * 2" })
	}
	expect(source).toEqual(before)
	for (const expression of ["-1 mm", "0 mm", "1 deg", "#missing"]) {
		const invalid = structuredClone(saved)
		requireValue(invalid.variables?.[1]).expression = expression
		expect(() => materializeSketch(invalid)).toThrow()
	}
})

it("renames exact references in variable and dimension expressions without mutating the source", async () => {
	const { renameSketchVariable } = await import("../src/sketch-variables")
	const source: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: [],
		variables: [
			{ name: "r", kind: "length", expression: "10 mm" },
			{ name: "radius", kind: "length", expression: "#r + 1 mm" },
			{ name: "R", kind: "length", expression: "2 mm" }
		],
		relations: [{ id: "d", type: "diameter", entityId: "circle", value: 32, expression: "#r + #radius + #R + #r - 1 mm" }]
	}
	const before = structuredClone(source)
	const result = renameSketchVariable(source, "r", "base")
	expect(result.variables?.[1]?.expression).toBe("#base + 1 mm")
	expect(result.relations?.[0]?.expression).toBe("#base + #radius + #R + #base - 1 mm")
	expect(evaluateSketchVariables(requireValue(result.variables)).get("radius")?.value).toBe(11)
	for (const name of ["radius", "2bad", "bad name", "x".repeat(65)]) expect(() => renameSketchVariable(source, "r", name)).toThrow()
	expect(source).toEqual(before)
})
