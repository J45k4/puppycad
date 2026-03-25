import { describe, expect, it } from "bun:test"

import { derivePartQuickActionsModel } from "../src/part-quick-actions"

describe("derivePartQuickActionsModel", () => {
	it("hides the rail when no plane is selected", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "view",
			selectedPlaneName: null,
			selectedPlaneVisible: false,
			activeSketchTool: "line",
			sketchPointCount: 0,
			isSketchClosed: false,
			hasSketchBreaks: false,
			hasExtrudedModel: false,
			hasPendingLineStart: false
		})

		expect(model.visible).toBe(false)
		expect(model.primaryActions).toEqual([])
	})

	it("shows only sketch when a visible plane is selected in view mode", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "view",
			selectedPlaneName: "Front",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			sketchPointCount: 0,
			isSketchClosed: false,
			hasSketchBreaks: false,
			hasExtrudedModel: false,
			hasPendingLineStart: false
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
			selectedPlaneName: "Top",
			selectedPlaneVisible: true,
			activeSketchTool: "rectangle",
			sketchPointCount: 2,
			isSketchClosed: false,
			hasSketchBreaks: false,
			hasExtrudedModel: false,
			hasPendingLineStart: false
		})

		expect(model.visible).toBe(true)
		expect(model.primaryActions.map((action) => action.id)).toEqual(["exit-sketch"])
		expect(model.sketchToolActions).toEqual([
			{ id: "tool-line", label: "Line", active: false },
			{ id: "tool-rectangle", label: "Rectangle", active: true }
		])
		expect(model.commandActions.map((action) => action.id)).toEqual(["undo", "reset", "finish-sketch", "extrude"])
		expect(model.showHeightInput).toBe(true)
		expect(model.showStatus).toBe(true)
	})

	it("matches current disabled rules for sketch commands", () => {
		const openModel = derivePartQuickActionsModel({
			activeTool: "sketch",
			selectedPlaneName: "Right",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			sketchPointCount: 2,
			isSketchClosed: false,
			hasSketchBreaks: true,
			hasExtrudedModel: false,
			hasPendingLineStart: false
		})

		expect(openModel.commandActions).toEqual([
			{ id: "undo", label: "Undo", disabled: false },
			{ id: "reset", label: "Reset", disabled: false },
			{ id: "finish-sketch", label: "Finish Sketch", disabled: true },
			{ id: "extrude", label: "Extrude", disabled: true }
		])

		const closedModel = derivePartQuickActionsModel({
			activeTool: "sketch",
			selectedPlaneName: "Right",
			selectedPlaneVisible: true,
			activeSketchTool: "line",
			sketchPointCount: 4,
			isSketchClosed: true,
			hasSketchBreaks: false,
			hasExtrudedModel: true,
			hasPendingLineStart: false
		})

		expect(closedModel.commandActions).toEqual([
			{ id: "undo", label: "Undo", disabled: true },
			{ id: "reset", label: "Reset", disabled: false },
			{ id: "finish-sketch", label: "Finish Sketch", disabled: true },
			{ id: "extrude", label: "Extrude", disabled: false }
		])
	})

	it("hides the rail again when the selected plane is cleared or hidden", () => {
		const model = derivePartQuickActionsModel({
			activeTool: "sketch",
			selectedPlaneName: "Front",
			selectedPlaneVisible: false,
			activeSketchTool: "line",
			sketchPointCount: 3,
			isSketchClosed: false,
			hasSketchBreaks: false,
			hasExtrudedModel: false,
			hasPendingLineStart: false
		})

		expect(model.visible).toBe(false)
	})
})
