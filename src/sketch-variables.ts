import { parseSketchValue, type SketchQuantity, type SketchValueKind } from "./sketch-value"

export type SketchVariable = { name: string; kind: SketchValueKind; expression: string }

/** Evaluate a complete variable set atomically. Names are case-sensitive; units use mm/degrees. */
export function evaluateSketchVariables(input: readonly SketchVariable[]): Map<string, SketchQuantity> {
	if (input.length > 256) throw Error("A sketch supports at most 256 variables.")
	const definitions = new Map<string, SketchVariable>()
	for (const variable of input) {
		if (!variable || typeof variable.name !== "string" || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(variable.name) || variable.name.length > 64)
			throw Error("Variable names require 1–64 letters, digits or underscores and cannot start with a digit.")
		if (!["scalar", "length", "angle"].includes(variable.kind) || typeof variable.expression !== "string") throw Error(`Invalid sketch variable: #${variable.name}`)
		if (definitions.has(variable.name)) throw Error(`Duplicate sketch variable: #${variable.name}`)
		definitions.set(variable.name, variable)
	}
	const values = new Map<string, SketchQuantity>()
	const depths = new Map<string, number>()
	const active: string[] = []
	const resolve = (name: string): SketchQuantity => {
		const cached = values.get(name)
		if (cached) return cached
		const definition = definitions.get(name)
		if (!definition) throw Error(`Unknown sketch variable: #${name}`)
		if (active.includes(name)) throw Error(`Cyclic sketch variables: ${[...active.slice(active.indexOf(name)), name].map((item) => `#${item}`).join(" → ")}`)
		if (active.length >= 64) throw Error("Sketch variable dependencies exceed 64 levels.")
		active.push(name)
		try {
			let depth = 1
			const value = {
				kind: definition.kind,
				value: parseSketchValue(definition.expression, definition.kind, (dependency) => {
					const result = resolve(dependency)
					depth = Math.max(depth, 1 + (depths.get(dependency) ?? 0))
					return result
				})
			}
			if (depth > 64) throw Error("Sketch variable dependencies exceed 64 levels.")
			depths.set(name, depth)
			values.set(name, value)
			return value
		} finally {
			active.pop()
		}
	}
	for (const name of definitions.keys()) resolve(name)
	return values
}

/** Validate serialized definitions and return an independent, canonical copy. */
export function normalizeSketchVariables(input: unknown): SketchVariable[] | undefined {
	if (input === undefined) return undefined
	if (!Array.isArray(input)) throw Error("Sketch variables must be an array.")
	const variables = input.map((value: unknown): SketchVariable => {
		if (!value || typeof value !== "object") throw Error("Invalid sketch variable definition.")
		const variable = value as Record<string, unknown>
		if (typeof variable.name !== "string" || typeof variable.expression !== "string" || !["scalar", "length", "angle"].includes(String(variable.kind)))
			throw Error("Invalid sketch variable definition.")
		return { name: variable.name, kind: variable.kind as SketchValueKind, expression: variable.expression }
	})
	evaluateSketchVariables(variables)
	return variables
}

/** Rename exact variable tokens in definitions and dimensions without touching similar names. */
export function renameSketchVariable(sketch: import("./schema").Sketch, name: string, replacement: string): import("./schema").Sketch {
	if (!sketch.variables?.some((variable) => variable.name === name)) throw Error(`Unknown sketch variable: #${name}`)
	if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(replacement) || replacement.length > 64) throw Error("Invalid variable name.")
	if (replacement !== name && sketch.variables.some((variable) => variable.name === replacement)) throw Error(`Duplicate sketch variable: #${replacement}`)
	const rewrite = (expression: string) => expression.replace(/#[A-Za-z_][A-Za-z0-9_]*/g, (token) => (token === `#${name}` ? `#${replacement}` : token))
	const result = structuredClone(sketch)
	result.variables = sketch.variables.map((variable) => ({ ...variable, name: variable.name === name ? replacement : variable.name, expression: rewrite(variable.expression) }))
	result.relations = sketch.relations?.map((relation) =>
		relation.expression === undefined ? structuredClone(relation) : { ...structuredClone(relation), expression: rewrite(relation.expression) }
	)
	evaluateSketchVariables(result.variables)
	return result
}
