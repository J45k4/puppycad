import { expect, it } from "bun:test"
import { ellipticArcBounds, nearestEllipticArcPoint, ellipticArcPoint, ellipticArcPortion, normalizeEllipticArc, sampleEllipticArc, type EllipticArc } from "../src/sketch-elliptic-arc"
import { requireValue } from "../src/required"
const source = (): EllipticArc => ({ id: "a", type: "ellipticArc", center: { x: 12, y: -9 }, width: 100, height: 6, rotation: 37, startAngle: 5.7, sweep: 4.1, segments: 128 })

it("retains exact rotated ellipse portions in either direction across the angular seam", () => {
	for (const sign of [-1, 1]) {
		const arc = source()
		arc.sweep *= sign
		const before = structuredClone(arc)
		const piece = ellipticArcPortion(arc, 0.2, 0.8, "piece")
		for (let i = 0; i <= 100; i++) {
			const expected = ellipticArcPoint(arc, 0.2 + (0.6 * i) / 100)
			const point = ellipticArcPoint(piece, i / 100)
			expect(Math.hypot(point.x - expected.x, point.y - expected.y)).toBeLessThan(1e-12)
		}
		expect(arc).toEqual(before)
		expect(piece.id).toBe("piece")
	}
})

it("samples open elliptic arcs within the requested chord error without adding a closing edge", () => {
	for (const sweep of [0.1, 4.1, -4.1, 2 * Math.PI]) {
		const arc = { ...source(), sweep }
		const points = sampleEllipticArc(arc, 0.005)
		expect(points[0]).toEqual(ellipticArcPoint(arc, 0))
		expect(points.at(-1)).toEqual(ellipticArcPoint(arc, 1))
		for (let i = 0; i < points.length - 1; i++) {
			const a = requireValue(points[i])
			const b = requireValue(points[i + 1])
			for (const t of [0.1, 0.5, 0.9]) {
				const actual = ellipticArcPoint(arc, (i + t) / (points.length - 1))
				expect(Math.hypot(actual.x - (a.x + t * (b.x - a.x)), actual.y - (a.y + t * (b.y - a.y)))).toBeLessThanOrEqual(0.005)
			}
		}
		if (sweep !== 2 * Math.PI) expect(points[0]).not.toEqual(points.at(-1))
	}
})

it("validates native data and bounds excessive sampling", () => {
	const arc = source()
	expect(normalizeEllipticArc(JSON.parse(JSON.stringify(arc)))).toEqual(arc)
	for (const change of [
		{ sweep: 0 },
		{ sweep: 7 },
		{ width: -1 },
		{ height: 0 },
		{ rotation: Number.NaN },
		{ center: { x: Number.POSITIVE_INFINITY, y: 0 } },
		{ segments: 7 },
		{ construction: 1 }
	])
		expect(() => normalizeEllipticArc({ ...arc, ...change })).toThrow()
	expect(() => sampleEllipticArc(arc, 1e-20)).toThrow("point limit")
	expect(() => sampleEllipticArc(arc, 0)).toThrow()
	expect(() => ellipticArcPortion(arc, 0.8, 0.2, "b")).toThrow()
	expect(() => ellipticArcPoint(arc, 2)).toThrow()
})

