import { expect, it } from "bun:test"
import type { ChamferFeature, CompositeFeature, ExtrudeFeature, FilletFeature, Part, SketchFeature } from "../src/contract"

it("models selector-based composite aliases and resolved geometry references", () => {
	const baseSketch: SketchFeature = {
		id: "base-sketch",
		type: "sketch",
		target: {
			type: "plane",
			plane: "XY"
		},
		entities: [],
		profiles: [
			{
				id: "profile-1",
				vertices: [
					{ x: 0, y: 0 },
					{ x: 20, y: 0 },
					{ x: 20, y: 10 },
					{ x: 0, y: 10 }
				],
				loops: [[0, 1, 2, 3]]
			}
		]
	}

	const baseExtrude: ExtrudeFeature = {
		id: "base-extrude",
		type: "extrude",
		target: {
			type: "sketchProfile",
			sketchFeatureId: "base-sketch",
			selector: {
				type: "containsPoint",
				point: { x: 10, y: 5 }
			}
		},
		extent: {
			type: "blind",
			distance: 12
		},
		operation: "newBody"
	}

	const reusableMount: CompositeFeature = {
		id: "servo-mount",
		type: "composite",
		features: [baseSketch, baseExtrude],
		aliases: [
			{
				id: "mount-face",
				type: "face",
				source: {
					type: "extrudeFace",
					extrudeFeatureId: "base-extrude",
					selector: {
						type: "cap",
						side: "top"
					}
				}
			},
			{
				id: "outer-edge",
				type: "edge",
				source: {
					type: "extrudeEdge",
					extrudeFeatureId: "base-extrude",
					selector: {
						type: "side",
						index: 0
					}
				}
			},
			{
				id: "mount-profile",
				type: "profile",
				source: {
					type: "sketchProfile",
					sketchFeatureId: "base-sketch",
					selector: {
						type: "containsPoint",
						point: { x: 10, y: 5 }
					}
				}
			}
		]
	}

	const faceSketch: SketchFeature = {
		id: "face-sketch",
		type: "sketch",
		target: {
			type: "face",
			face: {
				type: "compositeAlias",
				compositeFeatureId: "servo-mount",
				aliasId: "mount-face"
			}
		},
		entities: [],
		profiles: []
	}

	const edgeFillet: FilletFeature = {
		id: "edge-fillet",
		type: "fillet",
		target: {
			edge: {
				type: "compositeAlias",
				compositeFeatureId: "servo-mount",
				aliasId: "outer-edge"
			}
		},
		radius: 1.5
	}

	const edgeChamfer: ChamferFeature = {
		id: "edge-chamfer",
		type: "chamfer",
		target: {
			edge: {
				type: "compositeAlias",
				compositeFeatureId: "servo-mount",
				aliasId: "outer-edge"
			}
		},
		d1: 0.75
	}

	const part: Part = {
		id: "part-1",
		name: "Servo Bracket",
		features: [reusableMount, faceSketch, edgeFillet, edgeChamfer],
		solids: [
			{
				id: "solid-1",
				featureId: "base-extrude",
				vertices: [
					{ id: "v1", position: { x: 0, y: 0, z: 0 } },
					{ id: "v2", position: { x: 20, y: 0, z: 0 } }
				],
				edges: [{ id: "edge-1", vertexIds: ["v1", "v2"] }],
				faces: [{ id: "face-1", edgeIds: ["edge-1"] }]
			}
		],
		resolvedAliases: {
			"servo-mount": {
				"mount-face": {
					type: "face",
					solidId: "solid-1",
					faceId: "face-1"
				},
				"outer-edge": {
					type: "edge",
					solidId: "solid-1",
					edgeId: "edge-1"
				},
				"mount-profile": {
					type: "profile",
					sketchFeatureId: "base-sketch",
					profileId: "profile-1"
				}
			}
		}
	}

	expect(reusableMount.aliases).toHaveLength(3)
	expect(faceSketch.target.type).toBe("face")
	expect(baseExtrude.target.type).toBe("sketchProfile")
	expect(part.resolvedAliases?.["servo-mount"]?.["mount-face"]).toEqual({
		type: "face",
		solidId: "solid-1",
		faceId: "face-1"
	})
})
