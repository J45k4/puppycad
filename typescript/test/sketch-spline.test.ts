import { expect, it } from "bun:test"
import { nearestSplinePoint, normalizeSpline, sampleSpline, splineBezierSegments, splitSpline, splinePortionWithAnchors } from "../src/sketch-spline"
import { cubicBezierPoint } from "../src/sketch-bezier"
import { requireValue } from "../src/required"
it("retains native control data across JSON and independent segment extraction", () => {
	const spline = normalizeSpline({ id: "s", type: "spline", mode: "control", construction: true, points: Array.from({ length: 7 }, (_, i) => ({ x: i, y: i % 2 })) })
	const restored = normalizeSpline(JSON.parse(JSON.stringify(spline)))
	expect(restored).toEqual(spline)
	const segments = splineBezierSegments(restored)
	expect(segments).toHaveLength(2)
	expect(segments[0]?.[3]).toEqual(segments[1]?.[0])
	if (!segments[0]) throw Error("Missing segment")
	segments[0][3].x = 100
	expect(segments[1]?.[0].x).toBe(3)
	expect(restored.points[3]?.x).toBe(3)
})
it("samples open fit and control chains with exact parameter references and a single shared join", () => {
	for (const mode of ["fit", "control"] as const) {
		const points =
			mode === "fit"
				? [
						{ x: 0, y: 0 },
						{ x: 3, y: 4 },
						{ x: 8, y: 0 }
					]
				: Array.from({ length: 7 }, (_, i) => ({ x: i, y: i % 2 }))
		const spline = normalizeSpline({ id: "s", type: "spline", mode, points })
		const curves = splineBezierSegments(spline)
		const samples = sampleSpline(spline, 0.01)
		expect(samples[0]).toEqual({ point: requireValue(points[0]), segment: 0, t: 0 })
		expect(samples.at(-1)).toEqual({ point: requireValue(points.at(-1)), segment: 1, t: 1 })
		const join = requireValue(curves[0])[3]
		expect(samples.filter(({ point }) => point.x === join.x && point.y === join.y)).toHaveLength(1)
		let previous = -1
		for (const sample of samples) {
			expect(sample.segment + sample.t).toBeGreaterThan(previous)
			previous = sample.segment + sample.t
			const exact = cubicBezierPoint(requireValue(curves[sample.segment]), sample.t)
			expect(sample.point.x).toBeCloseTo(exact.x, 10)
			expect(sample.point.y).toBeCloseTo(exact.y, 10)
		}
		expect(() => sampleSpline(spline, 0)).toThrow()
		requireValue(samples[0]).point.x = 100
		expect(spline.points[0]?.x).toBe(0)
	}
})
it("retains fit points and rejects invalid native spline encodings", () => {
	const data = {
		id: "fit",
		type: "spline",
		mode: "fit",
		points: [
			{ x: 0, y: 0 },
			{ x: 5, y: 10 },
			{ x: 10, y: 0 }
		]
	}
	const spline = normalizeSpline(data)
	expect(splineBezierSegments(spline)).toHaveLength(2)
	expect(spline.points).toEqual(data.points)
	for (const invalid of [
		{ ...data, mode: "control" },
		{ ...data, construction: "yes" },
		{
			...data,
			points: [
				{ x: Number.NaN, y: 0 },
				{ x: 1, y: 1 }
			]
		},
		{ ...data, points: [data.points[0], data.points[0]] }
	])
		expect(() => normalizeSpline(invalid)).toThrow()
})
it("keeps splines open until an explicit closing edge exists and persists native points and constraints", async () => {
	const { materializeSketch } = await import("../src/cad/sketch")
	const { solveSketch, normalizeSketchRelations } = await import("../src/sketch-solver")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const spline = normalizeSpline({
		id: "spline",
		type: "spline",
		mode: "fit",
		points: [
			{ x: 0, y: 0 },
			{ x: 5, y: 5 },
			{ x: 10, y: 0 }
		]
	})
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [spline],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	expect(materializeSketch(sketch).profiles).toHaveLength(0)
	expect(solveSketch(sketch.entities, []).degreesOfFreedom).toBe(6)
	sketch.relations = normalizeSketchRelations([{ id: "fixed", type: "fixed", anchor: { entityId: "spline", point: "point0" }, position: { x: 0, y: 0 } }])
	expect(solveSketch(sketch.entities, sketch.relations ?? []).degreesOfFreedom).toBe(4)
	sketch.entities.push({ id: "chord", type: "line", p0: { x: 10, y: 0 }, p1: { x: 0, y: 0 } })
	expect(materializeSketch(sketch).profiles).toHaveLength(1)
	const state = createPartRuntimeState({ features: [sketch] })
	expect([...state.cad.nodes.values()].some((n) => n.type === "sketchSpline")).toBe(true)
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.entities[0]).toEqual(spline)
	expect(saved.relations).toEqual(sketch.relations)
	expect(saved.profiles).toHaveLength(1)
	spline.construction = true
	expect(materializeSketch(sketch).profiles).toHaveLength(0)
})
it("transforms native spline points without changing fit mode and selects the open curve", async () => {
	const { translateSketchEntity } = await import("../src/sketch-pattern")
	const { rotateSketchEntity } = await import("../src/sketch-circular-pattern")
	const { mirrorSketchEntity } = await import("../src/sketch-mirror")
	const { scaleSketchEntity } = await import("../src/sketch-translate")
	const { sketchEntityInBox } = await import("../src/sketch-selection")
	const spline = normalizeSpline({
		id: "s",
		type: "spline",
		mode: "control",
		points: [
			{ x: 0, y: 0 },
			{ x: 0, y: 10 },
			{ x: 10, y: 10 },
			{ x: 10, y: 0 }
		]
	})
	const moved = translateSketchEntity(spline, "m", { x: 5, y: 2 })
	expect(moved).toMatchObject({
		mode: "control",
		points: [
			{ x: 5, y: 2 },
			{ x: 5, y: 12 },
			{ x: 15, y: 12 },
			{ x: 15, y: 2 }
		]
	})
	const rotated = rotateSketchEntity(spline, "r", { x: 0, y: 0 }, 90)
	const mirrored = mirrorSketchEntity(spline, { id: "axis", type: "line", p0: { x: 0, y: 0 }, p1: { x: 0, y: 1 } }, "m")
	const scaled = scaleSketchEntity(spline, "s", { x: 0, y: 0 }, 2)
	if (rotated.type !== "spline" || mirrored.type !== "spline" || scaled.type !== "spline") throw Error("Lost spline")
	expect(rotated.points[1]?.x).toBeCloseTo(-10)
	expect(mirrored.points[2]).toEqual({ x: -10, y: 10 })
	expect(scaled.points[2]).toEqual({ x: 20, y: 20 })
	expect(sketchEntityInBox(spline, { minX: -1, maxX: 11, minY: -1, maxY: 8 }, false)).toBe(true)
	expect(sketchEntityInBox(spline, { minX: 4, maxX: 6, minY: -1, maxY: 1 }, true)).toBe(false)
	expect(sketchEntityInBox(spline, { minX: 4, maxX: 6, minY: 7, maxY: 8 }, true)).toBe(true)
})
it("round-trips native fit and control splines through the project-file normalizer", async () => {
	const { normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	for (const mode of ["fit", "control"] as const) {
		const spline = normalizeSpline({
			id: "curve",
			type: "spline",
			mode,
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			],
			construction: true
		})
		const sketch: import("../src/schema").Sketch = {
			id: "sketch",
			type: "sketch",
			dirty: false,
			target: { type: "plane", plane: "XY" },
			entities: [spline],
			relations: [{ id: "fixed", type: "fixed", anchor: { entityId: "curve", point: "point1" }, position: { x: 0, y: 10 } }],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}
		const state = createPartRuntimeState({ features: [sketch] })
		const data = { features: [sketch], cad: serializePCadState(state.cad), tree: state.tree }
		const file = requireValue(normalizeProjectFile({ version: 4, items: [{ id: "part", type: "part", name: "Spline", data }], selectedPath: [0] }))
		const roundTrip = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = roundTrip.items[0]
		if (!part || !("type" in part) || part.type !== "part" || !part.data) throw Error("Missing part")
		const loaded = createPartRuntimeState(part.data)
		const restored = materializePartFeatures(loaded.cad, loaded.tree)[0]
		if (restored?.type !== "sketch") throw Error("Missing sketch")
		expect(restored.entities[0]).toEqual(spline)
		expect(restored.relations).toEqual(sketch.relations)
		expect(restored.profiles).toHaveLength(0)
		for (const sketchId of ["", "   "]) {
			const malformed = structuredClone(data)
			const node = malformed.cad.nodes.find((n) => n.type === "sketchSpline")
			if (node?.type !== "sketchSpline") throw Error("Missing spline node")
			malformed.cad = { ...malformed.cad, nodes: malformed.cad.nodes.map((n) => (n.id === node.id ? { ...node, sketchId } : n)) }
			expect(() => normalizeProjectFile({ version: 4, items: [{ id: "part", type: "part", name: "Spline", data: malformed }], selectedPath: [0] })).toThrow("Invalid sketch spline")
		}
	}
})

