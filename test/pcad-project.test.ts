import { unlink } from "node:fs/promises"
import { describe, expect, it } from "bun:test"
import type { PartProjectItemData, Project } from "../src/contract"
import { PCadPart, PCadProject, PuppyCadClient } from "../src/pcad/project"
import { getProject, getProjectEvents, getProjectFileUrl, postProjectCommands, putProject } from "../src/server/save-project"

function createPartDocument(): PartProjectItemData {
	return {
		features: [
			{
				type: "sketch",
				id: "sketch-1",
				name: "Sketch 1",
				dirty: false,
				target: { type: "plane", plane: "XY" },
				entities: [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 10 } }],
				dimensions: [],
				vertices: [],
				loops: [],
				profiles: [{ id: "sketch-1-profile-1", outerLoopId: "loop-1", holeLoopIds: [] }]
			},
			{
				type: "extrude",
				id: "extrude-1",
				name: "Extrude 1",
				target: { type: "profileRef", sketchId: "sketch-1", profileId: "sketch-1-profile-1" },
				depth: 10
			}
		]
	}
}

function createProject(part: PartProjectItemData): Project {
	return {
		version: 4,
		revision: 0,
		items: [
			{
				id: "part-1",
				type: "part",
				name: "Part",
				data: part
			}
		],
		selectedPath: null
	}
}

function createServerFetch(projectId: string): (input: RequestInfo | URL, init?: RequestInit) => Promise<Response> {
	return async (input, init) => {
		const url = new URL(input.toString(), "http://localhost")
		const request = new Request(url, init)
		const projectPath = `/api/projects/${encodeURIComponent(projectId)}`
		if (url.pathname === projectPath && request.method === "GET") {
			return getProject(request, projectId)
		}
		if (url.pathname === projectPath && request.method === "PUT") {
			return putProject(request, projectId)
		}
		if (url.pathname === `${projectPath}/commands` && request.method === "POST") {
			return postProjectCommands(request, projectId)
		}
		return Response.json({ ok: false, message: `Unhandled test route ${request.method} ${url.pathname}` }, { status: 404 })
	}
}

class TestServerEventSource {
	public onerror: (() => void) | null = null
	private readonly listeners = new Map<string, Set<(event: MessageEvent<string>) => void>>()
	private reader: ReadableStreamDefaultReader<Uint8Array> | null = null
	private closed = false

	public constructor(
		private readonly url: string,
		private readonly projectId: string
	) {
		queueMicrotask(() => {
			void this.start()
		})
	}

	public addEventListener(type: string, listener: (event: MessageEvent<string>) => void): void {
		const listeners = this.listeners.get(type) ?? new Set<(event: MessageEvent<string>) => void>()
		listeners.add(listener)
		this.listeners.set(type, listeners)
	}

	public close(): void {
		this.closed = true
		void this.reader?.cancel()
	}

	private async start(): Promise<void> {
		try {
			const response = await getProjectEvents(new Request(new URL(this.url, "http://localhost")), this.projectId)
			if (!response.body) {
				throw new Error("Expected SSE response body")
			}
			this.reader = response.body.getReader()
			const decoder = new TextDecoder()
			let buffer = ""
			while (!this.closed) {
				const chunk = await this.reader.read()
				if (chunk.done) {
					break
				}
				buffer += decoder.decode(chunk.value, { stream: true })
				const events = buffer.split("\n\n")
				buffer = events.pop() ?? ""
				for (const event of events) {
					this.dispatch(event)
				}
			}
		} catch (_error) {
			if (!this.closed) {
				this.onerror?.()
			}
		}
	}

	private dispatch(rawEvent: string): void {
		const eventName = rawEvent
			.split("\n")
			.find((line) => line.startsWith("event: "))
			?.slice("event: ".length)
		const data = rawEvent
			.split("\n")
			.find((line) => line.startsWith("data: "))
			?.slice("data: ".length)
		if (!eventName || data === undefined) {
			return
		}
		for (const listener of this.listeners.get(eventName) ?? []) {
			listener({ data } as MessageEvent<string>)
		}
	}
}

