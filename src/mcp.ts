type JSONRPCId = string | number | null

interface JSONRPCRequest {
	jsonrpc: string
	id?: JSONRPCId
	method: string
	params?: unknown
}

interface JSONRPCSuccess {
	jsonrpc: "2.0"
	id: JSONRPCId
	result: unknown
}

interface JSONRPCError {
	jsonrpc: "2.0"
	id: JSONRPCId | null
	error: {
		code: number
		message: string
		data?: unknown
	}
}

type JSONRPCResponse = JSONRPCSuccess | JSONRPCError

interface ResourceDefinition {
	readonly uri: string
	readonly name: string
	readonly description: string
	readonly mimeType: string
	readonly filePath: string
}

const serverInfo = {
	name: "puppycad-mcp",
	version: "0.1.0"
}

const protocolVersion = "2024-06-25"

const resourceDefinitions: ResourceDefinition[] = [
	{
		uri: "puppycad://examples/puppybot",
		name: "PuppyBot example project",
		description: "Shows how PuppyCAD constructs a small PCB example.",
		mimeType: "text/typescript",
		filePath: "examples/puppybot.ts"
	}
]

const resourcesByUri = new Map<string, ResourceDefinition>(resourceDefinitions.map((definition) => [definition.uri, definition]))

function isResponse(value: unknown): value is Response {
	return typeof value === "object" && value !== null && typeof (value as Response).text === "function"
}

function createError(id: JSONRPCId | null, code: number, message: string, data?: unknown): JSONRPCError {
	return {
		jsonrpc: "2.0",
		id,
		error: {
			code,
			message,
			data
		}
	}
}

function createResult(id: JSONRPCId, result: unknown): JSONRPCSuccess {
	return {
		jsonrpc: "2.0",
		id,
		result
	}
}

export async function handleMCPRequest(payload: JSONRPCRequest): Promise<JSONRPCResponse | null> {
	if (payload.jsonrpc !== "2.0") {
		return createError(payload.id ?? null, -32600, "Invalid Request: jsonrpc must be '2.0'")
	}

	const id = payload.id ?? null
	const { method } = payload

	switch (method) {
		case "initialize": {
			if (id === null) {
				return createError(id, -32600, "Invalid Request: initialize must include an id")
			}

			return createResult(id, {
				protocolVersion,
				serverInfo,
				capabilities: {
					resources: {
						list: true,
						read: true
					},
					tools: {
						list: true,
						call: false
					}
				}
			})
		}
		case "ping": {
			if (id === null) {
				return null
			}

			return createResult(id, { ok: true })
		}
		case "resources/list": {
			if (id === null) {
				return null
			}

			return createResult(id, {
				resources: resourceDefinitions.map((definition) => ({
					uri: definition.uri,
					name: definition.name,
					description: definition.description,
					mimeType: definition.mimeType
				}))
			})
		}
		case "resources/read": {
			if (id === null) {
				return null
			}

			const params = payload.params
			if (typeof params !== "object" || params === null || !("uri" in params)) {
				return createError(id, -32602, "Invalid params: expected object with uri")
			}

			const uri = (params as { uri: unknown }).uri
			if (typeof uri !== "string") {
				return createError(id, -32602, "Invalid params: uri must be a string")
			}

			const definition = resourcesByUri.get(uri)
			if (!definition) {
				return createError(id, -32004, `Unknown resource: ${uri}`)
			}

			const file = Bun.file(definition.filePath)
			let text: string
			try {
				text = await file.text()
			} catch (error) {
				const message = error instanceof Error ? error.message : String(error)
				return createError(id, -32004, `Unable to load resource: ${message}`)
			}
			return createResult(id, {
				contents: [
					{
						uri: definition.uri,
						mimeType: definition.mimeType,
						text
					}
				]
			})
		}
		case "tools/list": {
			if (id === null) {
				return null
			}

			return createResult(id, {
				tools: []
			})
		}
		default: {
			return createError(id, -32601, `Method not found: ${method}`)
		}
	}
}

export async function handleJSONRPC(body: unknown): Promise<Response> {
	const createResponse = (payload: JSONRPCResponse | null) => (payload === null ? null : JSON.stringify(payload))

	if (Array.isArray(body)) {
		const responses: (string | null)[] = await Promise.all(
			body.map(async (entry) => {
				if (typeof entry !== "object" || entry === null) {
					return JSON.stringify(createError(null, -32600, "Invalid Request"))
				}

				const response = await handleMCPRequest(entry as JSONRPCRequest)
				return createResponse(response)
			})
		)

		const filtered = responses.filter((value): value is string => value !== null)
		if (filtered.length === 0) {
			return new Response(null, { status: 204 })
		}

		return new Response(`[${filtered.join(",")}]`, {
			headers: {
				"content-type": "application/json"
			}
		})
	}

	if (typeof body !== "object" || body === null) {
		return new Response(JSON.stringify(createError(null, -32600, "Invalid Request")), {
			status: 400,
			headers: {
				"content-type": "application/json"
			}
		})
	}

	const response = await handleMCPRequest(body as JSONRPCRequest)
	if (response === null) {
		return new Response(null, { status: 204 })
	}

	return new Response(JSON.stringify(response), {
		headers: {
			"content-type": "application/json"
		}
	})
}

export const postMcp = async (req: Request) => {
	let body: unknown
	try {
		body = await req.json()
	} catch (error) {
		const message = error instanceof Error ? error.message : "Unable to parse JSON body"
		return new Response(JSON.stringify(createError(null, -32700, `Parse error: ${message}`)), {
			status: 400,
			headers: {
				"content-type": "application/json"
			}
		})
	}

	return handleJSONRPC(body)
}