it("splits fit and control splines exactly without moving either resulting curve", () => {
	for (const mode of ["fit", "control"] as const) {
		const spline = normalizeSpline({ id: "source", type: "spline", mode, construction: true, points: Array.from({ length: 10 }, (_, i) => ({ x: i * 3, y: Math.sin(i) * 10 })) })
		const original = structuredClone(spline)
		const curves = splineBezierSegments(spline)
		for (const t of [0, 0.17, 0.5, 1]) {
			const [left, right] = splitSpline(spline, 1, t, "right")
			const a = splineBezierSegments(left)
			const b = splineBezierSegments(right)
			expect(left.mode).toBe("control")
			expect(right.construction).toBe(true)
			expect(left.points.at(-1)).toEqual(right.points[0])
			for (let i = 0; i <= 20; i++) {
				const u = i / 20
				if (t > 0) {
					const actual = cubicBezierPoint(requireValue(a.at(-1)), u)
					const expected = cubicBezierPoint(requireValue(curves[1]), u * t)
					expect(actual.x).toBeCloseTo(expected.x, 10)
					expect(actual.y).toBeCloseTo(expected.y, 10)
				}
				if (t < 1) {
					const actual = cubicBezierPoint(requireValue(b[0]), u)
					const expected = cubicBezierPoint(requireValue(curves[1]), t + u * (1 - t))
					expect(actual.x).toBeCloseTo(expected.x, 10)
					expect(actual.y).toBeCloseTo(expected.y, 10)
				}
			}
			expect(a[0]).toEqual(curves[0])
			requireValue(left.points[0]).x = 999
			expect(spline).toEqual(original)
			expect(right.points[0]?.x).not.toBe(999)
		}
		expect(() => splitSpline(spline, 0, 0, "right")).toThrow("inside")
		expect(() => splitSpline(spline, curves.length - 1, 1, "right")).toThrow("inside")
		for (const [segment, t] of [
			[-1, 0.5],
			[0.5, 0.5],
			[0, Number.NaN],
			[0, 2]
		])
			expect(() => splitSpline(spline, requireValue(segment), requireValue(t), "right")).toThrow("parameter")
		expect(() => splitSpline(spline, 0, 0.5, "source")).toThrow("IDs")
	}
})

