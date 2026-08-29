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

	const sourceUrl = pathToFileURL(filePath)
	sourceUrl.searchParams.set("puppycadMtime", String(sourceStat.mtimeMs))
	let module: ModelModule
	try {
		module = (await import(sourceUrl.href)) as ModelModule
	} catch (error) {
		throw new Error(`Unable to execute TypeScript model source ${filePath}: ${error instanceof Error ? error.message : String(error)}`)
	}

	const candidate = module.default ?? module.model
	if (!isModelDefinition(candidate)) {
		throw new Error(`TypeScript model source ${filePath} must default-export a model from defineModel(), or export it as "model".`)
	}
	return candidate
}

function formatFileError(error: unknown): string {
	if (error && typeof error === "object" && "code" in error) {
		return `${String((error as { code?: unknown }).code)}: ${String((error as { message?: unknown }).message ?? "file error")}`
	}
	return error instanceof Error ? error.message : String(error)
}
