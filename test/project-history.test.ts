import { afterEach, beforeEach, describe, expect, it } from "bun:test"
import { Window as HappyDOMWindow } from "happy-dom"
import type { Project } from "../src/contract"
import { ProjectView } from "../src/ui/project"

let domWindow: HappyDOMWindow
let originalFetch: typeof globalThis.fetch | undefined
let originalConsoleLog: typeof console.log

describe("ProjectView history", () => {
	beforeEach(() => {
		originalFetch = globalThis.fetch
		originalConsoleLog = console.log
		console.log = () => undefined
		domWindow = new HappyDOMWindow()
		globalThis.window = domWindow as unknown as typeof globalThis.window
		globalThis.document = domWindow.document as unknown as Document
		globalThis.HTMLElement = domWindow.HTMLElement as unknown as typeof globalThis.HTMLElement
		globalThis.KeyboardEvent = domWindow.KeyboardEvent as unknown as typeof globalThis.KeyboardEvent
		globalThis.fetch = undefined as unknown as typeof globalThis.fetch
	})

	afterEach(() => {
		globalThis.fetch = originalFetch as typeof globalThis.fetch
		console.log = originalConsoleLog
	})

	it("undoes and redoes browser-only project changes from keyboard shortcuts", async () => {
		const view = new ProjectView({
			projectId: "history-shortcuts",
			projectName: "Project",
			onBack: () => undefined
		})
		const treeView = view as unknown as {
			treeView: {
				addFolder: () => void
				buildProjectFile: () => Project
			}
		}

		treeView.treeView.addFolder()
		expect(treeView.treeView.buildProjectFile().items).toHaveLength(1)

		domWindow.document.dispatchEvent(new domWindow.KeyboardEvent("keydown", { key: "z", metaKey: true, bubbles: true }))
		await Promise.resolve()
		expect(treeView.treeView.buildProjectFile().items).toHaveLength(0)

		domWindow.document.dispatchEvent(new domWindow.KeyboardEvent("keydown", { key: "y", ctrlKey: true, bubbles: true }))
		await Promise.resolve()
		expect(treeView.treeView.buildProjectFile().items).toHaveLength(1)
	})
})