it("bounds only the retained rotated arc and targets the finite portion", () => {
	for (const sweep of [0.2, 4.1, -4.1, Math.PI * 2]) {
		const arc = { ...source(), sweep }
		const bounds = ellipticArcBounds(arc)
		const samples = Array.from({ length: 4001 }, (_, i) => ellipticArcPoint(arc, i / 4000))
		for (const point of samples) {
			expect(point.x).toBeGreaterThanOrEqual(bounds.min.x - 1e-10)
			expect(point.x).toBeLessThanOrEqual(bounds.max.x + 1e-10)
			expect(point.y).toBeGreaterThanOrEqual(bounds.min.y - 1e-10)
			expect(point.y).toBeLessThanOrEqual(bounds.max.y + 1e-10)
		}
		for (const target of [{ x: 0, y: 0 }, arc.center, { x: -100, y: 20 }, { x: 80, y: 90 }, ellipticArcPoint(arc, 0.37)]) {
			const hit = nearestEllipticArcPoint(arc, target)
			const sampled = Math.min(...samples.map((point) => Math.hypot(point.x - target.x, point.y - target.y)))
			expect(hit.distance).toBeLessThanOrEqual(sampled + 1e-8)
			expect(hit.point).toEqual(ellipticArcPoint(arc, hit.t))
		}
	}
})

it("selects finite endpoints when the closest supporting-ellipse point is outside the arc", () => {
	const arc = { ...source(), center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI / 2 }
	const hit = nearestEllipticArcPoint(arc, { x: -20, y: 0 })
	expect(hit.t).toBe(1)
	expect(hit.point.x).toBeCloseTo(0, 12)
	expect(hit.point.y).toBe(5)
	expect(() => nearestEllipticArcPoint(arc, { x: Number.NaN, y: 0 })).toThrow()
})

