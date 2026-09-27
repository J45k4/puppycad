import { SketchWorkspace } from "../src/ui/sketch-workspace"
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
	it("preserves assembly data and the selected document across a live update", () => {
		const view = new ProjectView({ projectId: "assembly-roundtrip", projectName: "Project", onBack: () => undefined })
		const tree = (view as unknown as { treeView: { restoreFromProjectFile: (project: Project, preserveSelection?: boolean) => void; buildProjectFile: () => Project } }).treeView
		const assembly = { id: "assembly", name: "Fixture", instances: [], connectors: [{ id: "world", instanceId: null, position: { x: 1, y: 2, z: 3 } }] }
		const project: Project = { version: 4, revision: 0, items: [{ id: "assembly", name: "Fixture", type: "assembly", data: assembly }], selectedPath: [0] }
		tree.restoreFromProjectFile(project)
		expect(tree.buildProjectFile().items[0]).toMatchObject({ data: assembly })
		tree.restoreFromProjectFile({ ...project, revision: 1, selectedPath: null }, true)
		expect(tree.buildProjectFile().selectedPath).toEqual([0])
		expect(tree.buildProjectFile().items[0]).toMatchObject({ data: assembly })
	})
	it("retains unsynced browser edits for explicit recovery before loading the server version", async () => {
		const view = new ProjectView({ projectId: "recovery", projectName: "Recovery", onBack: () => undefined })
		await Promise.resolve()
		const server: Project = { version: 4, revision: 7, items: [], selectedPath: null }
		const local: Project = {
			...server,
			items: [{ id: "assembly", name: "Unsynced assembly", type: "assembly", data: { id: "assembly", name: "Unsynced assembly", instances: [], connectors: [], mates: [] } }]
		}
		const tree = (
			view as unknown as {
				treeView: {
					pcadProject: { load: () => Promise<unknown> }
					readBrowserCopy: () => Promise<Project | null>
					storeBrowserRecovery: (project: Project | null) => Promise<void>
					loadFromServer: () => Promise<string>
					buildProjectFile: () => Project
					serverBacked: boolean
					recoveryButton: HTMLButtonElement
					syncStatus: HTMLElement
				}
			}
		).treeView
		let preserved: Project | null = null
		tree.readBrowserCopy = async () => local
		tree.storeBrowserRecovery = async (copy) => {
			preserved = copy
		}
		tree.pcadProject.load = async () => ({ project: server, revision: 7 })
		globalThis.fetch = originalFetch as typeof globalThis.fetch
		expect(await tree.loadFromServer()).toBe("loaded")
		expect(preserved as Project | null).toEqual(local)
		expect(tree.buildProjectFile().items).toHaveLength(0)
		expect(tree.recoveryButton.hidden).toBe(false)
		tree.recoveryButton.click()
		expect(tree.serverBacked).toBe(false)
		expect(tree.buildProjectFile().items[0]?.name).toBe("Unsynced assembly")
		expect(tree.syncStatus.textContent).toContain("Save to Server")
		// Recovery is a local review action; it must not publish over server state.
		expect(server.items).toHaveLength(0)
	})
	it("checkpoints command edits locally before a failed network save and shows a warning", async () => {
		const view = new ProjectView({ projectId: "offline-command", projectName: "Offline", onBack: () => undefined })
		await Promise.resolve()
		const tree = (
			view as unknown as {
				treeView: {
					serverBacked: boolean
					persistenceEnabled: boolean
					saveToIndexedDB: () => Promise<void>
					postCommand: () => Promise<void>
					enqueueCommand: (command: { type: "deleteItem"; itemId: string }) => void
					commandQueue: Promise<void>
					syncStatus: HTMLElement
				}
			}
		).treeView
		const calls: string[] = []
		tree.serverBacked = true
		tree.persistenceEnabled = true
		tree.saveToIndexedDB = async () => {
			calls.push("checkpoint")
		}
		tree.postCommand = async () => {
			calls.push("network")
			throw new Error("offline")
		}
		const previousError = console.error
		console.error = () => undefined
		try {
			tree.enqueueCommand({ type: "deleteItem", itemId: "example" })
			await tree.commandQueue
		} finally {
			console.error = previousError
		}
		expect(calls).toEqual(["checkpoint", "network"])
		expect(tree.syncStatus.textContent).toContain("not reached the server")
	})
	it("keeps sketch Undo and Redo isolated from project history", async () => {
		const view = new ProjectView({ projectId: "sketch-history", projectName: "Project", onBack: () => undefined })
		const tree = (view as unknown as { treeView: { addFolder: () => void; buildProjectFile: () => Project } }).treeView
		tree.addFolder()
		let saved: import("../src/schema").Sketch | undefined
		const workspace = new SketchWorkspace(
			{ id: "s", type: "sketch", dirty: false, target: { type: "plane", plane: "XY" }, entities: [], dimensions: [], vertices: [], loops: [], profiles: [] },
			(s) => {
				saved = s
			},
			() => undefined
		)
		document.body.append(workspace.root)
		const button = (label: string) => {
			const result = Array.from(workspace.root.querySelectorAll("button")).find((b) => b.textContent === label)
			if (!result) throw Error("Missing button")
			result.click()
		}
		button("Circle")
		const canvas = workspace.root.querySelector("svg")
		if (!canvas) throw Error("Missing canvas")
		for (const x of [500, 560]) canvas.dispatchEvent(new domWindow.PointerEvent("pointerdown", { bubbles: true, clientX: x, clientY: 350, button: 0 }) as unknown as Event)
		canvas.dispatchEvent(new domWindow.KeyboardEvent("keydown", { key: "z", ctrlKey: true, bubbles: true, cancelable: true }) as unknown as Event)
		await Promise.resolve()
		expect(tree.buildProjectFile().items).toHaveLength(1)
		expect(workspace.root.textContent).toContain("Entities (0)")
		canvas.dispatchEvent(new domWindow.KeyboardEvent("keydown", { key: "y", ctrlKey: true, bubbles: true, cancelable: true }) as unknown as Event)
		button("Finish sketch")
		expect(saved?.entities).toHaveLength(1)
		expect(tree.buildProjectFile().items).toHaveLength(1)
	})
	it("reports save progress inline and ignores duplicate saves without opening dialogs", async () => {
		const view = new ProjectView({ projectId: "nonmodal-save", projectName: "Project", onBack: () => undefined })
		const tree = (
			view as unknown as {
				treeView: {
					saveProjectToServer: () => Promise<void>
					saveProjectSnapshotToServer: () => Promise<void>
					storeBrowserRecovery: (project: Project | null) => Promise<void>
					connectProjectEvents: () => void
					serverSaveButton: HTMLButtonElement
					syncStatus: HTMLElement
				}
			}
		).treeView
		let alerts = 0
		window.alert = () => {
			alerts++
		}
		let complete: () => void = () => undefined
		let saves = 0
		tree.saveProjectSnapshotToServer = () => {
			saves++
			return new Promise<void>((resolve) => {
				complete = resolve
			})
		}
		tree.storeBrowserRecovery = async () => undefined
		tree.connectProjectEvents = () => undefined
		const first = tree.saveProjectToServer()
		expect(tree.serverSaveButton.disabled).toBe(true)
		expect(tree.syncStatus.textContent).toContain("Saving project")
		await tree.saveProjectToServer()
		expect(saves).toBe(1)
		complete()
		await first
		expect(tree.serverSaveButton.disabled).toBe(false)
		expect(tree.syncStatus.textContent).toBe("Project saved on server.")
		expect(tree.syncStatus.getAttribute("role")).toBe("status")
		expect(alerts).toBe(0)
	})
	it("keeps browser recovery on a failed save and shows a successful retry inline", async () => {
		const view = new ProjectView({ projectId: "save-retry", projectName: "Project", onBack: () => undefined })
		const tree = (
			view as unknown as {
				treeView: {
					saveProjectToServer: () => Promise<void>
					saveProjectSnapshotToServer: () => Promise<void>
					storeBrowserRecovery: (project: Project | null) => Promise<void>
					connectProjectEvents: () => void
					serverSaveButton: HTMLButtonElement
					syncStatus: HTMLElement
					recoveryProject: Project | null
				}
			}
		).treeView
		let alerts = 0
		window.alert = () => {
			alerts++
		}
		const backup: Project = { version: 4, revision: 0, items: [], selectedPath: null }
		tree.recoveryProject = backup
		const cleared: (Project | null)[] = []
		tree.storeBrowserRecovery = async (value) => {
			cleared.push(value)
		}
		tree.connectProjectEvents = () => undefined
		tree.saveProjectSnapshotToServer = async () => {
			throw Error("offline")
		}
		const previousError = console.error
		console.error = () => undefined
		try {
			await tree.saveProjectToServer()
		} finally {
			console.error = previousError
		}
		expect(tree.recoveryProject).toBe(backup)
		expect(cleared).toEqual([])
		expect(tree.syncStatus.getAttribute("role")).toBe("alert")
		expect(tree.syncStatus.textContent).toContain("offline")
		expect(tree.syncStatus.textContent).toContain("retry")
		expect(tree.serverSaveButton.disabled).toBe(false)
		tree.saveProjectSnapshotToServer = async () => undefined
		await tree.saveProjectToServer()
		expect(tree.recoveryProject).toBeNull()
		expect(cleared).toEqual([null])
		expect(tree.syncStatus.getAttribute("role")).toBe("status")
		expect(alerts).toBe(0)
	})
})
