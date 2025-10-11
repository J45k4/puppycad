import index from "./web/index.html"

console.log("Starting server on http://localhost:5337")

Bun.serve({
	port: 5337,
	routes: {
		"/": index
	}
})