it("materializes open elliptic arcs, closes only with a chord, and retains native geometry through project reload", async () => {
	const { materializeSketch } = await import("../src/cad/sketch")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const { createProjectFile, normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
	const arc = source()
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [arc],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	expect(materializeSketch(sketch).profiles).toHaveLength(0)
	sketch.entities.push({ id: "chord", type: "line", p0: ellipticArcPoint(arc, 1), p1: ellipticArcPoint(arc, 0) })
	const closed = materializeSketch(sketch)
	expect(closed.profiles).toHaveLength(1)
	for (const native of [false, true]) {
		const runtime = createPartRuntimeState({ features: [closed] })
		const file = createProjectFile({
			items: [{ id: "p", type: "part", name: "Elliptic arc", data: { features: [closed], ...(native ? { cad: serializePCadState(runtime.cad), tree: runtime.tree } : {}) } }],
			selectedPath: null
		})
		const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
		const part = restored.items[0] as import("../src/contract").ProjectPartDocument
		const state = createPartRuntimeState(requireValue(part.data))
		const saved = materializePartFeatures(state.cad, state.tree)[0]
		if (saved?.type !== "sketch") throw Error("Missing sketch")
		expect(saved.entities[0]).toEqual(arc)
		expect(saved.profiles).toHaveLength(1)
	}
})

it("solves elliptic arc dimensions and keeps native endpoint anchors", async () => {
	const { solveSketch } = await import("../src/sketch-solver")
	const { entityAnchorPoint, entityAnchorNames } = await import("../src/sketch-curves")
	const arc = source()
	expect(entityAnchorNames(arc)).toEqual(["p0", "p1", "center"])
	expect(entityAnchorPoint(arc, "p1")).toEqual(ellipticArcPoint(arc, 1))
	expect(solveSketch([arc], []).degreesOfFreedom).toBe(7)
	const result = solveSketch(
		[arc],
		[
			{ id: "center", type: "fixed", anchor: { entityId: arc.id, point: "center" }, position: arc.center },
			{ id: "width", type: "width", entityId: arc.id, value: 80 },
			{ id: "height", type: "height", entityId: arc.id, value: 12 },
			{ id: "rotation", type: "rotation", entityId: arc.id, value: 20 },
			{ id: "sweep", type: "arcSweep", entityId: arc.id, value: 180 }
		]
	)
	expect(result.status).not.toBe("conflicting")
	expect(result.degreesOfFreedom).toBe(1)
	const solved = result.entities[0]
	if (solved?.type !== "ellipticArc") throw Error("Missing elliptic arc")
	expect(solved.width).toBeCloseTo(80, 5)
	expect(solved.height).toBeCloseTo(12, 5)
	expect(solved.sweep).toBeCloseTo(Math.PI, 6)
})

it("filters intersections to finite elliptic sweeps and snaps with both native references", async () => {
	const { finiteSketchCurveIntersections } = await import("../src/sketch-edit")
	const { sketchSnap } = await import("../src/sketch-snap")
	const arc: EllipticArc = { ...source(), center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0.1, sweep: Math.PI - 0.2 }
	const line: import("../src/schema").Line = { id: "line", type: "line", p0: { x: 0, y: -10 }, p1: { x: 0, y: 10 } }
	const hits = finiteSketchCurveIntersections(arc, line)
	expect(hits).toHaveLength(1)
	expect(requireValue(hits[0]).y).toBeCloseTo(5, 8)
	expect(finiteSketchCurveIntersections(line, arc)).toEqual(hits)
	const snap = sketchSnap([arc, line], { x: 0.01, y: 5.01 }, 10, { grid: false, geometry: true })
	expect(snap?.kind).toBe("intersection")
	expect(snap?.curves).toEqual([arc.id, line.id])
	const lower = { ...arc, id: "lower", startAngle: Math.PI + 0.1 }
	expect(finiteSketchCurveIntersections(lower, line)).toHaveLength(1)
	expect(requireValue(finiteSketchCurveIntersections(lower, line)[0]).y).toBeCloseTo(-5, 8)
	const shifted = { ...arc, id: "shifted", center: { x: 5, y: 0 } }
	const pair = finiteSketchCurveIntersections(arc, shifted)
	expect(pair).toHaveLength(1)
	expect(requireValue(pair[0]).y).toBeGreaterThan(0)
})

it("splits a native ellipse into a closed pair and trims it into a finite arc", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { materializeSketch } = await import("../src/cad/sketch")
	const ellipse: import("../src/schema").Ellipse = { id: "ellipse", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 128 }
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [ellipse],
		relations: [{ id: "center", type: "fixed", anchor: { entityId: ellipse.id, point: "center" }, position: ellipse.center }],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const before = structuredClone(sketch)
	const split = editSketchCurve(sketch, ellipse.id, { x: 10, y: 0 }, "Split", { x: -10, y: 0 })
	expect(split.sketch.entities.map((e) => e.type)).toEqual(["ellipticArc", "ellipticArc"])
	expect(split.removedRelations).toEqual([])
	expect(split.sketch.relations?.filter((r) => r.type === "coincident")).toHaveLength(2)
	expect(materializeSketch(split.sketch).profiles).toHaveLength(1)
	sketch.entities.push({ id: "boundary", type: "line", p0: { x: -20, y: 0 }, p1: { x: 20, y: 0 } })
	const trimmed = editSketchCurve(sketch, ellipse.id, { x: 0, y: 5 }, "Trim")
	const arc = trimmed.sketch.entities[0]
	if (arc?.type !== "ellipticArc") throw Error("Missing trimmed arc")
	expect(ellipticArcPoint(arc, 0.5).y).toBeCloseTo(-5, 8)
	expect(trimmed.removedRelations).toEqual([])
	expect(before.entities).toEqual([ellipse])
	expect(() => editSketchCurve(before, ellipse.id, { x: 10, y: 0 }, "Extend")).toThrow("extension")
})

