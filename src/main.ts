import index from "./ui/index.html"
import { postMcp } from "./mcp"
import { postProject } from "./server/save-project"

console.log("Starting server on http://localhost:5337")

const pngHeaders = {
	"Content-Type": "image/png"
}

const manifestHeaders = {
	"Content-Type": "application/manifest+json"
}

Bun.serve({
	port: 5337,
	routes: {
		"/favicon.png": {
			GET: () => new Response(Bun.file(new URL("./ui/favicon.png", import.meta.url)), { headers: pngHeaders })
		},
		"/icon-192.png": {
			GET: () => new Response(Bun.file(new URL("./ui/icon-192.png", import.meta.url)), { headers: pngHeaders })
		},
		"/icon-512.png": {
			GET: () => new Response(Bun.file(new URL("./ui/icon-512.png", import.meta.url)), { headers: pngHeaders })
		},
		"/manifest.webmanifest": {
			GET: () => new Response(Bun.file(new URL("./ui/manifest.webmanifest", import.meta.url)), { headers: manifestHeaders })
		},
		"/mcp": {
			POST: postMcp
		},
		"/api/projects": {
			POST: postProject
		},
		"/*": index
	},
	fetch() {
		return new Response("Not Found", { status: 404 })
	}
})
