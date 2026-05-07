import { mkdtemp, readFile, unlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { afterEach, describe, expect, it } from "bun:test"
import type { PartProjectItemData, Project } from "../src/contract"
import { extrudeSolidFeature } from "../src/cad/extrude"
import { runPuppycadCli } from "../src/cli"
import { PCadPart } from "../src/pcad/project"
import { normalizeProjectFile } from "../src/project-file"
import { getHealth, getProject, getProjectFileUrl, getProjects, persistProject, postProject, postProjectCommands } from "../src/server/save-project"

const createdProjectIds: string[] = []

afterEach(async () => {
	await Promise.all(
		createdProjectIds.splice(0).map(async (projectId) => {
			await unlink(getProjectFileUrl(projectId)).catch(() => undefined)
		})
	)
})

function createOutput() {
	const stdout: string[] = []
	const stderr: string[] = []
	return {
		output: {
			stdout: (message: string) => stdout.push(message),
			stderr: (message: string) => stderr.push(message)
		},
		stdout,
		stderr
	}
}

async function createTempDir(): Promise<string> {
	return mkdtemp(join(tmpdir(), "puppycad-cli-"))
}

describe("puppycad CLI", () => {
	it("prints top-level help", async () => {
		const { output, stdout, stderr } = createOutput()
		const code = await runPuppycadCli(["--help"], { output, version: "9.9.9" })

		expect(code).toBe(0)
		expect(stderr).toEqual([])
		expect(stdout.join("\n")).toContain("Usage: puppycad [global options] <command> [options]")
		expect(stdout.join("\n")).toContain("init [file]")
		expect(stdout.join("\n")).toContain("inspect [project-id|file]")
	})

	it("prints the configured version", async () => {
		const { output, stdout, stderr } = createOutput()
		const code = await runPuppycadCli(["--version"], { output, version: "9.9.9" })

		expect(code).toBe(0)
		expect(stderr).toEqual([])
		expect(stdout).toEqual(["9.9.9"])
	})

	it("creates a project file with an initial PCad part", async () => {
		const cwd = await createTempDir()
		const { output, stdout, stderr } = createOutput()
		const code = await runPuppycadCli(["init", "starter.pcad", "--part-name", "Bracket"], { cwd, output })

		expect(code).toBe(0)
		expect(stderr).toEqual([])
		expect(stdout[0]).toContain(join(cwd, "starter.pcad"))

		const project = normalizeProjectFile(JSON.parse(await readFile(join(cwd, "starter.pcad"), "utf8")))
		expect(project).not.toBeNull()
		expect(project?.selectedPath).toEqual([0])
		expect(project?.items[0]).toMatchObject({
			id: "part-1",
			type: "part",
			name: "Bracket"
		})
	})

	it("creates a constrained cube in a local file through cad commands", async () => {
		const cwd = await createTempDir()
		const output = createOutput()
		const run = async (args: string[]) => runPuppycadCli(args, { cwd, output: output.output })

		expect(await run(["init", "cube.pcad", "--part-name", "10mm constrained cube"])).toBe(0)
		expect(await run(["cad", "sketch", "create", "cube.pcad", "--part", "part-1", "--sketch", "sketch-cube-base", "--plane", "XY", "--name", "Cube base sketch"])).toBe(0)
		expect(
			await run([
				"cad",
				"sketch",
				"rectangle",
				"cube.pcad",
				"--part",
				"part-1",
				"--sketch",
				"sketch-cube-base",
				"--entity",
				"cube-base-rect",
				"--x0",
				"0",
				"--y0",
				"0",
				"--x1",
				"8",
				"--y1",
				"6"
			])
		).toBe(0)
		expect(
			await run([
				"cad",
				"dimension",
				"set",
				"cube.pcad",
				"--part",
				"part-1",
				"--sketch",
				"sketch-cube-base",
				"--entity",
				"cube-base-rect",
				"--type",
				"rectangleWidth",
				"--value",
				"10",
				"--id",
				"dim-cube-width-10mm"
			])
		).toBe(0)
		expect(
			await run([
				"cad",
				"dimension",
				"set",
				"cube.pcad",
				"--part",
				"part-1",
				"--sketch",
				"sketch-cube-base",
				"--entity",
				"cube-base-rect",
				"--type",
				"rectangleHeight",
				"--value",
				"10",
				"--id",
				"dim-cube-height-10mm"
			])
		).toBe(0)
		expect(await run(["cad", "sketch", "finish", "cube.pcad", "--part", "part-1", "--sketch", "sketch-cube-base"])).toBe(0)
		expect(
			await run([
				"cad",
				"extrude",
				"create",
				"cube.pcad",
				"--part",
				"part-1",
				"--extrude",
				"extrude-cube-10mm",
				"--sketch",
				"sketch-cube-base",
				"--depth",
				"10",
				"--name",
				"10mm cube"
			])
		).toBe(0)
		expect(output.stderr).toEqual([])

		const project = normalizeProjectFile(JSON.parse(await readFile(join(cwd, "cube.pcad"), "utf8")))
		const part = project?.items[0]
		if (!part || !("type" in part) || part.type !== "part" || !part.data) {
			throw new Error("Expected part item")
		}
		const sketch = part.data.features.find((feature) => feature.type === "sketch")
		const extrude = part.data.features.find((feature) => feature.type === "extrude")
		expect(sketch).toMatchObject({
			id: "sketch-cube-base",
			dimensions: [
				{ id: "dim-cube-width-10mm", type: "rectangleWidth", entityId: "cube-base-rect", value: 10 },
				{ id: "dim-cube-height-10mm", type: "rectangleHeight", entityId: "cube-base-rect", value: 10 }
			]
		})
		if (!extrude || extrude.type !== "extrude") {
			throw new Error("Expected extrude")
		}
		const { solid } = extrudeSolidFeature(part.data, extrude)
		const xs = solid.vertices.map((vertex) => vertex.position.x)
		const ys = solid.vertices.map((vertex) => vertex.position.y)
		const zs = solid.vertices.map((vertex) => vertex.position.z)
		expect(Math.max(...xs) - Math.min(...xs)).toBe(10)
		expect(Math.max(...ys) - Math.min(...ys)).toBe(10)
		expect(Math.max(...zs) - Math.min(...zs)).toBe(10)
	})

	it("refuses to overwrite existing files without --force", async () => {
		const cwd = await createTempDir()
		await writeFile(join(cwd, "starter.pcad"), "{}", "utf8")
		const { output, stderr } = createOutput()
		const code = await runPuppycadCli(["init", "starter.pcad"], { cwd, output })

		expect(code).toBe(1)
		expect(stderr.join("\n")).toContain("already exists")
	})

	it("inspects and summarizes a normalized project file", async () => {
		const cwd = await createTempDir()
		const initOutput = createOutput()
		await runPuppycadCli(["init", "starter.pcad", "--part-name", "Bracket"], { cwd, output: initOutput.output })

		const { output, stdout, stderr } = createOutput()
		const code = await runPuppycadCli(["inspect", "starter.pcad"], { cwd, output })

		expect(code).toBe(0)
		expect(stderr).toEqual([])
		expect(stdout.join("\n")).toContain("Version: 4")
		expect(stdout.join("\n")).toContain("part: 1")
		expect(stdout.join("\n")).toContain("Bracket (part-1): 0 features")
	})

	it("returns an error for invalid project files", async () => {
		const cwd = await createTempDir()
		await writeFile(join(cwd, "bad.pcad"), '{"version":99,"items":[]}', "utf8")
		const { output, stderr } = createOutput()
		const code = await runPuppycadCli(["inspect", "bad.pcad"], { cwd, output })

		expect(code).toBe(1)
		expect(stderr.join("\n")).toContain("Invalid PuppyCAD project file")
	})

	it("writes server-first config keys to the standard JSON shape", async () => {
		const cwd = await createTempDir()
		const configPath = join(cwd, "config.json")
		const outputA = createOutput()
		const outputB = createOutput()

		expect(await runPuppycadCli(["config", "set", "server-url", "http://server.test"], { cwd, output: outputA.output, env: { PUPPYCAD_CONFIG_PATH: configPath } })).toBe(0)
		expect(await runPuppycadCli(["config", "set", "default-project", "project-1"], { cwd, output: outputB.output, env: { PUPPYCAD_CONFIG_PATH: configPath } })).toBe(0)

		expect(JSON.parse(await readFile(configPath, "utf8"))).toEqual({
			serverUrl: "http://server.test",
			defaultProject: "project-1"
		})
	})

	it("creates, lists, and inspects projects through server handlers", async () => {
		const { output, stdout, stderr } = createOutput()
		const fetch = createServerFetch()
		const createCode = await runPuppycadCli(["--server-url", "http://server.test", "project", "create", "Bracket", "--json"], { output, fetch })

		expect(createCode).toBe(0)
		expect(stderr).toEqual([])
		const created = JSON.parse(stdout.at(-1) ?? "{}") as { projectId?: string; project?: Project }
		expect(created.projectId).toBeTruthy()
		if (!created.projectId) {
			throw new Error("Expected created project id")
		}
		createdProjectIds.push(created.projectId)
		expect(created.project?.items[0]?.name).toBe("Bracket")

		const listOutput = createOutput()
		const listCode = await runPuppycadCli(["--server-url", "http://server.test", "project", "list", "--json"], { output: listOutput.output, fetch })
		expect(listCode).toBe(0)
		const listed = JSON.parse(listOutput.stdout.join("\n")) as { projects: { projectId: string }[] }
		expect(listed.projects.some((project) => project.projectId === created.projectId)).toBe(true)

		const inspectOutput = createOutput()
		const inspectCode = await runPuppycadCli(["--server-url", "http://server.test", "project", "inspect", created.projectId, "--json"], { output: inspectOutput.output, fetch })
		expect(inspectCode).toBe(0)
		const inspected = JSON.parse(inspectOutput.stdout.join("\n")) as { projectId: string; stats: { parts: { name: string }[] } }
		expect(inspected.projectId).toBe(created.projectId)
		expect(inspected.stats.parts[0]?.name).toBe("Bracket")
	})

	it("queries features, graph, and eval through server project loads", async () => {
		const projectId = `cli-server-test-${crypto.randomUUID()}`
		createdProjectIds.push(projectId)
		await persistProject(projectId, createProject(new PCadPart(createPartDocument()).getDocument()))

		const fetch = createServerFetch()
		const featuresOutput = createOutput()
		const featuresCode = await runPuppycadCli(["--server-url", "http://server.test", "query", "features", projectId, "--json"], { output: featuresOutput.output, fetch })
		expect(featuresCode).toBe(0)
		const features = JSON.parse(featuresOutput.stdout.join("\n")) as { projectId: string; features: { id: string; type: string; name?: string; partId: string; status: string }[] }
		expect(features.projectId).toBe(projectId)
		expect(features.features).toContainEqual({ id: "sketch-1", type: "sketch", name: "Sketch 1", partId: "part-1", status: "ok" })
		expect(features.features).toContainEqual({ id: "extrude-1", type: "extrude", name: "Extrude 1", partId: "part-1", status: "ok" })

		const graphOutput = createOutput()
		const graphCode = await runPuppycadCli(["--server-url", "http://server.test", "graph", projectId, "--json"], { output: graphOutput.output, fetch })
		expect(graphCode).toBe(0)
		const graph = JSON.parse(graphOutput.stdout.join("\n")) as { nodes: { id: string }[]; edges: { from: string; to: string; partId: string; type: string }[] }
		expect(graph.nodes.map((node) => node.id).sort()).toEqual(["extrude-1", "sketch-1"])
		expect(graph.edges).toContainEqual({ from: "sketch-1", to: "extrude-1", partId: "part-1", type: "dependency" })

		const evalOutput = createOutput()
		const evalCode = await runPuppycadCli(["--server-url", "http://server.test", "eval", projectId, "--json"], { output: evalOutput.output, fetch })
		expect(evalCode).toBe(0)
		expect(JSON.parse(evalOutput.stdout.join("\n"))).toMatchObject({ status: "ok", projectId, features: 2 })
	})

	it("sets sketch dimension constraints through server commands", async () => {
		const projectId = `cli-dimension-test-${crypto.randomUUID()}`
		createdProjectIds.push(projectId)
		await persistProject(projectId, createProject(new PCadPart(createPartDocument()).getDocument()))

		const fetch = createServerFetch()
		const output = createOutput()
		const code = await runPuppycadCli(
			[
				"--server-url",
				"http://server.test",
				"--json",
				"cad",
				"dimension",
				"set",
				projectId,
				"--part",
				"part-1",
				"--sketch",
				"sketch-1",
				"--entity",
				"rect-1",
				"--type",
				"rectangleWidth",
				"--value",
				"12",
				"--id",
				"dim-width"
			],
			{ output: output.output, fetch }
		)

		expect(code).toBe(0)
		expect(output.stderr).toEqual([])
		expect(JSON.parse(output.stdout.join("\n"))).toMatchObject({
			projectId,
			partId: "part-1",
			sketchId: "sketch-1",
			dimension: { id: "dim-width", type: "rectangleWidth", entityId: "rect-1", value: 12 }
		})

		const response = await getProject(new Request(`http://server.test/api/projects/${projectId}`), projectId)
		const payload = (await response.json()) as { project?: Project }
		const part = payload.project?.items[0]
		if (!part || !("type" in part) || part.type !== "part" || !part.data) {
			throw new Error("Expected part item")
		}
		const partData = part.data
		const sketch = partData.features.find((feature) => feature.type === "sketch")
		expect(sketch).toMatchObject({
			type: "sketch",
			id: "sketch-1",
			dimensions: [{ id: "dim-width", type: "rectangleWidth", entityId: "rect-1", value: 12 }]
		})
		expect(partData.cad?.nodes).toContainEqual({
			id: "dim-width",
			type: "sketchConstraint",
			sketchId: "sketch-1",
			constraint: { type: "rectangleWidth", entityId: "rect-1", value: 12 }
		})
	})

	it("queries generated geometry from server project snapshots", async () => {
		const projectId = `cli-geometry-test-${crypto.randomUUID()}`
		createdProjectIds.push(projectId)
		await persistProject(projectId, createProject(new PCadPart(createPartDocument()).getDocument()))

		const fetch = createServerFetch()
		const geometryOutput = createOutput()
		const geometryCode = await runPuppycadCli(["--server-url", "http://server.test", "query", "geometry", projectId, "--json"], { output: geometryOutput.output, fetch })
		expect(geometryCode).toBe(0)
		const geometry = JSON.parse(geometryOutput.stdout.join("\n")) as {
			projectId: string
			bodies: { id: string; sourceId: string; vertices: unknown[]; edges: unknown[]; faces: unknown[]; bbox: { size: { x: number; y: number; z: number } } }[]
			errors: unknown[]
		}
		expect(geometry.projectId).toBe(projectId)
		expect(geometry.errors).toEqual([])
		expect(geometry.bodies).toHaveLength(1)
		expect(geometry.bodies[0]).toMatchObject({ id: "extrude-1-solid", sourceId: "extrude-1" })
		expect(geometry.bodies[0]?.vertices).toHaveLength(8)
		expect(geometry.bodies[0]?.edges).toHaveLength(16)
		expect(geometry.bodies[0]?.faces).toHaveLength(6)
		expect(geometry.bodies[0]?.bbox.size).toEqual({ x: 10, y: 10, z: 10 })

		const facesOutput = createOutput()
		const facesCode = await runPuppycadCli(["--server-url", "http://server.test", "query", "faces", projectId, "--body", "extrude-1-solid", "--json"], {
			output: facesOutput.output,
			fetch
		})
		expect(facesCode).toBe(0)
		expect(JSON.parse(facesOutput.stdout.join("\n"))).toMatchObject({ faces: expect.arrayContaining([expect.objectContaining({ bodyId: "extrude-1-solid" })]) })
	})

	it("renders a server project preview to a PNG path", async () => {
		const cwd = await createTempDir()
		const projectId = `cli-render-test-${crypto.randomUUID()}`
		createdProjectIds.push(projectId)
		const partDocument = createPartDocument()
		const sketch = partDocument.features.find((feature) => feature.type === "sketch")
		if (sketch?.type === "sketch") {
			sketch.dimensions = [{ id: "dim-width", type: "rectangleWidth", entityId: "rect-1", value: 10 }]
		}
		await persistProject(projectId, createProject(new PCadPart(partDocument).getDocument()))

		const fetch = createServerFetch()
		const output = createOutput()
		const pngBytes = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10])
		const code = await runPuppycadCli(["--server-url", "http://server.test", "render", projectId, "--out", "preview.png", "--width", "320", "--height", "240", "--show-dimensions"], {
			cwd,
			output: output.output,
			fetch,
			renderPng: async (bodies, options) => {
				expect(bodies).toHaveLength(1)
				expect(options.width).toBe(320)
				expect(options.height).toBe(240)
				expect(options.labels).toHaveLength(1)
				expect(options.labels?.[0]?.text).toBe("10mm")
				expect(options.labels?.[0]?.position.x).toBe(5)
				expect(options.labels?.[0]?.position.y).toBeCloseTo(-1.8)
				expect(options.labels?.[0]?.position.z).toBe(0)
				return pngBytes
			}
		})

		expect(code).toBe(0)
		expect(output.stderr).toEqual([])
		expect(output.stdout.join("\n")).toContain("Rendered")
		expect(new Uint8Array(await readFile(join(cwd, "preview.png")))).toEqual(pngBytes)
	})
})

function createServerFetch(): (input: RequestInfo | URL, init?: RequestInit) => Promise<Response> {
	return async (input, init) => {
		const url = new URL(input.toString(), "http://server.test")
		const request = new Request(url, init)
		if (url.pathname === "/health" && request.method === "GET") {
			return getHealth(request)
		}
		if (url.pathname === "/api/projects" && request.method === "GET") {
			return getProjects(request)
		}
		if (url.pathname === "/api/projects" && request.method === "POST") {
			return postProject(request)
		}
		const projectMatch = url.pathname.match(/^\/api\/projects\/([^/]+)$/)
		if (projectMatch?.[1] && request.method === "GET") {
			return getProject(request, decodeURIComponent(projectMatch[1]))
		}
		const commandMatch = url.pathname.match(/^\/api\/projects\/([^/]+)\/commands$/)
		if (commandMatch?.[1] && request.method === "POST") {
			return postProjectCommands(request, decodeURIComponent(commandMatch[1]))
		}
		return Response.json({ ok: false, message: `Unhandled test route ${request.method} ${url.pathname}` }, { status: 404 })
	}
}

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