it("uses only retained elliptic boundaries when trimming lines and splines or extending lines", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { splineBezierSegments } = await import("../src/sketch-spline")
	const boundary: EllipticArc = { id: "boundary", type: "ellipticArc", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI, segments: 128 }
	const make = (entity: import("../src/schema").SketchEntity): import("../src/schema").Sketch => ({
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [entity, boundary],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	})
	const line: import("../src/schema").Line = { id: "target", type: "line", p0: { x: 0, y: -10 }, p1: { x: 0, y: 10 } }
	const trimmed = editSketchCurve(make(line), line.id, { x: 0, y: 9 }, "Trim").sketch.entities[0]
	if (trimmed?.type !== "line") throw Error("Expected retained line")
	expect(trimmed.p0.y).toBe(-10)
	expect(trimmed.p1.y).toBeCloseTo(5, 8)
	const short = { ...line, p0: { x: 0, y: 0 }, p1: { x: 0, y: 2 } }
	const extended = editSketchCurve(make(short), short.id, short.p1, "Extend").sketch.entities[0]
	if (extended?.type !== "line") throw Error("Expected extended line")
	expect(extended.p1.y).toBeCloseTo(5, 8)
	expect(() => editSketchCurve(make(short), short.id, short.p0, "Extend")).toThrow("No boundary")
	const spline: import("../src/schema").Spline = {
		id: "target",
		type: "spline",
		mode: "control",
		points: [
			{ x: 0, y: -10 },
			{ x: 0, y: -3 },
			{ x: 0, y: 3 },
			{ x: 0, y: 10 }
		]
	}
	const retained = editSketchCurve(make(spline), spline.id, { x: 0, y: 9 }, "Trim").sketch.entities[0]
	if (retained?.type !== "spline") throw Error("Expected retained spline")
	const segments = splineBezierSegments(retained)
	expect(requireValue(segments[0])[0].y).toBe(-10)
	expect(requireValue(segments.at(-1))[3].y).toBeCloseTo(5, 7)
})

it("trims circles and extends circular arcs against finite elliptic boundaries", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { arcPoint } = await import("../src/sketch-curves")
	const boundary: EllipticArc = { id: "boundary", type: "ellipticArc", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI, segments: 128 }
	const circle: import("../src/schema").Circle = { id: "target", type: "circle", center: { x: 0, y: 0 }, radius: 7, segments: 128 }
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [circle, boundary],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const trimmed = editSketchCurve(sketch, circle.id, { x: 0, y: 7 }, "Trim").sketch.entities[0]
	if (trimmed?.type !== "arc") throw Error("Expected circular arc")
	expect(trimmed.sweep).toBeGreaterThan(Math.PI)
	expect(arcPoint(trimmed, 0.5).y).toBeCloseTo(-7, 7)
	for (const t of [0, 1]) {
		const p = arcPoint(trimmed, t)
		expect(p.y).toBeGreaterThan(0)
		expect(p.x ** 2 / 100 + p.y ** 2 / 25).toBeCloseTo(1, 8)
	}
	sketch.entities[0] = { ...circle, type: "arc", startAngle: -Math.PI / 2, sweep: Math.PI / 2 }
	const extended = editSketchCurve(sketch, circle.id, { x: 7, y: 0 }, "Extend").sketch.entities[0]
	if (extended?.type !== "arc") throw Error("Expected extended arc")
	const end = arcPoint(extended, 1)
	expect(end.y).toBeGreaterThan(0)
	expect(end.x ** 2 / 100 + end.y ** 2 / 25).toBeCloseTo(1, 8)
	expect(extended.startAngle).toBe(-Math.PI / 2)
})

