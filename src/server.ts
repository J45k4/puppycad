import index from "./index.html"
import { postMcp } from "./mcp"

console.log("Starting server on http://localhost:5337")

Bun.serve({
	port: 5337,
	routes: {
		"/mcp": {
			POST: postMcp
		},
		"/*": index
	},
	fetch() {
		return new Response("Not Found", { status: 404 })
	}
})
