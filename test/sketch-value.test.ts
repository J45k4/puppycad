import { expect, it } from "bun:test"
import { parseSketchValue } from "../src/sketch-value"

it("converts explicit length and angle units to sketch storage units", () => {
	for (const [text, value] of [
		["2 in", 50.8],
		["1 ft", 304.8],
		["3cm", 30],
		[".5 m", 500],
		["1000 µm", 1],
		["1000 μm", 1],
		["1e3 um", 1],
		["2 INCH", 50.8]
	] as const)
		expect(parseSketchValue(text, "length")).toBeCloseTo(value, 10)
	expect(parseSketchValue("pi rad", "angle")).toBeCloseTo(180, 10)
	expect(parseSketchValue("-90°", "angle")).toBe(-90)
	expect(parseSketchValue("360 deg", "angle")).toBe(360)
	expect(parseSketchValue("-1.2e-3", "length")).toBe(-0.0012)
})
it("evaluates arithmetic with precedence, parentheses and compatible dimensions", () => {
	expect(parseSketchValue("2 * (1 in + 5 mm)", "length")).toBeCloseTo(60.8, 10)
	expect(parseSketchValue("(2 + 3) cm / 2", "length")).toBe(25)
	expect(parseSketchValue("(pi / 2) rad", "angle")).toBeCloseTo(90, 10)
	expect(parseSketchValue("1 + 2 * 3 - 4 / 2", "length")).toBe(5)
	expect(parseSketchValue("1 in / 25.4 mm", "scalar")).toBe(1)
	expect(parseSketchValue("-(30 deg + 15 deg)", "angle")).toBe(-45)
})
it("rejects incompatible units, invalid syntax and non-finite results without executing input", () => {
	for (const value of [
		"",
		"NaN",
		"Infinity",
		"1/0",
		"0/0",
		"1e309",
		"1e308 * 10",
		"1 deg",
		"1 mm + 2 deg",
		"1 in + 2",
		"1 mm * 2 mm",
		"1 / 2 mm",
		"(1+2",
		"1+2)",
		"2 unknown",
		"2 ** 3",
		"globalThis.process.exit()",
		"1; 2",
		"constructor",
		`${"(".repeat(40)}1${")".repeat(40)}`,
		"1".repeat(513)
	])
		expect(() => parseSketchValue(value, "length")).toThrow()
	expect(() => parseSketchValue("2 mm", "scalar")).toThrow()
	expect(() => parseSketchValue("2 cm", "angle")).toThrow()
})