it("trims native ellipses against finite rectangle and polygon edges", async () => {
	const { finiteSketchCurveIntersections, editSketchCurve } = await import("../src/sketch-edit")
	const ellipse: import("../src/schema").Ellipse = { id: "target", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 128 }
	const outlines: import("../src/schema").SketchEntity[] = [
		{ id: "boundary", type: "cornerRectangle", p0: { x: -20, y: 0 }, p1: { x: 20, y: 20 } },
		{ id: "boundary", type: "rectangle", center: { x: 0, y: 10 }, width: 40, height: 20, rotation: 0 },
		{ id: "boundary", type: "polygon", center: { x: 0, y: 20 }, radius: Math.sqrt(800), rotation: 45, sides: 4 }
	]
	for (const boundary of outlines) {
		const hits = finiteSketchCurveIntersections(ellipse, boundary)
		expect(hits).toHaveLength(2)
		expect(finiteSketchCurveIntersections(boundary, ellipse)).toEqual(hits)
		for (const hit of hits) {
			expect(hit.y).toBeCloseTo(0, 7)
			expect(Math.abs(hit.x)).toBeCloseTo(10, 7)
		}
		const sketch: import("../src/schema").Sketch = {
			id: "s",
			type: "sketch",
			dirty: false,
			target: { type: "plane", plane: "XY" },
			entities: [ellipse, boundary],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}
		const trimmed = editSketchCurve(sketch, ellipse.id, { x: 0, y: 5 }, "Trim").sketch.entities[0]
		if (trimmed?.type !== "ellipticArc") throw Error("Expected native elliptic arc")
		expect(ellipticArcPoint(trimmed, 0.5).y).toBeCloseTo(-5, 7)
		expect(sketch.entities[0]).toEqual(ellipse)
	}
	const distant: import("../src/schema").CornerRectangle = { id: "far", type: "cornerRectangle", p0: { x: 20, y: 0 }, p1: { x: 30, y: 10 } }
	expect(finiteSketchCurveIntersections(ellipse, distant)).toEqual([])
	const tangent: import("../src/schema").CornerRectangle = { id: "touch", type: "cornerRectangle", p0: { x: 10, y: 0 }, p1: { x: 20, y: 10 } }
	expect(finiteSketchCurveIntersections(ellipse, tangent)).toHaveLength(1)
})

it("retains elliptic endpoints across angular rounding without including removed intervals", async () => {
	const { ellipticArcParameter } = await import("../src/sketch-elliptic-arc")
	const { finiteSketchCurveIntersections } = await import("../src/sketch-edit")
	for (const startAngle of [0, 0.7, Math.PI, 2 * Math.PI - 0.01]) {
		for (const sign of [-1, 1]) {
			const arc = { ...source(), startAngle, sweep: sign * 1.2 }
			expect(ellipticArcParameter(arc, startAngle - sign * 2e-14)).toBe(0)
			expect(ellipticArcParameter(arc, startAngle + arc.sweep + sign * 2e-14)).toBe(1)
			expect(ellipticArcParameter(arc, startAngle - sign * 1e-7)).toBeNull()
			expect(ellipticArcParameter(arc, startAngle + arc.sweep + sign * 1e-7)).toBeNull()
			expect(ellipticArcParameter(arc, startAngle + arc.sweep * 0.4)).toBeCloseTo(0.4, 12)
			const endpoint = ellipticArcPoint(arc, 0)
			const line: import("../src/schema").Line = { id: "radial", type: "line", p0: endpoint, p1: { x: 2 * endpoint.x - arc.center.x, y: 2 * endpoint.y - arc.center.y } }
			const hits = finiteSketchCurveIntersections(arc, line)
			expect(hits).toHaveLength(1)
			expect(requireValue(hits[0]).x).toBeCloseTo(endpoint.x, 7)
			expect(requireValue(hits[0]).y).toBeCloseTo(endpoint.y, 7)
		}
	}
	expect(ellipticArcParameter(source(), Number.NaN)).toBeNull()
	for (const sign of [-1, 1]) {
		const tiny = { ...source(), startAngle: 0, sweep: sign * 1e-15 }
		expect(ellipticArcParameter(tiny, sign * 5e-16)).toBeCloseTo(0.5, 12)
	}
})

