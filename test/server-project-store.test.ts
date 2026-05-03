import { afterEach, describe, expect, it } from "bun:test"
import { unlink } from "node:fs/promises"
import type { Project } from "../src/contract"
import { getProject, getProjectEvents, getProjectFileUrl, persistProject, postProjectCommands, putProject } from "../src/server/save-project"

const createdProjectIds: string[] = []

afterEach(async () => {
	await Promise.all(
		createdProjectIds.splice(0).map(async (projectId) => {
			try {
				await unlink(getProjectFileUrl(projectId))
			} catch {
				// best-effort cleanup
			}
		})
	)
})

describe("server project store", () => {
	it("GET loads a saved canonical project and returns 404 for missing files", async () => {
		const missing = await getProject(new Request("http://localhost/api/projects/missing-project"), "missing-project")
		expect(missing.status).toBe(404)

		const projectId = createProjectId()
		await persistProject(projectId, createProject())

		const response = await getProject(new Request(`http://localhost/api/projects/${projectId}`), projectId)
		expect(response.status).toBe(200)
		const body = (await response.json()) as { ok: boolean; revision: number; project: Project }
		expect(body.ok).toBe(true)
		expect(body.revision).toBe(0)
		expect(body.project.items[0]).toMatchObject({ id: "part-1", type: "part" })
	})

	it("POST commands persists the canonical project and increments revision", async () => {
		const projectId = createProjectId()
		await persistProject(projectId, createProject())

		const response = await postProjectCommands(
			new Request(`http://localhost/api/projects/${projectId}/commands`, {
				method: "POST",
				body: JSON.stringify({
					clientId: "client-a",
					baseRevision: 0,
					commands: [{ type: "renameNode", nodeId: "part-1", name: "Server Part" }]
				})
			}),
			projectId
		)

		expect(response.status).toBe(200)
		const body = (await response.json()) as { ok: boolean; revision: number; project: Project }
		expect(body.ok).toBe(true)
		expect(body.revision).toBe(1)
		expect(body.project.items[0]?.name).toBe("Server Part")

		const reloaded = await getProject(new Request(`http://localhost/api/projects/${projectId}`), projectId)
		const reloadedBody = (await reloaded.json()) as { project: Project }
		expect(reloadedBody.project.items[0]?.name).toBe("Server Part")
		expect(reloadedBody.project.revision).toBe(1)
	})

	it("PUT creates missing projects and increments existing canonical saves", async () => {
		const projectId = createProjectId()
		const createResponse = await putProject(createPutRequest(projectId, createProject(), "client-a"), projectId)
		expect(createResponse.status).toBe(200)
		const created = (await createResponse.json()) as { revision: number; project: Project }
		expect(created.revision).toBe(0)
		expect(created.project.items[0]?.name).toBe("Part")

		const nextProject = {
			...created.project,
			items: [{ id: "part-1", type: "part" as const, name: "Snapshot Part", data: { features: [] } }]
		}
		const updateResponse = await putProject(createPutRequest(projectId, nextProject, "client-a"), projectId)
		expect(updateResponse.status).toBe(200)
		const updated = (await updateResponse.json()) as { revision: number; project: Project }
		expect(updated.revision).toBe(1)
		expect(updated.project.items[0]?.name).toBe("Snapshot Part")
	})

	it("SSE subscribers receive full snapshot PUT broadcasts", async () => {
		const projectId = createProjectId()
		await persistProject(projectId, createProject())

		const events = await getProjectEvents(new Request(`http://localhost/api/projects/${projectId}/events?clientId=client-b`), projectId)
		const reader = events.body?.getReader()
		if (!reader) {
			throw new Error("Expected SSE body")
		}
		await readSseChunk(reader)

		await putProject(
			createPutRequest(
				projectId,
				{
					...createProject(),
					items: [{ id: "part-1", type: "part", name: "Snapshot Broadcast", data: { features: [] } }]
				},
				"client-a"
			),
			projectId
		)
		const changed = await readSseChunk(reader)
		expect(changed).toContain("event: projectChanged")
		expect(changed).toContain("Snapshot Broadcast")
		expect(changed).toContain('"commands":[]')
		await reader.cancel()
	})

	it("serializes concurrent command posts for the same project", async () => {
		const projectId = createProjectId()
		await persistProject(projectId, createProject())

		const [left, right] = await Promise.all([
			postProjectCommands(createCommandRequest(projectId, "left", [{ type: "createFolder", id: "folder-left", name: "Left" }]), projectId),
			postProjectCommands(createCommandRequest(projectId, "right", [{ type: "createFolder", id: "folder-right", name: "Right" }]), projectId)
		])
		expect(left.status).toBe(200)
		expect(right.status).toBe(200)

		const project = (await (await getProject(new Request(`http://localhost/api/projects/${projectId}`), projectId)).json()) as { project: Project }
		expect(project.project.revision).toBe(2)
		expect(project.project.items.map((item) => item.id).sort()).toEqual(["folder-left", "folder-right", "part-1"])
	})

	it("SSE subscribers receive accepted command batches", async () => {
		const projectId = createProjectId()
		await persistProject(projectId, createProject())

		const events = await getProjectEvents(new Request(`http://localhost/api/projects/${projectId}/events?clientId=client-b`), projectId)
		expect(events.status).toBe(200)
		const reader = events.body?.getReader()
		if (!reader) {
			throw new Error("Expected SSE body")
		}

		const connected = await readSseChunk(reader)
		expect(connected).toContain("event: connected")

		await postProjectCommands(createCommandRequest(projectId, "client-a", [{ type: "renameNode", nodeId: "part-1", name: "Broadcast Part" }]), projectId)
		const changed = await readSseChunk(reader)
		expect(changed).toContain("event: projectChanged")
		expect(changed).toContain("Broadcast Part")
		await reader.cancel()
	})
})

function createProjectId(): string {
	const projectId = `test-${crypto.randomUUID()}`
	createdProjectIds.push(projectId)
	return projectId
}

function createProject(): Project {
	return {
		version: 4,
		revision: 0,
		items: [{ id: "part-1", type: "part", name: "Part", data: { features: [] } }],
		selectedPath: [0]
	}
}

function createCommandRequest(projectId: string, clientId: string, commands: unknown[]): Request {
	return new Request(`http://localhost/api/projects/${projectId}/commands`, {
		method: "POST",
		body: JSON.stringify({
			clientId,
			baseRevision: 0,
			commands
		})
	})
}

function createPutRequest(projectId: string, project: Project, clientId: string): Request {
	return new Request(`http://localhost/api/projects/${projectId}`, {
		method: "PUT",
		headers: {
			"Content-Type": "application/json",
			"X-PuppyCAD-Client-Id": clientId
		},
		body: JSON.stringify(project)
	})
}

async function readSseChunk(reader: ReadableStreamDefaultReader<Uint8Array>): Promise<string> {
	const result = await reader.read()
	if (result.done || !result.value) {
		return ""
	}
	return new TextDecoder().decode(result.value)
}
