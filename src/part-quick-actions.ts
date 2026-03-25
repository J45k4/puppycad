export type ReferencePlaneName = "Top" | "Front" | "Right"

export type PartQuickActionId = "start-sketch" | "exit-sketch" | "tool-line" | "tool-rectangle" | "undo" | "reset" | "finish-sketch" | "extrude"

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
	selectedPlaneName: ReferencePlaneName | null
	selectedPlaneVisible: boolean
	activeSketchTool: "line" | "rectangle" | null
	sketchPointCount: number
	isSketchClosed: boolean
	hasSketchBreaks: boolean
	hasExtrudedModel: boolean
	hasPendingLineStart: boolean
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
	if (!state.selectedPlaneName || !state.selectedPlaneVisible) {
		return HIDDEN_MODEL
	}

	if (state.activeTool === "view") {
		return {
			visible: true,
			title: `${state.selectedPlaneName} Plane`,
			description: `Start a sketch on the ${state.selectedPlaneName.toLowerCase()} reference plane.`,
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
		title: `Sketch: ${state.selectedPlaneName}`,
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
				disabled: state.isSketchClosed || (state.sketchPointCount === 0 && !state.hasPendingLineStart)
			},
			{
				id: "reset",
				label: "Reset",
				disabled: state.sketchPointCount === 0 && !state.hasExtrudedModel && !state.hasPendingLineStart
			},
			{
				id: "finish-sketch",
				label: "Finish Sketch",
				disabled: state.isSketchClosed || state.sketchPointCount < 3 || state.hasSketchBreaks
			},
			{
				id: "extrude",
				label: "Extrude",
				disabled: !state.isSketchClosed
			}
		],
		showHeightInput: true,
		showStatus: true
	}
}
