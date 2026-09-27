import { expect, it } from "bun:test"
import { PartBuilder, v2 } from "../src/sdk"
import { extrudeSolidFeature, getExtrudedFaceDescriptors } from "../src/cad/extrude"
import { sketchSupportEdges } from "../src/sketch-context"
import { requireValue } from "../src/required"
it("projects a supporting extrusion into top and side sketch frames without duplicate edges", () => {
	const builder = new PartBuilder()
	builder.extrude("base", { outline: [v2(0, 0), v2(10, 0), v2(10, 20), v2(0, 20)], depth: 5 })
	const before = structuredClone(builder.document)
	const feature = requireValue(builder.document.features.find((feature) => feature.type === "extrude"))
	if (feature.type !== "extrude") throw Error("Missing extrusion")
	const faces = getExtrudedFaceDescriptors(extrudeSolidFeature(builder.document, feature))
	const top = requireValue(faces.find((face) => face.label === "Top Face"))
	const edges = sketchSupportEdges(builder.document, { type: "face", face: { type: "extrudeFace", extrudeId: feature.id, faceId: top.faceId } })
	expect(edges).toHaveLength(4)
	expect(Math.max(...edges.flat().map((p) => p.x))).toBeCloseTo(10, 8)
	expect(Math.max(...edges.flat().map((p) => p.y))).toBeCloseTo(20, 8)
	const side = requireValue(faces.find((face) => face.label === "Side Face 1"))
	const sideEdges = sketchSupportEdges(builder.document, { type: "face", face: { type: "extrudeFace", extrudeId: feature.id, faceId: side.faceId } })
	expect(sideEdges).toHaveLength(4)
	expect(Math.max(...sideEdges.flat().map((p) => p.y))).toBeCloseTo(5, 8)
	expect(builder.document).toEqual(before)
	expect(sketchSupportEdges(builder.document, { type: "plane", plane: "XY" })).toEqual([])
})
