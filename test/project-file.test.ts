import { describe, expect, it } from "bun:test"
import type { PartProjectItemData, ProjectNode, SchemanticProjectItemData } from "../src/contract"
import { PART_PROJECT_DEFAULT_HEIGHT, normalizeProjectFile } from "../src/project-file"

describe("normalizeProjectFile", () => {
	it("normalizes schemantic project items", () => {
		const input = {
			version: 2,
			items: [
				{
					type: "schemantic",
					name: "   ",
					data: {
						components: [
							{
								id: 1,
								x: 10,
								y: 5,
								width: 12,
								height: 3,
								data: { type: 42 }
							},
							{ id: 1, x: 0, y: 0 },
							{ id: 2, x: 1, y: null }
						],
						connections: [
							{
								from: {
									componentId: 1,
									edge: "left",
									ratio: -0.2
								},
								to: {
									componentId: 1,
									edge: "right",
									ratio: 2
								}
							},
							{
								from: {
									componentId: 99,
									edge: "top",
									ratio: 0.3
								},
								to: {
									componentId: 1,
									edge: "bottom",
									ratio: 0.6
								}
							}
						]
					}
				}
			],
			selectedPath: [0]
		}

		const file = normalizeProjectFile(input)
		expect(file).not.toBeNull()
		if (!file) {
			throw new Error("normalizeProjectFile returned null")
		}

		expect(file.items).toHaveLength(1)
		const item = file.items[0] as Extract<ProjectNode, { type: "schemantic" }>

		expect(item.type).toBe("schemantic")
		expect(item.name).toBe("Schemantic 1")
		expect(item.data).toBeDefined()

		const data = item.data as SchemanticProjectItemData
		expect(data.components).toEqual([
			{
				id: 1,
				x: 10,
				y: 5,
				width: 12,
				height: 3,
				data: { type: "42" }
			}
		])
		expect(data.connections).toEqual([
			{
				from: { componentId: 1, edge: "left", ratio: 0 },
				to: { componentId: 1, edge: "right", ratio: 1 }
			}
		])

		expect(file.selectedPath).toEqual([0])
	})

	it("adapts legacy part project items into schema features", () => {
		const input = {
			version: 3,
			items: [
				{
					type: "part",
					name: " ",
					data: {
						sketchPoints: [
							{ x: 0, y: 0 },
							{ x: 10, y: 5 },
							{ x: 5, y: undefined }
						],
						isSketchClosed: "yes",
						extrudedModel: {
							base: [
								{ x: 0, y: 0 },
								{ x: 10, y: 0 },
								{ x: 0, y: 10 },
								{ x: 2, y: "bad" }
							],
							height: "invalid",
							scale: 2,
							rawHeight: null
						},
						height: Number.POSITIVE_INFINITY
					}
				}
			],
			selectedPath: [0]
		}

		const file = normalizeProjectFile(input)
		expect(file).not.toBeNull()
		if (!file) {
			throw new Error("normalizeProjectFile returned null")
		}

		expect(file.items).toHaveLength(1)
		const item = file.items[0] as Extract<ProjectNode, { type: "part" }>

		expect(item.type).toBe("part")
		expect(item.name).toBe("Part 1")

		const data = item.data as PartProjectItemData
		expect(data.features).toHaveLength(3)
		expect(data.features[0]).toMatchObject({
			type: "sketch",
			name: "Sketch 1",
			dirty: true,
			target: {
				type: "plane",
				plane: "XY"
			},
			entities: [{ type: "line" }]
		})
		expect(data.features[1]).toMatchObject({
			type: "sketch",
			name: "Legacy Sketch 1",
			dirty: false,
			target: {
				type: "plane",
				plane: "XY"
			}
		})
		expect(data.features[2]).toMatchObject({
			type: "extrude",
			depth: PART_PROJECT_DEFAULT_HEIGHT
		})
	})

	it("normalizes other project file types and nested folders", () => {
		const input = {
			version: 2,
			items: [
				{ type: "pcb", name: "" },
				{ type: "assembly", name: "Pcb 1" },
				{ type: "diagram", name: "   Custom Diagram   " },
				{
					kind: "folder",
					name: "",
					items: [
						{ type: "diagram", name: "" },
						{ type: "part", name: "" }
					]
				}
			],
			selectedPath: [3, 0]
		}

		const file = normalizeProjectFile(input)
		expect(file).not.toBeNull()
		if (!file) {
			throw new Error("normalizeProjectFile returned null")
		}

		expect(file.items).toHaveLength(4)

		const pcbEntry = file.items[0]
		if (!pcbEntry || !("type" in pcbEntry) || pcbEntry.type !== "pcb") {
			throw new Error("Expected first item to be a PCB project entry")
		}
		expect(pcbEntry).toEqual({ type: "pcb", name: "Pcb 1" })

		const assemblyEntry = file.items[1]
		if (!assemblyEntry || !("type" in assemblyEntry) || assemblyEntry.type !== "assembly") {
			throw new Error("Expected second item to be an assembly project entry")
		}
		expect(assemblyEntry).toEqual({ type: "assembly", name: "Assembly 1" })

		const diagramEntry = file.items[2]
		if (!diagramEntry || !("type" in diagramEntry) || diagramEntry.type !== "diagram") {
			throw new Error("Expected third item to be a diagram project entry")
		}
		expect(diagramEntry).toEqual({ type: "diagram", name: "Custom Diagram" })

		const folderEntry = file.items[3]
		if (!folderEntry || !("kind" in folderEntry) || folderEntry.kind !== "folder") {
			throw new Error("Expected fourth item to be a folder entry")
		}
		expect(folderEntry.name).toBe("Folder 1")
		expect(folderEntry.visible).toBeUndefined()
		expect(folderEntry.items).toHaveLength(2)

		const nestedDiagram = folderEntry.items[0]
		if (!nestedDiagram || !("type" in nestedDiagram) || nestedDiagram.type !== "diagram") {
			throw new Error("Expected first nested item to be a diagram entry")
		}
		expect(nestedDiagram).toEqual({ type: "diagram", name: "Diagram 1" })

		const nestedPart = folderEntry.items[1]
		if (!nestedPart || !("type" in nestedPart) || nestedPart.type !== "part") {
			throw new Error("Expected second nested item to be a part entry")
		}
		expect(nestedPart.name).toBe("Part 1")
		const nestedPartData = nestedPart.data as PartProjectItemData
		expect(nestedPartData).toEqual({ features: [] })

		expect(file.selectedPath).toEqual([3, 0])
	})

	it("preserves visibility flags on entries and part state", () => {
		const input = {
			version: 2,
			items: [
				{
					kind: "folder",
					name: "Folder",
					visible: false,
					items: [
						{
							type: "part",
							name: "Part",
							visible: false,
							data: {
								sketchPoints: [],
								sketchName: undefined,
								isSketchClosed: false,
								height: 30,
								previewDistance: 40,
								previewRotation: { yaw: 0, pitch: 0 },
								sketchVisible: false,
								referencePlaneVisibility: {
									Front: false,
									Top: true,
									Right: false
								}
							}
						}
					]
				}
			],
			selectedPath: null
		}

		const file = normalizeProjectFile(input)
		expect(file).not.toBeNull()
		if (!file) {
			throw new Error("normalizeProjectFile returned null")
		}

		const folderEntry = file.items[0]
		if (!folderEntry || !("kind" in folderEntry) || folderEntry.kind !== "folder") {
			throw new Error("Expected folder entry")
		}
		expect(folderEntry.visible).toBe(false)
		const partEntry = folderEntry.items[0]
		if (!partEntry || !("type" in partEntry) || partEntry.type !== "part") {
			throw new Error("Expected nested part entry")
		}
		expect(partEntry.visible).toBe(false)
		expect(partEntry.data).toEqual({ features: [] })
	})

	it("preserves schema-based part features", () => {
		const input = {
			version: 3,
			items: [
				{
					type: "part",
					name: "Part",
					data: {
						cad: {
							nodes: [
								{ id: "plane-front", type: "referencePlane", name: "Front", plane: "XY" },
								{ id: "sketch-1", type: "sketch", name: "Sketch 1", targetId: "plane-front", entities: [], dimensions: [] },
								{ id: "extrude-1", type: "extrude", sketchId: "sketch-1", profileId: "sketch-1-profile-1", operation: "newBody", depth: 18 }
							],
							rootNodeIds: ["plane-front", "extrude-1"]
						},
						tree: {
							orderedNodeIds: ["sketch-1", "extrude-1"],
							dirtySketchIds: []
						},
						features: [
							{
								type: "sketch",
								id: "sketch-1",
								name: "Sketch 1",
								dirty: false,
								target: {
									type: "plane",
									plane: "XZ"
								},
								entities: [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 10 } }],
								dimensions: [
									{ id: "dimension-width", type: "rectangleWidth", entityId: "rect-1", value: 10 },
									{ id: "dimension-missing-entity", type: "rectangleWidth", entityId: "missing-rect", value: 12 },
									{ id: "dimension-invalid-value", type: "rectangleHeight", entityId: "rect-1", value: 0 },
									{ id: "dimension-invalid-type", type: "lineLength", entityId: "rect-1", value: 10 }
								]
							},
							{
								type: "extrude",
								id: "extrude-1",
								target: {
									type: "profileRef",
									sketchId: "sketch-1",
									profileId: "sketch-1-profile-1"
								},
								depth: 18
							},
							{
								type: "chamfer",
								id: "chamfer-1",
								target: {
									edge: {
										type: "extrudeEdge",
										extrudeId: "extrude-1",
										edgeId: "extrude-1-solid-edge-1"
									}
								},
								d1: 1.25
							}
						],
						solids: [
							{
								id: "extrude-1-solid",
								featureId: "extrude-1",
								vertices: [{ id: "v1", position: { x: 0, y: 0, z: 0 } }],
								edges: [{ id: "e1", vertexIds: ["v1", "v1"] }],
								faces: [{ id: "f1", edgeIds: ["e1"] }]
							}
						]
					}
				}
			],
			selectedPath: [0]
		}

		const file = normalizeProjectFile(input)
		expect(file).not.toBeNull()
		if (!file) {
			throw new Error("normalizeProjectFile returned null")
		}

		const part = file.items[0]
		if (!part || !("type" in part) || part.type !== "part") {
			throw new Error("Expected part item")
		}

		expect(part.data).toMatchObject({
			cad: {
				nodes: [
					{ id: "plane-front", type: "referencePlane", name: "Front", plane: "XY" },
					{ id: "sketch-1", type: "sketch", name: "Sketch 1", targetId: "plane-front", entities: [], dimensions: [] },
					{ id: "extrude-1", type: "extrude", sketchId: "sketch-1", profileId: "sketch-1-profile-1", operation: "newBody", depth: 18 }
				],
				rootNodeIds: ["plane-front", "extrude-1"]
			},
			tree: {
				orderedNodeIds: ["sketch-1", "extrude-1"]
			},
			features: [
				{
					type: "sketch",
					id: "sketch-1",
					target: {
						type: "plane",
						plane: "XZ"
					},
					dimensions: [{ id: "dimension-width", type: "rectangleWidth", entityId: "rect-1", value: 10 }],
					profiles: [{ id: "sketch-1-profile-1" }]
				},
				{
					type: "extrude",
					id: "extrude-1",
					depth: 18
				},
				{
					type: "chamfer",
					id: "chamfer-1",
					target: {
						edge: {
							type: "extrudeEdge",
							extrudeId: "extrude-1",
							edgeId: "extrude-1-solid-edge-1"
						}
					},
					d1: 1.25
				}
			],
			solids: [
				{
					id: "extrude-1-solid",
					featureId: "extrude-1",
					vertices: [{ id: "v1", position: { x: 0, y: 0, z: 0 } }],
					edges: [{ id: "e1", vertexIds: ["v1", "v1"] }],
					faces: [{ id: "f1", edgeIds: ["e1"] }]
				}
			]
		})
	})

	it("preserves face-target sketch references in schema-based part features", () => {
		const input = {
			version: 3,
			items: [
				{
					type: "part",
					name: "Part",
					data: {
						features: [
							{
								type: "sketch",
								id: "sketch-1",
								dirty: false,
								target: {
									type: "plane",
									plane: "XY"
								},
								entities: [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 10 } }]
							},
							{
								type: "extrude",
								id: "extrude-1",
								target: {
									type: "profileRef",
									sketchId: "sketch-1",
									profileId: "sketch-1-profile-1"
								},
								depth: 8
							},
							{
								type: "sketch",
								id: "sketch-2",
								dirty: false,
								target: {
									type: "face",
									face: {
										type: "extrudeFace",
										extrudeId: "extrude-1",
										faceId: "extrude-1-solid-face-6"
									}
								},
								entities: [{ id: "line-1", type: "line", p0: { x: 1, y: 1 }, p1: { x: 4, y: 1 } }]
							}
						]
					}
				}
			],
			selectedPath: [0]
		}

		const file = normalizeProjectFile(input)
		expect(file).not.toBeNull()
		if (!file) {
			throw new Error("normalizeProjectFile returned null")
		}

		const part = file.items[0]
		if (!part || !("type" in part) || part.type !== "part") {
			throw new Error("Expected part item")
		}
		if (!part.data) {
			throw new Error("Expected part data")
		}

		expect(part.data.features[2]).toMatchObject({
			type: "sketch",
			id: "sketch-2",
			target: {
				type: "face",
				face: {
					type: "extrudeFace",
					extrudeId: "extrude-1",
					faceId: "extrude-1-solid-face-6"
				}
			}
		})
	})

	it("skips unsupported legacy extrusions and records a warning", () => {
		const input = {
			version: 3,
			items: [
				{
					type: "part",
					name: "Part",
					data: {
						extrudedModels: [
							{
								base: [
									{ x: 0, y: 0 },
									{ x: 1, y: 0 },
									{ x: 0, y: 1 }
								],
								height: 1,
								scale: 10,
								rawHeight: 10,
								origin: { x: 3, y: 0, z: 0 },
								rotation: { x: 0, y: 0, z: 0, w: 1 }
							}
						]
					}
				}
			],
			selectedPath: [0]
		}

		const file = normalizeProjectFile(input)
		expect(file).not.toBeNull()
		if (!file) {
			throw new Error("normalizeProjectFile returned null")
		}

		const part = file.items[0]
		if (!part || !("type" in part) || part.type !== "part") {
			throw new Error("Expected part item")
		}

		expect(part.data).toEqual({
			features: [],
			migrationWarnings: ["Skipped unsupported legacy extrusion 1."]
		})
	})
})
