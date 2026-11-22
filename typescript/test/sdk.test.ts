import { requireValue } from "../src/required"
import { afterEach, describe, expect, it } from "bun:test"
import { unlink } from "node:fs/promises"
import { Vector3 } from "three"
import { AssemblyBuilder, PartBuilder, PuppyCad, circle, rectangle, v2, transformMatrix, defineModel } from "../src/sdk"
import { exportPartStl } from "../src/part-mesh"
import { getProject, getProjectEvents, getProjectFileUrl, loadProject, postProject, postProjectCommands, postProjectUndo, putProject } from "../src/server/save-project"
import { buildFlowerHolder } from "../examples/flower-holder-live"
import type { ProjectPartDocument } from "../src/contract"
const ids: string[] = []
afterEach(async () => {
	await Promise.all(ids.splice(0).map((id) => unlink(getProjectFileUrl(id))))
})
const serverFetch = async (input: RequestInfo | URL, init?: RequestInit) => {
	const request = new Request(input, init)
	const url = new URL(request.url)
	if (url.pathname === "/api/projects" && request.method === "POST") {
		const response = await postProject(request)
		ids.push(
			(
				(await response.clone().json()) as {
					projectId: string
				}
			).projectId
		)
		return response
	}
	const match = url.pathname.match(/^\/api\/projects\/([^/]+)(\/commands)?$/)
	if (!match?.[1]) return new Response("Not found", { status: 404 })
	return match[2] ? postProjectCommands(request, decodeURIComponent(match[1])) : getProject(request, decodeURIComponent(match[1]))
}
const cad = new PuppyCad({ fetch: serverFetch })
describe("live SDK", () => {
	it("persists incremental updates, broadcasts them, preserves unrelated edits and supports undo", async () => {
		const project = await cad.createProject()
		const events = await getProjectEvents(new Request("http://localhost/events"), project.id)
		const reader = requireValue(events.body).getReader()
		await reader.read()
		try {
			await project.part("base", { outline: rectangle(v2(0, 0), 20, 10), depth: 3 })
			const event = new TextDecoder().decode((await reader.read()).value)
			expect(event).toContain('"type":"projectChanged"')
			expect(event).toContain('"revision":1')
			const other = await cad.openProject(project.id)
			await other.commands([{ type: "createItem", id: "manual", documentType: "part", name: "Manual work" }])
			await project.part("base", { outline: rectangle(v2(0, 0), 20, 10), depth: 7 })
			const saved = requireValue(await loadProject(project.id))
			expect(saved.items.map((item) => item.id)).toEqual(["base", "manual"])
			expect(requireValue((saved.items[0] as ProjectPartDocument).data).features[1]).toMatchObject({ depth: 7 })
			await postProjectUndo(new Request("http://localhost/undo", { method: "POST", body: JSON.stringify({ clientId: "test", baseRevision: saved.revision }) }), project.id)
			expect(requireValue((requireValue(await loadProject(project.id)).items[0] as ProjectPartDocument).data).features[1]).toMatchObject({ depth: 3 })
		} finally {
			await reader.cancel()
		}
	})
	it("applies an existing model as one server transaction", async () => {
		const project = await cad.createProject()
		const model = defineModel({ id: "legacy", name: "Legacy model" }, (scope) => {
			scope.body("base", { outline: rectangle(v2(0, 0), 10, 10), depth: 2 })
		})
		await project.model(model)
		expect(project.snapshot?.revision).toBe(1)
		expect(project.snapshot?.items.map((item) => item.id)).toEqual(["legacy/base", "legacy/assembly"])
	})

	it("rejects a bad assembly atomically, then accepts the next operation", async () => {
		const project = await cad.createProject()
		const snapshot = await loadProject(project.id)
		await expect(
			project.commands([
				{ type: "createItem", id: "rolled-back", documentType: "part" },
				{
					type: "upsertDocument",
					document: { id: "invalid", type: "assembly", name: "Invalid", data: { id: "invalid", name: "Invalid", instances: [{ id: "instance", partId: "missing" }] } }
				}
			])
		).rejects.toThrow("does not exist")
		expect(await loadProject(project.id)).toEqual(snapshot)
		await project.part("ok", { outline: circle(v2(0, 0), 5), depth: 2 })
		expect(requireValue(await loadProject(project.id)).items).toHaveLength(1)
	})
	it("rejects stale whole-project saves after an SDK edit", async () => {
		const project = await cad.createProject()
		const stale = await loadProject(project.id)
		await project.part("live", { outline: circle(v2(0, 0), 5), depth: 3 })
		const response = await putProject(new Request("http://localhost/project", { method: "PUT", body: JSON.stringify(stale) }), project.id)
		expect(response.status).toBe(409)
		expect((await loadProject(project.id))?.items[0]?.id).toBe("live")
	})

	it("builds reusable printable parts and reruns without duplicates", async () => {
		const project = await cad.createProject()
		await buildFlowerHolder(project)
		const first = requireValue(await loadProject(project.id))
		expect(first.items).toHaveLength(6)
		const assembly = requireValue(first.items.find((node) => !("kind" in node) && node.type === "assembly"))
		if ("kind" in assembly || assembly.type !== "assembly") throw new Error("Expected assembly")
		expect(requireValue(assembly.data).instances).toHaveLength(5)
		expect(requireValue(assembly.data).mates).toHaveLength(9)
		expect(requireValue(requireValue(requireValue(requireValue(assembly.data).instances.find((instance) => instance.id === "centre-holder")).transform).translation).z).toBeCloseTo(
			321.58972024917603
		)
		for (const node of first.items) {
			if (!("kind" in node) && node.type === "part") {
				const stl = await project.exportStl({ kind: "part", id: node.id })
				expectClosedMesh(stl)
			}
		}
		await buildFlowerHolder(project)
		expect(requireValue(await loadProject(project.id)).items).toEqual(first.items)
	}, 60000)
})
describe("part and assembly builders", () => {
	it("keeps a hole open in a closed printable mesh", () => {
		const builder = new PartBuilder()
		builder.extrude("ring", { outline: rectangle(v2(0, 0), 20, 20), holes: [rectangle(v2(0, 0), 10, 10)], depth: 5 })
		const stl = exportPartStl(builder.document)
		expectClosedMesh(stl)
		const vertices = parseVertices(stl)
		let volume = 0
		for (let i = 0; i < vertices.length; i += 3) volume += requireValue(vertices[i]).dot(new Vector3().crossVectors(requireValue(vertices[i + 1]), requireValue(vertices[i + 2]))) / 6
		expect(volume).toBeCloseTo(1500)
	})
	it("exports face-attached features as a fused closed STL", () => {
		const builder = new PartBuilder()
		const top = builder.extrude("base", { outline: circle(v2(0, 0), 20), depth: 4 })
		builder.extrude("wall", { outline: circle(v2(0, 0), 20), holes: [circle(v2(0, 0), 17)], depth: 30, on: top })
		expectClosedMesh(exportPartStl(builder.document))
	})
	it("aligns fixed connectors in 3D without moving the part definition", () => {
		const builder = new AssemblyBuilder("a")
		const parent = builder.instance("parent", { kind: "part", id: "p" }, { translation: { x: 10, y: 20, z: 30 }, rotation: { x: 0, y: 0, z: 90 } })
		const child = builder.instance("child", { kind: "part", id: "p" })
		builder.fasten("mate", builder.connector("a", parent, { x: 5, y: 0, z: 2 }), builder.connector("b", child, { x: 0, y: 0, z: 3 }))
		const actual = new Vector3(0, 0, 3).applyMatrix4(transformMatrix(requireValue(builder.data.instances[1]).transform))
		expect(actual.x).toBeCloseTo(10)
		expect(actual.y).toBeCloseTo(25)
		expect(actual.z).toBeCloseTo(32)
		expect(requireValue(builder.data.instances[1]).partId).toBe("p")
	})
})
function parseVertices(stl: string): Vector3[] {
	return [...stl.matchAll(/^vertex (\S+) (\S+) (\S+)$/gm)].map((match) => new Vector3(Number(match[1]), Number(match[2]), Number(match[3])))
}
function expectClosedMesh(stl: string): void {
	const vertices = parseVertices(stl)
	expect(vertices.length).toBeGreaterThan(0)
	const key = (p: Vector3) =>
		p
			.toArray()
			.map((value) => value.toFixed(4))
			.join(",")
	const edges = new Map<string, number>()
	for (let i = 0; i < vertices.length; i += 3)
		for (let j = 0; j < 3; j++) {
			const a = key(requireValue(vertices[i + j]))
			const b = key(requireValue(vertices[i + ((j + 1) % 3)]))
			const edge = [a, b].sort().join("/")
			edges.set(edge, (edges.get(edge) ?? 0) + 1)
		}
	expect([...edges.values()].every((count) => count === 2)).toBe(true)
}
