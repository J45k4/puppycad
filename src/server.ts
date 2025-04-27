import index from "./index.html"

Bun.serve({
	port: 5337,
	routes: {
		"/": index,
	}
})