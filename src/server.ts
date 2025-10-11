import { stat } from "node:fs/promises"
import { join, normalize, sep } from "node:path"

const isWithinRoot = (root: string, candidate: string) => {
	const normalizedRoot = root.endsWith(sep) ? root : `${root}${sep}`
	return candidate === root || candidate.startsWith(normalizedRoot)
}

export const createServer = (rootDir: string, port: number) => {
	const normalizedRoot = normalize(rootDir)

	return Bun.serve({
		port,
		async fetch(request) {
			const url = new URL(request.url)
			const decodedPath = decodeURIComponent(url.pathname)
			const initialRelative = decodedPath === "/" ? "/index.html" : decodedPath
			let resolvedPath = normalize(join(normalizedRoot, initialRelative))

			if (!isWithinRoot(normalizedRoot, resolvedPath)) {
				return new Response("Not Found", { status: 404 })
			}

			try {
				let fileStats = await stat(resolvedPath)
				if (fileStats.isDirectory()) {
					resolvedPath = normalize(join(resolvedPath, "index.html"))
					if (!isWithinRoot(normalizedRoot, resolvedPath)) {
						return new Response("Not Found", { status: 404 })
					}
					fileStats = await stat(resolvedPath)
					if (!fileStats.isFile()) {
						return new Response("Not Found", { status: 404 })
					}
				}
				if (!fileStats.isFile()) {
					return new Response("Not Found", { status: 404 })
				}
				return new Response(Bun.file(resolvedPath))
			} catch {
				return new Response("Not Found", { status: 404 })
			}
		}
	})
}
