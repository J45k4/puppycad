import { expect, it } from "bun:test"
import modeling from "@jscad/modeling"
import { circularLoft, circularLoftInterior } from "../src/loft"
import { evaluateSolid } from "../src/solid-model"
import { PartBuilder } from "../src/sdk"
import { flowerHolderParts } from "../examples/flower-holder-parts"
import type { PartDocument } from "../src/schema"

it("lofts two circular sections with the expected frustum volume", () => {
	const loft = { bottomRadius: 85, topRadius: 112.5, height: 100, segments: 144 }
	const factor = (loft.segments * Math.sin((2 * Math.PI) / loft.segments)) / 2
	const expected = ((factor * loft.height) / 3) * (85 ** 2 + 85 * 112.5 + 112.5 ** 2)
	expect(modeling.measurements.measureVolume(circularLoft(loft))).toBeCloseTo(expected, 5)
	expect(modeling.measurements.measureBoundingBox(circularLoft(loft))).toEqual([
		[-112.5, -112.5, 0],
		[112.5, 112.5, 100]
	])
})
it("loft then shell matches the reconstructed Flowergirl bowl volume and bounds", () => {
	const document: PartDocument = {
		features: [],
		solidSteps: [
			{ id: "loft", type: "loft", operation: "join", loft: { bottomRadius: 85, topRadius: 112.5, height: 100, segments: 144 } },
			{ id: "shell", type: "shell", operation: "cut", sourceId: "loft", thickness: 2.5 }
		]
	}
	const original = new PartBuilder()
	flowerHolderParts.find((p) => p.id === "bowl")?.build(original)
	const a = evaluateSolid(document)
	const b = evaluateSolid(original.document)
	expect(modeling.measurements.measureVolume(a)).toBeCloseTo(modeling.measurements.measureVolume(b), 4)
	const ab = modeling.measurements.measureBoundingBox(a).flat()
	const bb = modeling.measurements.measureBoundingBox(b).flat()
	for (let i = 0; i < ab.length; i++) expect(ab[i]).toBeCloseTo(bb[i] ?? 0, 8)
})
it("rejects invalid lofts and shells instead of emitting degenerate geometry", () => {
	const loft = { bottomRadius: 10, topRadius: 15, height: 20, segments: 96 }
	expect(() => circularLoft({ ...loft, height: 0 })).toThrow()
	expect(() => circularLoftInterior(loft, 0)).toThrow()
	expect(() => circularLoftInterior(loft, 19)).toThrow()
	expect(() => evaluateSolid({ features: [], solidSteps: [{ id: "shell", type: "shell", operation: "cut", sourceId: "missing", thickness: 2 }] })).toThrow()
})
