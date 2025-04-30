import { promises as fs } from "fs"
import { pathToFileURL } from "url"
import path from "path"
import index from "./index.html"
import { Entity, Schematic } from "./puppycad"

/**
 * Dynamically loads all JS or TS modules from the given folder and returns their exports keyed by filename.
 */
export async function loadExportsFromFolder(folderPath: string): Promise<Record<string, any>> {
	const modules: Record<string, any> = {}
	const files = await fs.readdir(folderPath)
	for (const file of files) {
		const ext = file.split(".").pop()
		if (ext !== "js" && ext !== "ts") continue
		const fullPath = path.join(folderPath, file)
		const fileUrl = pathToFileURL(fullPath).href
		try {
			const mod = await import(fileUrl)
			modules[file] = mod
		} catch (e) {
			console.error(`Failed to import ${file}:`, e)
		}
	}
	return modules
}

export const createServer = async (folderPath: string, port?: number) => {
	// Load modules from the folder
	const modules = await loadExportsFromFolder(folderPath)

	// Flatten all named exports from each module into a single list
	const exportsList: any[] = []
	for (const mod of Object.values(modules)) {
		for (const exported of Object.values(mod)) {
			exportsList.push(exported)
		}
	}

	const visited = new Set<Entity>()
	const serialized: Record<string, any> = {}

	// Optionally, identify Schematic instances
	exportsList.forEach((item: Entity) => {
		if (item instanceof Entity) {
			item.visit(entity => {
				serialized[entity.id] = entity
			}, visited)
		}
	})

	// console.log("serialized", Array.from(serialized.values()))

	Bun.serve({
		port: port || 5337,
		routes: {
			"/": index,
			"/project/items": (req) => {
				return new Response(JSON.stringify(serialized), { status: 200 })
			}
		}
	})
}