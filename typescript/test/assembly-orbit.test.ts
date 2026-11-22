import { describe, expect, it } from "bun:test"
import { Window } from "happy-dom"
import { OrthographicCamera, Vector3 } from "three"
import { bindAssemblyOrbit, panAssemblyTarget } from "../src/ui/assembly-orbit"
function setup() {
	const window = new Window()
	const viewport = window.document.createElement("div") as unknown as HTMLElement
	const movements: number[][] = []
	const captures: number[] = []
	const pans: number[][] = []
	const starts: number[][] = []
	viewport.setPointerCapture = (id) => {
		captures.push(id)
	}
	bindAssemblyOrbit(
		viewport,
		(x, y) => movements.push([x, y]),
		(x, y) => pans.push([x, y]),
		(x, y) => starts.push([x, y])
	)
	const event = (type: string, options: Partial<PointerEvent> = {}) => {
		const e = new window.Event(type, { cancelable: true })
		Object.assign(e, { pointerType: "mouse", pointerId: 1, isPrimary: true, button: 2, buttons: 2, clientX: 20, clientY: 30 }, options)
		viewport.dispatchEvent(e as unknown as Event)
		return e
	}
	return { viewport, movements, captures, pans, starts, event }
}
describe("assembly right-drag rotation", () => {
	it("rotates continuously while right is held, captures the pointer, and stops on release", () => {
		const s = setup()
		expect(s.event("pointerdown").defaultPrevented).toBe(true)
		s.event("pointermove", { clientX: 35, clientY: 50 })
		s.event("pointermove", { clientX: 40, clientY: 45 })
		expect(s.movements).toEqual([
			[15, 20],
			[5, -5]
		])
		expect(s.captures).toEqual([1])
		expect(s.starts).toEqual([[20, 30]])
		s.event("pointerup", { buttons: 0 })
		s.event("pointermove", { clientX: 90, clientY: 90 })
		expect(s.movements).toHaveLength(2)
		expect(s.viewport.style.cursor).toBe("grab")
	})
	it("ignores left drags and prevents the browser context menu", () => {
		const s = setup()
		for (const button of [0]) {
			s.event("pointerdown", { button })
			s.event("pointermove", { clientX: 50 })
		}
		expect(s.movements).toHaveLength(0)
		expect(s.captures).toHaveLength(0)
		expect(s.event("contextmenu").defaultPrevented).toBe(true)
	})
	it("ignores other pointers and stops after capture loss, cancellation or a missed release", () => {
		for (const ending of ["lostpointercapture", "pointercancel", "missed-release"]) {
			const s = setup()
			s.event("pointerdown")
			s.event("pointermove", { pointerId: 2, clientX: 80 })
			expect(s.movements).toHaveLength(0)
			if (ending === "missed-release") s.event("pointermove", { buttons: 0 })
			else s.event(ending)
			s.event("pointermove", { clientX: 80 })
			expect(s.movements).toHaveLength(0)
		}
	})
	it("retains primary touch drag rotation", () => {
		const s = setup()
		s.event("pointerdown", { pointerType: "touch", button: 0 })
		s.event("pointermove", { pointerType: "touch", buttons: 1, clientX: 25, clientY: 32 })
		expect(s.movements).toEqual([[5, 2]])
	})
})

describe("assembly middle-drag panning", () => {
	it("pans only while middle is held and does not rotate", () => {
		const s = setup()
		expect(s.event("pointerdown", { button: 1, buttons: 4 }).defaultPrevented).toBe(true)
		s.event("pointermove", { buttons: 4, clientX: 45, clientY: 20 })
		expect(s.pans).toEqual([[25, -10]])
		expect(s.movements).toHaveLength(0)
		s.event("pointermove", { buttons: 2, clientX: 50 })
		s.event("pointermove", { buttons: 4, clientX: 60 })
		expect(s.pans).toHaveLength(1)
	})
	it("moves the model by the dragged pixels in the current camera plane at different zooms", () => {
		for (const zoom of [1, 3]) {
			const camera = new OrthographicCamera(-200, 200, 100, -100, 0.1, 2000)
			camera.zoom = zoom
			camera.up.set(0, 0, 1)
			camera.position.set(300, -400, 250)
			const target = new Vector3()
			camera.lookAt(target)
			camera.updateProjectionMatrix()
			camera.updateMatrixWorld()
			const before = new Vector3().project(camera)
			panAssemblyTarget(camera, target, 60, -25, 500)
			camera.position.add(target)
			camera.lookAt(target)
			camera.updateMatrixWorld()
			const after = new Vector3().project(camera)
			expect(((after.x - before.x) * 1000) / 2).toBeCloseTo(60, 6)
			expect((-(after.y - before.y) * 500) / 2).toBeCloseTo(-25, 6)
		}
	})
})

it("does not choose a rotation pivot on middle drag", () => {
	const s = setup()
	s.event("pointerdown", { button: 1, buttons: 4 })
	s.event("pointermove", { clientX: 45, buttons: 4 })
	expect(s.starts).toEqual([])
})
