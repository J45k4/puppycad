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
})
