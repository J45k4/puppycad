export type ReferencePlaneName = "Top" | "Front" | "Right"

export type PartQuickActionId = "start-sketch" | "exit-sketch" | "tool-line" | "tool-rectangle" | "undo" | "reset" | "finish-sketch" | "extrude" | "delete-extrude"

export type PartQuickActionItem = {
	id: PartQuickActionId
	label: string
	active?: boolean
	disabled?: boolean
}

export type PartQuickActionsModel = {
	visible: boolean
	title: string
	description: string
	primaryActions: PartQuickActionItem[]
	sketchToolActions: PartQuickActionItem[]
	commandActions: PartQuickActionItem[]
	showHeightInput: boolean
	showStatus: boolean
}

export type PartQuickActionsState = {
	activeTool: "view" | "sketch"
	selectedExtrudeLabel: string | null
	selectedPlaneLabel: string | null
	selectedPlaneVisible: boolean
	activeSketchTool: "line" | "rectangle" | null
	canUndo: boolean
	canReset: boolean
	canFinishSketch: boolean
	canExtrude: boolean
}

const HIDDEN_MODEL: PartQuickActionsModel = {
	visible: false,
	title: "",
	description: "",
	primaryActions: [],
	sketchToolActions: [],
	commandActions: [],
	showHeightInput: false,
	showStatus: false
}

export function derivePartQuickActionsModel(state: PartQuickActionsState): PartQuickActionsModel {
	if (state.selectedExtrudeLabel) {
		return {
			visible: true,
			title: `Extrude: ${state.selectedExtrudeLabel}`,
			description: "Adjust the blind depth below or delete this extrude feature.",
			primaryActions: [],
			sketchToolActions: [],
			commandActions: [
				{
					id: "delete-extrude",
					label: "Delete Extrude"
				}
			],
			showHeightInput: true,
			showStatus: true
		}
	}

	if (!state.selectedPlaneLabel || !state.selectedPlaneVisible) {
		return HIDDEN_MODEL
	}

	if (state.activeTool === "view") {
		return {
			visible: true,
			title: `${state.selectedPlaneLabel} Plane`,
			description: `Start a sketch on the ${state.selectedPlaneLabel.toLowerCase()} reference plane.`,
			primaryActions: [
				{
					id: "start-sketch",
					label: "Sketch"
				}
			],
			sketchToolActions: [],
			commandActions: [],
			showHeightInput: false,
			showStatus: false
		}
	}

	return {
		visible: true,
		title: `Sketch: ${state.selectedPlaneLabel}`,
		description: "Choose a drawing tool or use the sketch actions below.",
		primaryActions: [
			{
				id: "exit-sketch",
				label: "Exit Sketch"
			}
		],
		sketchToolActions: [
			{
				id: "tool-line",
				label: "Line",
				active: state.activeSketchTool === "line"
			},
			{
				id: "tool-rectangle",
				label: "Rectangle",
				active: state.activeSketchTool === "rectangle"
			}
		],
		commandActions: [
			{
				id: "undo",
				label: "Undo",
				disabled: !state.canUndo
			},
			{
				id: "reset",
				label: "Reset",
				disabled: !state.canReset
			},
			{
				id: "finish-sketch",
				label: "Finish Sketch",
				disabled: !state.canFinishSketch
			},
			{
				id: "extrude",
				label: "Extrude",
				disabled: !state.canExtrude
			}
		],
		showHeightInput: true,
		showStatus: true
	}
}
