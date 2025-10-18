import { describe, expect, it } from "bun:test"
import { PART_PROJECT_DEFAULT_HEIGHT, PART_PROJECT_DEFAULT_ROTATION, type PartProjectItemData, type ProjectFileEntry, type SchemanticProjectItemData, normalizeProjectFile } from "../src/project-file"

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
		const item = file.items[0] as Extract<ProjectFileEntry, { type: "schemantic" }>

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

	it("normalizes part project items", () => {
		const input = {
			version: 2,
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
						height: Number.POSITIVE_INFINITY,
						previewRotation: { yaw: Number.NaN, pitch: 0.2 }
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
		const item = file.items[0] as Extract<ProjectFileEntry, { type: "part" }>

		expect(item.type).toBe("part")
		expect(item.name).toBe("Part 1")

		const data = item.data as PartProjectItemData
		expect(data.sketchPoints).toEqual([
			{ x: 0, y: 0 },
			{ x: 10, y: 5 }
		])
		expect(data.isSketchClosed).toBe(false)
		expect(data.height).toBe(PART_PROJECT_DEFAULT_HEIGHT)
		expect(data.previewRotation.yaw).toBeCloseTo(PART_PROJECT_DEFAULT_ROTATION.yaw)
		expect(data.previewRotation.pitch).toBeCloseTo(0.2)

		expect(data.extrudedModel).toEqual({
			base: [
				{ x: 0, y: 0 },
				{ x: 10, y: 0 },
				{ x: 0, y: 10 }
			],
			height: PART_PROJECT_DEFAULT_HEIGHT,
			scale: 2,
			rawHeight: PART_PROJECT_DEFAULT_HEIGHT
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
		expect(nestedPartData.sketchPoints).toEqual([])
		expect(nestedPartData.height).toBe(PART_PROJECT_DEFAULT_HEIGHT)

		expect(file.selectedPath).toEqual([3, 0])
	})
})
