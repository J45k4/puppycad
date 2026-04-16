import { describe, expect, it } from "bun:test"

import { derivePartQuickActionsModel } from "../src/part-quick-actions"

describe("derivePartQuickActionsModel", () => {
	it("hides the rail when no plane is selected", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "view",
			selectedExtrudeLabel: null,
			selectedPlaneLabel: null,
			selectedPlaneVisible: false,
			activeSketchTool: "line",
			canUndo: false,
			canReset: false,
			canFinishSketch: false,
			canExtrude: false
		})

		expect(model.visible).toBe(false)
		expect(model.primaryActions).toEqual([])
	})

	it("shows only sketch when a visible plane is selected in view mode", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "view",
			selectedExtrudeLabel: null,
			selectedPlaneLabel: "Front",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			canUndo: false,
			canReset: false,
			canFinishSketch: false,
			canExtrude: false
		})

		expect(model.visible).toBe(true)
		expect(model.primaryActions.map((action) => action.id)).toEqual(["start-sketch"])
		expect(model.sketchToolActions).toEqual([])
		expect(model.commandActions).toEqual([])
		expect(model.showHeightInput).toBe(false)
	})

	it("shows sketch tools and commands in sketch mode", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "sketch",
			selectedExtrudeLabel: null,
			selectedPlaneLabel: "Top",
			selectedPlaneVisible: true,
			activeSketchTool: "rectangle",
			canUndo: true,
			canReset: true,
			canFinishSketch: false,
			canExtrude: false
		})

		expect(model.visible).toBe(true)
		expect(model.primaryActions.map((action) => action.id)).toEqual(["exit-sketch"])
		expect(model.sketchToolActions).toEqual([
			{ id: "tool-line", label: "Line", active: false },
			{ id: "tool-rectangle", label: "Rectangle", active: true }
		])
		expect(model.commandActions).toEqual([
			{ id: "undo", label: "Undo", disabled: false },
			{ id: "reset", label: "Reset", disabled: false },
			{ id: "finish-sketch", label: "Finish Sketch", disabled: true },
			{ id: "extrude", label: "Extrude", disabled: true }
		])
		expect(model.showHeightInput).toBe(true)
		expect(model.showStatus).toBe(true)
	})

	it("reflects finish and extrude readiness", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "sketch",
			selectedExtrudeLabel: null,
			selectedPlaneLabel: "Right",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			canUndo: false,
			canReset: false,
			canFinishSketch: true,
			canExtrude: true
		})

		expect(model.commandActions).toEqual([
			{ id: "undo", label: "Undo", disabled: true },
			{ id: "reset", label: "Reset", disabled: true },
			{ id: "finish-sketch", label: "Finish Sketch", disabled: false },
			{ id: "extrude", label: "Extrude", disabled: false }
		])
	})

	it("shows extrude editing controls when an extrude is selected", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "view",
			selectedExtrudeLabel: "Extrude 1",
			selectedPlaneLabel: null,
			selectedPlaneVisible: false,
			activeSketchTool: null,
			canUndo: false,
			canReset: false,
			canFinishSketch: false,
			canExtrude: false
		})

		expect(model.visible).toBe(true)
		expect(model.title).toBe("Extrude: Extrude 1")
		expect(model.commandActions).toEqual([{ id: "delete-extrude", label: "Delete Extrude" }])
		expect(model.showHeightInput).toBe(true)
		expect(model.showStatus).toBe(true)
	})
})
