import { beforeEach, expect, it } from "bun:test"
import { Window } from "happy-dom"
import { SketchWorkspace } from "../src/ui/sketch-workspace"
import { SolidFeaturePanel } from "../src/ui/solid-features"
import { PartBuilder, circle, v2 } from "../src/sdk"
import type { Sketch, PartDocument } from "../src/schema"
import { requireValue } from "../src/required"
let dom: Window
beforeEach(() => {
	dom = new Window()
	globalThis.document = dom.document as unknown as Document
	globalThis.window = dom as unknown as typeof window
})
function blank(): Sketch {
	return { id: "sketch", type: "sketch", dirty: false, target: { type: "plane", plane: "XY" }, entities: [], dimensions: [], vertices: [], loops: [], profiles: [] }
}
function click(root: HTMLElement, label: string) {
	const button = Array.from(root.querySelectorAll("button")).find((b) => b.textContent === label)
	requireValue(button).click()
}
function pointer(target: Element, x: number, y: number) {
	target.dispatchEvent(new dom.PointerEvent("pointerdown", { bubbles: true, clientX: x, clientY: y, button: 0 }) as unknown as Event)
}
function input(root: HTMLElement, label: string, value: number) {
	const el = requireValue(root.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`))
	el.value = String(value)
	el.dispatchEvent(new dom.Event("change") as unknown as Event)
}
it("draws a circle on the canvas, edits its driving dimension and commits only on Finish", () => {
	const original = blank()
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Circle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 350)
	click(editor.root, "Select")
	pointer(requireValue(canvas.querySelector('[data-entity-id="circle-1"]')), 560, 350)
	click(editor.root, "Add driving dimension")
	input(editor.root, "Dimension (mm)", 60)
	expect(editor.root.textContent).toContain("Fully constrained")
	expect(original.entities).toHaveLength(0)
	expect(saved).toBeUndefined()
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ type: "circle", radius: expect.closeTo(30, 4) })
	expect(saved?.relations?.find((r) => r.type === "diameter")).toMatchObject({ type: "diameter", value: 60 })
	expect(saved?.profiles).toHaveLength(1)
})
it("retains construction geometry without creating a solid profile and supports local undo/cancel", () => {
	let saved: Sketch | undefined
	let cancelled = false
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => {
			cancelled = true
		}
	)
	document.body.append(editor.root)
	const check = requireValue(editor.root.querySelector<HTMLInputElement>('input[type="checkbox"]'))
	check.checked = true
	check.dispatchEvent(new dom.Event("change") as unknown as Event)
	click(editor.root, "Circle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 350)
	expect(canvas.querySelector('[data-entity-id="circle-1"]')?.getAttribute("stroke-dasharray")).toBe("9 5")
	click(editor.root, "Undo")
	expect(canvas.querySelector('[data-entity-id="circle-1"]')).toBeNull()
	click(editor.root, "Redo")
	expect(canvas.querySelector('[data-entity-id="circle-1"]')).not.toBeNull()
	click(editor.root, "Cancel sketch")
	expect(cancelled).toBe(true)
	expect(saved).toBeUndefined()
})
it("blocks Finish on conflicting dimensions and lets the user remove the conflicting relation", () => {
	const sketch = blank()
	sketch.entities = [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 32 }]
	sketch.relations = [
		{ id: "d1", type: "diameter", entityId: "c", value: 20 },
		{ id: "d2", type: "diameter", entityId: "c", value: 30 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	expect(editor.root.textContent).toContain("Conflicting constraints")
	click(editor.root, "Finish sketch")
	expect(saved).toBeUndefined()
	click(editor.root, "diameter 30 · d2")
	click(editor.root, "Delete constraint")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toHaveLength(1)
})
it("applies a workspace dimension back to an extrusion without replacing constrained entity IDs", () => {
	const builder = new PartBuilder()
	builder.extrude("disc", { outline: circle(v2(0, 0), 10, 32), depth: 5 })
	let saved: PartDocument | undefined
	const panel = new SolidFeaturePanel(builder.document, (next) => {
		saved = next
	})
	document.body.append(panel.root)
	click(panel.root, "Edit sketch")
	const workspace = requireValue(document.querySelector<HTMLElement>('[aria-label="Sketch workspace"]'))
	const path = requireValue(workspace.querySelector("[data-entity-id]"))
	pointer(path, 600, 350)
	click(workspace, "Add driving dimension")
	input(workspace, "Dimension (mm)", 40)
	click(workspace, "Finish sketch")
	click(panel.root, "Apply changes")
	const sketch = saved?.features.find((f) => f.type === "sketch")
	expect(sketch?.relations?.[0]).toMatchObject({ type: "diameter", value: 40 })
	expect(sketch?.entities[0]).toMatchObject({ type: "circle", radius: expect.closeTo(20, 4) })
})
it("creates and saves a standalone plane sketch, then extrudes that same sketch", () => {
	let saved: PartDocument | undefined
	const panel = new SolidFeaturePanel({ features: [], solidSteps: [] }, (next) => {
		saved = next
	})
	document.body.append(panel.root)
	const plane = requireValue(panel.root.querySelector<HTMLSelectElement>('[aria-label="Sketch plane"]'))
	plane.value = "YZ"
	plane.dispatchEvent(new dom.Event("change") as unknown as Event)
	click(panel.root, "New sketch")
	const workspace = requireValue(document.querySelector<HTMLElement>('[aria-label="Sketch workspace"]'))
	click(workspace, "Circle")
	const canvas = requireValue(workspace.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 350)
	click(workspace, "Finish sketch")
	click(panel.root, "Apply changes")
	expect(saved?.solidSteps).toHaveLength(0)
	expect(saved?.features[0]).toMatchObject({ type: "sketch", target: { type: "plane", plane: "YZ" } })
	input(panel.root, "New extrusion depth", 7)
	click(panel.root, "Extrude sketch")
	click(panel.root, "Apply changes")
	expect(saved?.solidSteps).toHaveLength(1)
	expect(saved?.features[1]).toMatchObject({ type: "extrude", depth: 7, target: { sketchId: saved?.features[0]?.id } })
})
it("keeps chained line endpoints coincident and infers horizontal and vertical relations", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(next) => {
			saved = next
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Line")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 350)
	// The selected endpoint must still accept drawing clicks instead of starting a drag.
	pointer(requireValue(canvas.querySelector('[aria-label="line-1 p1"]')), 560, 290)
	pointer(canvas, 500, 290)
	pointer(canvas, 500, 350)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(4)
	expect(saved?.profiles).toHaveLength(1)
	expect(saved?.relations?.filter((r) => r.type === "coincident")).toHaveLength(4)
	expect(saved?.relations?.filter((r) => r.type === "horizontal" || r.type === "vertical")).toHaveLength(4)
})
it("adds radial distance without swapping the selected circles' radii", () => {
	const sketch = blank()
	sketch.entities = [
		{ id: "outer", type: "circle", center: v2(0, 0), radius: 113, segments: 32 },
		{ id: "pitch", type: "circle", center: v2(0, 0), radius: 105, segments: 32, construction: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(next) => {
			saved = next
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select outer")
	const second = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select pitch"))
	second.dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	click(editor.root, "Radial distance")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ radius: expect.closeTo(113, 6) })
	expect(saved?.entities[1]).toMatchObject({ radius: expect.closeTo(105, 6) })
	expect(saved?.relations?.[0]).toMatchObject({ type: "radiusDifference", value: 8 })
})
it("preserves standalone construction geometry and the source sketch when deleting its extrusion", () => {
	const source = blank()
	source.relations = []
	source.entities = [
		{ id: "boundary", type: "circle", center: v2(0, 0), radius: 20, segments: 32 },
		{ id: "guide", type: "circle", center: v2(0, 0), radius: 10, segments: 32, construction: true }
	]
	let saved: PartDocument | undefined
	const panel = new SolidFeaturePanel({ solidSteps: [], features: [source] }, (next) => {
		saved = next
	})
	document.body.append(panel.root)
	click(panel.root, "Extrude sketch")
	click(panel.root, "Apply changes")
	expect(saved?.features.find((f) => f.type === "sketch")?.entities).toHaveLength(2)
	click(panel.root, "Delete feature")
	click(panel.root, "Apply changes")
	expect(saved?.solidSteps).toHaveLength(0)
	expect(saved?.features).toHaveLength(1)
	expect(saved?.features[0]).toMatchObject({ type: "sketch", id: source.id })
})
it("draws a native three-point arc and snaps a closing chord to its endpoints", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "3-point arc")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 470, 350)
	pointer(canvas, 530, 350)
	pointer(canvas, 500, 320)
	expect(canvas.querySelector('[data-entity-id="arc-1"]')?.getAttribute("d")).not.toEndWith("Z")
	click(editor.root, "Line")
	pointer(canvas, 472, 351)
	pointer(canvas, 529, 352)
	click(editor.root, "Select")
	click(editor.root, "Select arc-1")
	click(editor.root, "Add driving dimension")
	input(editor.root, "Dimension (mm)", 20)
	click(editor.root, "Select arc-1")
	click(editor.root, "Dimension arc sweep")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ type: "arc", radius: expect.closeTo(20, 5) })
	expect(saved?.entities[1]?.type).toBe("line")
	expect(saved?.relations?.filter((r) => r.type === "coincident")).toHaveLength(2)
	expect(saved?.relations?.filter((r) => r.type === "horizontal")).toHaveLength(1)
	expect(saved?.profiles).toHaveLength(1)
})
it("trims a circle on the canvas, undoes and redoes the edit, and finishes a native arc", () => {
	const source = blank()
	source.entities = [
		{ id: "circle", type: "circle", center: v2(0, 0), radius: 10, segments: 128 },
		{ id: "chord", type: "line", p0: v2(-10, 0), p1: v2(10, 0) }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Trim")
	const canvas = requireValue(editor.root.querySelector("svg"))
	// Fit maps this 20 mm circle to +/-252 pixels around the default canvas center.
	pointer(requireValue(canvas.querySelector('[data-entity-id="circle"]')), 500, 600)
	expect(editor.root.querySelector('[aria-label="arc circle"]')).not.toBeNull()
	click(editor.root, "Undo")
	expect(editor.root.querySelector('[aria-label="circle circle"]')).not.toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]?.type).toBe("arc")
	expect(saved?.profiles).toHaveLength(1)
	expect(source.entities[0]?.type).toBe("circle")
})
it("creates an editable linked offset through the toolbar and restores it with undo", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(0, 0), radius: 20, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(requireValue(canvas.querySelector('[data-entity-id="c"]')), 560, 350)
	click(editor.root, "Offset")
	input(editor.root, "Dimension (mm)", 8)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(original.entities).toHaveLength(1)
	expect(saved?.entities).toHaveLength(2)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "offset", a: "c", value: 8 }))
	const a = saved?.entities[0]
	const b = saved?.entities[1]
	if (a?.type !== "circle" || b?.type !== "circle") throw Error("Expected native circles")
	expect(b.radius - a.radius).toBeCloseTo(8, 5)
})
it("offsets a multi-selected closed line chain using one editable dimension", () => {
	const original = blank()
	original.entities = [
		{ id: "bottom", type: "line", p0: v2(0, 0), p1: v2(20, 0) },
		{ id: "right", type: "line", p0: v2(20, 0), p1: v2(20, 20) },
		{ id: "top", type: "line", p0: v2(20, 20), p1: v2(0, 20) },
		{ id: "left", type: "line", p0: v2(0, 20), p1: v2(0, 0) }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	for (const e of original.entities) {
		const button = Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === `Select ${e.id}`)
		requireValue(button).dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	}
	click(editor.root, "Offset")
	input(editor.root, "Dimension (mm)", 7)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(8)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "offsetChain", value: 7, targets: expect.arrayContaining(["offset-1", "offset-2", "offset-3", "offset-4"]) }))
	expect(saved?.profiles.length).toBeGreaterThan(0)
	expect(original.entities).toHaveLength(4)
})
it("adds a midpoint from the toolbar, supports undo, and removes it with the target curve", () => {
	const original = blank()
	original.entities = [
		{ id: "point", type: "circle", center: v2(4, 6), radius: 1, segments: 32, construction: true },
		{ id: "edge", type: "line", p0: v2(0, 0), p1: v2(20, 0) }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select point")
	const target = Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select edge")
	requireValue(target).dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	click(editor.root, "Midpoint")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "midpoint", a: { entityId: "point", point: "center" }, b: "edge" }))
	const point = saved?.entities[0]
	const edge = saved?.entities[1]
	if (point?.type !== "circle" || edge?.type !== "line") throw Error("Missing geometry")
	expect(point.center.x).toBeCloseTo((edge.p0.x + edge.p1.x) / 2, 5)
	expect(point.center.y).toBeCloseTo((edge.p0.y + edge.p1.y) / 2, 5)
	let deleted: Sketch | undefined
	const second = new SketchWorkspace(
		requireValue(saved),
		(s) => {
			deleted = s
		},
		() => undefined
	)
	document.body.append(second.root)
	click(second.root, "Select edge")
	click(second.root, "Delete selected")
	click(second.root, "Finish sketch")
	expect(deleted?.relations).toEqual([])
	expect(original.entities).toHaveLength(2)
})
it("draws a slot with a live preview, dimensions both sizes, and persists it through PCad", async () => {
	const original = blank()
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Slot")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 600, 350)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: 550, clientY: 320 }) as unknown as Event)
	expect(canvas.querySelector('[data-preview="slot"]')).not.toBeNull()
	pointer(canvas, 550, 320)
	click(editor.root, "Select")
	click(editor.root, "Select slot-1")
	click(editor.root, "Add driving dimension")
	input(editor.root, "Dimension (mm)", 12)
	click(editor.root, "Select slot-1")
	click(editor.root, "Dimension slot center distance")
	input(editor.root, "Dimension (mm)", 40)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(original.entities).toHaveLength(0)
	const slot = saved?.entities[0]
	if (slot?.type !== "capsule") throw Error("Missing native slot")
	expect(slot.width).toBeCloseTo(12, 5)
	expect(Math.hypot(slot.to.x - slot.from.x, slot.to.y - slot.from.y)).toBeCloseTo(40, 5)
	expect(saved?.profiles).toHaveLength(1)
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [requireValue(saved)] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing saved sketch")
	expect(result.entities[0]?.type).toBe("capsule")
	expect(result.relations).toHaveLength(3)
	expect(result.profiles).toHaveLength(1)
})
it("cancels an unfinished slot and keeps invalid width clicks editable", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Slot")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 500, 350)
	expect(editor.root.textContent).toContain("different second point")
	pointer(canvas, 600, 350)
	pointer(canvas, 550, 350)
	expect(editor.root.textContent).toContain("give the slot a width")
	window.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape" }) as unknown as Event)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(0)
})
it("adds symmetry from three selections and clears the relation when its axis is deleted", () => {
	const original = blank()
	original.entities = [
		{ id: "a", type: "circle", center: v2(-10, 5), radius: 1, segments: 32, construction: true },
		{ id: "b", type: "circle", center: v2(12, 4), radius: 1, segments: 32, construction: true },
		{ id: "axis", type: "line", p0: v2(0, -20), p1: v2(0, 20), construction: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select a")
	for (const id of ["b", "axis"]) {
		const target = Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === `Select ${id}`)
		requireValue(target).dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	}
	click(editor.root, "Symmetric")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "symmetric", symmetryLine: "axis" }))
	let deleted: Sketch | undefined
	const second = new SketchWorkspace(
		requireValue(saved),
		(s) => {
			deleted = s
		},
		() => undefined
	)
	document.body.append(second.root)
	click(second.root, "Select axis")
	click(second.root, "Delete selected")
	click(second.root, "Finish sketch")
	expect(deleted?.relations).toEqual([])
	expect(original.relations).toBeUndefined()
})
it("creates linked mirrored geometry from the toolbar and updates it from the source dimension", () => {
	const original = blank()
	original.entities = [
		{ id: "c", type: "circle", center: v2(20, 0), radius: 3, segments: 64 },
		{ id: "axis", type: "line", p0: v2(0, -20), p1: v2(0, 20), construction: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	const axis = Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select axis")
	requireValue(axis).dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	click(editor.root, "Mirror")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Select c")
	click(editor.root, "Add driving dimension")
	input(editor.root, "Dimension (mm)", 12)
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "mirror", a: "c", b: "mirror-1", symmetryLine: "axis" }))
	const copy = saved?.entities.find((e) => e.id === "mirror-1")
	if (copy?.type !== "circle") throw Error("Missing mirrored circle")
	expect(copy.radius).toBeCloseTo(6, 5)
	expect(saved?.profiles).toHaveLength(2)
	expect(original.entities).toHaveLength(2)
	let deleted: Sketch | undefined
	const second = new SketchWorkspace(
		requireValue(saved),
		(s) => {
			deleted = s
		},
		() => undefined
	)
	document.body.append(second.root)
	click(second.root, "Select axis")
	click(second.root, "Delete selected")
	click(second.root, "Finish sketch")
	expect(deleted?.relations?.some((r) => r.type === "mirror")).toBe(false)
	expect(deleted?.entities).toHaveLength(2)
})
it("adds internal tangency with the larger circle as container regardless of selection order", () => {
	const original = blank()
	original.entities = [
		{ id: "small", type: "circle", center: v2(8, 0), radius: 5, segments: 64, construction: true },
		{ id: "large", type: "circle", center: v2(0, 0), radius: 20, segments: 64, construction: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select small")
	const large = Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select large")
	requireValue(large).dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	click(editor.root, "Internal tangent")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "internalTangent", a: "large", b: "small" }))
	const small = saved?.entities[0]
	const big = saved?.entities[1]
	if (small?.type !== "circle" || big?.type !== "circle") throw Error("Missing circles")
	expect(Math.hypot(small.center.x - big.center.x, small.center.y - big.center.y) + small.radius).toBeCloseTo(big.radius, 5)
})
it("draws a clockwise center arc with a linked center and editable radius", () => {
	const original = blank()
	original.entities = [{ id: "center", type: "circle", center: v2(0, 0), radius: 1, segments: 32, construction: true }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Center arc")
	const label = Array.from(editor.root.querySelectorAll("label")).find((l) => l.textContent === "Clockwise arc")
	requireValue(label?.querySelector("input")).click()
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 600, 350)
	pointer(canvas, 500, 250)
	click(editor.root, "Select")
	click(editor.root, "Select arc-1")
	click(editor.root, "Add driving dimension")
	input(editor.root, "Dimension (mm)", 10)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const arc = saved?.entities.find((e) => e.id === "arc-1")
	if (arc?.type !== "arc") throw Error("Missing native arc")
	expect(arc.radius).toBeCloseTo(10, 5)
	expect(arc.sweep).toBeCloseTo((-3 * Math.PI) / 2, 5)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "coincident", a: { entityId: "arc-1", point: "center" }, b: { entityId: "center", point: "center" } }))
	expect(saved?.profiles).toHaveLength(0)
	expect(original.entities).toHaveLength(1)
})
it("draws centered and three-point rectangles with editable rotation and durable native geometry", async () => {
	for (const tool of ["Center rectangle", "3-point rectangle"]) {
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			blank(),
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		click(editor.root, tool)
		const canvas = requireValue(editor.root.querySelector("svg"))
		pointer(canvas, 500, 350)
		if (tool === "3-point rectangle") pointer(canvas, 600, 300)
		canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: 560, clientY: 250 }) as unknown as Event)
		expect(canvas.querySelector('[data-preview="rectangle"]')).not.toBeNull()
		pointer(canvas, 560, 250)
		click(editor.root, "Select")
		click(editor.root, "Select rectangle-1")
		click(editor.root, "Dimension width")
		input(editor.root, "Dimension (mm)", 40)
		click(editor.root, "Select rectangle-1")
		click(editor.root, "Dimension height")
		input(editor.root, "Dimension (mm)", 20)
		click(editor.root, "Select rectangle-1")
		click(editor.root, "Dimension rotation")
		input(editor.root, "Angle (degrees)", 30)
		click(editor.root, "Undo")
		click(editor.root, "Redo")
		click(editor.root, "Finish sketch")
		const rectangle = saved?.entities[0]
		if (rectangle?.type !== "rectangle") throw Error("Missing native rectangle")
		expect(rectangle.width).toBeCloseTo(40, 5)
		expect(rectangle.height).toBeCloseTo(20, 5)
		expect(rectangle.rotation).toBeCloseTo(30, 5)
		expect(saved?.profiles).toHaveLength(1)
		const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
		const runtime = createPartRuntimeState({ features: [requireValue(saved)] })
		const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
		const result = materializePartFeatures(restored.cad, restored.tree)[0]
		if (result?.type !== "sketch") throw Error("Missing sketch")
		expect(result.relations).toContainEqual(expect.objectContaining({ type: "rotation", value: 30 }))
		expect(result.profiles).toHaveLength(1)
	}
})
it("exposes native rectangle corner handles for fixed-point constraints", () => {
	const original = blank()
	original.entities = [{ id: "r", type: "rectangle", center: v2(0, 0), width: 20, height: 10, rotation: 30 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select r")
	const handle = requireValue(editor.root.querySelector('[aria-label="r p2"]'))
	pointer(handle, 400, 400)
	click(editor.root, "Fix")
	click(editor.root, "Select r")
	click(editor.root, "Dimension width")
	input(editor.root, "Dimension (mm)", 40)
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "fixed", anchor: { entityId: "r", point: "p2" } }))
	const box = saved?.entities[0]
	if (box?.type !== "rectangle") throw Error("Missing rectangle")
	expect(box.width).toBeCloseTo(40, 5)
})
it("preserves a snapped first corner when drawing a three-point rectangle", () => {
	const original = blank()
	original.entities = [{ id: "point", type: "circle", center: v2(0, 0), radius: 1, segments: 32, construction: true }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "3-point rectangle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 600, 350)
	pointer(canvas, 600, 250)
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "coincident", a: { entityId: "rectangle-1", point: "p2" }, b: { entityId: "point", point: "center" } }))
})
it("owns only one sketch session and cannot commit discarded or disposed workspaces", () => {
	let saved: PartDocument | undefined
	const panel = new SolidFeaturePanel({ features: [], solidSteps: [] }, (s) => {
		saved = s
	})
	document.body.append(panel.root)
	click(panel.root, "New sketch")
	const first = requireValue(document.querySelector<HTMLElement>("[data-sketch-workspace]"))
	expect(document.activeElement).toBe(first.querySelector("svg"))
	click(panel.root, "New sketch")
	const second = requireValue(document.querySelector<HTMLElement>("[data-sketch-workspace]"))
	expect(first.isConnected).toBe(false)
	expect(document.querySelectorAll("[data-sketch-workspace]")).toHaveLength(1)
	click(first, "Finish sketch")
	click(second, "Circle")
	const canvas = requireValue(second.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 350)
	click(panel.root, "Apply changes")
	expect(saved).toBeUndefined()
	expect(panel.root.textContent).toContain("Finish or cancel")
	click(panel.root, "Discard changes")
	expect(second.isConnected).toBe(false)
	click(second, "Finish sketch")
	click(panel.root, "Apply changes")
	expect(saved?.features).toHaveLength(0)
	click(panel.root, "New sketch")
	const third = requireValue(document.querySelector<HTMLElement>("[data-sketch-workspace]"))
	panel.dispose()
	panel.dispose()
	expect(third.isConnected).toBe(false)
	click(third, "Finish sketch")
	expect(saved?.features).toHaveLength(0)
})
it("fillets selected lines with a driving radius and restores the sharp corner with Undo", () => {
	const original = blank()
	original.entities = [
		{ id: "a", type: "line", p0: v2(0, 0), p1: v2(20, 0) },
		{ id: "b", type: "line", p0: v2(20, 0), p1: v2(20, 20) }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select a")
	const second = Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select b")
	requireValue(second).dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	input(editor.root, "Fillet radius (mm)", 3)
	click(editor.root, "Sketch fillet")
	click(editor.root, "Undo")
	expect(editor.root.textContent).toContain("Entities (2)")
	click(editor.root, "Redo")
	click(editor.root, "radius 3 · fillet-radius-1")
	input(editor.root, "Dimension (mm)", 4)
	click(editor.root, "Finish sketch")
	const arc = saved?.entities.find((e) => e.type === "arc")
	expect(arc?.radius).toBeCloseTo(4, 5)
	expect(saved?.relations?.filter((r) => r.type === "endpointTangent")).toHaveLength(2)
	expect(original.entities).toHaveLength(2)
})
it("creates and edits a linear pattern with count, step, undo and redo", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(0, 0), radius: 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Linear pattern")
	input(editor.root, "Pattern columns", 4)
	input(editor.root, "Pattern step X (mm)", 25)
	input(editor.root, "Pattern step Y (mm)", 5)
	input(editor.root, "Pattern rows", 2)
	input(editor.root, "Row step X (mm)", 3)
	input(editor.root, "Row step Y (mm)", 30)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(8)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "linearPattern", step: { x: 25, y: 5 }, columns: 4, rowStep: { x: 3, y: 30 } }))
	expect(saved?.profiles).toHaveLength(8)
	expect(original.entities).toHaveLength(1)
})
it("creates a circular pattern and edits its count, center and open angle with local history", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(30, 0), radius: 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Circular pattern")
	input(editor.root, "Pattern count", 3)
	input(editor.root, "Pattern center X (mm)", 5)
	input(editor.root, "Pattern center Y (mm)", 1)
	input(editor.root, "Pattern angle (degrees)", -180)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(3)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "circularPattern", angle: -180, center: { x: 5, y: 1 } }))
	expect(saved?.profiles).toHaveLength(3)
	expect(original.entities).toHaveLength(1)
})
it("drags the circular pattern center as one undoable edit", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(30, 0), radius: 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Circular pattern")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const handle = requireValue(canvas.querySelector('[data-pattern-center="circular-pattern-1"]'))
	const x = Number(handle.getAttribute("cx"))
	const y = Number(handle.getAttribute("cy"))
	pointer(handle, x, y)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + 30, clientY: y + 15 }) as unknown as Event)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + 60, clientY: y + 30 }) as unknown as Event)
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	const changedX = Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern center X (mm)"]')?.value)
	expect(changedX).toBeGreaterThan(0)
	click(editor.root, "Undo")
	click(editor.root, "circularPattern · circular-pattern-1")
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern center X (mm)"]')?.value)).toBe(0)
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const pattern = saved?.relations?.find((r) => r.type === "circularPattern")
	if (pattern?.type !== "circularPattern") throw Error("Missing circular pattern")
	expect(pattern.center.x).toBeCloseTo(changedX, 5)
	expect(pattern.center.y).toBeLessThan(0)
	expect(saved?.entities).toHaveLength(4)
})
it("attaches, drives and detaches a circular center through its reference selector", () => {
	const original = blank()
	original.entities = [
		{ id: "c", type: "circle", center: v2(30, 0), radius: 2, segments: 64 },
		{ id: "axis", type: "line", p0: v2(0, 0), p1: v2(10, 0), construction: true }
	]
	original.relations = [
		{ id: "origin", type: "fixed", anchor: { entityId: "axis", point: "p0" }, position: v2(0, 0) },
		{ id: "horizontal", type: "horizontal", entityId: "axis" },
		{ id: "axis-length", type: "length", entityId: "axis", value: 10 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Circular pattern")
	const select = requireValue(editor.root.querySelector<HTMLSelectElement>('select[aria-label="Pattern center reference"]'))
	const option = requireValue(Array.from(select.options).find((o) => o.textContent === "axis · p1"))
	select.value = option.value
	select.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(editor.root.querySelector('input[aria-label="Pattern center X (mm)"]')).toBeNull()
	click(editor.root, "length 10 · axis-length")
	input(editor.root, "Dimension (mm)", 15)
	click(editor.root, "circularPattern · circular-pattern-1")
	const detach = requireValue(editor.root.querySelector<HTMLSelectElement>('select[aria-label="Pattern center reference"]'))
	detach.value = ""
	detach.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern center X (mm)"]')?.value)).toBeCloseTo(15, 5)
	click(editor.root, "Undo")
	click(editor.root, "Finish sketch")
	const pattern = saved?.relations?.find((r) => r.type === "circularPattern")
	if (pattern?.type !== "circularPattern") throw Error("Missing pattern")
	expect(pattern.centerAnchor).toEqual({ entityId: "axis", point: "p1" })
})
it("removes a circular pattern relation when its center reference is deleted", () => {
	const original = blank()
	original.entities = [
		{ id: "c", type: "circle", center: v2(30, 0), radius: 2, segments: 64 },
		{ id: "axis", type: "line", p0: v2(0, 0), p1: v2(10, 0), construction: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Circular pattern")
	const select = requireValue(editor.root.querySelector<HTMLSelectElement>('select[aria-label="Pattern center reference"]'))
	select.value = requireValue(Array.from(select.options).find((o) => o.textContent === "axis · p1")).value
	select.dispatchEvent(new dom.Event("change") as unknown as Event)
	click(editor.root, "Select axis")
	click(editor.root, "Delete selected")
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.some((r) => r.type === "circularPattern")).toBe(false)
	expect(saved?.entities).toHaveLength(4)
	expect(saved?.profiles).toHaveLength(4)
})
it("opens a circular pattern with its angle handle and undoes the whole drag", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(30, 0), radius: 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Circular pattern")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const center = requireValue(canvas.querySelector("[data-pattern-center]"))
	const handle = requireValue(canvas.querySelector("[data-pattern-angle]"))
	const x = Number(center.getAttribute("cx"))
	const y = Number(center.getAttribute("cy"))
	const radius = Number(handle.getAttribute("cx")) - x
	expect(canvas.querySelector("[data-pattern-angle-guide]")).not.toBeNull()
	pointer(handle, x + radius, y)
	for (const degrees of [45, 90])
		canvas.dispatchEvent(
			new dom.PointerEvent("pointermove", {
				bubbles: true,
				clientX: x + radius * Math.cos((degrees * Math.PI) / 180),
				clientY: y + radius * Math.sin((degrees * Math.PI) / 180)
			}) as unknown as Event
		)
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern angle (degrees)"]')?.value)).toBeCloseTo(270, 5)
	click(editor.root, "Undo")
	click(editor.root, "circularPattern · circular-pattern-1")
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern angle (degrees)"]')?.value)).toBe(360)
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "circularPattern", angle: expect.closeTo(270, 5) }))
	expect(saved?.profiles).toHaveLength(4)
})
it("snaps clockwise angle drags and cancels them without creating history", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(30, 0), radius: 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Circular pattern")
	input(editor.root, "Pattern angle (degrees)", -360)
	const canvas = requireValue(editor.root.querySelector("svg"))
	const center = requireValue(canvas.querySelector("[data-pattern-center]"))
	const handle = requireValue(canvas.querySelector("[data-pattern-angle]"))
	const x = Number(center.getAttribute("cx"))
	const y = Number(center.getAttribute("cy"))
	const radius = Number(handle.getAttribute("cx")) - x
	pointer(handle, x + radius, y)
	canvas.dispatchEvent(
		new dom.PointerEvent("pointermove", {
			bubbles: true,
			clientX: x + radius * Math.cos((38 * Math.PI) / 180),
			clientY: y - radius * Math.sin((38 * Math.PI) / 180),
			shiftKey: true
		}) as unknown as Event
	)
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern angle (degrees)"]')?.value)).toBe(-315)
	for (const cancel of ["pointercancel", "Escape"]) {
		const nextHandle = requireValue(canvas.querySelector("[data-pattern-angle]"))
		pointer(nextHandle, Number(nextHandle.getAttribute("cx")), Number(nextHandle.getAttribute("cy")))
		canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x, clientY: y - radius }) as unknown as Event)
		if (cancel === "Escape") canvas.dispatchEvent(new dom.KeyboardEvent("keydown", { bubbles: true, key: "Escape" }) as unknown as Event)
		else canvas.dispatchEvent(new dom.PointerEvent("pointercancel", { bubbles: true }) as unknown as Event)
		expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern angle (degrees)"]')?.value)).toBe(-315)
	}
	click(editor.root, "Undo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "circularPattern", angle: -360 }))
})
it("drags independent column and row spacing in a grid with local history", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(0, 0), radius: 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Linear pattern")
	input(editor.root, "Pattern rows", 2)
	const canvas = requireValue(editor.root.querySelector("svg"))
	const origin = requireValue(canvas.querySelector("[data-pattern-origin]"))
	const x = Number(origin.getAttribute("cx"))
	const y = Number(origin.getAttribute("cy"))
	const column = requireValue(canvas.querySelector('[data-pattern-step="column"]'))
	const scale = (Number(column.getAttribute("cx")) - x) / 20
	const drag = (axis: string, dx: number, dy: number) => {
		const handle = requireValue(canvas.querySelector(`[data-pattern-step="${axis}"]`))
		pointer(handle, Number(handle.getAttribute("cx")), Number(handle.getAttribute("cy")))
		canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + dx * scale, clientY: y - dy * scale }) as unknown as Event)
		canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	}
	drag("column", 30, 5)
	drag("row", -10, 25)
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Row step X (mm)"]')?.value)).toBe(-10)
	click(editor.root, "Undo")
	click(editor.root, "linearPattern · linear-pattern-1")
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Row step X (mm)"]')?.value)).toBe(0)
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern step X (mm)"]')?.value)).toBe(30)
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "linearPattern", step: { x: 30, y: 5 }, rowStep: { x: -10, y: 25 } }))
	expect(saved?.entities).toHaveLength(6)
	expect(saved?.profiles).toHaveLength(6)
})
it("locks spacing direction with Shift and cancels a spacing drag", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(0, 0), radius: 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Linear pattern")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const origin = requireValue(canvas.querySelector("[data-pattern-origin]"))
	const x = Number(origin.getAttribute("cx"))
	const y = Number(origin.getAttribute("cy"))
	const handle = requireValue(canvas.querySelector('[data-pattern-step="column"]'))
	const scale = (Number(handle.getAttribute("cx")) - x) / 20
	expect(canvas.querySelector('[data-pattern-step="row"]')).toBeNull()
	pointer(handle, x + 20 * scale, y)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + 30 * scale, clientY: y - 10 * scale, shiftKey: true }) as unknown as Event)
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern step Y (mm)"]')?.value)).toBe(0)
	const next = requireValue(canvas.querySelector('[data-pattern-step="column"]'))
	pointer(next, x + 30 * scale, y)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + 50 * scale, clientY: y }) as unknown as Event)
	canvas.dispatchEvent(new dom.PointerEvent("pointercancel", { bubbles: true }) as unknown as Event)
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Pattern step X (mm)"]')?.value)).toBe(30)
	click(editor.root, "Undo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "linearPattern", step: { x: 20, y: 0 } }))
})
it("rejects a zero-spacing drag without recording an edit", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(0, 0), radius: 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Linear pattern")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const origin = requireValue(canvas.querySelector("[data-pattern-origin]"))
	const handle = requireValue(canvas.querySelector('[data-pattern-step="column"]'))
	pointer(handle, Number(handle.getAttribute("cx")), Number(handle.getAttribute("cy")))
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: Number(origin.getAttribute("cx")), clientY: Number(origin.getAttribute("cy")) }) as unknown as Event)
	expect(editor.root.textContent).toContain("nonzero")
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	click(editor.root, "Undo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(1)
})
it("rejects spacing that conflicts with fixed source and copy positions", async () => {
	const { createLinearSketchPattern } = await import("../src/sketch-pattern")
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(0, 0), radius: 2, segments: 64 }]
	original.relations = [{ id: "fixed-source", type: "fixed", anchor: { entityId: "c", point: "center" }, position: v2(0, 0) }]
	const created = createLinearSketchPattern(original, ["c"], 3, v2(20, 0))
	created.sketch.relations?.push({ id: "fixed-copy", type: "fixed", anchor: { entityId: "pattern-copy-1", point: "center" }, position: v2(20, 0) })
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		created.sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "linearPattern · linear-pattern-1")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const origin = requireValue(canvas.querySelector("[data-pattern-origin]"))
	const handle = requireValue(canvas.querySelector('[data-pattern-step="column"]'))
	const x = Number(origin.getAttribute("cx"))
	const y = Number(origin.getAttribute("cy"))
	const endpoint = Number(handle.getAttribute("cx"))
	pointer(handle, endpoint, y)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + (endpoint - x) * 1.5, clientY: y }) as unknown as Event)
	expect(editor.root.textContent).toContain("spacing conflicts")
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "linearPattern", step: { x: 20, y: 0 } }))
})
it("draws a tangent arc from a line endpoint with preview, durable tangency and history", () => {
	const original = blank()
	original.entities = [{ id: "line", type: "line", p0: v2(0, 0), p1: v2(10, 0) }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select line")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const p0 = requireValue(canvas.querySelector('[aria-label="line p0"]'))
	const p1 = requireValue(canvas.querySelector('[aria-label="line p1"]'))
	const x = Number(p1.getAttribute("cx"))
	const y = Number(p1.getAttribute("cy"))
	const scale = (x - Number(p0.getAttribute("cx"))) / 10
	click(editor.root, "Tangent arc")
	pointer(canvas, x, y)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + 5 * scale, clientY: y - 5 * scale }) as unknown as Event)
	expect(canvas.querySelector('[data-preview="tangent-arc"]')).not.toBeNull()
	pointer(canvas, x + 5 * scale, y - 5 * scale)
	click(editor.root, "Undo")
	expect(canvas.querySelector('[data-entity-id="arc-1"]')).toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(2)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "smoothJoin", a: { entityId: "line", point: "p1" }, b: { entityId: "arc-1", point: "p0" } }))
	expect(saved?.entities[1]).toMatchObject({ type: "arc", radius: expect.closeTo(5, 5), sweep: expect.closeTo(Math.PI / 2, 5) })
	expect(original.entities).toHaveLength(1)
})
it("continues an existing arc into a closed profile using the tangent arc tool", () => {
	const original = blank()
	original.entities = [{ id: "upper", type: "arc", center: v2(0, 0), radius: 10, startAngle: 0, sweep: Math.PI, segments: 128 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select upper")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const start = requireValue(canvas.querySelector('[aria-label="upper p1"]'))
	const end = requireValue(canvas.querySelector('[aria-label="upper p0"]'))
	const x0 = Number(start.getAttribute("cx"))
	const y0 = Number(start.getAttribute("cy"))
	const x1 = Number(end.getAttribute("cx"))
	const y1 = Number(end.getAttribute("cy"))
	click(editor.root, "Tangent arc")
	pointer(canvas, x0, y0)
	pointer(canvas, x1, y1)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(2)
	expect(saved?.profiles).toHaveLength(1)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "smoothJoin", a: { entityId: "upper", point: "p1" }, b: { entityId: "arc-1", point: "p0" } }))
})
it("previews a three-point circle, rejects collinear points, and supports retry and history", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "3-point circle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 350)
	pointer(canvas, 530, 350)
	expect(editor.root.textContent).toContain("collinear")
	expect(canvas.querySelector('[data-entity-id="circle-1"]')).toBeNull()
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: 530, clientY: 320 }) as unknown as Event)
	expect(canvas.querySelector('[data-preview="three-point-circle"]')).not.toBeNull()
	pointer(canvas, 530, 320)
	click(editor.root, "Undo")
	expect(canvas.querySelector('[data-entity-id="circle-1"]')).toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ type: "circle", center: { x: 10, y: 0 }, radius: 10 })
	expect(saved?.profiles).toHaveLength(1)
})
it("constrains a three-point circle to the existing anchors used to place it", () => {
	const original = blank()
	original.entities = [
		{ id: "a", type: "circle", center: v2(0, 0), radius: 0.2, segments: 32, construction: true },
		{ id: "b", type: "circle", center: v2(10, 0), radius: 0.2, segments: 32, construction: true },
		{ id: "c", type: "circle", center: v2(0, 10), radius: 0.2, segments: 32, construction: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const canvas = requireValue(editor.root.querySelector("svg"))
	const coordinates = ["a", "b", "c"].map((id) => {
		click(editor.root, `Select ${id}`)
		const handle = requireValue(canvas.querySelector(`[aria-label="${id} center"]`))
		return { x: Number(handle.getAttribute("cx")), y: Number(handle.getAttribute("cy")) }
	})
	click(editor.root, "3-point circle")
	for (const point of coordinates) pointer(canvas, point.x, point.y)
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.filter((r) => r.type === "pointOnCurve")).toHaveLength(3)
	expect(saved?.entities).toHaveLength(4)
	expect(saved?.profiles).toHaveLength(1)
	expect(saved?.entities[3]).toMatchObject({ type: "circle", center: { x: expect.closeTo(5, 5), y: expect.closeTo(5, 5) }, radius: expect.closeTo(Math.sqrt(50), 5) })
})
it("places and fixes a visible native point with no solid profile", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Point")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 530, 320)
	const marker = requireValue(canvas.querySelector('[data-entity-id="point-1"]'))
	expect(marker.getAttribute("d")).toContain("h 8")
	click(editor.root, "Fix")
	expect(editor.root.textContent).toContain("Fully constrained")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toEqual([{ id: "point-1", type: "point", center: { x: 10, y: 10 }, construction: false }])
	expect(saved?.profiles).toHaveLength(0)
})
it("snaps drawn lines to a native point and keeps the coincidence", () => {
	const original = blank()
	original.entities = [{ id: "p", type: "point", center: v2(0, 0) }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select p")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const handle = requireValue(canvas.querySelector('[aria-label="p center"]'))
	const x = Number(handle.getAttribute("cx"))
	const y = Number(handle.getAttribute("cy"))
	click(editor.root, "Line")
	pointer(canvas, x + 2, y)
	pointer(canvas, x + 100, y)
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "coincident", b: { entityId: "p", point: "center" } }))
	expect(saved?.entities).toHaveLength(2)
	expect(saved?.profiles).toHaveLength(0)
})
it("draws a polygon with vertex handles, changes its sides and driving radius, and undoes edits", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Polygon")
	input(editor.root, "Polygon sides", 6)
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: 530, clientY: 350 }) as unknown as Event)
	expect(canvas.querySelector('[data-preview="polygon"]')).not.toBeNull()
	pointer(canvas, 530, 350)
	expect(canvas.querySelector('[aria-label="polygon-1 vertex5"]')).not.toBeNull()
	input(editor.root, "Polygon side count", 4)
	expect(canvas.querySelector('[aria-label="polygon-1 vertex5"]')).toBeNull()
	click(editor.root, "Add driving dimension")
	input(editor.root, "Dimension (mm)", 15)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ type: "polygon", sides: 4, radius: expect.closeTo(15, 5) })
	expect(saved?.profiles).toHaveLength(1)
})
it("draws a native ellipse with preview, axis dimensions, rotation and history", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Ellipse")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 350)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: 500, clientY: 320 }) as unknown as Event)
	expect(canvas.querySelector('[data-preview="ellipse"]')).not.toBeNull()
	pointer(canvas, 500, 320)
	expect(canvas.querySelector('[aria-label="ellipse-1 p3"]')).not.toBeNull()
	click(editor.root, "Dimension width")
	input(editor.root, "Dimension (mm)", 50)
	click(editor.root, "Select ellipse-1")
	click(editor.root, "Dimension height")
	input(editor.root, "Dimension (mm)", 30)
	click(editor.root, "Select ellipse-1")
	click(editor.root, "Dimension rotation")
	input(editor.root, "Angle (degrees)", 30)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ type: "ellipse", width: expect.closeTo(50, 5), height: expect.closeTo(30, 5), rotation: expect.closeTo(30, 5) })
	expect(saved?.profiles).toHaveLength(1)
})
it("adds point-on-ellipse and ellipse-line tangency through the constraint toolbar", () => {
	const original = blank()
	original.entities = [
		{ id: "e", type: "ellipse", center: v2(0, 0), width: 20, height: 10, rotation: 30, segments: 128 },
		{ id: "p", type: "point", center: v2(0, 0) },
		{ id: "line", type: "line", p0: v2(-30, 2), p1: v2(30, 2) }
	]
	original.relations = [
		{ id: "center", type: "fixed", anchor: { entityId: "e", point: "center" }, position: v2(0, 0) },
		{ id: "width", type: "width", entityId: "e", value: 20 },
		{ id: "height", type: "height", entityId: "e", value: 10 },
		{ id: "rotation", type: "rotation", entityId: "e", value: 30 },
		{ id: "horizontal", type: "horizontal", entityId: "line" }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const shiftSelect = (id: string) =>
		requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === `Select ${id}`)).dispatchEvent(
			new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
		)
	click(editor.root, "Select p")
	shiftSelect("e")
	click(editor.root, "Point on curve")
	click(editor.root, "Select line")
	shiftSelect("e")
	click(editor.root, "Tangent")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "pointOnCurve", b: "e" }))
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "tangent", a: "line", b: "e" }))
})
it("box-selects contained or crossing geometry with Shift-add and cancellation", () => {
	const original = blank()
	original.entities = [
		{ id: "inside", type: "point", center: v2(0, 0) },
		{ id: "crossing", type: "line", p0: v2(-10, 1), p1: v2(10, 1) },
		{ id: "outside", type: "point", center: v2(20, 0) }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select inside")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const origin = requireValue(canvas.querySelector('[aria-label="inside center"]'))
	const x = Number(origin.getAttribute("cx"))
	const y = Number(origin.getAttribute("cy"))
	click(editor.root, "Select outside")
	const other = requireValue(canvas.querySelector('[aria-label="outside center"]'))
	const scale = (Number(other.getAttribute("cx")) - x) / 20
	const selected = (id: string) =>
		Array.from(editor.root.querySelectorAll("button"))
			.find((b) => b.textContent === `Select ${id}`)
			?.getAttribute("aria-pressed") === "true"
	const drag = (crossing: boolean, shift = false, cancel = false) => {
		canvas.dispatchEvent(
			new dom.PointerEvent("pointerdown", { bubbles: true, button: 0, clientX: x + (crossing ? 2 : -2) * scale, clientY: y + 2 * scale, shiftKey: shift }) as unknown as Event
		)
		canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + (crossing ? -2 : 2) * scale, clientY: y - 2 * scale }) as unknown as Event)
		expect(canvas.querySelector("[data-selection-box]")?.getAttribute("data-selection-box")).toBe(crossing ? "crossing" : "contained")
		canvas.dispatchEvent(new dom.PointerEvent(cancel ? "pointercancel" : "pointerup", { bubbles: true }) as unknown as Event)
	}
	drag(false)
	expect(selected("inside")).toBe(true)
	expect(selected("crossing")).toBe(false)
	expect(selected("outside")).toBe(false)
	drag(true)
	expect(selected("inside")).toBe(true)
	expect(selected("crossing")).toBe(true)
	click(editor.root, "Select outside")
	drag(false, true)
	expect(selected("inside")).toBe(true)
	expect(selected("outside")).toBe(true)
	drag(true, false, true)
	expect(selected("inside")).toBe(true)
	expect(selected("outside")).toBe(true)
	expect(selected("crossing")).toBe(false)
	click(editor.root, "Delete selected")
	click(editor.root, "Undo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(3)
})
it("creates a read-only reference dimension, updates it live, and converts the current value to driving", () => {
	const original = blank()
	original.entities = [{ id: "c", type: "circle", center: v2(0, 0), radius: 10, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	click(editor.root, "Add reference dimension")
	expect(editor.root.querySelector('input[aria-label="Dimension (mm)"]')).toBeNull()
	expect(editor.root.querySelector('output[aria-label="Reference dimension value"]')?.textContent).toBe("(20)")
	expect(editor.root.querySelector('svg text[aria-label*="(Ø20)"]')).not.toBeNull()
	click(editor.root, "Select c")
	click(editor.root, "Add driving dimension")
	input(editor.root, "Dimension (mm)", 30)
	click(editor.root, "diameter 30 · dimension-1")
	expect(editor.root.querySelector('output[aria-label="Reference dimension value"]')?.textContent).toBe("(30)")
	const toggle = requireValue(
		Array.from(editor.root.querySelectorAll("label"))
			.find((label) => label.textContent?.includes("Reference dimension"))
			?.querySelector("input")
	)
	toggle.checked = false
	toggle.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(Number(editor.root.querySelector<HTMLInputElement>('input[aria-label="Dimension (mm)"]')?.value)).toBeCloseTo(30, 5)
	click(editor.root, "Undo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.find((r) => r.id === "dimension-1")).toMatchObject({ reference: true, value: expect.closeTo(30, 5) })
	expect(saved?.entities[0]).toMatchObject({ radius: expect.closeTo(15, 5) })
})

it("edits a canvas dimension with Enter, cancels drafts and undoes the committed geometry", () => {
	const sketch = blank()
	sketch.entities = [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 32 }]
	sketch.relations = [{ id: "diam", type: "diameter", entityId: "c", value: 20 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const open = () => {
		const label = requireValue(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]'))
		label.dispatchEvent(new dom.MouseEvent("dblclick", { bubbles: true }) as unknown as Event)
		return requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Edit dimension value"]'))
	}
	let field = open()
	field.value = "40"
	const cancel = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Cancel dimension"))
	const cancelKey = new dom.KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true })
	cancel.dispatchEvent(cancelKey as unknown as Event)
	expect(cancelKey.defaultPrevented).toBe(false)
	expect(field.isConnected).toBe(true)
	field.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as unknown as Event)
	expect(editor.root.querySelector("[data-dimension-editor]")).toBeNull()
	expect(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')?.textContent).toBe("Ø20")
	field = open()
	field.value = "40"
	field.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Enter", bubbles: true }) as unknown as Event)
	expect(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')?.textContent).toBe("Ø40")
	click(editor.root, "Undo")
	expect(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')?.textContent).toBe("Ø20")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ radius: expect.closeTo(20, 5) })
	expect(sketch.entities[0]).toMatchObject({ radius: 10 })
})
it("keeps invalid and conflicting canvas dimension drafts editable and leaves reference labels read-only", () => {
	const sketch = blank()
	sketch.entities = [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 32 }]
	sketch.relations = [
		{ id: "diam", type: "diameter", entityId: "c", value: 20 },
		{ id: "radius", type: "radius", entityId: "c", value: 10 },
		{ id: "ref", type: "diameter", entityId: "c", value: 20, reference: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const label = requireValue(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]'))
	label.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Enter", bubbles: true }) as unknown as Event)
	const field = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Edit dimension value"]'))
	for (const value of ["", "abc", "-1", "30"]) {
		field.value = value
		click(editor.root, "Apply dimension")
		expect(field.isConnected).toBe(true)
		expect(field.getAttribute("aria-invalid")).toBe("true")
		expect(editor.root.querySelector('[data-dimension-editor] [role="alert"]')?.textContent).not.toBe("")
	}
	expect(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')?.textContent).toBe("Ø20")
	field.value = "20"
	click(editor.root, "Apply dimension")
	expect(field.isConnected).toBe(false)
	requireValue(editor.root.querySelector('svg text[aria-label^="Constraint ref:"]')).dispatchEvent(new dom.MouseEvent("dblclick", { bubbles: true }) as unknown as Event)
	expect(editor.root.querySelector("[data-dimension-editor]")).toBeNull()
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ radius: 10 })
})

it("accepts length units and arithmetic in properties and canvas labels and persists the converted dimension", async () => {
	const sketch = blank()
	sketch.entities = [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 32 }]
	sketch.relations = [{ id: "diam", type: "diameter", entityId: "c", value: 20 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "diameter 20 · diam")
	let field = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Dimension (mm)"]'))
	field.value = "2 in"
	field.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')?.textContent).toBe("Ø50.8")
	field = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Dimension (mm)"]'))
	field.value = "90 deg"
	field.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(field.getAttribute("aria-invalid")).toBe("true")
	expect(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')?.textContent).toBe("Ø50.8")
	requireValue(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')).dispatchEvent(new dom.MouseEvent("dblclick", { bubbles: true }) as unknown as Event)
	field = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Edit dimension value"]'))
	field.value = "2 * (1 in + 5 mm)"
	click(editor.root, "Apply dimension")
	click(editor.root, "Finish sketch")
	expect(structuredClone(saved?.entities[0])).toMatchObject({ radius: expect.closeTo(30.4, 5) })
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [requireValue(saved)] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const loaded = materializePartFeatures(restored.cad, restored.tree)[0]
	if (loaded?.type !== "sketch") throw Error("Missing sketch")
	expect(loaded.relations?.[0]).toMatchObject({ value: expect.closeTo(60.8, 5) })
})
it("converts radians for angle fields without accepting length units", () => {
	const sketch = blank()
	sketch.entities = [{ id: "e", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 32 }]
	sketch.relations = [{ id: "rot", type: "rotation", entityId: "e", value: 0 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "rotation 0 · rot")
	const field = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Angle (degrees)"]'))
	field.value = "2 cm"
	field.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(field.getAttribute("aria-invalid")).toBe("true")
	field.value = "(pi / 2) rad"
	field.dispatchEvent(new dom.Event("change") as unknown as Event)
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ rotation: expect.closeTo(90, 5) })
})

it("previews the exact snap target before placement and clears feedback when snapping is disabled", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const canvas = requireValue(editor.root.querySelector("svg"))
	const move = (x: number, y: number) => canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x, clientY: y }) as unknown as Event)
	click(editor.root, "Point")
	move(502, 351)
	expect(canvas.querySelector('[data-snap-target="origin"]')).not.toBeNull()
	move(560, 350)
	expect(canvas.querySelector('[data-snap-target="grid"]')).not.toBeNull()
	pointer(canvas, 560, 350)
	click(editor.root, "Line")
	move(563, 351)
	expect(canvas.querySelector('[data-snap-target="anchor"]')?.getAttribute("aria-label")).toBe("Snap to point-1 center")
	expect(canvas.querySelector('[data-snap-target="anchor"]')?.getAttribute("x")).toBe("554")
	pointer(canvas, 563, 351)
	pointer(canvas, 620, 350)
	const toggle = requireValue(
		Array.from(editor.root.querySelectorAll("label"))
			.find((label) => label.textContent === "Snap to geometry")
			?.querySelector("input")
	)
	toggle.checked = false
	toggle.dispatchEvent(new dom.Event("change") as unknown as Event)
	const gridToggle = requireValue(
		Array.from(editor.root.querySelectorAll("label"))
			.find((label) => label.textContent === "Snap to grid")
			?.querySelector("input")
	)
	gridToggle.checked = false
	gridToggle.dispatchEvent(new dom.Event("change") as unknown as Event)
	move(563, 351)
	expect(canvas.querySelector("[data-snap-target]")).toBeNull()
	click(editor.root, "Select")
	move(563, 351)
	expect(canvas.querySelector("[data-snap-target]")).toBeNull()
	click(editor.root, "Finish sketch")
	expect(saved?.entities.find((e) => e.type === "line")).toMatchObject({ p0: { x: 20, y: 0 } })
	expect(saved?.relations?.some((r) => r.type === "coincident" && r.b.entityId === "point-1")).toBe(true)
})

it("snaps geometry independently of grid and can suppress inferred relations while retaining explicit constraints", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const canvas = requireValue(editor.root.querySelector("svg"))
	const toggle = (name: string, checked: boolean) => {
		const field = requireValue(
			Array.from(editor.root.querySelectorAll("label"))
				.find((label) => label.textContent === name)
				?.querySelector("input")
		)
		field.checked = checked
		field.dispatchEvent(new dom.Event("change") as unknown as Event)
	}
	toggle("Snap to grid", false)
	toggle("Snap to geometry", false)
	click(editor.root, "Point")
	pointer(canvas, 561.2, 350)
	toggle("Snap to geometry", true)
	toggle("Automatic constraints", false)
	click(editor.root, "Line")
	pointer(canvas, 564, 350)
	pointer(canvas, 620, 350)
	click(editor.root, "Select")
	// No inferred coincidence or horizontal relation; the explicit toolbar remains available.
	expect(editor.root.textContent).toContain("0 constraints")
	pointer(requireValue(canvas.querySelector('[data-entity-id="line-1"]')), 590, 350)
	click(editor.root, "Horizontal")
	click(editor.root, "Finish sketch")
	expect(saved?.entities.find((e) => e.id === "point-1")).toMatchObject({ center: { x: expect.closeTo(20.4, 6), y: 0 } })
	expect(saved?.entities.find((e) => e.id === "line-1")).toMatchObject({ p0: { x: expect.closeTo(20.4, 6), y: 0 } })
	expect(saved?.relations).toHaveLength(1)
	expect(saved?.relations?.[0]?.type).toBe("horizontal")
})
it("preserves the tangent-arc tool's explicit join when automatic constraints and snapping are off", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	for (const name of ["Automatic constraints", "Snap to grid", "Snap to geometry"]) {
		const field = requireValue(
			Array.from(editor.root.querySelectorAll("label"))
				.find((label) => label.textContent === name)
				?.querySelector("input")
		)
		field.checked = false
		field.dispatchEvent(new dom.Event("change") as unknown as Event)
	}
	const canvas = requireValue(editor.root.querySelector("svg"))
	click(editor.root, "Line")
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 350)
	click(editor.root, "Tangent arc")
	pointer(canvas, 562, 350)
	pointer(canvas, 590, 320)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(2)
	expect(saved?.relations).toHaveLength(1)
	expect(saved?.relations?.[0]?.type).toBe("smoothJoin")
})
it("chamfers selected lines, edits both setbacks, and restores the corner through Undo", () => {
	const original = blank()
	original.entities = [
		{ id: "a", type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 0 } },
		{ id: "b", type: "line", p0: { x: 0, y: 0 }, p1: { x: 0, y: 20 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select a")
	const second = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select b"))
	second.dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	input(editor.root, "First chamfer setback (mm)", 3)
	input(editor.root, "Second chamfer setback (mm)", 5)
	click(editor.root, "Sketch chamfer")
	expect(editor.root.querySelector('[data-entity-id="chamfer-1"]')).not.toBeNull()
	input(editor.root, "Second setback (mm)", 7)
	input(editor.root, "Dimension (mm)", 4)
	click(editor.root, "Undo")
	click(editor.root, "Undo")
	click(editor.root, "Undo")
	expect(editor.root.querySelector('[data-entity-id="chamfer-1"]')).toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Redo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.find((r) => r.type === "chamfer")).toMatchObject({ value: 4, secondValue: 7 })
	expect(original.entities).toHaveLength(2)
})
it("authors a distance-angle chamfer and switches its mode with local history", () => {
	const original = blank()
	original.entities = [
		{ id: "a", type: "line", p0: { x: 0, y: 0 }, p1: { x: 20, y: 0 } },
		{ id: "b", type: "line", p0: { x: 0, y: 0 }, p1: { x: 0, y: 20 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select a")
	requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select b")).dispatchEvent(
		new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
	)
	const toggle = (checked: boolean) => {
		const field = requireValue(
			Array.from(editor.root.querySelectorAll("label"))
				.find((label) => label.textContent === "Chamfer distance and angle")
				?.querySelector("input")
		)
		field.checked = checked
		field.dispatchEvent(new dom.Event("change") as unknown as Event)
	}
	input(editor.root, "First chamfer setback (mm)", 3)
	toggle(true)
	input(editor.root, "Chamfer angle (degrees)", 30)
	click(editor.root, "Sketch chamfer")
	expect(editor.root.querySelector('svg text[aria-label^="Constraint chamfer-setback-"]')?.textContent).toBe("3 × 30°")
	toggle(false)
	expect(Number(editor.root.querySelector<HTMLInputElement>('[aria-label="Second setback (mm)"]')?.value)).toBeCloseTo(Math.sqrt(3), 5)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "chamfer 3 · chamfer-setback-1")
	toggle(true)
	input(editor.root, "Chamfer angle (degrees)", 120)
	expect(editor.root.textContent).toContain("Chamfer angle must fit")
	input(editor.root, "Chamfer angle (degrees)", 45)
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.find((r) => r.type === "chamfer")).toMatchObject({ mode: "distance-angle", secondValue: 45 })
})
it("adds a Normal constraint from the toolbar with undo and redo", () => {
	const original = blank()
	original.entities = [
		{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 64 },
		{ id: "line", type: "line", p0: { x: 10, y: 0 }, p1: { x: 20, y: 2 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select circle")
	requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select line")).dispatchEvent(
		new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
	)
	click(editor.root, "Normal")
	expect(editor.root.textContent).toContain("normal · constraint-1")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("normal · constraint-1")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.[0]).toMatchObject({ type: "normal", a: { entityId: "line", point: "p0" }, b: "circle" })
	const line = saved?.entities.find((e) => e.id === "line")
	const circle = saved?.entities.find((e) => e.id === "circle")
	if (line?.type !== "line" || circle?.type !== "circle") throw Error("Missing geometry")
	const radial = { x: line.p0.x - circle.center.x, y: line.p0.y - circle.center.y }
	expect(Math.hypot(radial.x, radial.y)).toBeCloseTo(circle.radius, 5)
	expect((line.p1.x - line.p0.x) * radial.y - (line.p1.y - line.p0.y) * radial.x).toBeCloseTo(0, 4)
})
it("applies selection filters to entity lists, canvas clicks and box selection without hiding geometry", () => {
	const original = blank()
	original.entities = [
		{ id: "p", type: "point", center: { x: 0, y: 0 } },
		{ id: "line", type: "line", p0: { x: -10, y: -10 }, p1: { x: 10, y: -10 } },
		{ id: "guide", type: "line", construction: true, p0: { x: -10, y: 10 }, p1: { x: 10, y: 10 } },
		{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 64 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const canvas = requireValue(editor.root.querySelector("svg"))
	const filter = (value: string) => {
		const field = requireValue(editor.root.querySelector<HTMLSelectElement>('[aria-label="Selection filter"]'))
		field.value = value
		field.dispatchEvent(new dom.Event("change") as unknown as Event)
	}
	click(editor.root, "Select circle")
	filter("Construction")
	expect(editor.root.textContent).not.toContain("Select circle")
	expect(editor.root.textContent).toContain("Select guide")
	expect(canvas.querySelectorAll("[data-entity-id]")).toHaveLength(4)
	expect(canvas.querySelector('[data-entity-id="circle"]')?.getAttribute("aria-disabled")).toBe("true")
	filter("Points")
	expect(editor.root.textContent).toContain("Select p")
	expect(editor.root.textContent).not.toContain("Select guide")
	filter("Curves")
	expect(editor.root.textContent).toContain("Select circle")
	expect(editor.root.textContent).not.toContain("Select p")
	filter("Non-construction")
	expect(editor.root.textContent).not.toContain("Select guide")
	filter("Lines")
	pointer(requireValue(canvas.querySelector('[data-entity-id="circle"]')), 500, 350)
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	expect(canvas.querySelector('[data-entity-id="circle"]')?.getAttribute("stroke-width")).toBe("2")
	pointer(canvas, 0, 0)
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: 1000, clientY: 700 }) as unknown as Event)
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	expect(canvas.querySelector('[data-entity-id="line"]')?.getAttribute("stroke-width")).toBe("3")
	expect(canvas.querySelector('[data-entity-id="guide"]')?.getAttribute("stroke-width")).toBe("3")
	expect(canvas.querySelector('[data-entity-id="circle"]')?.getAttribute("stroke-width")).toBe("2")
	canvas.dispatchEvent(new dom.KeyboardEvent("keydown", { bubbles: true, key: "Delete" }) as unknown as Event)
	click(editor.root, "Finish sketch")
	expect(saved?.entities.map((e) => e.id)).toEqual(["p", "circle"])
	expect(original.entities).toHaveLength(4)
})
it("shows a redundant relation and lets the user remove it with undo support", () => {
	const sketch = blank()
	sketch.entities = [{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 64 }]
	sketch.relations = [
		{ id: "first", type: "diameter", entityId: "circle", value: 20 },
		{ id: "second", type: "diameter", entityId: "circle", value: 20 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	expect(editor.root.textContent).toContain("Locally redundant: second")
	expect(editor.root.querySelector('[data-redundant-relation="first"]')).toBeNull()
	click(editor.root, "diameter 20 · second · redundant")
	expect(editor.root.textContent).toContain("Locally redundant with earlier constraints")
	click(editor.root, "Delete constraint")
	expect(editor.root.querySelector("[data-redundant-relation]")).toBeNull()
	click(editor.root, "Undo")
	expect(editor.root.querySelector('[data-redundant-relation="second"]')).not.toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.map((r) => r.id)).toEqual(["first"])
	expect(saved?.entities[0]).toMatchObject({ radius: 10 })
})
it("shows per-entity constraint states in a mixed sketch and updates them after constraint removal", () => {
	const sketch = blank()
	sketch.entities = [
		{ id: "fixed", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 64 },
		{ id: "free", type: "circle", center: { x: 30, y: 0 }, radius: 5, segments: 64 }
	]
	sketch.relations = [
		{ id: "center", type: "fixed", anchor: { entityId: "fixed", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "radius", type: "radius", entityId: "fixed", value: 10 }
	]
	const editor = new SketchWorkspace(
		sketch,
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	const state = (id: string) => editor.root.querySelector(`[data-entity-id="${id}"]`)?.getAttribute("data-constraint-state")
	expect(state("fixed")).toBe("fully-constrained")
	expect(state("free")).toBe("underconstrained")
	expect(editor.root.querySelector('[data-state-entity="fixed"]')?.textContent).toContain("Fully constrained")
	expect(editor.root.querySelector('[data-entity-id="fixed"]')?.getAttribute("stroke")).toBe("#172033")
	expect(editor.root.querySelector('[data-entity-id="free"]')?.getAttribute("stroke")).toBe("#2563eb")
	click(editor.root, "radius 10 · radius")
	click(editor.root, "Delete constraint")
	expect(state("fixed")).toBe("underconstrained")
	click(editor.root, "Undo")
	expect(state("fixed")).toBe("fully-constrained")
	editor.dispose()
})
it("trims a line at an analytic ellipse intersection from the canvas and preserves it through history", () => {
	const sketch = blank()
	sketch.entities = [
		{ id: "e", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 8 },
		{ id: "l", type: "line", p0: { x: -20, y: 2.5 }, p1: { x: 20, y: 2.5 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select l")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const a = requireValue(canvas.querySelector('[aria-label="l p0"]'))
	const b = requireValue(canvas.querySelector('[aria-label="l p1"]'))
	const x = Number(a.getAttribute("cx")) + (Number(b.getAttribute("cx")) - Number(a.getAttribute("cx"))) / 8
	const y = Number(a.getAttribute("cy"))
	click(editor.root, "Trim")
	pointer(requireValue(canvas.querySelector('[data-entity-id="l"]')), x, y)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const line = saved?.entities.find((e) => e.id === "l")
	if (line?.type !== "line") throw Error("Missing line")
	expect(line.p0.x).toBeCloseTo(-5 * Math.sqrt(3), 8)
	expect(line.p0.y).toBeCloseTo(2.5, 8)
})
it("trims a circle against an ellipse through the canvas without using sampled boundary edges", () => {
	const sketch = blank()
	sketch.entities = [
		{ id: "ellipse", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 8 },
		{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 7, segments: 8 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select ellipse")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const center = requireValue(canvas.querySelector('[aria-label="ellipse center"]'))
	const axis = requireValue(canvas.querySelector('[aria-label="ellipse p0"]'))
	const x = Number(center.getAttribute("cx")) + 0.7 * (Number(axis.getAttribute("cx")) - Number(center.getAttribute("cx")))
	const y = Number(center.getAttribute("cy"))
	click(editor.root, "Trim")
	pointer(requireValue(canvas.querySelector('[data-entity-id="circle"]')), x, y)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const arc = saved?.entities.find((e) => e.id === "circle")
	if (arc?.type !== "arc") throw Error("Missing trimmed circle")
	expect(Math.abs(7 * Math.cos(arc.startAngle))).toBeCloseTo(Math.sqrt(32), 8)
})
it("moves and independently copies selected geometry through property controls with undo", () => {
	const sketch = blank()
	sketch.entities = [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 64 }]
	sketch.relations = [
		{ id: "fixed", type: "fixed", anchor: { entityId: "c", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "radius", type: "radius", entityId: "c", value: 5 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	input(editor.root, "Move X (mm)", 20)
	input(editor.root, "Move Y (mm)", -5)
	click(editor.root, "Move selected")
	input(editor.root, "Move X (mm)", 30)
	input(editor.root, "Move Y (mm)", 0)
	click(editor.root, "Copy selected")
	expect(editor.root.querySelector('[data-entity-id="copy-c-1"]')).not.toBeNull()
	click(editor.root, "Undo")
	expect(editor.root.querySelector('[data-entity-id="copy-c-1"]')).toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities.find((e) => e.id === "c")).toMatchObject({ center: { x: 20, y: -5 } })
	expect(saved?.entities.find((e) => e.id === "copy-c-1")).toMatchObject({ center: { x: 50, y: -5 } })
	expect(sketch.entities[0]).toMatchObject({ center: { x: 0, y: 0 } })
})
it("scales selected dimensioned geometry and undoes the whole operation", () => {
	const sketch = blank()
	sketch.entities = [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 64 }]
	sketch.relations = [
		{ id: "fixed", type: "fixed", anchor: { entityId: "c", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "radius", type: "radius", entityId: "c", value: 5 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	input(editor.root, "Scale center X (mm)", -20)
	input(editor.root, "Scale factor", 2)
	click(editor.root, "Scale copy")
	expect(editor.root.querySelector('[data-entity-id="copy-c-1"]')).not.toBeNull()
	click(editor.root, "Undo")
	expect(editor.root.querySelector('[data-entity-id="copy-c-1"]')).toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Select copy-c-1")
	input(editor.root, "Scale center X (mm)", 20)
	input(editor.root, "Scale factor", 0.5)
	click(editor.root, "Scale selected")
	click(editor.root, "Finish sketch")
	expect(saved?.entities.find((e) => e.id === "copy-c-1")).toMatchObject({ center: { x: 20, y: 0 }, radius: 5 })
	expect(saved?.entities.find((e) => e.id === "c")).toMatchObject({ center: { x: 0, y: 0 }, radius: 5 })
})
it("rotates a constrained line and independent copy through the UI with undo", () => {
	const sketch = blank()
	sketch.entities = [{ id: "l", type: "line", p0: { x: 10, y: 0 }, p1: { x: 20, y: 0 } }]
	sketch.relations = [
		{ id: "start", type: "fixed", anchor: { entityId: "l", point: "p0" }, position: { x: 10, y: 0 } },
		{ id: "h", type: "horizontal", entityId: "l" },
		{ id: "len", type: "length", entityId: "l", value: 10 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select l")
	input(editor.root, "Rotate angle (degrees)", 90)
	click(editor.root, "Rotate copy")
	expect(editor.root.querySelector('[data-entity-id="copy-l-1"]')).not.toBeNull()
	click(editor.root, "Undo")
	expect(editor.root.querySelector('[data-entity-id="copy-l-1"]')).toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Select copy-l-1")
	click(editor.root, "Rotate selected")
	click(editor.root, "Finish sketch")
	expect(saved?.entities.find((e) => e.id === "l")).toMatchObject({ p0: { x: 10, y: 0 } })
	const line = saved?.entities.find((e) => e.id === "copy-l-1")
	if (line?.type !== "line") throw Error("Missing line")
	expect(line.p0.x).toBeCloseTo(-10, 6)
	expect(line.p0.y).toBeCloseTo(0, 6)
	expect(line.p1.x).toBeCloseTo(-20, 6)
	expect(line.p1.y).toBeCloseTo(0, 6)
})
it("moves a group with a canvas handle, cancels its preview, and commits one undoable axis-locked drag", () => {
	const sketch = blank()
	sketch.entities = [
		{ id: "a", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } },
		{ id: "b", type: "line", p0: { x: 0, y: 10 }, p1: { x: 10, y: 10 } },
		{ id: "p", type: "point", center: { x: 0, y: 0 } }
	]
	sketch.relations = [
		{ id: "fixed", type: "fixed", anchor: { entityId: "a", point: "p0" }, position: { x: 0, y: 0 } },
		{ id: "join", type: "coincident", a: { entityId: "a", point: "p0" }, b: { entityId: "p", point: "center" } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select a")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const start = requireValue(canvas.querySelector('[aria-label="a p0"]'))
	const end = requireValue(canvas.querySelector('[aria-label="a p1"]'))
	const scale = (Number(end.getAttribute("cx")) - Number(start.getAttribute("cx"))) / 10
	requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select b")).dispatchEvent(
		new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
	)
	const begin = () => {
		const handle = requireValue(canvas.querySelector("[data-group-move-handle]"))
		const x = Number(handle.getAttribute("cx"))
		const y = Number(handle.getAttribute("cy"))
		pointer(handle, x, y)
		return { x, y }
	}
	let p = begin()
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: p.x + 5 * scale, clientY: p.y - 3 * scale }) as unknown as Event)
	expect(editor.root.textContent).toContain("Will detach: join")
	canvas.dispatchEvent(new dom.PointerEvent("pointercancel", { bubbles: true }) as unknown as Event)
	expect(editor.root.textContent).not.toContain("Will detach")
	expect(editor.root.textContent).toContain("coincident · join")
	p = begin()
	for (const dx of [5, 7]) canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: p.x + dx * scale, clientY: p.y - 2 * scale, shiftKey: true }) as unknown as Event)
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	expect(editor.root.textContent).toContain("Detached external constraints: join")
	click(editor.root, "Undo")
	expect(editor.root.textContent).toContain("coincident · join")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities.find((e) => e.id === "a")).toMatchObject({ p0: { x: 7, y: 0 } })
	expect(saved?.entities.find((e) => e.id === "b")).toMatchObject({ p0: { x: 7, y: 10 } })
	expect(saved?.entities.find((e) => e.id === "p")).toMatchObject({ center: { x: 0, y: 0 } })
	expect(saved?.relations?.some((r) => r.id === "join")).toBe(false)
})
it("rotates a selected group with angle snapping and restores cancelled previews", () => {
	const sketch = blank()
	sketch.entities = [{ id: "l", type: "line", p0: { x: 10, y: 0 }, p1: { x: 20, y: 0 } }]
	sketch.relations = [
		{ id: "fixed", type: "fixed", anchor: { entityId: "l", point: "p0" }, position: { x: 10, y: 0 } },
		{ id: "h", type: "horizontal", entityId: "l" },
		{ id: "len", type: "length", entityId: "l", value: 10 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select l")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const begin = () => {
		const handle = requireValue(canvas.querySelector("[data-group-rotate-handle]"))
		const pivot = requireValue(canvas.querySelector("[data-group-rotation-pivot]"))
		const x = Number(handle.getAttribute("cx"))
		const y = Number(handle.getAttribute("cy"))
		const cx = Number(pivot.getAttribute("cx"))
		const cy = Number(pivot.getAttribute("cy"))
		pointer(handle, x, y)
		return (degrees: number, shift = false) => {
			const angle = (degrees * Math.PI) / 180
			canvas.dispatchEvent(
				new dom.PointerEvent("pointermove", {
					bubbles: true,
					clientX: cx + (x - cx) * Math.cos(angle) + (y - cy) * Math.sin(angle),
					clientY: cy - (x - cx) * Math.sin(angle) + (y - cy) * Math.cos(angle),
					shiftKey: shift
				}) as unknown as Event
			)
		}
	}
	let move = begin()
	move(70)
	canvas.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as unknown as Event)
	expect(editor.root.textContent).toContain("horizontal · h")
	move = begin()
	move(28, true)
	expect(editor.root.textContent).toContain("Rotate 30°")
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	click(editor.root, "Undo")
	expect(editor.root.textContent).toContain("horizontal · h")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const line = saved?.entities[0]
	if (line?.type !== "line") throw Error("Missing line")
	expect(line.p0.x).toBeCloseTo(10 * Math.cos(Math.PI / 6), 6)
	expect(line.p0.y).toBeCloseTo(5, 6)
	expect(saved?.relations?.find((r) => r.id === "h")).toMatchObject({ type: "rotation", value: expect.closeTo(30, 6) })
})
it("unwraps group rotation across the atan2 seam", () => {
	const sketch = blank()
	sketch.entities = [{ id: "p", type: "point", center: { x: 10, y: 0 } }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select p")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const handle = requireValue(canvas.querySelector("[data-group-rotate-handle]"))
	const pivot = requireValue(canvas.querySelector("[data-group-rotation-pivot]"))
	const x = Number(handle.getAttribute("cx"))
	const y = Number(handle.getAttribute("cy"))
	const cx = Number(pivot.getAttribute("cx"))
	const cy = Number(pivot.getAttribute("cy"))
	pointer(handle, x, y)
	for (const degrees of [90, 170, 190]) {
		const angle = (degrees * Math.PI) / 180
		canvas.dispatchEvent(
			new dom.PointerEvent("pointermove", {
				bubbles: true,
				clientX: cx + (x - cx) * Math.cos(angle) + (y - cy) * Math.sin(angle),
				clientY: cy - (x - cx) * Math.sin(angle) + (y - cy) * Math.cos(angle)
			}) as unknown as Event
		)
	}
	expect(editor.root.textContent).toContain("Rotate 190°")
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	click(editor.root, "Finish sketch")
	const point = saved?.entities[0]
	if (point?.type !== "point") throw Error("Missing point")
	expect(point.center.x).toBeCloseTo(10 * Math.cos((190 * Math.PI) / 180), 6)
	expect(point.center.y).toBeCloseTo(10 * Math.sin((190 * Math.PI) / 180), 6)
})
it("scales a group with a snapped canvas handle and restores cancelled previews", () => {
	const sketch = blank()
	sketch.entities = [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 64 }]
	sketch.relations = [
		{ id: "center", type: "fixed", anchor: { entityId: "c", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "radius", type: "radius", entityId: "c", value: 5 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select c")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const begin = () => {
		const handle = requireValue(canvas.querySelector("[data-group-scale-handle]"))
		const pivot = requireValue(canvas.querySelector("[data-group-scale-pivot]"))
		const x = Number(handle.getAttribute("x")) + 9
		const y = Number(handle.getAttribute("y")) + 9
		const cx = Number(pivot.getAttribute("data-center-x"))
		const cy = Number(pivot.getAttribute("data-center-y"))
		handle.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Enter", bubbles: true }) as unknown as Event)
		expect(document.activeElement?.getAttribute("aria-label")).toBe("Scale factor")
		pointer(handle, x, y)
		return (factor: number, shift = false) =>
			canvas.dispatchEvent(
				new dom.PointerEvent("pointermove", { bubbles: true, clientX: cx + (x - cx) * factor, clientY: cy + (y - cy) * factor, shiftKey: shift }) as unknown as Event
			)
	}
	let move = begin()
	move(1.8)
	canvas.dispatchEvent(new dom.PointerEvent("pointercancel", { bubbles: true }) as unknown as Event)
	expect(editor.root.textContent).toContain("radius 5 · radius")
	move = begin()
	move(1.94, true)
	expect(editor.root.textContent).toContain("Scale 1.9×")
	move(-0.5)
	expect(editor.root.textContent).toContain("Scale factor must remain positive")
	move(2.04, true)
	expect(editor.root.textContent).toContain("Scale 2×")
	canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	click(editor.root, "Undo")
	expect(editor.root.textContent).toContain("radius 5 · radius")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ radius: 10, center: { x: 0, y: 0 } })
	expect(saved?.relations?.find((r) => r.id === "radius")).toMatchObject({ value: 10 })
})
it("searches sketch tools with keyboard navigation, cancellation and pointer activation", () => {
	const editor = new SketchWorkspace(
		blank(),
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	const canvas = requireValue(editor.root.querySelector("svg"))
	const key = (target: Element, value: string) => target.dispatchEvent(new dom.KeyboardEvent("keydown", { bubbles: true, key: value }) as unknown as Event)
	key(canvas, "s")
	let search = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Search sketch tools"]'))
	expect(document.activeElement).toBe(search)
	search.value = "circle"
	search.dispatchEvent(new dom.Event("input") as unknown as Event)
	expect(editor.root.querySelectorAll('[role="option"]')).toHaveLength(3)
	key(search, "ArrowDown")
	key(search, "Enter")
	expect(editor.root.querySelector('[aria-label="Tool search"]')).toBeNull()
	expect(Array.from(editor.root.querySelectorAll('button[aria-pressed="true"]')).map((b) => b.textContent)).toContain("3-point circle")
	click(editor.root, "Search tools")
	search = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Search sketch tools"]'))
	search.value = "nothing matches"
	search.dispatchEvent(new dom.Event("input") as unknown as Event)
	key(search, "Enter")
	expect(editor.root.textContent).toContain("No matching tools")
	key(search, "Escape")
	expect(document.activeElement).toBe(canvas)
	click(editor.root, "Search tools")
	search = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Search sketch tools"]'))
	search.value = "center arc"
	search.dispatchEvent(new dom.Event("input") as unknown as Event)
	requireValue(editor.root.querySelector<HTMLButtonElement>('[role="option"]')).click()
	expect(Array.from(editor.root.querySelectorAll('button[aria-pressed="true"]')).map((b) => b.textContent)).toContain("Center arc")
	const field = requireValue(editor.root.querySelector("input"))
	key(field, "s")
	expect(editor.root.querySelector('[aria-label="Tool search"]')).toBeNull()
	editor.dispose()
})

it("adds internal tangency with the larger arc as container and preserves it through undo and finish", () => {
	const original = blank()
	original.entities = [
		{ id: "small", type: "circle", center: v2(8, 0), radius: 5, segments: 64, construction: true },
		{ id: "large", type: "arc", center: v2(0, 0), radius: 20, startAngle: -Math.PI / 2, sweep: Math.PI, segments: 64, construction: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select small")
	const large = Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select large")
	requireValue(large).dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	click(editor.root, "Internal tangent")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "internalTangent", a: "large", b: "small" }))
	const small = saved?.entities[0]
	const big = saved?.entities[1]
	if (small?.type !== "circle" || big?.type !== "arc") throw Error("Missing circles")
	expect(Math.hypot(small.center.x - big.center.x, small.center.y - big.center.y) + small.radius).toBeCloseTo(big.radius, 5)
})
it("previews and places a point at an analytic ellipse intersection", () => {
	const sketch = blank()
	const ellipse = { id: "e", type: "ellipse" as const, center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 8, construction: true }
	sketch.entities = [ellipse, { ...ellipse, id: "other", rotation: 90 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(value) => {
			saved = value
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Point")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const coordinate = Math.sqrt(20)
	const x = 500 + coordinate * 25.2 + 1
	const y = 350 - coordinate * 25.2 - 1
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x, clientY: y }) as unknown as Event)
	expect(canvas.querySelector('[data-snap-target="intersection"]')?.getAttribute("aria-label")).toBe("Snap to Intersection")
	pointer(canvas, x, y)
	click(editor.root, "Finish sketch")
	const point = saved?.entities.find((entity) => entity.type === "point")
	if (point?.type !== "point") throw Error("Missing point")
	expect(point.center.x).toBeCloseTo(coordinate, 8)
	expect(point.center.y).toBeCloseTo(coordinate, 8)
})
it("places a point at a finite line-circle intersection", () => {
	const sketch = blank()
	sketch.entities = [
		{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 8, construction: true },
		{ id: "line", type: "line", p0: { x: -10, y: 3 }, p1: { x: 10, y: 3 }, construction: true }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(value) => {
			saved = value
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Point")
	const canvas = requireValue(editor.root.querySelector("svg"))
	const x = 500 + 4 * 25.2 + 1
	const y = 350 - 3 * 25.2 + 1
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x, clientY: y }) as unknown as Event)
	expect(canvas.querySelector('[data-snap-target="intersection"]')).not.toBeNull()
	pointer(canvas, x, y)
	click(editor.root, "Finish sketch")
	const point = saved?.entities.find((entity) => entity.type === "point")
	if (point?.type !== "point") throw Error("Missing point")
	expect(point.center.x).toBeCloseTo(4, 8)
	expect(point.center.y).toBeCloseTo(3, 8)
})
it("persists inferred intersection constraints and follows edits to their source curves", async () => {
	const { solveSketch } = await import("../src/sketch-solver")
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	for (const automatic of [true, false]) {
		const sketch = blank()
		sketch.entities = [
			{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 5, segments: 8, construction: true },
			{ id: "line", type: "line", p0: { x: -10, y: 3 }, p1: { x: 10, y: 3 }, construction: true }
		]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			sketch,
			(value) => {
				saved = value
			},
			() => undefined
		)
		document.body.append(editor.root)
		if (!automatic) {
			const field = requireValue(
				Array.from(editor.root.querySelectorAll("label"))
					.find((label) => label.textContent === "Automatic constraints")
					?.querySelector("input")
			)
			field.checked = false
			field.dispatchEvent(new dom.Event("change") as unknown as Event)
		}
		click(editor.root, "Point")
		const canvas = requireValue(editor.root.querySelector("svg"))
		pointer(canvas, 500 + 4 * 25.2 + 1, 350 - 3 * 25.2 + 1)
		click(editor.root, "Undo")
		click(editor.root, "Redo")
		click(editor.root, "Finish sketch")
		const accepted = requireValue(saved)
		const relations = accepted.relations?.filter((r) => r.type === "pointOnCurve") ?? []
		expect(relations).toHaveLength(automatic ? 2 : 0)
		if (!automatic) continue
		const runtime = createPartRuntimeState({ features: [accepted] })
		const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
		const persisted = materializePartFeatures(restored.cad, restored.tree)[0]
		if (persisted?.type !== "sketch") throw Error("Missing sketch")
		const result = solveSketch(persisted.entities, [
			...(persisted.relations ?? []),
			{ id: "center", type: "fixed", anchor: { entityId: "circle", point: "center" }, position: { x: 0, y: 0 } },
			{ id: "radius", type: "radius", entityId: "circle", value: 6 },
			{ id: "start", type: "fixed", anchor: { entityId: "line", point: "p0" }, position: { x: -10, y: 3 } },
			{ id: "end", type: "fixed", anchor: { entityId: "line", point: "p1" }, position: { x: 10, y: 3 } }
		])
		expect(result.status).not.toBe("conflicting")
		const point = result.entities.find((entity) => entity.type === "point")
		if (point?.type !== "point") throw Error("Missing point")
		expect(point.center.x).toBeCloseTo(Math.sqrt(27), 5)
		expect(point.center.y).toBeCloseTo(3, 5)
	}
})
it("retains a surviving point-on-curve dependency through UI trim and history", () => {
	const source = blank()
	source.entities = [
		{ id: "base", type: "line", p0: v2(0, 0), p1: v2(20, 0), construction: true },
		{ id: "cut1", type: "line", p0: v2(8, -5), p1: v2(8, 5), construction: true },
		{ id: "cut2", type: "line", p0: v2(12, -5), p1: v2(12, 5), construction: true },
		{ id: "point", type: "point", center: v2(16, 0) }
	]
	source.relations = [{ id: "on", type: "pointOnCurve", a: { entityId: "point", point: "center" }, b: "base" }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(value) => {
			saved = value
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Trim")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(requireValue(canvas.querySelector('[data-entity-id="base"]')), 500, 350)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual({ id: "on", type: "pointOnCurve", a: { entityId: "point", point: "center" }, b: "base-split-1" })
	expect(source.relations[0]).toMatchObject({ b: "base" })
})
it("previews a midpoint snap and persists its inferred constraint through source edits", async () => {
	const source = blank()
	source.entities = [{ id: "line", type: "line", p0: v2(0, 0), p1: v2(20, 0), construction: true }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(value) => {
			saved = value
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Point")
	const canvas = requireValue(editor.root.querySelector("svg"))
	canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: 501, clientY: 351 }) as unknown as Event)
	expect(canvas.querySelector('[data-snap-target="midpoint"]')?.getAttribute("aria-label")).toBe("Snap to Midpoint")
	pointer(canvas, 501, 351)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const accepted = requireValue(saved)
	expect(accepted.relations).toContainEqual(expect.objectContaining({ type: "midpoint", b: "line" }))
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [accepted] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const persisted = materializePartFeatures(restored.cad, restored.tree)[0]
	if (persisted?.type !== "sketch") throw Error("Missing sketch")
	const { solveSketch } = await import("../src/sketch-solver")
	const result = solveSketch(persisted.entities, [
		...(persisted.relations ?? []),
		{ id: "start", type: "fixed", anchor: { entityId: "line", point: "p0" }, position: v2(0, 0) },
		{ id: "end", type: "fixed", anchor: { entityId: "line", point: "p1" }, position: v2(30, 10) }
	])
	expect(result.status).not.toBe("conflicting")
	const point = result.entities.find((entity) => entity.type === "point")
	if (point?.type !== "point") throw Error("Missing point")
	expect(point.center.x).toBeCloseTo(15, 5)
	expect(point.center.y).toBeCloseTo(5, 5)
})
it("creates a new sketch on an extrusion face and retains its support when the source depth changes", async () => {
	const builder = new PartBuilder()
	builder.extrude("base", { outline: circle(v2(0, 0), 30, 32), depth: 25 })
	let saved: PartDocument | undefined
	const panel = new SolidFeaturePanel(builder.document, (next) => {
		saved = next
	})
	document.body.append(panel.root)
	const support = requireValue(panel.root.querySelector<HTMLSelectElement>('[aria-label="Sketch plane"]'))
	const top = requireValue(Array.from(support.options).find((option) => option.textContent === "base: Top Face"))
	support.value = top.value
	support.dispatchEvent(new dom.Event("change") as unknown as Event)
	click(panel.root, "New sketch")
	const workspace = requireValue(document.querySelector<HTMLElement>('[aria-label="Sketch workspace"]'))
	expect(workspace.textContent).toContain("Attached face")
	click(workspace, "Circle")
	const canvas = requireValue(workspace.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 530, 350)
	click(workspace, "Finish sketch")
	click(panel.root, "Apply changes")
	const attached = saved?.features.find((feature) => feature.type === "sketch" && feature.id === "Sketch 1")
	if (attached?.type !== "sketch") throw Error("Missing attached sketch")
	expect(attached.target.type).toBe("face")
	input(panel.root, "New extrusion depth", 7)
	click(panel.root, "Extrude sketch")
	click(panel.root, "Apply changes")
	const accepted = structuredClone(requireValue(saved))
	const extrusion = accepted.features.find((feature) => feature.type === "extrude" && feature.target.sketchId === attached.id)
	if (extrusion?.type !== "extrude") throw Error("Missing attached extrusion")
	const { extrudeSolidFeature } = await import("../src/cad/extrude")
	expect(extrudeSolidFeature(accepted, extrusion).frame.origin.z).toBeCloseTo(25, 8)
	const base = accepted.features.find((feature) => feature.type === "extrude" && feature.id !== extrusion.id)
	if (base?.type !== "extrude") throw Error("Missing base")
	base.depth = 30
	expect(extrudeSolidFeature(accepted, extrusion).frame.origin.z).toBeCloseTo(30, 8)
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState(accepted)
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const features = materializePartFeatures(restored.cad, restored.tree)
	const restoredSketch = features.find((feature) => feature.id === attached.id)
	if (restoredSketch?.type !== "sketch") throw Error("Missing restored sketch")
	expect(restoredSketch.target).toEqual(attached.target)
	const restoredExtrusion = features.find((feature) => feature.id === extrusion.id)
	if (restoredExtrusion?.type !== "extrude") throw Error("Missing restored extrusion")
	expect(extrudeSolidFeature({ features }, restoredExtrusion).frame.origin.z).toBeCloseTo(30, 8)
	expect(builder.document.features.filter((feature) => feature.type === "sketch")).toHaveLength(1)
})
it("retargets a sketch support and excludes self and descendant extrusion faces", async () => {
	const builder = new PartBuilder()
	builder.extrude("base", { outline: circle(v2(0, 0), 20, 16), depth: 25 })
	builder.extrude("child", { outline: circle(v2(0, 0), 5, 16), depth: 5 })
	const base = requireValue(builder.document.features.find((feature) => feature.type === "extrude" && feature.id.includes("base")))
	const child = requireValue(builder.document.features.find((feature) => feature.type === "extrude" && feature.id.includes("child")))
	if (base.type !== "extrude" || child.type !== "extrude") throw Error("Missing extrusions")
	const sketch = requireValue(builder.document.features.find((feature) => feature.id === child.target.sketchId))
	if (sketch.type !== "sketch") throw Error("Missing sketch")
	const { extrudeSolidFeature, getExtrudedFaceDescriptors } = await import("../src/cad/extrude")
	const top = requireValue(getExtrudedFaceDescriptors(extrudeSolidFeature(builder.document, base)).find((face) => face.label === "Top Face"))
	sketch.target = { type: "face", face: { type: "extrudeFace", extrudeId: base.id, faceId: top.faceId } }
	let saved: PartDocument | undefined
	const panel = new SolidFeaturePanel(builder.document, (next) => {
		saved = next
	})
	document.body.append(panel.root)
	panel.selectNavigation(JSON.stringify(["standalone-sketch", sketch.id]))
	const support = requireValue(panel.root.querySelector<HTMLSelectElement>('[aria-label="Sketch support"]'))
	expect(support.selectedOptions[0]?.textContent).toBe("base: Top Face")
	expect(Array.from(support.options).some((option) => option.textContent?.startsWith("child:"))).toBe(false)
	support.value = "YZ"
	support.dispatchEvent(new dom.Event("change") as unknown as Event)
	click(panel.root, "Apply changes")
	const moved = saved?.features.find((feature) => feature.id === sketch.id)
	if (moved?.type !== "sketch") throw Error("Missing moved sketch")
	expect(moved.target).toEqual({ type: "plane", plane: "YZ" })
	expect(moved.entities).toEqual(sketch.entities)
	expect(sketch.target.type).toBe("face")
	const originalPanel = new SolidFeaturePanel(builder.document, () => undefined)
	document.body.append(originalPanel.root)
	originalPanel.selectNavigation(JSON.stringify(["standalone-sketch", base.target.sketchId]))
	const baseSupport = requireValue(originalPanel.root.querySelector<HTMLSelectElement>('[aria-label="Sketch support"]'))
	expect(Array.from(baseSupport.options).map((option) => option.value)).toEqual(["XY", "XZ", "YZ"])
	const reordered = structuredClone(builder.document)
	reordered.solidSteps?.reverse()
	const reorderedPanel = new SolidFeaturePanel(reordered, () => undefined)
	document.body.append(reorderedPanel.root)
	reorderedPanel.selectNavigation(JSON.stringify(["feature", "base"]))
	const reorderedSupport = requireValue(reorderedPanel.root.querySelector<HTMLSelectElement>('[aria-label="Sketch support"]'))
	expect(Array.from(reorderedSupport.options).map((option) => option.value)).toEqual(["XY", "XZ", "YZ"])
})
it("displays support edges without adding selectable sketch geometry and toggles their visibility", () => {
	const sketch = blank()
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		sketch,
		(value) => {
			saved = value
		},
		() => undefined,
		[[v2(-10, -10), v2(10, -10), v2(10, 10)]]
	)
	document.body.append(editor.root)
	const edge = requireValue(editor.root.querySelector("[data-support-edge]"))
	expect(edge.getAttribute("pointer-events")).toBe("none")
	expect(editor.root.querySelectorAll("[data-entity-id]")).toHaveLength(0)
	const checkbox = requireValue(
		Array.from(editor.root.querySelectorAll("label"))
			.find((label) => label.textContent === "Show support")
			?.querySelector("input")
	)
	checkbox.checked = false
	checkbox.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(editor.root.querySelector("[data-support-edge]")).toBeNull()
	checkbox.checked = true
	checkbox.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(editor.root.querySelector("[data-support-edge]")).not.toBeNull()
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(0)
	expect(saved?.relations).toHaveLength(0)
})
it("fits the first measured viewport once, without refitting later resizes", () => {
	const originals = { ResizeObserver: globalThis.ResizeObserver, requestAnimationFrame: globalThis.requestAnimationFrame, cancelAnimationFrame: globalThis.cancelAnimationFrame }
	let notify: () => void = () => undefined
	let frame: () => void = () => undefined
	let disconnected = false
	Object.assign(globalThis, {
		ResizeObserver: class {
			constructor(callback: () => void) {
				notify = callback
			}
			observe() {}
			disconnect() {
				disconnected = true
			}
		},
		requestAnimationFrame: (callback: () => void) => {
			frame = callback
			return 1
		},
		cancelAnimationFrame: () => undefined
	})
	try {
		const source = blank()
		source.entities = [{ id: "circle", type: "circle", center: v2(0, 0), radius: 10, segments: 32 }]
		const editor = new SketchWorkspace(
			source,
			() => undefined,
			() => undefined
		)
		document.body.append(editor.root)
		const svg = requireValue(editor.root.querySelector("svg"))
		let width = 1600
		let height = 900
		svg.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: width, bottom: height, width, height, toJSON: () => ({}) })
		notify()
		frame()
		const firstX = () => Number(requireValue(svg.querySelector('[data-entity-id="circle"]')?.getAttribute("d")).match(/M\s*([\d.]+)/)?.[1])
		expect(firstX()).toBeCloseTo(800 + 324, 5)
		width = 2000
		height = 1800
		notify()
		frame()
		expect(firstX()).toBeCloseTo(1000 + 324, 5)
		click(editor.root, "Fit sketch")
		expect(firstX()).toBeCloseTo(1000 + 648, 5)
		editor.dispose()
		expect(disconnected).toBe(true)
	} finally {
		Object.assign(globalThis, originals)
	}
})
it("shows an unresolved face support and clears the diagnosis after reattaching to a plane", () => {
	const source = blank()
	source.target = { type: "face", face: { type: "extrudeFace", extrudeId: "missing", faceId: "top" } }
	source.entities = [{ id: "point", type: "point", center: v2(2, 3) }]
	const panel = new SolidFeaturePanel({ features: [source], solidSteps: [] }, () => undefined)
	document.body.append(panel.root)
	panel.selectNavigation(JSON.stringify(["standalone-sketch", source.id]))
	click(panel.root, "Edit sketch")
	let workspace = requireValue(document.querySelector<HTMLElement>('[aria-label="Sketch workspace"]'))
	expect(workspace.querySelector("[data-support-error]")?.textContent).toContain("Missing supporting extrusion")
	expect(workspace.querySelector("[data-support-error]")?.getAttribute("role")).toBe("alert")
	expect(workspace.querySelectorAll("[data-entity-id]")).toHaveLength(1)
	click(workspace, "Point")
	expect(workspace.querySelector("[data-support-error]")).not.toBeNull()
	click(workspace, "Cancel sketch")
	const support = requireValue(panel.root.querySelector<HTMLSelectElement>('[aria-label="Sketch support"]'))
	support.value = "XY"
	support.dispatchEvent(new dom.Event("change") as unknown as Event)
	click(panel.root, "Edit sketch")
	workspace = requireValue(document.querySelector<HTMLElement>('[aria-label="Sketch workspace"]'))
	expect(workspace.querySelector("[data-support-error]")).toBeNull()
	expect(workspace.textContent).toContain("XY plane")
	click(workspace, "Cancel sketch")
	expect(source.target.type).toBe("face")
})
it("constrains two existing circles symmetrically without making copies", async () => {
	const source = blank()
	source.entities = [
		{ id: "a", type: "circle", center: v2(3, 4), radius: 2, segments: 32, construction: true },
		{ id: "b", type: "circle", center: v2(-4, 3), radius: 5, segments: 32, construction: true },
		{ id: "axis", type: "line", p0: v2(0, 0), p1: v2(0, 10), construction: true }
	]
	source.relations = [
		{ id: "a-center", type: "fixed", anchor: { entityId: "a", point: "center" }, position: v2(3, 4) },
		{ id: "a-radius", type: "radius", entityId: "a", value: 2 },
		{ id: "axis-start", type: "fixed", anchor: { entityId: "axis", point: "p0" }, position: v2(0, 0) },
		{ id: "axis-end", type: "fixed", anchor: { entityId: "axis", point: "p1" }, position: v2(0, 10) }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(next) => {
			saved = next
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select a")
	for (const id of ["b", "axis"])
		requireValue(Array.from(editor.root.querySelectorAll("button")).find((button) => button.textContent === `Select ${id}`)).dispatchEvent(
			new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
		)
	click(editor.root, "Symmetric entities")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const result = requireValue(saved)
	expect(result.entities.map((entity) => entity.id)).toEqual(["a", "b", "axis"])
	const b = result.entities[1]
	if (b?.type !== "circle") throw Error("Missing circle")
	expect(b.center.x).toBeCloseTo(-3, 5)
	expect(b.center.y).toBeCloseTo(4, 5)
	expect(b.radius).toBeCloseTo(2, 5)
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [result] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const persisted = materializePartFeatures(restored.cad, restored.tree)[0]
	if (persisted?.type !== "sketch") throw Error("Missing restored sketch")
	expect(persisted.relations).toContainEqual(expect.objectContaining({ type: "mirror", a: "a", b: "b", symmetryLine: "axis" }))
})

it("persists a line-target Normal constraint after toolbar undo and redo", async () => {
	const original = blank()
	original.entities = [
		{ id: "source", type: "line", p0: { x: 3, y: 1 }, p1: { x: 4, y: 6 } },
		{ id: "target", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select source")
	requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select target")).dispatchEvent(
		new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
	)
	expect(editor.root.querySelector('[aria-label="Fillet radius (mm)"]')).not.toBeNull()
	expect(editor.root.querySelector('[aria-label="First chamfer setback (mm)"]')).not.toBeNull()
	click(editor.root, "Normal")
	expect(editor.root.querySelector('[aria-label="Fillet radius (mm)"]')).toBeNull()
	expect(editor.root.querySelector('[aria-label="First chamfer setback (mm)"]')).toBeNull()
	expect(editor.root.textContent).toContain("normal · constraint-1")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("normal · constraint-1")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	if (!saved) throw Error("Missing sketch")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [saved] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const sketch = materializePartFeatures(restored.cad, restored.tree)[0]
	if (sketch?.type !== "sketch") throw Error("Missing restored sketch")
	expect(sketch.relations?.[0]).toMatchObject({ type: "normal", a: { entityId: "source", point: "p0" }, b: "target" })
	const source = sketch.entities.find((e) => e.id === "source")
	const target = sketch.entities.find((e) => e.id === "target")
	if (source?.type !== "line" || target?.type !== "line") throw Error("Missing lines")
	const { sketchNormalResidual } = await import("../src/sketch-normal")
	for (const residual of sketchNormalResidual(source, "p0", target)) expect(residual).toBeCloseTo(0, 5)
})

it("focuses constraint properties and restores geometry controls when reselecting an entity", () => {
	const original = blank()
	original.entities = [{ id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }]
	original.relations = [{ id: "horizontal", type: "horizontal", entityId: "line" }]
	const editor = new SketchWorkspace(
		original,
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select line")
	expect(editor.root.querySelector('[aria-label="Move X (mm)"]')).not.toBeNull()
	expect(editor.root.querySelector('[aria-label="Move selected group"]')).not.toBeNull()
	click(editor.root, "horizontal · horizontal")
	expect(editor.root.textContent).toContain("Delete constraint")
	expect(editor.root.textContent).not.toContain("Add driving dimension")
	expect(editor.root.querySelector('[aria-label="Move X (mm)"]')).toBeNull()
	expect(editor.root.querySelector('[aria-label="Move selected group"]')).toBeNull()
	click(editor.root, "Select line")
	expect(editor.root.textContent).toContain("Add driving dimension")
	expect(editor.root.querySelector('[aria-label="Move X (mm)"]')).not.toBeNull()
	expect(editor.root.querySelector('[aria-label="Move selected group"]')).not.toBeNull()
	editor.dispose()
})

it("deletes the focused constraint with Delete or Backspace and preserves its geometry through history", () => {
	for (const key of ["Delete", "Backspace"]) {
		const original = blank()
		original.entities = [{ id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }]
		original.relations = [{ id: "h", type: "horizontal", entityId: "line" }]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			original,
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		click(editor.root, "Select line")
		click(editor.root, "horizontal · h")
		editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key, bubbles: true }) as unknown as Event)
		expect(editor.root.textContent).toContain("Entities (1)")
		expect(editor.root.textContent).not.toContain("horizontal · h")
		click(editor.root, "Undo")
		expect(editor.root.textContent).toContain("horizontal · h")
		click(editor.root, "Redo")
		click(editor.root, "Finish sketch")
		expect(saved?.entities).toEqual(original.entities)
		expect(saved?.relations).toEqual([])
		expect(original.relations).toHaveLength(1)
	}
})

it("anchors origin-snapped points only when automatic constraints and geometry snapping are enabled", () => {
	for (const disabled of [null, "Automatic constraints", "Snap to geometry"]) {
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			blank(),
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		if (disabled) {
			const label = requireValue(Array.from(editor.root.querySelectorAll("label")).find((l) => l.textContent === disabled))
			const checkbox = requireValue(label.querySelector<HTMLInputElement>("input"))
			checkbox.checked = false
			checkbox.dispatchEvent(new dom.Event("change") as unknown as Event)
		}
		click(editor.root, "Point")
		pointer(requireValue(editor.root.querySelector("svg")), 500, 350)
		if (!disabled) {
			expect(editor.root.textContent).toContain("Fully constrained")
			click(editor.root, "Undo")
			expect(editor.root.textContent).toContain("Entities (0)")
			click(editor.root, "Redo")
		}
		click(editor.root, "Finish sketch")
		expect(saved?.entities).toHaveLength(1)
		expect(saved?.relations ?? []).toEqual(disabled ? [] : [{ id: "constraint-1", type: "fixed", anchor: { entityId: "point-1", point: "center" }, position: { x: 0, y: 0 } }])
	}
})

it("keeps a corner rectangle attached to the origin while changing both dimensions", async () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Rectangle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 560, 320)
	click(editor.root, "Dimension width")
	input(editor.root, "Dimension (mm)", 40)
	click(editor.root, "Select rectangle-1")
	click(editor.root, "Dimension height")
	input(editor.root, "Dimension (mm)", 30)
	expect(editor.root.textContent).toContain("Fully constrained")
	click(editor.root, "Finish sketch")
	if (!saved) throw Error("Missing sketch")
	expect(saved.relations).toContainEqual({ id: "constraint-1", type: "fixed", anchor: { entityId: "rectangle-1", point: "p0" }, position: { x: 0, y: 0 } })
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const state = createPartRuntimeState({ features: [saved] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(state.cad))), tree: state.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing restored sketch")
	expect(result.entities[0]).toMatchObject({ type: "cornerRectangle", p0: { x: expect.closeTo(0, 5), y: expect.closeTo(0, 5) }, p1: { x: expect.closeTo(40, 5), y: expect.closeTo(30, 5) } })
	expect(result.relations).toEqual(saved.relations)
})

it("does not infer coincidence with an existing anchor when geometry snapping is disabled", () => {
	for (const enabled of [true, false]) {
		const original = blank()
		original.entities = [{ id: "existing", type: "point", center: { x: 0, y: 0 } }]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			original,
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		const label = requireValue(Array.from(editor.root.querySelectorAll("label")).find((l) => l.textContent === "Snap to geometry"))
		const checkbox = requireValue(label.querySelector<HTMLInputElement>("input"))
		checkbox.checked = enabled
		checkbox.dispatchEvent(new dom.Event("change") as unknown as Event)
		click(editor.root, "Point")
		pointer(requireValue(editor.root.querySelector("svg")), 500, 350)
		click(editor.root, "Finish sketch")
		expect(saved?.entities).toHaveLength(2)
		expect(saved?.relations).toEqual(enabled ? [{ id: "constraint-1", type: "coincident", a: { entityId: "point-1", point: "center" }, b: { entityId: "existing", point: "center" } }] : [])
	}
})

it("toggles construction for the entire mixed selection in one undo step", () => {
	const original = blank()
	original.entities = [
		{ id: "a", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 32, construction: true },
		{ id: "b", type: "circle", center: { x: 30, y: 0 }, radius: 10, segments: 32 },
		{ id: "c", type: "circle", center: { x: 60, y: 0 }, radius: 10, segments: 32 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const select = () => {
		click(editor.root, "Select a")
		requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select b")).dispatchEvent(
			new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
		)
	}
	const control = () =>
		requireValue(requireValue(Array.from(editor.root.querySelectorAll("label")).find((l) => l.textContent === "Construction geometry")).querySelector<HTMLInputElement>("input"))
	select()
	expect(control().indeterminate).toBe(true)
	control().checked = true
	control().dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(control().indeterminate).toBe(false)
	expect(control().checked).toBe(true)
	click(editor.root, "Undo")
	select()
	expect(control().indeterminate).toBe(true)
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities.map((e) => !!e.construction)).toEqual([true, true, false])
	expect(original.entities.map((e) => !!e.construction)).toEqual([true, false, false])
})

it("selects all filter-eligible geometry with Ctrl or Cmd A without adding history", () => {
	for (const modifier of ["ctrlKey", "metaKey"]) {
		const original = blank()
		original.entities = [
			{ id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } },
			{ id: "point", type: "point", center: { x: 0, y: 0 } }
		]
		const editor = new SketchWorkspace(
			original,
			() => undefined,
			() => undefined
		)
		document.body.append(editor.root)
		const selectAll = () => editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", [modifier]: true, bubbles: true }) as unknown as Event)
		selectAll()
		expect(editor.root.querySelectorAll('button[aria-pressed="true"]')).toHaveLength(3)
		const filter = requireValue(editor.root.querySelector<HTMLSelectElement>('[aria-label="Selection filter"]'))
		filter.value = "Lines"
		filter.dispatchEvent(new dom.Event("change") as unknown as Event)
		selectAll()
		expect(editor.root.querySelectorAll('button[aria-pressed="true"]')).toHaveLength(2)
		const undo = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo"))
		expect(undo.disabled).toBe(true)
		const field = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Move X (mm)"]'))
		const event = new dom.KeyboardEvent("keydown", { key: "a", [modifier]: true, bubbles: true, cancelable: true })
		field.dispatchEvent(event as unknown as Event)
		expect(event.defaultPrevented).toBe(false)
		editor.dispose()
	}
})

it("clears geometry and constraint focus with Escape without deleting or adding history", () => {
	const original = blank()
	original.entities = [{ id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 0 } }]
	original.relations = [{ id: "h", type: "horizontal", entityId: "line" }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select line")
	click(editor.root, "horizontal · h")
	const key = (value: string) => editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: value, bubbles: true }) as unknown as Event)
	key("Escape")
	expect(editor.root.textContent).not.toContain("Delete constraint")
	expect(editor.root.querySelectorAll('button[aria-pressed="true"]')).toHaveLength(1)
	key("Delete")
	expect(requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")).disabled).toBe(true)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toEqual(original.entities)
	expect(saved?.relations).toEqual(original.relations)
})

it("constrains all selected circles Equal in one undoable operation", () => {
	const original = blank()
	original.entities = [10, 20, 30].map((radius, i) => ({ id: `c${i}`, type: "circle", center: { x: i * 70, y: 0 }, radius, segments: 32 }))
	original.relations = [{ id: "radius", type: "radius", entityId: "c0", value: 10 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	click(editor.root, "Equal")
	expect(editor.root.textContent).toContain("equal · constraint-1")
	expect(editor.root.textContent).toContain("equal · constraint-2")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("equal ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.filter((r) => r.type === "equal")).toHaveLength(2)
	for (const entity of saved?.entities ?? []) {
		if (entity.type !== "circle") throw Error("Missing circle")
		expect(entity.radius).toBeCloseTo(10, 5)
	}
	expect(original.entities[1]).toMatchObject({ radius: 20 })
})

it("aligns all selected lines horizontally or vertically in one history step", () => {
	for (const command of ["Horizontal", "Vertical"]) {
		const original = blank()
		original.entities = [0, 20, 40].map((y, i) => ({ id: `l${i}`, type: "line", p0: { x: 0, y }, p1: { x: 10, y: y + 5 } }))
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			original,
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
		click(editor.root, command)
		expect(editor.root.textContent).toContain(`${command.toLowerCase()} · constraint-3`)
		click(editor.root, "Undo")
		expect(editor.root.textContent).not.toContain("constraint-3")
		click(editor.root, "Redo")
		click(editor.root, "Finish sketch")
		expect(saved?.relations).toHaveLength(3)
		for (const entity of saved?.entities ?? []) {
			if (entity.type !== "line") throw Error("Missing line")
			const axis = command === "Horizontal" ? "y" : "x"
			expect(entity.p0[axis]).toBeCloseTo(entity.p1[axis], 5)
		}
	}
})

it("adds only missing axis constraints and avoids an undo step when all are present", () => {
	const original = blank()
	original.entities = [0, 20].map((y, i) => ({ id: `l${i}`, type: "line", p0: { x: 0, y }, p1: { x: 10, y } }))
	original.relations = [{ id: "existing", type: "horizontal", entityId: "l0" }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const all = () => editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	all()
	click(editor.root, "Horizontal")
	all()
	click(editor.root, "Horizontal")
	expect(editor.root.textContent).toContain("All selected lines already have Horizontal constraints.")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("constraint-1")
	expect(editor.root.textContent).toContain("horizontal · existing")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toHaveLength(2)
	expect(saved?.relations?.[0]?.id).toBe("existing")
})

it("makes three lines parallel without equalizing their lengths", () => {
	const original = blank()
	original.entities = [10, 20, 30].map((length, i) => ({ id: `l${i}`, type: "line", p0: { x: 0, y: i * 30 }, p1: { x: length, y: i * 30 + 5 } }))
	original.relations = [10, 20, 30].map((value, i) => ({ id: `length${i}`, type: "length", entityId: `l${i}`, value }))
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	click(editor.root, "Parallel")
	expect(editor.root.textContent).toContain("parallel · constraint-2")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("parallel ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.filter((r) => r.type === "parallel")).toHaveLength(2)
	const first = saved?.entities[0]
	if (first?.type !== "line") throw Error("Missing first line")
	for (const [i, entity] of (saved?.entities ?? []).entries()) {
		if (entity.type !== "line") throw Error("Missing line")
		const dx = entity.p1.x - entity.p0.x
		const dy = entity.p1.y - entity.p0.y
		expect(Math.hypot(dx, dy)).toBeCloseTo((i + 1) * 10, 5)
		expect(dx * (first.p1.y - first.p0.y) - dy * (first.p1.x - first.p0.x)).toBeCloseTo(0, 4)
	}
})

it("makes circles and elliptical curves concentric as a group while preserving radii", () => {
	const original = blank()
	original.entities = [
		{ id: "a", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 32 },
		{ id: "b", type: "ellipse", center: { x: 40, y: 20 }, width: 40, height: 20, rotation: 25, segments: 32 },
		{ id: "c", type: "arc", center: { x: 80, y: -20 }, radius: 30, startAngle: 0, sweep: Math.PI, segments: 32 },
		{ id: "d", type: "ellipticArc", center: { x: -30, y: 50 }, width: 40, height: 20, rotation: 25, startAngle: 0.3, sweep: -2.1, segments: 32 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	click(editor.root, "Concentric")
	expect(editor.root.textContent).toContain("concentric · constraint-2")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("concentric ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const first = saved?.entities[0]
	if (first?.type !== "circle") throw Error("Missing first circle")
	for (const [i, entity] of (saved?.entities ?? []).entries()) {
		if (entity.type !== "circle" && entity.type !== "arc" && entity.type !== "ellipse" && entity.type !== "ellipticArc") throw Error("Missing curve")
		expect(entity.center.x).toBeCloseTo(first.center.x, 5)
		expect(entity.center.y).toBeCloseTo(first.center.y, 5)
		if (entity.type === "ellipse" || entity.type === "ellipticArc") expect(entity).toMatchObject({ width: 40, height: 20, rotation: 25 })
		else expect(entity.radius).toBeCloseTo((i + 1) * 10, 5)
	}
	expect(saved?.relations).toHaveLength(3)
	const arc = saved?.entities[3]
	expect(arc).toMatchObject({ startAngle: 0.3, sweep: -2.1 })
})
it("rejects incompatible group constraints without partial changes or history", () => {
	for (const command of ["Horizontal", "Vertical", "Equal", "Parallel", "Concentric", "Collinear"]) {
		const original = blank()
		original.entities = [
			{ id: "line", type: "line", p0: { x: 0, y: 0 }, p1: { x: 10, y: 5 } },
			{ id: "circle", type: "circle", center: { x: 30, y: 0 }, radius: 10, segments: 32 }
		]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			original,
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
		click(editor.root, command)
		expect(editor.root.textContent).toContain("Select")
		expect(requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")).disabled).toBe(true)
		click(editor.root, "Finish sketch")
		expect(saved?.entities).toEqual(original.entities)
		expect(saved?.relations).toEqual([])
	}
})

it("makes separated selected lines collinear through the toolbar and history", () => {
	const original = blank()
	original.entities = [0, 20, 40].map((y, i) => ({ id: `l${i}`, type: "line", p0: { x: i * 30, y }, p1: { x: i * 30 + 10, y: y + 5 } }))
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	click(editor.root, "Collinear")
	expect(editor.root.textContent).toContain("collinear · constraint-2")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("collinear ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const a = saved?.entities[0]
	if (a?.type !== "line") throw Error("Missing line")
	const dx = a.p1.x - a.p0.x
	const dy = a.p1.y - a.p0.y
	for (const b of saved?.entities.slice(1) ?? []) {
		if (b.type !== "line") throw Error("Missing line")
		expect((b.p0.x - a.p0.x) * dy - (b.p0.y - a.p0.y) * dx).toBeCloseTo(0, 4)
		expect((b.p1.x - a.p0.x) * dy - (b.p1.y - a.p0.y) * dx).toBeCloseTo(0, 4)
	}
	expect(saved?.relations).toHaveLength(2)
})

it("reuses transitive Equal links and connects each missing group once", () => {
	const original = blank()
	original.entities = [0, 1, 2, 3].map((i) => ({ id: `c${i}`, type: "circle", center: { x: i * 30, y: 0 }, radius: 10, segments: 32 }))
	original.relations = [
		{ id: "first", type: "equal", a: "c1", b: "c0" },
		{ id: "second", type: "equal", a: "c2", b: "c3" }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const all = () => editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	all()
	click(editor.root, "Equal")
	all()
	click(editor.root, "Equal")
	expect(editor.root.textContent).toContain("already connected by Equal")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("equal · constraint-1")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toHaveLength(3)
	expect(saved?.relations).toContainEqual({ id: "constraint-1", type: "equal", a: "c0", b: "c2" })
})

it("aligns two selected points horizontally or vertically while leaving spacing free", () => {
	for (const command of ["Horizontal", "Vertical"]) {
		const original = blank()
		original.entities = [
			{ id: "a", type: "point", center: { x: 0, y: 0 } },
			{ id: "b", type: "point", center: { x: 20, y: 10 } }
		]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			original,
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
		click(editor.root, command)
		expect(editor.root.textContent).toContain("3 degrees of freedom")
		click(editor.root, "Undo")
		expect(editor.root.textContent).toContain("4 degrees of freedom")
		click(editor.root, "Redo")
		click(editor.root, "Finish sketch")
		const a = saved?.entities[0]
		const b = saved?.entities[1]
		if (a?.type !== "point" || b?.type !== "point") throw Error("Missing points")
		const axis = command === "Horizontal" ? "y" : "x"
		expect(a.center[axis]).toBeCloseTo(b.center[axis], 5)
		expect(saved?.relations?.[0]).toMatchObject({ type: "distance", axis, value: 0 })
	}
})

it("aligns a group of points in one undoable operation", () => {
	const original = blank()
	original.entities = [0, 1, 2].map((i) => ({ id: `p${i}`, type: "point", center: { x: i * 20, y: i * 10 } }))
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	click(editor.root, "Vertical")
	expect(editor.root.textContent).toContain("4 degrees of freedom")
	click(editor.root, "Undo")
	expect(editor.root.textContent).toContain("6 degrees of freedom")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const first = saved?.entities[0]
	if (first?.type !== "point") throw Error("Missing point")
	for (const entity of saved?.entities ?? []) {
		if (entity.type !== "point") throw Error("Missing point")
		expect(entity.center.x).toBeCloseTo(first.center.x, 5)
	}
	expect(saved?.relations).toHaveLength(2)
})
it("draws, edits and undoes native fit and control splines without an implicit closing edge", () => {
	for (const tool of ["Fit spline", "Control spline"]) {
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			blank(),
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		click(editor.root, tool)
		const canvas = requireValue(editor.root.querySelector("svg"))
		pointer(canvas, 530, 350)
		pointer(canvas, 560, 290)
		pointer(canvas, 620, 290)
		if (tool === "Control spline") pointer(canvas, 650, 350)
		click(editor.root, "Finish spline")
		const path = requireValue(canvas.querySelector('[data-entity-id="spline-1"]'))
		expect(path.getAttribute("d")).not.toContain("Z")
		expect(editor.root.querySelectorAll("[data-anchor]")).toBeDefined()
		input(editor.root, "Spline point 2 Y (mm)", 30)
		click(editor.root, "Undo")
		click(editor.root, "Redo")
		click(editor.root, "Finish sketch")
		const spline = saved?.entities[0]
		if (spline?.type !== "spline") throw Error("Missing spline")
		expect(spline.mode).toBe(tool === "Fit spline" ? "fit" : "control")
		expect(spline.points[1]?.y).toBeCloseTo(30)
		expect(saved?.profiles).toHaveLength(0)
	}
})
it("cancels incomplete spline placement without adding geometry", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Control spline")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 530, 350)
	pointer(canvas, 560, 290)
	click(editor.root, "Finish sketch")
	expect(saved).toBeUndefined()
	canvas.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as unknown as Event)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(0)
})
it("shows a non-interactive control polygon only for selected control splines", () => {
	const original = blank()
	original.entities = [
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			]
		}
	]
	const editor = new SketchWorkspace(
		original,
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	const selector = '[data-spline-control-polygon="s"]'
	expect(editor.root.querySelector(selector)).toBeNull()
	click(editor.root, "Select s")
	const before = requireValue(editor.root.querySelector(selector))
	expect(before.getAttribute("pointer-events")).toBe("none")
	expect(before.getAttribute("d")?.match(/L /g)).toHaveLength(3)
	expect(before.getAttribute("d")).not.toContain("Z")
	const path = before.getAttribute("d")
	input(editor.root, "Spline point 2 X (mm)", 3)
	expect(editor.root.querySelector(selector)?.getAttribute("d")).not.toBe(path)
	click(editor.root, "Undo")
	click(editor.root, "Select s")
	expect(editor.root.querySelector(selector)?.getAttribute("d")).toBe(path)
	const canvas = requireValue(editor.root.querySelector("svg"))
	canvas.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as unknown as Event)
	expect(editor.root.querySelector(selector)).toBeNull()
	expect(original.entities[0]).toMatchObject({
		points: [
			{ x: 0, y: 0 },
			{ x: 0, y: 10 },
			{ x: 10, y: 10 },
			{ x: 10, y: 0 }
		]
	})
	editor.dispose()
	const fit = structuredClone(original)
	const spline = fit.entities[0]
	if (spline?.type !== "spline") throw Error("Missing spline")
	spline.mode = "fit"
	const fitEditor = new SketchWorkspace(
		fit,
		() => undefined,
		() => undefined
	)
	document.body.append(fitEditor.root)
	click(fitEditor.root, "Select s")
	expect(fitEditor.root.querySelector(selector)).toBeNull()
	fitEditor.dispose()
})
it("drags native spline handles with fixed endpoints and groups the drag into one history step", () => {
	for (const mode of ["fit", "control"] as const) {
		const original = blank()
		original.entities = [
			{
				id: "s",
				type: "spline",
				mode,
				points: [
					{ x: 0, y: 0 },
					{ x: 0, y: 10 },
					{ x: 10, y: 10 },
					{ x: 10, y: 0 }
				]
			}
		]
		original.relations = [
			{ id: "start", type: "fixed", anchor: { entityId: "s", point: "point0" }, position: { x: 0, y: 0 } },
			{ id: "end", type: "fixed", anchor: { entityId: "s", point: "point3" }, position: { x: 10, y: 0 } }
		]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			original,
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		click(editor.root, "Select s")
		const canvas = requireValue(editor.root.querySelector("svg"))
		const handle = requireValue(canvas.querySelector('[aria-label="s point1"]'))
		const x = Number(handle.getAttribute("cx"))
		const y = Number(handle.getAttribute("cy"))
		pointer(handle, x, y)
		for (const offset of [20, 40]) canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + offset, clientY: y + offset }) as unknown as Event)
		canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
		const changed = Number(editor.root.querySelector<HTMLInputElement>('[aria-label="Spline point 2 X (mm)"]')?.value)
		expect(changed).toBeGreaterThan(0)
		click(editor.root, "Undo")
		click(editor.root, "Select s")
		expect(Number(editor.root.querySelector<HTMLInputElement>('[aria-label="Spline point 2 X (mm)"]')?.value)).toBe(0)
		expect(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")?.disabled).toBe(true)
		click(editor.root, "Redo")
		click(editor.root, "Finish sketch")
		const spline = saved?.entities[0]
		if (spline?.type !== "spline") throw Error("Missing spline")
		expect(spline.mode).toBe(mode)
		expect(spline.points[1]?.x).toBeCloseTo(changed, 4)
		expect(spline.points[0]).toEqual({ x: 0, y: 0 })
		expect(spline.points[3]).toEqual({ x: 10, y: 0 })
		expect(saved?.relations).toEqual(original.relations)
		expect(original.entities[0]).toMatchObject({
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			]
		})
	}
})
it("rejects fixed spline handle motion and cancels free handle motion without recording history", () => {
	const original = blank()
	original.entities = [
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			]
		}
	]
	original.relations = [{ id: "fixed", type: "fixed", anchor: { entityId: "s", point: "point0" }, position: { x: 0, y: 0 } }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		original,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	const canvas = requireValue(editor.root.querySelector("svg"))
	for (const name of ["point0", "point1"]) {
		const handle = requireValue(canvas.querySelector(`[aria-label="s ${name}"]`))
		const x = Number(handle.getAttribute("cx"))
		const y = Number(handle.getAttribute("cy"))
		pointer(handle, x, y)
		canvas.dispatchEvent(new dom.PointerEvent("pointermove", { bubbles: true, clientX: x + 50, clientY: y + 30 }) as unknown as Event)
		if (name === "point0") {
			expect(editor.root.textContent).toContain("That point is constrained")
			canvas.dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
		} else canvas.dispatchEvent(new dom.KeyboardEvent("keydown", { bubbles: true, key: "Escape" }) as unknown as Event)
		expect(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")?.disabled).toBe(true)
	}
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toEqual(original.entities)
	expect(saved?.relations).toEqual(original.relations)
})

it("splits a spline from a canvas click and restores it with Undo", () => {
	const source = blank()
	source.entities = [
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			]
		}
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const path = requireValue(editor.root.querySelector('[data-entity-id="s"]'))
	const coordinates =
		requireValue(path.getAttribute("d"))
			.match(/-?\d+(?:\.\d+)?(?:e[+-]?\d+)?/gi)
			?.map(Number) ?? []
	const middle = Math.floor(coordinates.length / 4) * 2
	click(editor.root, "Split")
	pointer(requireValue(editor.root.querySelector('[data-entity-id="s"]')), requireValue(coordinates[middle]), requireValue(coordinates[middle + 1]))
	expect(editor.root.querySelector('[data-entity-id="s-split-1"]')).not.toBeNull()
	click(editor.root, "Undo")
	expect(editor.root.querySelector('[data-entity-id="s-split-1"]')).toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(2)
	expect(saved?.relations?.some((r) => r.type === "coincident")).toBe(true)
	expect(source.entities).toHaveLength(1)
	editor.dispose()
})
it("adds and undoes a point-on-spline relation through the sketch controls", () => {
	const source = blank()
	source.entities = [
		{ id: "p", type: "point", center: { x: 5, y: 12 } },
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			]
		}
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select p")
	const curve = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select s"))
	curve.dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	click(editor.root, "Point on curve")
	expect(editor.root.textContent).toContain("Point on curve ·")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("Point on curve ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "pointOnCurve", a: { entityId: "p", point: "center" }, b: "s" }))
	expect(source.relations ?? []).toEqual([])
	editor.dispose()
})
it("creates and undoes a normal relation to a spline through the toolbar", () => {
	const source = blank()
	source.entities = [
		{ id: "line", type: "line", p0: { x: 5, y: 9 }, p1: { x: 6, y: 20 } },
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 10 },
				{ x: 10, y: 10 },
				{ x: 10, y: 0 }
			]
		}
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select line")
	const curve = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select s"))
	curve.dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	click(editor.root, "Normal")
	expect(editor.root.textContent).toContain("normal ·")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("normal ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "normal", a: { entityId: "line", point: "p0" }, b: "s" }))
	expect(source.relations ?? []).toEqual([])
	editor.dispose()
})
it("rolls back a rejected spline-join normal without adding undo history", () => {
	const source = blank()
	source.entities = [
		{ id: "line", type: "line", p0: { x: 3, y: 0 }, p1: { x: 3, y: -5 } },
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 2, y: 0 },
				{ x: 3, y: 0 },
				{ x: 3, y: 1 },
				{ x: 3, y: 2 },
				{ x: 3, y: 3 }
			]
		}
	]
	const original = structuredClone(source)
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select line")
	const curve = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select s"))
	curve.dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	click(editor.root, "Normal")
	expect(editor.root.textContent).toContain("smooth spline join")
	expect(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")?.disabled).toBe(true)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toEqual(original.entities)
	expect(saved?.relations ?? []).toEqual([])
	expect(source).toEqual(original)
	editor.dispose()
})
it("creates a spline tangent join from selected endpoints and undoes it", () => {
	const source = blank()
	source.entities = [
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 2, y: 1 },
				{ x: 3, y: 1 }
			]
		},
		{ id: "l", type: "line", p0: { x: 3, y: 1 }, p1: { x: 6, y: 2 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	const second = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select l"))
	second.dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	const firstHandle = requireValue(editor.root.querySelector('[aria-label="s point3"]'))
	firstHandle.dispatchEvent(new dom.PointerEvent("pointerdown", { bubbles: true, button: 0, shiftKey: true }) as unknown as Event)
	requireValue(editor.root.querySelector("svg")).dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	const lastHandle = requireValue(editor.root.querySelector('[aria-label="l p0"]'))
	lastHandle.dispatchEvent(new dom.PointerEvent("pointerdown", { bubbles: true, button: 0, shiftKey: true }) as unknown as Event)
	requireValue(editor.root.querySelector("svg")).dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	click(editor.root, "Tangent")
	expect(editor.root.textContent).toContain("Tangent join ·")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("Tangent join ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "smoothJoin", a: { entityId: "s", point: "point3" }, b: { entityId: "l", point: "p0" } }))
	editor.dispose()
})
it("starts a tangent arc from a spline endpoint in the canvas", () => {
	const source = blank()
	source.entities = [
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 10, y: 0 },
				{ x: 20, y: 10 },
				{ x: 30, y: 10 }
			]
		}
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	const handle = requireValue(editor.root.querySelector('[aria-label="s point3"]'))
	const x = Number(handle.getAttribute("cx"))
	const y = Number(handle.getAttribute("cy"))
	click(editor.root, "Tangent arc")
	pointer(requireValue(editor.root.querySelector("svg")), x, y)
	pointer(requireValue(editor.root.querySelector("svg")), x + 40, y - 40)
	click(editor.root, "Finish sketch")
	expect(saved?.entities.some((e) => e.type === "arc")).toBe(true)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "smoothJoin", a: { entityId: "s", point: "point3" } }))
	expect(source.entities).toHaveLength(1)
	editor.dispose()
})
it("cancels tangent arc placement from a spline without changing geometry or history", () => {
	const source = blank()
	source.entities = [
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 10, y: 0 },
				{ x: 20, y: 10 },
				{ x: 30, y: 10 }
			]
		}
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	const handle = requireValue(editor.root.querySelector('[aria-label="s point3"]'))
	const x = Number(handle.getAttribute("cx"))
	const y = Number(handle.getAttribute("cy"))
	click(editor.root, "Tangent arc")
	pointer(requireValue(editor.root.querySelector("svg")), x, y)
	document.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as unknown as Event)
	expect(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")?.disabled).toBe(true)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toEqual(source.entities)
	expect(saved?.relations ?? []).toEqual([])
	editor.dispose()
})
it("reports a stationary spline endpoint immediately when starting a tangent arc", () => {
	const source = blank()
	source.entities = [
		{
			id: "s",
			type: "spline",
			mode: "control",
			points: [
				{ x: 0, y: 0 },
				{ x: 0, y: 0 },
				{ x: 20, y: 10 },
				{ x: 30, y: 10 }
			]
		}
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	const handle = requireValue(editor.root.querySelector('[aria-label="s point0"]'))
	const x = Number(handle.getAttribute("cx"))
	const y = Number(handle.getAttribute("cy"))
	click(editor.root, "Tangent arc")
	pointer(requireValue(editor.root.querySelector("svg")), x, y)
	expect(editor.root.textContent).toContain("nonzero spline endpoint tangent")
	expect(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")?.disabled).toBe(true)
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toEqual(source.entities)
	expect(saved?.relations ?? []).toEqual([])
	editor.dispose()
})
it("draws a diameter circle from opposite points with native geometry and undo", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Diameter circle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 470, 350)
	pointer(canvas, 530, 350)
	expect(canvas.querySelector('[data-entity-id="circle-1"]')).not.toBeNull()
	click(editor.root, "Undo")
	expect(canvas.querySelector('[data-entity-id="circle-1"]')).toBeNull()
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ type: "circle", center: { x: expect.closeTo(0, 5), y: expect.closeTo(0, 5) }, radius: expect.closeTo(10, 5) })
	expect(saved?.profiles).toHaveLength(1)
	editor.dispose()
})
it("creates diagonal diameter circles as construction geometry and ignores coincident picks", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const construction = requireValue(Array.from(editor.root.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')).find((e) => e.parentElement?.textContent === "Construction"))
	construction.click()
	click(editor.root, "Diameter circle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 470, 320)
	pointer(canvas, 470, 320)
	expect(canvas.querySelector('[data-entity-id="circle-1"]')).toBeNull()
	pointer(canvas, 470, 320)
	pointer(canvas, 530, 380)
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({ type: "circle", construction: true, center: { x: expect.closeTo(0, 5), y: expect.closeTo(0, 5) }, radius: expect.closeTo(Math.sqrt(200), 5) })
	expect(saved?.profiles).toHaveLength(0)
	editor.dispose()
})
it("retains incidence constraints at snapped diameter-circle picks", async () => {
	const source = blank()
	source.entities = [
		{ id: "a", type: "point", center: { x: -10, y: 0 } },
		{ id: "b", type: "point", center: { x: 10, y: 0 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const coordinates: number[][] = []
	for (const id of ["a", "b"]) {
		click(editor.root, `Select ${id}`)
		const handle = requireValue(editor.root.querySelector(`[aria-label="${id} center"]`))
		coordinates.push([Number(handle.getAttribute("cx")), Number(handle.getAttribute("cy"))])
	}
	click(editor.root, "Diameter circle")
	for (const point of coordinates) pointer(requireValue(editor.root.querySelector("svg")), requireValue(point[0]), requireValue(point[1]))
	click(editor.root, "Finish sketch")
	for (const id of ["a", "b"]) expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "pointOnCurve", a: { entityId: id, point: "center" }, b: "circle-1" }))
	expect(source.entities).toHaveLength(2)
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const { createProjectFile, normalizeProjectFile, serializeProjectFile } = await import("../src/project-file")
	const { solveSketch } = await import("../src/sketch-solver")
	const sketch = requireValue(saved)
	const runtime = createPartRuntimeState({ features: [sketch] })
	const file = createProjectFile({
		items: [{ id: "p", type: "part", name: "Diameter references", data: { features: [sketch], cad: serializePCadState(runtime.cad), tree: runtime.tree } }],
		selectedPath: null
	})
	const restored = requireValue(normalizeProjectFile(JSON.parse(serializeProjectFile(file))))
	const part = restored.items[0] as import("../src/contract").ProjectPartDocument
	const state = createPartRuntimeState(requireValue(part.data))
	const loaded = materializePartFeatures(state.cad, state.tree)[0]
	if (loaded?.type !== "sketch") throw Error("Missing sketch")
	expect(loaded.relations).toEqual(sketch.relations)
	expect(loaded.relations?.some((r) => r.type === "fixed")).toBe(false)
	const positions = [
		{ x: -12, y: 5 },
		{ x: 5, y: 12 }
	]
	const solved = solveSketch(loaded.entities, [
		...(loaded.relations ?? []),
		...positions.map((position, i): import("../src/sketch-solver").SketchRelation => ({
			id: `fix${i}`,
			type: "fixed",
			anchor: { entityId: i === 0 ? "a" : "b", point: "center" },
			position
		}))
	])
	expect(solved.status).not.toBe("conflicting")
	expect(
		solveSketch(loaded.entities, [
			...(loaded.relations ?? []),
			{ id: "move-a", type: "fixed", anchor: { entityId: "a", point: "center" }, position: { x: -15, y: 2 } },
			{ id: "move-b", type: "fixed", anchor: { entityId: "b", point: "center" }, position: { x: 12, y: 8 } }
		]).status
	).not.toBe("conflicting")
	const circle = solved.entities.find((e) => e.type === "circle")
	if (circle?.type !== "circle") throw Error("Missing circle")
	for (const point of positions) expect(Math.hypot(point.x - circle.center.x, point.y - circle.center.y)).toBeCloseTo(circle.radius, 5)

	editor.dispose()
})
it("does not infer circle incidence when geometry snapping or automatic constraints is disabled", () => {
	for (const tool of ["Diameter circle", "3-point circle"])
		for (const disabled of ["Snap to geometry", "Automatic constraints"]) {
			const source = blank()
			source.entities = [
				{ id: "a", type: "point", center: { x: -10, y: 0 } },
				{ id: "b", type: "point", center: { x: 10, y: 0 } },
				{ id: "c", type: "point", center: { x: 0, y: 10 } }
			]
			let saved: Sketch | undefined
			const editor = new SketchWorkspace(
				source,
				(s) => {
					saved = s
				},
				() => undefined
			)
			document.body.append(editor.root)
			const coordinates: number[][] = []
			for (const id of tool === "Diameter circle" ? ["a", "b"] : ["a", "b", "c"]) {
				click(editor.root, `Select ${id}`)
				const handle = requireValue(editor.root.querySelector(`[aria-label="${id} center"]`))
				coordinates.push([Number(handle.getAttribute("cx")), Number(handle.getAttribute("cy"))])
			}
			const label = requireValue(Array.from(editor.root.querySelectorAll("label")).find((l) => l.textContent === disabled))
			requireValue(label.querySelector("input")).click()
			click(editor.root, tool)
			for (const point of coordinates) pointer(requireValue(editor.root.querySelector("svg")), requireValue(point[0]), requireValue(point[1]))
			click(editor.root, "Finish sketch")
			expect(saved?.entities.some((e) => e.type === "circle")).toBe(true)
			expect(saved?.relations ?? []).toEqual([])
			editor.dispose()
		}
})
it("infers an origin constraint only for an explicitly picked circle center", () => {
	for (const tool of ["Circle", "Diameter circle", "3-point circle"]) {
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			blank(),
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		click(editor.root, tool)
		const canvas = requireValue(editor.root.querySelector("svg"))
		const points =
			tool === "Circle"
				? [
						[500, 350],
						[530, 350]
					]
				: tool === "Diameter circle"
					? [
							[470, 350],
							[530, 350]
						]
					: [
							[470, 350],
							[530, 350],
							[500, 320]
						]
		for (const point of points) pointer(canvas, requireValue(point[0]), requireValue(point[1]))
		click(editor.root, "Finish sketch")
		expect(saved?.entities[0]).toMatchObject({ type: "circle", center: { x: expect.closeTo(0, 5), y: expect.closeTo(0, 5) }, radius: expect.closeTo(10, 5) })
		if (tool === "Circle") expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "fixed", anchor: { entityId: "circle-1", point: "center" }, position: { x: 0, y: 0 } }))
		else expect(saved?.relations ?? []).toEqual([])
		editor.dispose()
	}
})
it("draws a rotated rectangle about a picked center", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "3-point center rectangle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 530, 320)
	pointer(canvas, 500, 290)
	click(editor.root, "Finish sketch")
	expect(saved?.entities[0]).toMatchObject({
		type: "rectangle",
		center: { x: expect.closeTo(0, 5), y: expect.closeTo(0, 5) },
		width: expect.closeTo(20 * Math.SQRT2, 5),
		height: expect.closeTo(20 * Math.SQRT2, 5),
		rotation: expect.closeTo(45, 5)
	})
	expect(saved?.profiles).toHaveLength(1)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "fixed", anchor: { entityId: "rectangle-1", point: "center" } }))
	editor.dispose()
})
it("recovers from a flat centered rectangle pick and records only the completed shape", () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "3-point center rectangle")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	pointer(canvas, 530, 320)
	pointer(canvas, 560, 290)
	expect(editor.root.textContent).toContain("set the height")
	expect(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")?.disabled).toBe(true)
	pointer(canvas, 500, 290)
	expect(canvas.querySelector('[data-entity-id="rectangle-1"]')).not.toBeNull()
	click(editor.root, "Undo")
	expect(canvas.querySelector('[data-entity-id="rectangle-1"]')).toBeNull()
	expect(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")?.disabled).toBe(true)
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities).toHaveLength(1)
	expect(saved?.profiles).toHaveLength(1)
	editor.dispose()
})
it("shows placement instructions for the selected diameter and centered rectangle tools", () => {
	const editor = new SketchWorkspace(
		blank(),
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Diameter circle")
	expect(editor.root.querySelector('[aria-label="Sketch tool instructions"]')?.textContent).toContain("two opposite points")
	click(editor.root, "3-point center rectangle")
	expect(editor.root.querySelector('[aria-label="Sketch tool instructions"]')?.textContent).toContain("midpoint of one side")
	click(editor.root, "Select")
	expect(editor.root.querySelector('[aria-label="Sketch tool instructions"]')?.textContent).toContain("Shift-click")
	editor.dispose()
})

it("groups tools into compact menus while retaining search and direct sketch actions", () => {
	const editor = new SketchWorkspace(
		blank(),
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	const tools = requireValue(editor.root.querySelector('[aria-label="Sketch tools"]'))
	expect(Array.from(tools.querySelectorAll("summary")).map((item) => item.textContent)).toEqual(["Draw", "Edit", "Constraints", "Options"])
	expect(tools.querySelector('[aria-label="Draw tools"] button')?.textContent).toBe("Fit spline")
	expect(tools.querySelector('[aria-label="Constraints tools"]')?.textContent).toContain("Coincident")
	expect(tools.querySelector('[aria-label="Options tools"]')?.textContent).toContain("Snap to geometry")
	click(editor.root, "Diameter circle")
	expect(tools.querySelector("summary")?.textContent).toBe("Draw · Diameter circle")
	for (const [name, group] of [
		["Trim", "Edit"],
		["Split", "Edit"],
		["Extend", "Edit"],
		["Dimension", "Constraints"]
	]) {
		const panel = requireValue(tools.querySelector(`[aria-label="${group} tools"]`))
		expect(Array.from(panel.querySelectorAll("button")).some((button) => button.textContent === name)).toBe(true)
		click(editor.root, requireValue(name))
		expect(Array.from(tools.querySelectorAll("summary")).map((item) => item.textContent)).toEqual([
			"Draw",
			group === "Edit" ? `Edit · ${name}` : "Edit",
			group === "Constraints" ? `Constraints · ${name}` : "Constraints",
			"Options"
		])
	}
	const finish = Array.from(tools.children).find((item) => item.textContent === "Finish sketch")
	expect(finish?.tagName).toBe("BUTTON")
	editor.dispose()
})

it("dismisses tool menus outside their panel and returns keyboard focus after tool selection", () => {
	const editor = new SketchWorkspace(
		blank(),
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	const menus = () => Array.from(editor.root.querySelectorAll<HTMLDetailsElement>('[aria-label="Sketch tools"] details'))
	const draw = requireValue(menus()[0])
	draw.open = true
	pointer(requireValue(draw.querySelector("button")), 0, 0)
	expect(draw.open).toBe(true)
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, 500, 350)
	expect(draw.open).toBe(false)
	draw.open = true
	click(editor.root, "Circle")
	expect(document.activeElement).toBe(canvas)
	const options = requireValue(menus()[3])
	options.open = true
	const summary = requireValue(options.querySelector("summary"))
	summary.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as unknown as Event)
	expect(options.open).toBe(false)
	expect(document.activeElement).toBe(summary)
	expect(editor.root.querySelector("summary")?.textContent).toBe("Draw · Circle")
	editor.dispose()
})

it("constrains a center rectangle symmetric to a corner rectangle in either selection order", async () => {
	for (const order of [
		["a", "b"],
		["b", "a"]
	]) {
		const source = blank()
		source.entities = [
			{ id: "a", type: "cornerRectangle", p0: v2(4, 2), p1: v2(10, 6) },
			{ id: "b", type: "rectangle", center: v2(-7, 4), width: 6, height: 4, rotation: 180 },
			{ id: "axis", type: "line", p0: v2(0, 0), p1: v2(0, 10), construction: true }
		]
		source.relations = [
			{ id: "a0", type: "fixed", anchor: { entityId: "a", point: "p0" }, position: v2(4, 2) },
			{ id: "a1", type: "fixed", anchor: { entityId: "a", point: "p1" }, position: v2(10, 6) },
			{ id: "axis0", type: "fixed", anchor: { entityId: "axis", point: "p0" }, position: v2(0, 0) },
			{ id: "axis1", type: "fixed", anchor: { entityId: "axis", point: "p1" }, position: v2(0, 10) }
		]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			source,
			(next) => {
				saved = next
			},
			() => undefined
		)
		document.body.append(editor.root)
		click(editor.root, `Select ${order[0]}`)
		for (const id of [order[1], "axis"])
			requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === `Select ${id}`)).dispatchEvent(
				new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
			)
		click(editor.root, "Symmetric entities")
		click(editor.root, "Undo")
		click(editor.root, "Redo")
		click(editor.root, "Finish sketch")
		const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
		const runtime = createPartRuntimeState({ features: [requireValue(saved)] })
		const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
		const sketch = materializePartFeatures(restored.cad, restored.tree)[0]
		if (sketch?.type !== "sketch") throw Error("Missing restored sketch")
		expect(sketch.relations).toContainEqual(expect.objectContaining({ type: "mirror", a: "a", b: "b", symmetryLine: "axis" }))
		const { solveSketch } = await import("../src/sketch-solver")
		const relations = requireValue(sketch.relations).map((r) => (r.id === "a1" && r.type === "fixed" ? { ...r, position: v2(14, 8) } : r))
		const result = solveSketch(sketch.entities, relations)
		expect(result.status).toBe("fully-constrained")
		expect(result.entities[1]).toMatchObject({
			type: "rectangle",
			center: { x: expect.closeTo(-9, 5), y: expect.closeTo(5, 5) },
			width: expect.closeTo(10, 5),
			height: expect.closeTo(6, 5)
		})
	}
})

it("closes a fit spline into a native periodic profile and preserves closure on reload", async () => {
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		blank(),
		(next) => {
			saved = next
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Fit spline")
	const canvas = requireValue(editor.root.querySelector("svg"))
	for (const [x, y] of [
		[440, 350],
		[500, 290],
		[560, 350],
		[500, 410]
	])
		pointer(canvas, requireValue(x), requireValue(y))
	click(editor.root, "Close spline")
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const sketch = requireValue(saved)
	expect(sketch.entities[0]).toMatchObject({ type: "spline", mode: "fit", closed: true })
	expect(sketch.profiles).toHaveLength(1)
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing sketch")
	expect(result.entities[0]).toMatchObject({ closed: true })
	expect(result.profiles).toHaveLength(1)
})

it("closes and reopens an existing constrained fit spline with transactional history", () => {
	const source = blank()
	source.entities = [{ id: "s", type: "spline", mode: "fit", points: [v2(-20, 0), v2(0, 20), v2(20, 0), v2(0, -20)] }]
	source.relations = [{ id: "fixed", type: "fixed", anchor: { entityId: "s", point: "point0" }, position: v2(-20, 0) }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(next) => {
			saved = next
		},
		() => undefined
	)
	document.body.append(editor.root)
	const toggle = () => {
		click(editor.root, "Select s")
		const label = requireValue(Array.from(editor.root.querySelectorAll("label")).find((item) => item.textContent === "Closed spline"))
		requireValue(label.querySelector("input")).click()
	}
	toggle()
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	toggle()
	click(editor.root, "Undo")
	click(editor.root, "Finish sketch")
	expect(requireValue(saved).entities[0]).toMatchObject({ closed: true, points: [v2(-20, 0), v2(0, 20), v2(20, 0), v2(0, -20)] })
	expect(requireValue(saved).relations).toEqual(source.relations)
	expect(requireValue(saved).profiles).toHaveLength(1)
	expect(source.entities[0]).not.toHaveProperty("closed")
})

it("rejects closing a two-point fit spline without adding an undo entry", () => {
	const source = blank()
	source.entities = [{ id: "s", type: "spline", mode: "fit", points: [v2(-20, 0), v2(20, 0)] }]
	const editor = new SketchWorkspace(
		source,
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	const label = requireValue(Array.from(editor.root.querySelectorAll("label")).find((item) => item.textContent === "Closed spline"))
	requireValue(label.querySelector("input")).click()
	expect(editor.root.querySelector('[role="status"]')?.textContent).toContain("3–4096")
	expect(requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")).disabled).toBe(true)
	editor.dispose()
})

it("splits a periodic spline into one open chain with independent coincident endpoints", () => {
	const source = blank()
	source.entities = [{ id: "s", type: "spline", mode: "fit", closed: true, points: [v2(20, 0), v2(0, 20), v2(-20, 0), v2(0, -20)] }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(next) => {
			saved = next
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	click(editor.root, "Split")
	const handle = requireValue(editor.root.querySelector('[aria-label="s point0"]'))
	pointer(handle, 560, 350)
	click(editor.root, "Undo")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const result = requireValue(saved)
	expect(result.entities).toHaveLength(1)
	const spline = result.entities[0]
	if (spline?.type !== "spline") throw Error("Missing spline")
	expect(spline.closed).toBe(false)
	expect(spline.mode).toBe("control")
	expect(spline.points[0]).toEqual(spline.points.at(-1))
	expect(result.relations).toHaveLength(0)
	expect(source.entities[0]).toMatchObject({ closed: true })
})

it("persists both inferred spline intersection references and follows a moved crossing", async () => {
	const source = blank()
	source.entities = [
		{ id: "a", type: "spline", mode: "fit", points: [v2(0, 0), v2(10, 10)] },
		{ id: "b", type: "spline", mode: "fit", points: [v2(0, 10), v2(10, 0)] }
	]
	source.relations = source.entities.flatMap((entity) =>
		entity.type === "spline"
			? entity.points.map((position, i): import("../src/sketch-solver").SketchRelation => ({
					id: `${entity.id}-${i}`,
					type: "fixed",
					anchor: { entityId: entity.id, point: `point${i}` },
					position
				}))
			: []
	)
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(next) => {
			saved = next
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Point")
	pointer(requireValue(editor.root.querySelector("svg")), 500, 350)
	click(editor.root, "Finish sketch")
	const sketch = requireValue(saved)
	expect(sketch.relations?.filter((r) => r.type === "pointOnCurve")).toHaveLength(2)
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [sketch] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const loaded = materializePartFeatures(restored.cad, restored.tree)[0]
	if (loaded?.type !== "sketch") throw Error("Missing sketch")
	const { solveSketch } = await import("../src/sketch-solver")
	const changed = requireValue(loaded.relations).map((r) => (r.id === "a-1" && r.type === "fixed" ? { ...r, position: v2(20, 10) } : r))
	const result = solveSketch(loaded.entities, changed)
	expect(result.status).toBe("fully-constrained")
	const marker = result.entities.find((e) => e.type === "point")
	if (marker?.type !== "point") throw Error("Missing intersection point")
	expect(marker.center.x).toBeCloseTo(20 / 3, 5)
	expect(marker.center.y).toBeCloseTo(10 / 3, 5)
})

it("explains spline completion, closure and trim behavior for the active tool", () => {
	const editor = new SketchWorkspace(
		blank(),
		() => undefined,
		() => undefined
	)
	document.body.append(editor.root)
	const help = () => editor.root.querySelector('[aria-label="Sketch tool instructions"]')?.textContent
	click(editor.root, "Fit spline")
	expect(help()).toContain("Close spline creates a smooth loop")
	click(editor.root, "Control spline")
	expect(help()).toContain("two control handles")
	click(editor.root, "Trim")
	expect(help()).toContain("Without crossings, Trim removes the whole curve")
	click(editor.root, "Split")
	expect(help()).toContain("opens it at the clicked point")
	expect(help()).toContain("circle or ellipse, pick two")
	click(editor.root, "Extend")
	expect(help()).toContain("elliptic arc")
	expect(help()).toContain("Spline extension is not available")
	editor.dispose()
})

it("retains dimension arithmetic through editing history and PCad reload", async () => {
	const { createPartRuntimeState, materializePartFeatures, serializePCadState } = await import("../src/pcad/part-state")
	const source = blank()
	source.entities = [{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 64 }]
	source.relations = [{ id: "diameter", type: "diameter", entityId: "circle", value: 20 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "diameter 20 · diameter")
	const field = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Dimension (mm)"]'))
	field.value = "(1 in + 4.6 mm) / 2"
	field.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(editor.root.querySelector<HTMLInputElement>('[aria-label="Dimension (mm)"]')?.value).toBe("(1 in + 4.6 mm) / 2")
	click(editor.root, "Undo")
	click(editor.root, "diameter 20 · diameter")
	expect(editor.root.querySelector<HTMLInputElement>('[aria-label="Dimension (mm)"]')?.value).toBe("20")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const result = requireValue(saved)
	expect(result.relations?.[0]).toMatchObject({ value: 15, expression: "(1 in + 4.6 mm) / 2" })
	const state = createPartRuntimeState({ features: [result] })
	const restored = createPartRuntimeState({ features: [], cad: serializePCadState(state.cad), tree: state.tree })
	const loaded = materializePartFeatures(restored.cad, restored.tree)[0]
	if (loaded?.type !== "sketch") throw Error("Missing sketch")
	expect(loaded.relations).toEqual(result.relations)
	const reopened = new SketchWorkspace(
		loaded,
		() => undefined,
		() => undefined
	)
	document.body.append(reopened.root)
	click(reopened.root, "diameter 15 · diameter")
	expect(reopened.root.querySelector<HTMLInputElement>('[aria-label="Dimension (mm)"]')?.value).toBe("(1 in + 4.6 mm) / 2")
	reopened.dispose()
})

it("authors a variable, binds a dimension and updates geometry transactionally", () => {
	const source = blank()
	source.entities = [{ id: "circle", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 64 }]
	source.relations = [{ id: "diameter", type: "diameter", entityId: "circle", value: 20 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const text = (label: string, value: string) => {
		const field = requireValue(editor.root.querySelector<HTMLInputElement>(`[aria-label="${label}"]`))
		field.value = value
		field.dispatchEvent(new dom.Event("change") as unknown as Event)
	}
	click(editor.root, "Add variable")
	text("Variable variable1 name", "radius")
	click(editor.root, "diameter 20 · diameter")
	text("Dimension (mm)", "#radius * 2")
	text("Variable radius name", "size")
	expect(editor.root.querySelector<HTMLInputElement>('[aria-label="Dimension (mm)"]')?.value).toBe("#size * 2")
	text("Variable size expression", "2 cm")
	expect(editor.root.textContent).toContain("diameter 40")
	click(editor.root, "Delete #size")
	expect(editor.root.textContent).toContain("Unknown sketch variable: #size")
	expect(editor.root.querySelector<HTMLInputElement>('[aria-label="Variable size expression"]')?.value).toBe("2 cm")
	click(editor.root, "Undo")
	expect(editor.root.textContent).toContain("diameter 20")
	click(editor.root, "Redo")
	expect(editor.root.textContent).toContain("diameter 40")
	text("Variable size expression", "-1 mm")
	expect(editor.root.querySelector<HTMLInputElement>('[aria-label="Variable size expression"]')?.value).toBe("2 cm")
	click(editor.root, "Finish sketch")
	const result = requireValue(saved)
	expect(result.variables).toEqual([{ name: "size", kind: "length", expression: "2 cm" }])
	expect(result.relations?.[0]).toMatchObject({ value: 40, expression: "#size * 2" })
	const circle = result.entities[0]
	if (circle?.type !== "circle") throw Error("Missing circle")
	expect(circle.radius).toBeCloseTo(20, 6)
	expect(source.variables).toBeUndefined()
})

it("binds a canvas dimension to a variable even when the numeric value is unchanged", () => {
	const source = blank()
	source.entities = [{ id: "c", type: "circle", center: { x: 0, y: 0 }, radius: 10, segments: 32 }]
	source.variables = [{ name: "radius", kind: "length", expression: "10 mm" }]
	source.relations = [{ id: "diam", type: "diameter", entityId: "c", value: 20 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	const open = () => {
		requireValue(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')).dispatchEvent(new dom.MouseEvent("dblclick", { bubbles: true }) as unknown as Event)
		return requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Edit dimension value"]'))
	}
	const enter = (field: HTMLInputElement, value: string) => {
		field.value = value
		field.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Enter", bubbles: true }) as unknown as Event)
	}
	let field = open()
	enter(field, "#missing * 2")
	expect(field.isConnected).toBe(true)
	expect(editor.root.querySelector('[data-dimension-editor] [role="alert"]')?.textContent).toContain("Unknown sketch variable")
	enter(field, "#radius * 2")
	expect(field.isConnected).toBe(false)
	field = open()
	expect(field.value).toBe("#radius * 2")
	field.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as unknown as Event)
	click(editor.root, "Undo")
	field = open()
	expect(field.value).toBe("20")
	field.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "Escape", bubbles: true }) as unknown as Event)
	click(editor.root, "Redo")
	const variable = requireValue(editor.root.querySelector<HTMLInputElement>('[aria-label="Variable radius expression"]'))
	variable.value = "15 mm"
	variable.dispatchEvent(new dom.Event("change") as unknown as Event)
	expect(editor.root.querySelector('svg text[aria-label^="Constraint diam:"]')?.textContent).toBe("Ø30")
	click(editor.root, "Finish sketch")
	expect(saved?.relations?.[0]).toMatchObject({ value: 30, expression: "#radius * 2" })
	expect(source.relations?.[0]?.expression).toBeUndefined()
})

it("splits an ellipse from two native handles and restores it with Undo", () => {
	const source = blank()
	source.entities = [{ id: "ellipse", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, segments: 128 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select ellipse")
	click(editor.root, "Split")
	for (const name of ["p0", "p2"]) {
		const handle = requireValue(editor.root.querySelector(`[aria-label="ellipse ${name}"]`))
		pointer(handle, Number(handle.getAttribute("cx")), Number(handle.getAttribute("cy")))
	}
	expect(editor.root.querySelectorAll("[data-entity-id]")).toHaveLength(2)
	click(editor.root, "Undo")
	expect(editor.root.querySelectorAll("[data-entity-id]")).toHaveLength(1)
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.entities.map((e) => e.type)).toEqual(["ellipticArc", "ellipticArc"])
	expect(saved?.profiles).toHaveLength(1)
})

it("adds an elliptic arc normal from selection with undo and redo", () => {
	const source = blank()
	source.entities = [
		{ id: "curve", type: "ellipticArc", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI, segments: 64 },
		{ id: "line", type: "line", p0: { x: 0, y: 5 }, p1: { x: 1, y: 10 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select line")
	requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select curve")).dispatchEvent(
		new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event
	)
	click(editor.root, "Normal")
	expect(editor.root.textContent).toContain("normal ·")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("normal ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "normal", a: { entityId: "line", point: "p0" }, b: "curve" }))
	expect(source.relations ?? []).toEqual([])
	editor.dispose()
})

it("creates an elliptic tangent join from endpoints and retains it through PCad reload", async () => {
	const source = blank()
	source.entities = [
		{ id: "s", type: "ellipticArc", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI, segments: 64 },
		{ id: "l", type: "line", p0: { x: -10, y: 0 }, p1: { x: -9, y: -10 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	const second = requireValue(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Select l"))
	second.dispatchEvent(new dom.MouseEvent("click", { bubbles: true, shiftKey: true }) as unknown as Event)
	const firstHandle = requireValue(editor.root.querySelector('[aria-label="s p1"]'))
	firstHandle.dispatchEvent(new dom.PointerEvent("pointerdown", { bubbles: true, button: 0, shiftKey: true }) as unknown as Event)
	requireValue(editor.root.querySelector("svg")).dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	const lastHandle = requireValue(editor.root.querySelector('[aria-label="l p0"]'))
	lastHandle.dispatchEvent(new dom.PointerEvent("pointerdown", { bubbles: true, button: 0, shiftKey: true }) as unknown as Event)
	requireValue(editor.root.querySelector("svg")).dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
	click(editor.root, "Tangent")
	expect(editor.root.textContent).toContain("Tangent join ·")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("Tangent join ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "smoothJoin", a: { entityId: "s", point: "p1" }, b: { entityId: "l", point: "p0" } }))
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [requireValue(saved)] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const result = materializePartFeatures(restored.cad, restored.tree)[0]
	if (result?.type !== "sketch") throw Error("Missing sketch")
	expect(result.relations).toEqual(saved?.relations)
	expect(result.entities[0]?.type).toBe("ellipticArc")
	expect(source.relations ?? []).toEqual([])
	editor.dispose()
})

it("starts a tangent arc from an elliptic arc endpoint in the canvas", () => {
	const source = blank()
	source.entities = [{ id: "s", type: "ellipticArc", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI / 2, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select s")
	const handle = requireValue(editor.root.querySelector('[aria-label="s p1"]'))
	const x = Number(handle.getAttribute("cx"))
	const y = Number(handle.getAttribute("cy"))
	click(editor.root, "Tangent arc")
	pointer(requireValue(editor.root.querySelector("svg")), x, y)
	pointer(requireValue(editor.root.querySelector("svg")), x + 40, y - 40)
	click(editor.root, "Finish sketch")
	expect(saved?.entities.some((e) => e.type === "arc")).toBe(true)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "smoothJoin", a: { entityId: "s", point: "p1" } }))
	expect(source.entities).toHaveLength(1)
	editor.dispose()
})

it("fixes an elliptic center for whole-entity selection and preserves explicit endpoint selection", () => {
	for (const endpoint of [false, true]) {
		const source = blank()
		source.entities = [{ id: "curve", type: "ellipticArc", center: { x: 3, y: 4 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI, segments: 64 }]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			source,
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		click(editor.root, "Select curve")
		if (endpoint) {
			const handle = requireValue(editor.root.querySelector('[aria-label="curve p1"]'))
			pointer(handle, Number(handle.getAttribute("cx")), Number(handle.getAttribute("cy")))
			requireValue(editor.root.querySelector("svg")).dispatchEvent(new dom.PointerEvent("pointerup", { bubbles: true }) as unknown as Event)
		}
		click(editor.root, "Fix")
		click(editor.root, "Finish sketch")
		expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "fixed", anchor: { entityId: "curve", point: endpoint ? "p1" : "center" } }))
		expect(source.relations ?? []).toEqual([])
		editor.dispose()
	}
})

it("infers an elliptic midpoint while drawing a line and retains it through reload and resizing", async () => {
	const source = blank()
	source.entities = [{ id: "curve", type: "ellipticArc", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI, segments: 64 }]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select curve")
	const center = requireValue(editor.root.querySelector('[aria-label="curve center"]'))
	const end = requireValue(editor.root.querySelector('[aria-label="curve p0"]'))
	const x = Number(center.getAttribute("cx"))
	const y = Number(center.getAttribute("cy")) - Math.abs(Number(end.getAttribute("cx")) - x) / 2
	click(editor.root, "Line")
	const canvas = requireValue(editor.root.querySelector("svg"))
	pointer(canvas, x + 0.1, y + 0.1)
	pointer(canvas, x + 30, y - 50)
	click(editor.root, "Finish sketch")
	const accepted = requireValue(saved)
	expect(accepted.relations).toContainEqual(expect.objectContaining({ type: "midpoint", a: { entityId: "line-1", point: "p0" }, b: "curve" }))
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [accepted] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const persisted = materializePartFeatures(restored.cad, restored.tree)[0]
	if (persisted?.type !== "sketch") throw Error("Missing sketch")
	const { solveSketch } = await import("../src/sketch-solver")
	const result = solveSketch(persisted.entities, [
		...(persisted.relations ?? []),
		{ id: "center", type: "fixed", anchor: { entityId: "curve", point: "center" }, position: { x: 0, y: 0 } },
		{ id: "height", type: "height", entityId: "curve", value: 20 },
		{ id: "width", type: "width", entityId: "curve", value: 20 }
	])
	expect(result.status).not.toBe("conflicting")
	const line = result.entities.find((e) => e.id === "line-1")
	if (line?.type !== "line") throw Error("Missing line")
	expect(Math.hypot(line.p0.x, line.p0.y)).toBeCloseTo(10, 5)
	editor.dispose()
})

it("applies whole-curve elliptic tangency without requiring endpoint selection", () => {
	const source = blank()
	source.entities = [
		{ id: "curve", type: "ellipticArc", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 0, startAngle: 0, sweep: Math.PI, segments: 64 },
		{ id: "line", type: "line", p0: { x: -20, y: 5.2 }, p1: { x: 20, y: 5.2 } }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	click(editor.root, "Tangent")
	expect(editor.root.textContent).toContain("tangent ·")
	expect(editor.root.textContent).not.toContain("Select two curve endpoints")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("tangent ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "tangent", a: "curve", b: "line" }))
	expect(saved?.relations?.some((r) => r.type === "smoothJoin")).toBe(false)
	expect(source.relations ?? []).toEqual([])
	editor.dispose()
})

it("equalizes ellipse axes without changing centers, rotations or finite sweeps", () => {
	const source = blank()
	source.entities = [
		{ id: "a", type: "ellipse", center: { x: 0, y: 0 }, width: 20, height: 10, rotation: 15, segments: 64 },
		{ id: "b", type: "ellipticArc", center: { x: 40, y: 30 }, width: 30, height: 20, rotation: 35, startAngle: 0.2, sweep: -1.5, segments: 64 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	click(editor.root, "Equal")
	expect(editor.root.textContent).toContain("equal ·")
	click(editor.root, "Undo")
	expect(editor.root.textContent).not.toContain("equal ·")
	click(editor.root, "Redo")
	click(editor.root, "Finish sketch")
	const a = saved?.entities[0]
	const b = saved?.entities[1]
	if (a?.type !== "ellipse" || b?.type !== "ellipticArc") throw Error("Missing ellipses")
	expect(a.width).toBeCloseTo(b.width, 5)
	expect(a.height).toBeCloseTo(b.height, 5)
	expect(a.center).toEqual({ x: 0, y: 0 })
	expect(b.center).toEqual({ x: 40, y: 30 })
	expect(a.rotation).toBe(15)
	expect(b.rotation).toBe(35)
	expect(b.startAngle).toBe(0.2)
	expect(b.sweep).toBe(-1.5)
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "equal", a: "a", b: "b" }))
	editor.dispose()
})

it("recognizes shared ellipse dependencies before adding redundant Equal or Concentric relations", () => {
	for (const command of ["Equal", "Concentric"]) {
		const source = blank()
		const arc: import("../src/sketch-elliptic-arc").EllipticArc = {
			id: "a",
			type: "ellipticArc",
			center: { x: 0, y: 0 },
			width: 20,
			height: 10,
			rotation: 0,
			startAngle: 0,
			sweep: Math.PI,
			segments: 64
		}
		source.entities = [arc, { ...arc, id: "b", startAngle: Math.PI }]
		source.relations = [{ id: "shared", type: "sameEllipse", a: "a", b: "b" }]
		let saved: Sketch | undefined
		const editor = new SketchWorkspace(
			source,
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(editor.root)
		editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
		click(editor.root, command)
		expect(editor.root.textContent).toContain(`already connected by ${command}`)
		expect(Array.from(editor.root.querySelectorAll("button")).find((b) => b.textContent === "Undo")?.disabled).toBe(true)
		click(editor.root, "Finish sketch")
		expect(saved?.relations).toEqual(source.relations)
		editor.dispose()
	}
})

it("fixes whole selected circles and slots with one undo transaction", () => {
	const source = blank()
	source.entities = [
		{ id: "circle", type: "circle", center: { x: 10, y: 10 }, radius: 5, segments: 64 },
		{ id: "slot", type: "capsule", from: { x: 30, y: 0 }, to: { x: 50, y: 0 }, width: 8, arcSegments: 16 }
	]
	let saved: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			saved = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	editor.root.dispatchEvent(new dom.KeyboardEvent("keydown", { key: "a", ctrlKey: true, bubbles: true }) as unknown as Event)
	click(editor.root, "Fix")
	expect(editor.root.textContent).toContain("Fully constrained")
	click(editor.root, "Fix")
	expect(editor.root.textContent).toContain("already fully constrained")
	click(editor.root, "Undo")
	expect(editor.root.textContent).toContain("Underconstrained")
	click(editor.root, "Redo")
	expect(editor.root.textContent).toContain("Fully constrained")
	click(editor.root, "Finish sketch")
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "radius", entityId: "circle", value: 5 }))
	expect(saved?.relations).toContainEqual(expect.objectContaining({ type: "width", entityId: "slot", value: 8 }))
	expect(saved?.entities).toEqual(source.entities)
	expect(source.relations ?? []).toEqual([])
	editor.dispose()
})

it("unfixes owned constraints after PCad reload while preserving original dimensions", async () => {
	const source = blank()
	source.entities = [{ id: "circle", type: "circle", center: { x: 4, y: 5 }, radius: 10, segments: 64 }]
	source.relations = [{ id: "design-radius", type: "radius", entityId: "circle", value: 10 }]
	let fixed: Sketch | undefined
	const editor = new SketchWorkspace(
		source,
		(s) => {
			fixed = s
		},
		() => undefined
	)
	document.body.append(editor.root)
	click(editor.root, "Select circle")
	click(editor.root, "Fix")
	click(editor.root, "Finish sketch")
	const { createPartRuntimeState, serializePCadState, materializePartFeatures } = await import("../src/pcad/part-state")
	const runtime = createPartRuntimeState({ features: [requireValue(fixed)] })
	const restored = createPartRuntimeState({ features: [], cad: JSON.parse(JSON.stringify(serializePCadState(runtime.cad))), tree: runtime.tree })
	const saved = materializePartFeatures(restored.cad, restored.tree)[0]
	if (saved?.type !== "sketch") throw Error("Missing sketch")
	expect(saved.relations?.filter((r) => r.fixation === "circle")).toHaveLength(1)
	let released: Sketch | undefined
	const reopened = new SketchWorkspace(
		saved,
		(s) => {
			released = s
		},
		() => undefined
	)
	document.body.append(reopened.root)
	click(reopened.root, "Select circle")
	click(reopened.root, "Unfix")
	expect(reopened.root.textContent).toContain("Underconstrained")
	click(reopened.root, "Undo")
	expect(reopened.root.textContent).toContain("Fully constrained")
	click(reopened.root, "Redo")
	click(reopened.root, "Finish sketch")
	expect(released?.relations).toEqual(source.relations)
	expect(released?.entities).toEqual(source.entities)
	editor.dispose()
	reopened.dispose()
})

it("docks sketch mode into the existing viewport and properties panel and restores on exit", () => {
	const panel = new SolidFeaturePanel({ features: [], solidSteps: [] }, () => undefined)
	const viewport = document.createElement("div")
	const preview = document.createElement("canvas")
	viewport.append(preview)
	document.body.append(panel.root, viewport)
	panel.useSketchViewport(viewport)
	click(panel.root, "New sketch")
	const workspace = requireValue(viewport.querySelector<HTMLElement>('[aria-label="Sketch workspace"]'))
	expect(workspace.style.position).toBe("absolute")
	expect(panel.root.querySelector('[aria-label="Sketch properties"]')).not.toBeNull()
	expect(viewport.firstChild).toBe(preview)
	click(workspace, "Cancel sketch")
	expect(workspace.isConnected).toBe(false)
	expect(panel.root.textContent).toContain("Apply changes")
	expect(viewport.children).toHaveLength(1)
	click(panel.root, "New sketch")
	click(requireValue(viewport.querySelector<HTMLElement>('[aria-label="Sketch workspace"]')), "Finish sketch")
	expect(panel.root.textContent).toContain("Apply changes")
	expect(viewport.children).toHaveLength(1)
	click(panel.root, "New sketch")
	panel.dispose()
	expect(viewport.children).toHaveLength(1)
})
