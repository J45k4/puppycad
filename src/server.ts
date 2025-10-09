import { promises as fs } from "fs"
import { pathToFileURL } from "url"
import path from "path"
import index from "./index.html"
import { Entity, Schematic } from "./puppycad"


Bun.serve({
	port: 5337,
	routes: {
		"/": index,
		"/project/items": (req) => {
			return new Response(JSON.stringify(serialized), { status: 200 })
		}
	}
})