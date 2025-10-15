import { mkdir } from "node:fs/promises"
import { existsSync } from "node:fs"
import { normalizeProjectFile, serializeProjectFile } from "../project-file"

const SAVE_DIRECTORY_URL = new URL("../../workdir/saved-projects/", import.meta.url)

async function ensureDirectory() {
	if (existsSync(SAVE_DIRECTORY_URL)) {
		return
	}
	await mkdir(SAVE_DIRECTORY_URL, { recursive: true })
}

export async function postProject(request: Request): Promise<Response> {
	let payload: unknown
	try {
		payload = await request.json()
	} catch (error) {
		console.error("Failed to parse project payload", error)
		return Response.json({ error: "Invalid JSON body" }, { status: 400 })
	}

	const projectFile = normalizeProjectFile(payload)
	if (!projectFile) {
		return Response.json({ error: "Invalid project file" }, { status: 400 })
	}

	const timestamp = new Date().toISOString().replace(/[:.]/g, "-")
	const fileName = `puppycad-project-${timestamp}.json`
	const fileUrl = new URL(fileName, SAVE_DIRECTORY_URL)

	try {
		await ensureDirectory()
		await Bun.write(fileUrl, serializeProjectFile(projectFile))
	} catch (error) {
		console.error("Failed to persist project", error)
		return Response.json({ error: "Unable to persist project" }, { status: 500 })
	}

	return Response.json({ status: "ok", fileName }, { status: 201 })
}
