import { expect, it } from "bun:test"
import { spawnSync } from "node:child_process"
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import type { ProjectPartDocument } from "../src/contract"
import { createPartRuntimeState, materializePartFeatures, serializePCadState } from "../src/pcad/part-state"
import { createProjectFile, normalizeProjectFile, serializeProjectFile } from "../src/project-file"
import { requireValue } from "../src/required"
import type { Sketch } from "../src/schema"
import { threePointRectangle } from "../src/sketch-rectangle"

const ROOT = join(import.meta.dir, "..")
const FIXTURES = join(ROOT, "rust", "fixtures")

// Sketch count per top-level part for each native fixture.
const EXPECTED: Record<string, number[]> = {
	"dimensions.pcad": [1],
	"extruded-rectangle.pcad": [1],
	"graph-constrained-line.pcad": [1],
	"segmented-primitives.pcad": [1],
	"segmented-slot.pcad": [1],
	"splines.pcad": [1],
	"two-parts.pcad": [1, 1]
}

function sketchCounts(source: string): number[] {
	const project = requireValue(normalizeProjectFile(JSON.parse(source)))
	return project.items
		.filter((item): item is ProjectPartDocument => "type" in item && item.type === "part")
		.map((part) => {
			const state = createPartRuntimeState(requireValue(part.data))
			return materializePartFeatures(state.cad, state.tree).filter((feature) => feature.type === "sketch").length
		})
}

it("loads every native Rust fixture in the TypeScript loader", () => {
	const names = readdirSync(FIXTURES)
		.filter((name) => name.endsWith(".pcad"))
		.sort()
	expect(names).toEqual(Object.keys(EXPECTED).sort())
	for (const name of names) {
		expect({ name, counts: sketchCounts(readFileSync(join(FIXTURES, name), "utf8")) }).toEqual({ name, counts: EXPECTED[name] ?? [] })
	}
})

function webProject(): { source: string; sketch: Sketch } {
	const sketch: Sketch = {
		id: "web-sketch",
		type: "sketch",
		name: "Web sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [threePointRectangle("rect", { x: 0, y: 0 }, { x: 20, y: 0 }, { x: 20, y: 10 }), { id: "line", type: "line", p0: { x: -5, y: -5 }, p1: { x: 30, y: 15 } }],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const runtime = createPartRuntimeState({ features: [sketch] })
	const file = createProjectFile({
		items: [{ id: "web-part", type: "part", name: "Web part", data: { features: [sketch], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
		selectedPath: [0]
	})
	return { source: serializeProjectFile(file), sketch }
}

const cargo = spawnSync("cargo", ["--version"], { encoding: "utf8" })
const hasCargo = cargo.status === 0

it.skipIf(!hasCargo)(
	"loads web-saved projects round-tripped through the native app",
	() => {
		const dir = mkdtempSync(join(tmpdir(), "puppycad-rust-roundtrip-"))
		try {
			const { source, sketch } = webProject()
			const input = join(dir, "web.pcad")
			const output = join(dir, "native.pcad")
			writeFileSync(input, source)
			const result = spawnSync("cargo", ["run", "--quiet", "--manifest-path", join(ROOT, "rust", "Cargo.toml"), "--", "--roundtrip", input, output], {
				encoding: "utf8",
				timeout: 600_000
			})
			expect({ status: result.status, stderr: result.stderr }).toEqual({ status: 0, stderr: "" })
			const project = requireValue(normalizeProjectFile(JSON.parse(readFileSync(output, "utf8"))))
			const part = project.items[0] as ProjectPartDocument
			expect(part.name).toBe("Web part")
			const state = createPartRuntimeState(requireValue(part.data))
			const sketches = materializePartFeatures(state.cad, state.tree).filter((feature) => feature.type === "sketch")
			expect(sketches).toHaveLength(1)
			expect(sketches[0]?.type === "sketch" ? sketches[0].entities : null).toEqual(sketch.entities)
		} finally {
			rmSync(dir, { recursive: true, force: true })
		}
	},
	600_000
)

function dimensionSketch(): Sketch {
	return {
		id: "dim-sketch",
		type: "sketch",
		name: "Dimensions",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [
			{ id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 50.8, y: 0 } },
			{ id: "point", type: "point", center: { x: 10, y: 30 } }
		],
		relations: [
			{ id: "len", type: "length", entityId: "line", value: 50.8, expression: "#w * 2", labelPosition: { x: 25, y: -12 } },
			{ id: "dist", type: "distance", a: { entityId: "line", point: "p0" }, b: { entityId: "point", point: "center" }, value: Math.hypot(10, 30), reference: true },
			{ id: "dx", type: "distance", a: { entityId: "line", point: "p1" }, b: { entityId: "point", point: "center" }, axis: "x", value: 10 - 50.8 },
			{ id: "dy", type: "distance", a: { entityId: "line", point: "p0" }, b: { entityId: "point", point: "center" }, axis: "y", value: 30, expression: "#w + 4.6 mm" }
		],
		variables: [{ name: "w", kind: "length", expression: "1 in" }],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
}

it.skipIf(!hasCargo)(
	"keeps web dimension relations and sketch variables through the native app",
	() => {
		const dir = mkdtempSync(join(tmpdir(), "puppycad-rust-dimensions-"))
		try {
			const sketch = dimensionSketch()
			const runtime = createPartRuntimeState({ features: [sketch] })
			const file = createProjectFile({
				items: [{ id: "web-part", type: "part", name: "Web part", data: { features: [sketch], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
				selectedPath: [0]
			})
			const input = join(dir, "web.pcad")
			const output = join(dir, "native.pcad")
			writeFileSync(input, serializeProjectFile(file))
			const result = spawnSync("cargo", ["run", "--quiet", "--manifest-path", join(ROOT, "rust", "Cargo.toml"), "--", "--roundtrip", input, output], {
				encoding: "utf8",
				timeout: 600_000
			})
			expect({ status: result.status, stderr: result.stderr }).toEqual({ status: 0, stderr: "" })
			const project = requireValue(normalizeProjectFile(JSON.parse(readFileSync(output, "utf8"))))
			const part = project.items[0] as ProjectPartDocument
			const state = createPartRuntimeState(requireValue(part.data))
			const loaded = materializePartFeatures(state.cad, state.tree).find((feature) => feature.type === "sketch")
			expect(loaded?.type).toBe("sketch")
			if (loaded?.type !== "sketch") return
			expect(loaded.variables).toEqual(sketch.variables)
			const byId = new Map((loaded.relations ?? []).map((relation) => [relation.id, relation]))
			for (const relation of sketch.relations ?? []) expect(byId.get(relation.id)).toEqual(relation)
		} finally {
			rmSync(dir, { recursive: true, force: true })
		}
	},
	600_000
)

if (!hasCargo) {
	console.warn("rust-roundtrip: cargo not found; skipping native round-trip of web-saved projects")
}