it("extends either elliptic endpoint in both directions while preserving its supporting ellipse", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	for (const sign of [-1, 1]) {
		for (const start of [false, true]) {
			const arc: EllipticArc = { ...source(), startAngle: 5.7, sweep: sign * 0.8 }
			const wanted = start ? -0.3 : 1.3
			const point = ellipticArcPoint({ ...arc, startAngle: arc.startAngle + wanted * arc.sweep }, 0)
			const boundary: import("../src/schema").Line = { id: "boundary", type: "line", p0: point, p1: { x: point.x + (point.x - arc.center.x), y: point.y + (point.y - arc.center.y) } }
			const sketch: import("../src/schema").Sketch = {
				id: "s",
				type: "sketch",
				dirty: false,
				target: { type: "plane", plane: "XY" },
				entities: [arc, boundary],
				dimensions: [],
				vertices: [],
				loops: [],
				profiles: [],
				relations: [{ id: "center", type: "fixed", anchor: { entityId: arc.id, point: "center" }, position: arc.center }]
			}
			const result = editSketchCurve(sketch, arc.id, ellipticArcPoint(arc, start ? 0 : 1), "Extend")
			const extended = result.sketch.entities[0]
			if (extended?.type !== "ellipticArc") throw Error("Expected native elliptic arc")
			expect(extended.center).toEqual(arc.center)
			expect(extended.width).toBe(arc.width)
			expect(extended.height).toBe(arc.height)
			expect(extended.rotation).toBe(arc.rotation)
			expect(extended.sweep).toBeCloseTo(sign * 1.04, 8)
			const end = ellipticArcPoint(extended, start ? 0 : 1)
			expect(end.x).toBeCloseTo(point.x, 7)
			expect(end.y).toBeCloseTo(point.y, 7)
			const unchanged = ellipticArcPoint(extended, start ? 1 : 0)
			const old = ellipticArcPoint(arc, start ? 1 : 0)
			expect(unchanged.x).toBeCloseTo(old.x, 7)
			expect(unchanged.y).toBeCloseTo(old.y, 7)
			expect(result.removedRelations).toEqual([])
			expect(sketch.entities[0]).toEqual(arc)
			sketch.entities = [arc]
			expect(() => editSketchCurve(sketch, arc.id, ellipticArcPoint(arc, 1), "Extend")).toThrow("No boundary")
		}
	}
})

it("keeps split elliptic pieces on one supporting ellipse through deformation and PCad reload", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { materializeSketch } = await import("../src/cad/sketch")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const arc: EllipticArc = { ...source(), width: 20, height: 10, rotation: 10, startAngle: 0, sweep: 2 * Math.PI }
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [{ ...arc, type: "ellipse" }],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const split = editSketchCurve(sketch, arc.id, ellipticArcPoint(arc, 0), "Split", ellipticArcPoint(arc, 0.5)).sketch
	expect(split.relations?.filter((r) => r.type === "sameEllipse")).toHaveLength(1)
	const runtime = createPartRuntimeState({ features: [materializeSketch(split)] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations?.filter((r) => r.type === "sameEllipse")).toHaveLength(1)
	saved.relations?.push(
		{ id: "width", type: "width", entityId: arc.id, value: 30 },
		{ id: "height", type: "height", entityId: arc.id, value: 14 },
		{ id: "rotation", type: "rotation", entityId: arc.id, value: 35 },
		{ id: "center", type: "fixed", anchor: { entityId: arc.id, point: "center" }, position: { x: 5, y: 6 } }
	)
	const changed = materializeSketch(saved)
	expect(changed.profiles).toHaveLength(1)
	for (const entity of changed.entities) {
		if (entity.type !== "ellipticArc") throw Error("Expected native arcs")
		expect(entity.width).toBeCloseTo(30, 5)
		expect(entity.height).toBeCloseTo(14, 5)
		expect(entity.rotation).toBeCloseTo(35, 5)
		expect(entity.center.x).toBeCloseTo(5, 5)
		expect(entity.center.y).toBeCloseTo(6, 5)
	}
})