describe("PCadPart", () => {
	it("wraps part command application and exposes rebuilt serialized PCAD state", () => {
		const part = new PCadPart(createPartDocument())
		const document = part.applyCommand({ type: "setExtrudeDepth", extrudeId: "extrude-1", depth: 24 })

		expect(document.features[1]).toMatchObject({ type: "extrude", depth: 24 })
		expect(document.cad?.nodes.find((node) => node.id === "extrude-1")).toMatchObject({ type: "extrude", depth: 24 })
		expect(part.getPCadState().nodes.get("extrude-1")).toMatchObject({ type: "extrude", depth: 24 })
	})
})

describe("PCadProject", () => {
	it("syncs CAD commands through the same project server handlers used by the UI", async () => {
		const projectId = `pcad-project-test-${crypto.randomUUID()}`
		const client = new PuppyCadClient({ fetch: createServerFetch(projectId) })
		const clientA = new PCadProject({ projectId, clientId: "client-a", client })
		const clientB = new PCadProject({ projectId, clientId: "client-b", client })

		try {
			const saved = await clientA.putSnapshot(createProject(new PCadPart(createPartDocument()).getDocument()))
			expect(saved.revision).toBe(0)

			const loaded = await clientB.load()
			expect(loaded.project.items[0]).toMatchObject({ id: "part-1", type: "part" })

			const updated = await clientB.postCommand({
				type: "cad",
				partId: "part-1",
				command: { type: "setExtrudeDepth", extrudeId: "extrude-1", depth: 18 }
			})
			expect(updated.revision).toBe(1)

			const reloaded = await clientA.load()
			const partItem = reloaded.project.items[0]
			expect(partItem).toMatchObject({ id: "part-1", type: "part" })
			if (!partItem || !("type" in partItem) || partItem.type !== "part") {
				throw new Error("Expected part item")
			}
			expect(partItem.data?.features[1]).toMatchObject({ type: "extrude", depth: 18 })
			expect(partItem.data?.cad?.nodes.find((node) => node.id === "extrude-1")).toMatchObject({ type: "extrude", depth: 18 })
		} finally {
			await unlink(getProjectFileUrl(projectId)).catch(() => {})
		}
	})

	it("receives project changes from the real server event stream", async () => {
		const projectId = `pcad-project-events-test-${crypto.randomUUID()}`
		const client = new PuppyCadClient({
			fetch: createServerFetch(projectId),
			createEventSource: (url) => new TestServerEventSource(url, projectId)
		})
		const clientA = new PCadProject({ projectId, clientId: "client-a", client })
		const clientB = new PCadProject({ projectId, clientId: "client-b", client })

		try {
			await clientA.putSnapshot(createProject(new PCadPart(createPartDocument()).getDocument()))
			await new Promise<void>((resolve, reject) => {
				const timeout = setTimeout(() => reject(new Error("Timed out waiting for PCAD project event")), 1000)
				clientA.connectEvents({
					onConnected: () => {
						void clientB.postCommand({
							type: "cad",
							partId: "part-1",
							command: { type: "setExtrudeDepth", extrudeId: "extrude-1", depth: 32 }
						})
					},
					onProjectChanged: (result) => {
						if (result.originClientId !== "client-b") {
							return
						}
						const partItem = result.project.items[0]
						if (
							partItem &&
							"type" in partItem &&
							partItem.type === "part" &&
							partItem.data?.cad?.nodes.find((node) => node.id === "extrude-1" && node.type === "extrude" && node.depth === 32)
						) {
							clearTimeout(timeout)
							resolve()
						}
					},
					onError: () => {
						clearTimeout(timeout)
						reject(new Error("PCAD project event stream failed"))
					}
				})
			})
		} finally {
			clientA.disconnectEvents()
			await unlink(getProjectFileUrl(projectId)).catch(() => {})
		}
	})
})
