/** Sketch values are stored in millimeters and degrees. Expressions are evaluated, not executed. */
export type SketchValueKind = "scalar" | "length" | "angle"
export type SketchQuantity = { value: number; kind: SketchValueKind }
const units: Record<string, SketchQuantity> = {
	mm: { value: 1, kind: "length" },
	cm: { value: 10, kind: "length" },
	m: { value: 1000, kind: "length" },
	um: { value: 0.001, kind: "length" },
	µm: { value: 0.001, kind: "length" },
	μm: { value: 0.001, kind: "length" },
	in: { value: 25.4, kind: "length" },
	inch: { value: 25.4, kind: "length" },
	ft: { value: 304.8, kind: "length" },
	deg: { value: 1, kind: "angle" },
	"°": { value: 1, kind: "angle" },
	rad: { value: 180 / Math.PI, kind: "angle" }
}
export function parseSketchValue(text: string, expected: SketchValueKind, resolve?: (name: string) => SketchQuantity): number {
	if (text.length > 512) throw Error("Dimension expression is too long.")
	const tokens = (text.match(/#[a-z_][a-z0-9_]*|(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?|[a-zµμ°]+|[^\s]/gi) ?? []).map((token) => (token.startsWith("#") ? token : token.toLowerCase()))
	let index = 0
	const finite = (q: SketchQuantity): SketchQuantity => {
		if (!Number.isFinite(q.value)) throw Error("Dimension must be finite; division by zero is not allowed.")
		return q
	}
	const primary = (depth: number): SketchQuantity => {
		if (depth > 32) throw Error("Dimension expression is nested too deeply.")
		const token = tokens[index++]
		if (token === "+" || token === "-") {
			const q = primary(depth + 1)
			return { ...q, value: token === "-" ? -q.value : q.value }
		}
		let q: SketchQuantity
		if (token === "(") {
			q = sum(depth + 1)
			if (tokens[index++] !== ")") throw Error("Missing closing parenthesis.")
		} else if (token === "pi") q = { value: Math.PI, kind: "scalar" }
		else if (token?.startsWith("#")) {
			if (!resolve) throw Error(`Unknown sketch variable: ${token}`)
			q = finite({ ...resolve(token.slice(1)) })
			if (!["scalar", "length", "angle"].includes(q.kind)) throw Error(`Invalid units for sketch variable: ${token}`)
		} else if (token && /^(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/.test(token)) q = finite({ value: Number(token), kind: "scalar" })
		else throw Error("Enter a number, pi, or a parenthesized expression.")
		const unit = Object.hasOwn(units, tokens[index] ?? "") ? units[tokens[index] ?? ""] : undefined
		if (unit) {
			index++
			if (q.kind !== "scalar") throw Error("A unit can only follow a dimensionless value.")
			q = finite({ kind: unit.kind, value: q.value * unit.value })
		}
		return q
	}
	const product = (depth: number): SketchQuantity => {
		let a = primary(depth)
		while (tokens[index] === "*" || tokens[index] === "/") {
			const op = tokens[index++]
			const b = primary(depth)
			if (op === "*") {
				if (a.kind !== "scalar" && b.kind !== "scalar") throw Error("Multiplication requires at least one dimensionless value.")
				a = finite({ kind: a.kind === "scalar" ? b.kind : a.kind, value: a.value * b.value })
			} else {
				if (b.kind !== "scalar" && b.kind !== a.kind) throw Error("Cannot divide incompatible dimensions.")
				a = finite({ kind: b.kind === "scalar" ? a.kind : "scalar", value: a.value / b.value })
			}
		}
		return a
	}
	const sum = (depth: number): SketchQuantity => {
		let a = product(depth)
		while (tokens[index] === "+" || tokens[index] === "-") {
			const op = tokens[index++]
			const b = product(depth)
			if (a.kind !== b.kind) throw Error("Addition and subtraction require matching units; specify units on both values.")
			a = finite({ kind: a.kind, value: a.value + (op === "+" ? b.value : -b.value) })
		}
		return a
	}
	const result = sum(0)
	if (index !== tokens.length) throw Error(`Unexpected dimension token: ${tokens[index]}`)
	if (result.kind !== "scalar" && result.kind !== expected) throw Error(`Expected a ${expected} value, received ${result.kind} units.`)
	return result.value
}
