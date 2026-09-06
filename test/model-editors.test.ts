import { beforeEach, describe, expect, it } from "bun:test"
import { Window } from "happy-dom"
import { SolidFeaturePanel } from "../src/ui/solid-features"
import { AssemblyProperties, validateAssemblyEdit } from "../src/ui/assembly-properties"
import { PartBuilder, circle, rectangle, v2 } from "../src/sdk"
import type { PartDocument } from "../src/schema"
import type { Assembly, ProjectNode } from "../src/contract"
import { createPartGeometries, exportPartStl } from "../src/part-mesh"
import { flowerHolderParts } from "../examples/flower-holder-parts"
import { requireValue } from "../src/required"
import { stepLoops } from "../src/solid-edit"
let dom: Window
beforeEach(() => {
	dom = new Window()
	globalThis.document = dom.document as unknown as Document
	globalThis.window = dom as unknown as typeof window
})
function click(root: HTMLElement, label: string) {
	const b = Array.from(root.querySelectorAll("button")).find((b) => b.textContent === label)
	if (!b) throw Error(`Missing button: ${label}`)
	b.click()
}
function edit(root: HTMLElement, label: string, value: string, index = 0) {
	const input = requireValue(root.querySelectorAll<HTMLInputElement>(`input[aria-label="${label}"]`)[index])
	input.value = value
	input.dispatchEvent(new dom.Event("change") as unknown as Event)
}
function choose(root: HTMLElement, label: string, value: string) {
	const input = requireValue(root.querySelector<HTMLSelectElement>(`select[aria-label="${label}"]`))
	input.value = value
	input.dispatchEvent(new dom.Event("change") as unknown as Event)
}
function bounds(doc: PartDocument) {
	const g = requireValue(createPartGeometries(doc)[0])
	g.computeBoundingBox()
	const box = requireValue(g.boundingBox).clone()
	g.dispose()
	return box
}
function part(): PartDocument {
	const b = new PartBuilder()
	b.extrude("body", { outline: rectangle(v2(0, 0), 20, 20), holes: [circle(v2(0, 0), 2, 24)], depth: 5 })
	return b.document
}
describe("Solid feature UI", () => {
	it("edits depth and a hole, rebuilds geometry, and does not mutate the accepted input", () => {
		const original = part()
		let saved: PartDocument | undefined
		const panel = new SolidFeaturePanel(original, (next) => {
			saved = next
		})
		edit(panel.root, "Extrusion depth (mm)", "12")
		edit(panel.root, "Profile width (mm)", "6", 1)
		edit(panel.root, "Profile height (mm)", "6", 1)
		click(panel.root, "Apply changes")
		const next = requireValue(saved)
		expect(bounds(next).max.z).toBe(12)
		expect(bounds(original).max.z).toBe(5)
		const hole = requireValue(stepLoops(next, requireValue(next.solidSteps?.[0]))[1])
		expect(Math.max(...hole.map((p) => p.x))).toBeCloseTo(3)
		expect(Math.max(...hole.map((p) => p.y))).toBeCloseTo(3)
		expect(exportPartStl(next)).toContain("facet normal")
	})
	it("rejects an invalid depth atomically and allows correcting the same draft", () => {
		let accepted = part()
		const panel = new SolidFeaturePanel(accepted, (next) => {
			accepted = next
		})
		edit(panel.root, "Extrusion depth (mm)", "-3")
		click(panel.root, "Apply changes")
		expect(bounds(accepted).max.z).toBe(5)
		expect(panel.root.querySelector("[role=alert]")?.textContent).toContain("greater than zero")
		edit(panel.root, "Extrusion depth (mm)", "9")
		click(panel.root, "Apply changes")
		expect(bounds(accepted).max.z).toBe(9)
	})
	it("adds a boolean cut, edits its placement, reorders, deletes, and discards drafts", () => {
		let accepted = part()
		const panel = new SolidFeaturePanel(accepted, (next) => {
			accepted = next
		})
		click(panel.root, "Add extrusion")
		edit(panel.root, "Profile width (mm)", "4")
		edit(panel.root, "Profile height (mm)", "4")
		choose(panel.root, "Operation", "cut")
		click(panel.root, "Apply changes")
		expect(accepted.solidSteps).toHaveLength(2)
		click(panel.root, "Move up")
		click(panel.root, "Apply changes")
		expect(panel.root.querySelector("[role=alert]")).not.toBeNull()
		expect(accepted.solidSteps).toHaveLength(2)
		click(panel.root, "Discard changes")
		click(panel.root, "2. extrusion-2 · cut")
		click(panel.root, "Delete feature")
		click(panel.root, "Apply changes")
		expect(panel.root.querySelector("[role=alert]")?.textContent).toBeUndefined()
		expect(accepted.solidSteps).toHaveLength(1)
	})
	it("edits revolves and fillets through controls", () => {
		const b = new PartBuilder()
		b.revolve("shaft", { outline: [v2(0, 0), v2(8, 0), v2(8, 40), v2(0, 40)] })
		b.fillet("shaft", 2, [2])
		let saved = b.document
		const panel = new SolidFeaturePanel(saved, (next) => {
			saved = next
		})
		edit(panel.root, "Radius / distance (mm)", "3")
		edit(panel.root, "Revolve angle (degrees)", "270")
		edit(panel.root, "Profile height (mm)", "50")
		click(panel.root, "Apply changes")
		expect(bounds(saved).max.z).toBe(50)
		expect(saved.solidSteps?.[0]?.angle).toBe(270)
		expect(saved.solidSteps?.[0]?.finishes?.[0]?.radius).toBe(3)
	})
	for (const definition of flowerHolderParts)
		it(`can apply every ${definition.id} feature without losing geometry`, () => {
			const b = new PartBuilder()
			definition.build(b)
			const original = exportPartStl(b.document)
			let saved = b.document
			const panel = new SolidFeaturePanel(saved, (next) => {
				saved = next
			})
			for (const [i, step] of requireValue(b.document.solidSteps).entries()) click(panel.root, `${i + 1}. ${step.id} · ${step.operation}`)
			click(panel.root, "Apply changes")
			expect(panel.root.querySelector("[role=alert]")?.textContent).toBeUndefined()
			expect(exportPartStl(saved)).toBe(original)
		}, 30000)
})
const parts: ProjectNode[] = [{ type: "part", id: "p", name: "Part", data: { features: [] } }]
function assembly(): Assembly {
	return {
		id: "a",
		name: "Assembly",
		instances: [
			{ id: "base", partId: "p" },
			{ id: "post", partId: "p" }
		],
		connectors: [
			{ id: "base-top", instanceId: "base", position: { x: 0, y: 0, z: 10 } },
			{ id: "post-foot", instanceId: "post", position: { x: 0, y: 0, z: 0 } }
		],
		mates: [{ id: "joint", type: "fasten", a: { instanceId: "base", connectorId: "base-top" }, b: { instanceId: "post", connectorId: "post-foot" } }]
	}
}
describe("Assembly properties UI", () => {
	it("changes connector position, solves the pose and can rename/remove references safely", () => {
		let saved = assembly()
		const panel = new AssemblyProperties(
			saved,
			() => parts,
			(next) => {
				saved = next
			}
		)
		edit(panel.root, "Connector position (mm) Z", "25")
		click(panel.root, "Apply assembly changes")
		expect(saved.instances[1]?.transform?.translation?.z).toBe(25)
		edit(panel.root, "Connector name", "renamed")
		click(panel.root, "Apply assembly changes")
		expect(saved.mates?.[0]?.a.connectorId).toBe("renamed")
		click(panel.root, "Remove instance and connections")
		click(panel.root, "Apply assembly changes")
		expect(saved.instances).toHaveLength(1)
		expect(saved.mates).toHaveLength(0)
		expect(saved.connectors).toHaveLength(1)
	})
	it("edits free placements, adds instances and connectors, rejects invalid scale", () => {
		let saved: Assembly = { id: "a", name: "Assembly", instances: [{ id: "base", partId: "p" }] }
		const panel = new AssemblyProperties(
			saved,
			() => parts,
			(next) => {
				saved = next
			}
		)
		edit(panel.root, "Position (mm) X", "30")
		edit(panel.root, "Rotation (degrees) Z", "45")
		click(panel.root, "Apply assembly changes")
		expect(saved.instances[0]?.transform?.translation?.x).toBe(30)
		edit(panel.root, "Scale X", "0")
		click(panel.root, "Apply assembly changes")
		expect(saved.instances[0]?.transform?.scale?.x).toBe(1)
		expect(panel.root.querySelector("[role=alert]")?.textContent).toContain("positive")
		click(panel.root, "Discard assembly changes")
		click(panel.root, "Add instance")
		click(panel.root, "Add connector")
		click(panel.root, "Apply assembly changes")
		expect(saved.instances).toHaveLength(2)
		expect(saved.connectors).toHaveLength(1)
	})
	it("rejects duplicate names, missing parts, and inconsistent fixed loops", () => {
		const a = assembly()
		a.instances.push({ id: "post", partId: "p" })
		expect(() => validateAssemblyEdit(a, parts)).toThrow("unique")
		expect(() => validateAssemblyEdit(assembly(), [])).toThrow("Missing part")
		const b = assembly()
		b.connectors?.push({ id: "other", instanceId: "base", position: { x: 0, y: 0, z: 11 } })
		b.mates?.push({ id: "conflict", type: "fasten", a: { instanceId: "base", connectorId: "other" }, b: { instanceId: "post", connectorId: "post-foot" } })
		expect(() => validateAssemblyEdit(b, parts)).toThrow("Conflicting")
	})
})
