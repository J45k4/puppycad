#!/usr/bin/env node

import { Command } from "commander"
import fs from "node:fs"
import path from "node:path"
import { createServer } from "./server"

const program = new Command()

program.name("puppycad").description("PuppyCAD command-line interface").version("0.1.0")

program.command("serve <folderPath>")
	.description("Serve static files from the specified folder")
	.option("-p, --port <port>", "Port to listen on", "3000")
	.action((folderPath: string, options: { port: string }) => {
		const fullPath = path.resolve(process.cwd(), folderPath)
		if (!fs.existsSync(fullPath) || !fs.statSync(fullPath).isDirectory()) {
			console.error(`Error: ${fullPath} is not a valid directory.`)
			process.exit(1)
		}
		const port = Number.parseInt(options.port, 10)
		if (Number.isNaN(port)) {
			console.error(`Error: ${options.port} is not a valid port.`)
			process.exit(1)
		}
		createServer(fullPath, port)
	})

program.parse(process.argv)
