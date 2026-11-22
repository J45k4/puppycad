import { describe, expect, it } from "bun:test"
import type { Project } from "../src/contract"
import { applyCadCommand, applySyncedProjectCommands, ProjectCommandError } from "../src/project-commands"

describe("project commands", () => {
	it("creates, renames, moves, hides, and deletes project nodes by stable id", () => {
		const project: Project = {
			version: 4,
			revision: 0,
			items: [],
			selectedPath: null
		}

		const next = applySyncedProjectCommands(project, [
			{ type: "createFolder", id: "folder-1", name: "Folder" },
			{ type: "createItem", id: "part-1", documentType: "part", parentId: "folder-1", name: "Part" },
			{ type: "renameNode", nodeId: "part-1", name: "Renamed Part" },
			{ type: "setNodeVisibility", nodeId: "part-1", visible: false },
			{ type: "moveNode", nodeId: "part-1", parentId: null }
		])

		expect(next.items).toHaveLength(2)
		expect(next.items[1]).toMatchObject({
			id: "part-1",
			type: "part",
			name: "Renamed Part",
			visible: false
		})

		const deleted = applySyncedProjectCommands(next, [{ type: "deleteNode", nodeId: "folder-1" }])
		expect(deleted.items.map((item) => item.id)).toEqual(["part-1"])
		expect(project.items).toEqual([])
	})

	it("fails invalid project commands without mutating input", () => {
		const project: Project = {
			version: 4,
			revision: 0,
			items: [{ id: "part-1", type: "part", name: "Part", data: { features: [] } }],
			selectedPath: null
		}

		expect(() => applySyncedProjectCommands(project, [{ type: "renameNode", nodeId: "missing", name: "Name" }])).toThrow(ProjectCommandError)
		expect(project.items[0]?.name).toBe("Part")
	})
})

describe("cad commands", () => {
	it("updates feature parameters and rebuilds serialized PCAD state", () => {
		const part = applyCadCommand(
			{
				features: [
					{
						type: "sketch",
						id: "sketch-1",
						name: "Sketch 1",
						dirty: false,
						target: { type: "plane", plane: "XY" },
						entities: [{ id: "rect-1", type: "cornerRectangle", p0: { x: 0, y: 0 }, p1: { x: 10, y: 10 } }],
						dimensions: [],
						vertices: [],
						loops: [],
						profiles: [{ id: "sketch-1-profile-1", outerLoopId: "loop-1", holeLoopIds: [] }]
					},
					{
						type: "extrude",
						id: "extrude-1",
						name: "Extrude 1",
						target: { type: "profileRef", sketchId: "sketch-1", profileId: "sketch-1-profile-1" },
						depth: 10
					}
				]
			},
			{ type: "setExtrudeDepth", extrudeId: "extrude-1", depth: 20 }
		)

		expect(part.features[1]).toMatchObject({ type: "extrude", depth: 20 })
		expect(part.cad?.nodes.find((node) => node.id === "extrude-1")).toMatchObject({ type: "extrude", depth: 20 })
		expect(part.tree?.orderedNodeIds).toEqual(["sketch-1", "extrude-1"])
	})

	it("deletes sketches and dependent features through deleteNodeCascade", () => {
		const part = applyCadCommand(
			{
				features: [
					{
						type: "sketch",
						id: "sketch-1",
						dirty: false,
						target: { type: "plane", plane: "XY" },
						entities: [],
						dimensions: [],
						vertices: [],
						loops: [],
						profiles: [{ id: "profile-1", outerLoopId: "loop-1", holeLoopIds: [] }]
					},
					{
						type: "extrude",
						id: "extrude-1",
						target: { type: "profileRef", sketchId: "sketch-1", profileId: "profile-1" },
						depth: 5
					}
				]
			},
			{ type: "deleteNodeCascade", nodeId: "sketch-1" }
		)

		expect(part.features).toEqual([])
		expect(part.tree?.orderedNodeIds).toEqual([])
	})
})