it("targets native spline curves rather than their control polygons or closing chords", () => {
	const spline = normalizeSpline({
		id: "s",
		type: "spline",
		mode: "control",
		points: [
			{ x: 0, y: 0 },
			{ x: 0, y: 10 },
			{ x: 10, y: 10 },
			{ x: 10, y: 0 },
			{ x: 10, y: -10 },
			{ x: 20, y: -10 },
			{ x: 20, y: 0 }
		]
	})
	const original = structuredClone(spline)
	for (const [segment, t] of [
		[0, 0.137],
		[1, 0.781],
		[0, 1]
	]) {
		const expected = cubicBezierPoint(requireValue(splineBezierSegments(spline)[requireValue(segment)]), requireValue(t))
		const hit = nearestSplinePoint(spline, expected)
		expect(hit.distance).toBeLessThanOrEqual(1e-7)
		expect(Math.hypot(hit.point.x - expected.x, hit.point.y - expected.y)).toBeLessThanOrEqual(1e-7)
	}
	const above = nearestSplinePoint(spline, { x: 5, y: 12 })
	expect(above.segment).toBe(0)
	expect(above.t).toBeCloseTo(0.5, 4)
	expect(above.distance).toBeCloseTo(4.5, 6)
	const end = nearestSplinePoint(spline, { x: -5, y: -5 })
	expect(end.t).toBe(0)
	expect(end.point).toEqual({ x: 0, y: 0 })
	expect(spline).toEqual(original)
	for (const tolerance of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) expect(() => nearestSplinePoint(spline, { x: 0, y: 0 }, tolerance)).toThrow("finite")
	expect(() => nearestSplinePoint(spline, { x: Number.NaN, y: 0 })).toThrow("finite")
})

it("retains every unchanged control anchor when normalized cuts land on segment joins", () => {
	for (const count of [49, 98]) {
		const source = normalizeSpline({ id: "source", type: "spline", mode: "control", points: Array.from({ length: count * 3 + 1 }, (_, i) => ({ x: i, y: i % 3 })) })
		for (let join = 1; join < count; join++) {
			const left = splinePortionWithAnchors(source, 0, join / count, "left")
			const right = splinePortionWithAnchors(source, join / count, 1, "right")
			expect(left.spline.points).toEqual(source.points.slice(0, join * 3 + 1))
			expect(right.spline.points).toEqual(source.points.slice(join * 3))
			for (let i = 0; i <= join * 3; i++) expect(left.anchors.get(i)).toBe(i)
			for (let i = join * 3; i < source.points.length; i++) expect(right.anchors.get(i)).toBe(i - join * 3)
		}
	}
})

it("does not preserve subdivided handles for a cut close to but distinct from a segment join", () => {
	const source = normalizeSpline({ id: "source", type: "spline", mode: "control", points: Array.from({ length: 148 }, (_, i) => ({ x: i, y: i % 3 })) })
	for (const displacement of [-1e-8, 1e-8]) {
		const piece = splinePortionWithAnchors(source, 0, (1 + displacement) / 49, "piece")
		const changedHandle = displacement < 0 ? 1 : 4
		expect(piece.anchors.has(changedHandle)).toBe(false)
	}
})
