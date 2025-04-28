import index from "./index.html"
import styles from "./styles.css"

Bun.serve({
	port: 5337,
	routes: {
		"/": index,
		"/index.css": styles,
	}
})