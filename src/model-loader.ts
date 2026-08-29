import { stat } from "node:fs/promises"
import { pathToFileURL } from "node:url"
import { isModelDefinition, type ModelDefinition } from "./model-dsl"

type ModelModule = {
	default?: unknown
	model?: unknown
}

export function isTypeScriptModelPath(filePath: string): boolean {
	const lower = filePath.toLowerCase()
	return lower.endsWith(".pcad.ts") || lower.endsWith(".model.ts") || lower.endsWith(".pcad.mts") || lower.endsWith(".model.mts")
}

export async function loadModelSource(filePath: string): Promise<ModelDefinition> {
	if (!isTypeScriptModelPath(filePath)) {
		throw new Error(`PuppyCAD model sources must end in .pcad.ts, .model.ts, .pcad.mts, or .model.mts: ${filePath}`)
	}

	const sourceStat = await stat(filePath).catch((error: unknown) => {
		throw new Error(`Unable to read TypeScript model source: ${formatFileError(error)}`)
	})
	if (!sourceStat.isFile()) {
		throw new Error(`TypeScript model source is not a file: ${filePath}`)
	}

	let module: ModelModule
	try {
		module = await importModelInFreshWorker(pathToFileURL(filePath).href)
	} catch (error) {
		throw new Error(`Unable to execute TypeScript model source ${filePath}: ${error instanceof Error ? error.message : String(error)}`)
	}

	const candidate = module.default ?? module.model
	if (!isModelDefinition(candidate)) {
		throw new Error(`TypeScript model source ${filePath} must default-export a model from defineModel(), or export it as "model".`)
	}
	return candidate
}

type WorkerResult = { ok: true; module: ModelModule } | { ok: false; error: string }

function importModelInFreshWorker(sourceUrl: string): Promise<ModelModule> {
	const workerSource = `
		import(${JSON.stringify(sourceUrl)})
			.then((module) => postMessage({ ok: true, module: { default: module.default, model: module.model } }))
			.catch((error) => postMessage({ ok: false, error: error instanceof Error ? error.message : String(error) }))
	`
	const workerUrl = URL.createObjectURL(new Blob([workerSource], { type: "text/javascript" }))
	const worker = new Worker(workerUrl)

	return new Promise((resolve, reject) => {
		const cleanup = () => {
			worker.terminate()
			URL.revokeObjectURL(workerUrl)
		}
		worker.onmessage = (event: MessageEvent<WorkerResult>) => {
			cleanup()
			if (event.data.ok) {
				resolve(event.data.module)
			} else {
				reject(new Error(event.data.error))
			}
		}
		worker.onerror = (event) => {
			cleanup()
			reject(new Error(event.message || "Model worker failed."))
		}
	})
}

function formatFileError(error: unknown): string {
	if (error && typeof error === "object" && "code" in error) {
		return `${String((error as { code?: unknown }).code)}: ${String((error as { message?: unknown }).message ?? "file error")}`
	}
	return error instanceof Error ? error.message : String(error)
}