it("remaps shared ellipse copies and retains the dependency chain when splitting again", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { translateSketchSelection, rotateSketchSelection, scaleSketchSelection } = await import("../src/sketch-translate")
	const { materializeSketch } = await import("../src/cad/sketch")
	const { sketchRelationEntityIds } = await import("../src/sketch-relations")
	const arc: EllipticArc = { ...source(), center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: 2 * Math.PI }
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [{ ...arc, type: "ellipse" }],
		relations: [],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const split = editSketchCurve(sketch, arc.id, { x: 10, y: 0 }, "Split", { x: -10, y: 0 }).sketch
	const copied = translateSketchSelection(
		split,
		split.entities.map((e) => e.id),
		{ x: 50, y: 0 },
		true
	)
	const copiedRelation = copied.sketch.relations?.find((r) => r.type === "sameEllipse" && sketchRelationEntityIds(r).every((id) => copied.selected.includes(id)))
	expect(copiedRelation).toBeDefined()
	const rotated = rotateSketchSelection(copied.sketch, copied.selected, { x: 50, y: 0 }, 25)
	const scaled = scaleSketchSelection(rotated.sketch, copied.selected, { x: 50, y: 0 }, 2)
	const changed = materializeSketch(scaled.sketch)
	expect(changed.profiles).toHaveLength(2)
	for (const entity of changed.entities) {
		if (entity.type !== "ellipticArc") throw Error("Expected arc")
		expect(entity.width).toBeCloseTo(copied.selected.includes(entity.id) ? 40 : 20, 6)
		expect(entity.height).toBeCloseTo(copied.selected.includes(entity.id) ? 20 : 10, 6)
		expect(entity.rotation).toBeCloseTo(copied.selected.includes(entity.id) ? 25 : 0, 6)
	}
	const first = split.entities[0]
	if (first?.type !== "ellipticArc") throw Error("Expected source arc")
	const again = editSketchCurve(split, first.id, ellipticArcPoint(first, 0.5), "Split").sketch
	expect(again.entities).toHaveLength(3)
	expect(again.relations?.filter((r) => r.type === "sameEllipse")).toHaveLength(2)
	again.relations?.push({ id: "width", type: "width", entityId: first.id, value: 28 })
	const resized = materializeSketch(again)
	expect(resized.profiles).toHaveLength(1)
	for (const entity of resized.entities) {
		if (entity.type !== "ellipticArc") throw Error("Expected linked arc")
		expect(entity.width).toBeCloseTo(28, 5)
	}
})

it("selects finite elliptic arcs analytically with tiny crossing boxes and exact containment bounds", async () => {
	const { sketchEntityInBox } = await import("../src/sketch-selection")
	for (const sign of [-1, 1]) {
		const arc = { ...source(), sweep: sign * 2.1 }
		const bounds = ellipticArcBounds(arc)
		expect(sketchEntityInBox(arc, { minX: bounds.min.x, minY: bounds.min.y, maxX: bounds.max.x, maxY: bounds.max.y }, false)).toBe(true)
		expect(sketchEntityInBox(arc, { minX: bounds.min.x + 1e-5, minY: bounds.min.y, maxX: bounds.max.x, maxY: bounds.max.y }, false)).toBe(false)
		for (const t of [0.137, 0.371, 0.613, 0.891]) {
			const p = ellipticArcPoint(arc, t)
			expect(sketchEntityInBox(arc, { minX: p.x - 1e-6, maxX: p.x + 1e-6, minY: p.y - 1e-6, maxY: p.y + 1e-6 }, true)).toBe(true)
		}
		const removed = ellipticArcPoint({ ...arc, startAngle: arc.startAngle - sign * 0.2 }, 0)
		expect(sketchEntityInBox(arc, { minX: removed.x - 1e-5, maxX: removed.x + 1e-5, minY: removed.y - 1e-5, maxY: removed.y + 1e-5 }, true)).toBe(false)
	}
})

