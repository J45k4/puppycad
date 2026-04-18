import { describe, expect, it } from "bun:test"

import { derivePartQuickActionsModel } from "../src/part-quick-actions"

describe("derivePartQuickActionsModel", () => {
	it("hides the rail when no plane is selected", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "view",
			selectedExtrudeLabel: null,
			selectedFaceLabel: null,
			selectedPlaneLabel: null,
			selectedPlaneVisible: false,
			activeSketchTool: "line",
			canDimension: false,
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
			selectedFaceLabel: null,
			selectedPlaneLabel: "Front",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			canDimension: false,
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
			selectedFaceLabel: null,
			selectedPlaneLabel: "Top",
			selectedPlaneVisible: true,
			activeSketchTool: "rectangle",
			canDimension: false,
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
			selectedFaceLabel: null,
			selectedPlaneLabel: "Right",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			canDimension: false,
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
			selectedFaceLabel: null,
			selectedPlaneLabel: null,
			selectedPlaneVisible: false,
			activeSketchTool: null,
			canDimension: false,
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

	it("shows face sketch actions when a face is selected", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "view",
			selectedExtrudeLabel: "Extrude 1",
			selectedFaceLabel: "Top Face",
			selectedPlaneLabel: null,
			selectedPlaneVisible: false,
			activeSketchTool: null,
			canDimension: false,
			canUndo: false,
			canReset: false,
			canFinishSketch: false,
			canExtrude: false
		})

		expect(model.title).toBe("Face: Top Face")
		expect(model.primaryActions).toEqual([{ id: "start-sketch", label: "Sketch" }])
		expect(model.commandActions).toEqual([{ id: "delete-extrude", label: "Delete Extrude" }])
		expect(model.showHeightInput).toBe(true)
	})

	it("shows the dimension command only when the current sketch selection supports it", () => {
		const hidden = derivePartQuickActionsModel({
			activeTool: "sketch",
			selectedExtrudeLabel: null,
			selectedFaceLabel: null,
			selectedPlaneLabel: "Front",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			canDimension: false,
			canUndo: false,
			canReset: false,
			canFinishSketch: false,
			canExtrude: false
		})

		const visible = derivePartQuickActionsModel({
			activeTool: "sketch",
			selectedExtrudeLabel: null,
			selectedFaceLabel: null,
			selectedPlaneLabel: "Front",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			canDimension: true,
			canUndo: false,
			canReset: false,
			canFinishSketch: false,
			canExtrude: false
		})

		expect(hidden.commandActions.map((action) => action.id)).not.toContain("dimension")
		expect(visible.commandActions[0]).toEqual({ id: "dimension", label: "Dimension" })
	})
})
