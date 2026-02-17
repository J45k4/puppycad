import { beforeEach, describe, expect, it } from "bun:test"
import { Window } from "happy-dom"
import { DockLayout } from "../src/dock-layout"

describe("DockLayout", () => {
	beforeEach(() => {
		const window = new Window()
		globalThis.window = window as unknown as typeof globalThis.window
		globalThis.document = window.document as unknown as Document
	})

	it("serializes and restores split configuration and active pane", () => {
		const layout = new DockLayout()
		const initialPaneId = layout.getActivePaneId()
		expect(initialPaneId).toBeTruthy()
		if (!initialPaneId) {
			throw new Error("Expected initial pane id to be defined")
		}

		const secondPaneId = layout.splitPane(initialPaneId, "horizontal")
		expect(secondPaneId).toBeTruthy()
		if (!secondPaneId) {
			throw new Error("Expected horizontal split to create a new pane")
		}

		const thirdPaneId = layout.splitPane(secondPaneId, "vertical")
		expect(thirdPaneId).toBeTruthy()
		if (!thirdPaneId) {
			throw new Error("Expected vertical split to create a new pane")
		}

		layout.setActivePane(initialPaneId)

		const state = layout.getState()

		const restoredLayout = new DockLayout()
		restoredLayout.restoreState(state)

		expect(restoredLayout.getState()).toEqual(state)
		expect(restoredLayout.getPaneIds().length).toBe(layout.getPaneIds().length)
		expect(restoredLayout.getActivePaneId()).toBe(state.activePaneId)
	})

	it("closes panes and collapses empty splits", () => {
		const layout = new DockLayout()
		const firstPane = layout.getActivePaneId()
		expect(firstPane).toBeTruthy()
		if (!firstPane) {
			throw new Error("Expected first pane to exist")
		}

		const secondPane = layout.splitPane(firstPane, "horizontal")
		expect(secondPane).toBeTruthy()
		if (!secondPane) {
			throw new Error("Expected second pane to exist")
		}

		const thirdPane = layout.splitPane(secondPane, "vertical")
		expect(thirdPane).toBeTruthy()
		if (!thirdPane) {
			throw new Error("Expected third pane to exist")
		}

		layout.setActivePane(thirdPane)
		layout.closePane(thirdPane)

		expect(layout.getPaneIds()).toEqual([firstPane, secondPane])
		expect(layout.getActivePaneId()).toBe(firstPane)

		layout.closePane(secondPane)
		expect(layout.getPaneIds()).toEqual([firstPane])
		expect(layout.getActivePaneId()).toBe(firstPane)

		const state = layout.getState()
		expect(state.root.type).toBe("leaf")

		const restored = new DockLayout()
		restored.restoreState(state)

		const restoredActive = restored.getActivePaneId()
		expect(restoredActive).toBeTruthy()
		if (!restoredActive) {
			throw new Error("Expected restored layout to have an active pane")
		}

		expect(restored.getPaneIds()).toEqual([restoredActive])
	})

	it("repositions panes relative to other panes", () => {
		const layout = new DockLayout()
		const firstPane = layout.getActivePaneId()
		expect(firstPane).toBeTruthy()
		if (!firstPane) {
			throw new Error("Expected first pane to exist")
		}

		const secondPane = layout.splitPane(firstPane, "horizontal")
		expect(secondPane).toBeTruthy()
		if (!secondPane) {
			throw new Error("Expected second pane to exist")
		}

		const thirdPane = layout.splitPane(secondPane, "vertical")
		expect(thirdPane).toBeTruthy()
		if (!thirdPane) {
			throw new Error("Expected third pane to exist")
		}

		layout.movePane(thirdPane, firstPane, "left")

		expect(layout.getActivePaneId()).toBe(thirdPane)

		const state = layout.getState()
		if (state.root.type !== "split") {
			throw new Error("Expected root to be a split after moving pane")
		}

		expect(state.root.orientation).toBe("horizontal")
		expect(state.root.children.length).toBe(3)

		const leftmost = state.root.children[0]
		if (!leftmost) {
			throw new Error("Expected leftmost child to exist")
		}
		if (leftmost.type !== "leaf") {
			throw new Error("Expected leftmost child to be a leaf")
		}
		expect(leftmost.paneId).toBe(thirdPane)
	})

	it("repositions panes relative to the root area", () => {
		const layout = new DockLayout()
		const firstPane = layout.getActivePaneId()
		expect(firstPane).toBeTruthy()
		if (!firstPane) {
			throw new Error("Expected first pane to exist")
		}

		const secondPane = layout.splitPane(firstPane, "horizontal")
		expect(secondPane).toBeTruthy()
		if (!secondPane) {
			throw new Error("Expected second pane to exist")
		}

		layout.movePane(secondPane, null, "top")

		expect(layout.getActivePaneId()).toBe(secondPane)

		const state = layout.getState()
		if (state.root.type !== "split") {
			throw new Error("Expected root to be a split after moving pane to root")
		}

		expect(state.root.orientation).toBe("vertical")
		expect(state.root.children.length).toBe(2)

		const topPane = state.root.children[0]
		if (!topPane) {
			throw new Error("Expected top pane to exist")
		}
		if (topPane.type !== "leaf") {
			throw new Error("Expected top pane to be a leaf")
		}
		expect(topPane.paneId).toBe(secondPane)
	})

	it("swaps pane positions when moved to the center of another pane", () => {
		const layout = new DockLayout()
		const firstPane = layout.getActivePaneId()
		expect(firstPane).toBeTruthy()
		if (!firstPane) {
			throw new Error("Expected first pane to exist")
		}

		const secondPane = layout.splitPane(firstPane, "horizontal")
		expect(secondPane).toBeTruthy()
		if (!secondPane) {
			throw new Error("Expected second pane to exist")
		}

		const before = layout.getState()
		if (before.root.type !== "split") {
			throw new Error("Expected split root before swap")
		}
		const firstBefore = before.root.children[0]
		const secondBefore = before.root.children[1]
		if (!firstBefore || !secondBefore || firstBefore.type !== "leaf" || secondBefore.type !== "leaf") {
			throw new Error("Expected two leaf panes before swap")
		}
		expect(firstBefore.paneId).toBe(firstPane)
		expect(secondBefore.paneId).toBe(secondPane)

		layout.movePane(firstPane, secondPane, "center")

		const after = layout.getState()
		if (after.root.type !== "split") {
			throw new Error("Expected split root after swap")
		}
		const firstAfter = after.root.children[0]
		const secondAfter = after.root.children[1]
		if (!firstAfter || !secondAfter || firstAfter.type !== "leaf" || secondAfter.type !== "leaf") {
			throw new Error("Expected two leaf panes after swap")
		}
		expect(firstAfter.paneId).toBe(secondPane)
		expect(secondAfter.paneId).toBe(firstPane)
		expect(layout.getActivePaneId()).toBe(firstPane)
	})

	it("detects all edge positions and center for external drops", () => {
		const layout = new DockLayout()
		const paneId = layout.getActivePaneId()
		expect(paneId).toBeTruthy()
		if (!paneId) {
			throw new Error("Expected pane to exist")
		}

		const pane = layout.root.querySelector(`[data-pane-id="${paneId}"]`) as HTMLDivElement | null
		expect(pane).not.toBeNull()
		if (!pane) {
			throw new Error("Expected pane element to be present")
		}
		const content = pane.children[1] as HTMLDivElement | undefined
		if (!content) {
			throw new Error("Expected pane content element")
		}

		Object.defineProperty(content, "clientHeight", { value: 300, configurable: true })
		content.getBoundingClientRect = () =>
			({
				x: 0,
				y: 0,
				left: 0,
				top: 0,
				right: 300,
				bottom: 300,
				width: 300,
				height: 300,
				toJSON: () => ({})
			}) as DOMRect

		layout.canAcceptExternalDrop = () => true
		const positions: string[] = []
		layout.onExternalDrop = ({ position }) => {
			positions.push(position)
		}

		const createDragEvent = (type: string, clientX: number, clientY: number): DragEvent => {
			const event = new window.Event(type, { bubbles: true, cancelable: true }) as DragEvent
			Object.defineProperty(event, "clientX", { value: clientX, configurable: true })
			Object.defineProperty(event, "clientY", { value: clientY, configurable: true })
			Object.defineProperty(event, "dataTransfer", {
				value: {
					dropEffect: "move",
					getData: () => "",
					setData: () => undefined
				},
				configurable: true
			})
			return event
		}

		content.dispatchEvent(createDragEvent("dragover", 150, 10))
		content.dispatchEvent(createDragEvent("drop", 150, 10))

		content.dispatchEvent(createDragEvent("dragover", 10, 150))
		content.dispatchEvent(createDragEvent("drop", 10, 150))

		content.dispatchEvent(createDragEvent("dragover", 150, 150))
		content.dispatchEvent(createDragEvent("drop", 150, 150))

		content.dispatchEvent(createDragEvent("dragover", 290, 150))
		content.dispatchEvent(createDragEvent("drop", 290, 150))

		content.dispatchEvent(createDragEvent("dragover", 150, 290))
		content.dispatchEvent(createDragEvent("drop", 150, 290))

		expect(positions).toEqual(["top", "left", "center", "right", "bottom"])
	})

	it("toggles pane floating mode from the header button", () => {
		const layout = new DockLayout()
		const paneId = layout.getActivePaneId()
		expect(paneId).toBeTruthy()
		if (!paneId) {
			throw new Error("Expected pane to exist")
		}

		const pane = layout.root.querySelector(`[data-pane-id="${paneId}"]`) as HTMLDivElement | null
		expect(pane).not.toBeNull()
		if (!pane) {
			throw new Error("Expected pane element to exist")
		}

		const header = pane.children[0] as HTMLDivElement | undefined
		if (!header) {
			throw new Error("Expected header element")
		}
		const floatButton = header.querySelector('button[aria-label="Float pane"]') as HTMLButtonElement | null
		expect(floatButton).not.toBeNull()
		if (!floatButton) {
			throw new Error("Expected float button")
		}

		floatButton.click()
		expect(pane.style.position).toBe("absolute")
		expect(header.draggable).toBe(false)
		expect(floatButton.getAttribute("aria-label")).toBe("Dock pane")

		floatButton.click()
		expect(pane.style.position).toBe("")
		expect(header.draggable).toBe(true)
		expect(floatButton.getAttribute("aria-label")).toBe("Float pane")
	})

	it("moves floating panes immediately when dragging the header", () => {
		const layout = new DockLayout()
		const paneId = layout.getActivePaneId()
		expect(paneId).toBeTruthy()
		if (!paneId) {
			throw new Error("Expected pane to exist")
		}

		const pane = layout.root.querySelector(`[data-pane-id="${paneId}"]`) as HTMLDivElement | null
		expect(pane).not.toBeNull()
		if (!pane) {
			throw new Error("Expected pane element to exist")
		}

		const header = pane.children[0] as HTMLDivElement | undefined
		if (!header) {
			throw new Error("Expected header element")
		}
		const floatButton = header.querySelector('button[aria-label="Float pane"]') as HTMLButtonElement | null
		expect(floatButton).not.toBeNull()
		if (!floatButton) {
			throw new Error("Expected float button")
		}

		layout.root.getBoundingClientRect = () =>
			({
				x: 100,
				y: 50,
				left: 100,
				top: 50,
				right: 1100,
				bottom: 750,
				width: 1000,
				height: 700,
				toJSON: () => ({})
			}) as DOMRect
		pane.getBoundingClientRect = () =>
			({
				x: 180,
				y: 120,
				left: 180,
				top: 120,
				right: 540,
				bottom: 360,
				width: 360,
				height: 240,
				toJSON: () => ({})
			}) as DOMRect

		floatButton.click()
		expect(pane.style.left).toBe("80px")
		expect(pane.style.top).toBe("70px")

		header.dispatchEvent(
			new window.MouseEvent("mousedown", {
				bubbles: true,
				button: 0,
				clientX: 220,
				clientY: 160
			})
		)
		window.dispatchEvent(
			new window.MouseEvent("mousemove", {
				bubbles: true,
				clientX: 400,
				clientY: 260
			})
		)
		window.dispatchEvent(
			new window.MouseEvent("mouseup", {
				bubbles: true
			})
		)

		expect(pane.style.left).toBe("260px")
		expect(pane.style.top).toBe("170px")
	})

	it("resizes floating panes from the corner handle", () => {
		const layout = new DockLayout()
		const paneId = layout.getActivePaneId()
		expect(paneId).toBeTruthy()
		if (!paneId) {
			throw new Error("Expected pane to exist")
		}

		const pane = layout.root.querySelector(`[data-pane-id="${paneId}"]`) as HTMLDivElement | null
		expect(pane).not.toBeNull()
		if (!pane) {
			throw new Error("Expected pane element to exist")
		}

		const header = pane.children[0] as HTMLDivElement | undefined
		if (!header) {
			throw new Error("Expected header element")
		}
		const floatButton = header.querySelector('button[aria-label="Float pane"]') as HTMLButtonElement | null
		expect(floatButton).not.toBeNull()
		if (!floatButton) {
			throw new Error("Expected float button")
		}
		const resizeHandle = pane.querySelector('[data-dock-floating-resize-handle="corner"]') as HTMLDivElement | null
		expect(resizeHandle).not.toBeNull()
		if (!resizeHandle) {
			throw new Error("Expected floating resize handle")
		}

		layout.root.getBoundingClientRect = () =>
			({
				x: 100,
				y: 50,
				left: 100,
				top: 50,
				right: 1100,
				bottom: 750,
				width: 1000,
				height: 700,
				toJSON: () => ({})
			}) as DOMRect
		pane.getBoundingClientRect = () =>
			({
				x: 180,
				y: 120,
				left: 180,
				top: 120,
				right: 540,
				bottom: 360,
				width: 360,
				height: 240,
				toJSON: () => ({})
			}) as DOMRect

		floatButton.click()
		expect(resizeHandle.style.display).toBe("block")

		resizeHandle.dispatchEvent(
			new window.MouseEvent("mousedown", {
				bubbles: true,
				button: 0,
				clientX: 540,
				clientY: 360
			})
		)
		window.dispatchEvent(
			new window.MouseEvent("mousemove", {
				bubbles: true,
				clientX: 700,
				clientY: 500
			})
		)
		window.dispatchEvent(
			new window.MouseEvent("mouseup", {
				bubbles: true
			})
		)

		expect(pane.style.width).toBe("520px")
		expect(pane.style.height).toBe("380px")

		floatButton.click()
		expect(resizeHandle.style.display).toBe("none")
	})
})