it("uses exact slot sides and caps as curve-edit boundaries independently of display segments", async () => {
	const { finiteSketchCurveIntersections, editSketchCurve } = await import("../src/sketch-edit")
	const ellipse: import("../src/schema").Ellipse = { id: "ellipse", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 64 }
	for (const arcSegments of [3, 64]) {
		const slot: import("../src/schema").Capsule = { id: "slot", type: "capsule", from: { x: -20, y: 0 }, to: { x: 20, y: 0 }, width: 2, arcSegments }
		const hits = finiteSketchCurveIntersections(ellipse, slot)
		expect(hits).toHaveLength(4)
		for (const p of hits) {
			expect(Math.abs(p.y)).toBeCloseTo(1, 8)
			expect(Math.abs(p.x)).toBeCloseTo(10 * Math.sqrt(0.96), 7)
		}
		const line: import("../src/schema").Line = { id: "line", type: "line", p0: { x: 20, y: -3 }, p1: { x: 20, y: 3 } }
		expect(finiteSketchCurveIntersections(slot, line)).toHaveLength(2)
		const capLine = { ...line, p0: { x: 20.5, y: -3 }, p1: { x: 20.5, y: 3 } }
		const caps = finiteSketchCurveIntersections(slot, capLine)
		expect(caps).toHaveLength(2)
		for (const p of caps) expect(Math.abs(p.y)).toBeCloseTo(Math.sqrt(0.75), 7)
		const sketch: import("../src/schema").Sketch = {
			id: "s",
			type: "sketch",
			dirty: false,
			target: { type: "plane", plane: "XY" },
			entities: [capLine, slot],
			relations: [],
			dimensions: [],
			vertices: [],
			loops: [],
			profiles: []
		}
		const trimmed = editSketchCurve(sketch, capLine.id, { x: 20.5, y: 2 }, "Trim").sketch.entities[0]
		if (trimmed?.type !== "line") throw Error("Expected retained line")
		expect(trimmed.p1.y).toBeCloseTo(Math.sqrt(0.75), 7)
	}
})

it("retains ellipse dimension expressions but removes the old sweep when splitting a finite arc", async () => {
	const { editSketchCurve } = await import("../src/sketch-edit")
	const { materializeSketch } = await import("../src/cad/sketch")
	const arc: EllipticArc = { ...source(), center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI }
	const sketch: import("../src/schema").Sketch = {
		id: "s",
		type: "sketch",
		dirty: false,
		target: { type: "plane", plane: "XY" },
		entities: [arc],
		variables: [{ name: "width", kind: "length", expression: "20 mm" }],
		relations: [
			{ id: "width", type: "width", entityId: arc.id, value: 20, expression: "#width" },
			{ id: "height", type: "height", entityId: arc.id, value: 10 },
			{ id: "rotation", type: "rotation", entityId: arc.id, value: 0 },
			{ id: "sweep", type: "arcSweep", entityId: arc.id, value: 180 }
		],
		dimensions: [],
		vertices: [],
		loops: [],
		profiles: []
	}
	const result = editSketchCurve(sketch, arc.id, ellipticArcPoint(arc, 0.5), "Split")
	expect(result.removedRelations).toEqual(["sweep"])
	expect(result.sketch.relations).toContainEqual(expect.objectContaining({ id: "width", expression: "#width" }))
	const variable = result.sketch.variables?.[0]
	if (!variable) throw Error("Missing width variable")
	variable.expression = "30 mm"
	const changed = materializeSketch(result.sketch)
	expect(changed.entities).toHaveLength(2)
	for (const entity of changed.entities) {
		if (entity.type !== "ellipticArc") throw Error("Expected native arc")
		expect(entity.width).toBeCloseTo(30, 5)
		expect(entity.height).toBeCloseTo(10, 5)
		expect(entity.rotation).toBeCloseTo(0, 5)
		expect(Math.abs(entity.sweep)).toBeLessThan(Math.PI)
	}
	expect(sketch.variables?.[0]?.expression).toBe("20 mm")
	expect(sketch.relations?.find((r) => r.id === "sweep")).toBeDefined()
})
