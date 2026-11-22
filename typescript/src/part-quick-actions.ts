export type ReferencePlaneName = "Top" | "Front" | "Right"

export type PartQuickActionId = "start-sketch" | "exit-sketch" | "tool-line" | "tool-rectangle" | "dimension" | "undo" | "reset" | "finish-sketch" | "extrude" | "chamfer" | "delete-extrude"

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
	selectedFaceLabel: string | null
	selectedEdgeLabel?: string | null
	selectedCornerLabel?: string | null
	selectedPlaneLabel: string | null
	selectedPlaneVisible: boolean
	activeSketchTool: "line" | "rectangle" | null
	canDimension: boolean
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
	const selectedTargetLabel = state.selectedFaceLabel ?? state.selectedPlaneLabel
	const selectedTargetVisible = state.selectedFaceLabel ? true : state.selectedPlaneVisible

	if (state.selectedExtrudeLabel && state.activeTool === "view") {
		if (state.selectedFaceLabel) {
			return {
				visible: true,
				title: `Face: ${state.selectedFaceLabel}`,
				description: "Start a sketch on this face or adjust the parent extrude below.",
				primaryActions: [
					{
						id: "start-sketch",
						label: "Sketch"
					}
				],
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
		if (state.selectedEdgeLabel) {
			return {
				visible: true,
				title: `Edge: ${state.selectedEdgeLabel}`,
				description: "Chamfer this edge, adjust the parent extrude below, or delete it.",
				primaryActions: [
					{
						id: "chamfer",
						label: "Chamfer"
					}
				],
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
		if (state.selectedCornerLabel) {
			return {
				visible: true,
				title: `Corner: ${state.selectedCornerLabel}`,
				description: "Adjust the parent extrude below or delete it.",
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

	if (!selectedTargetLabel || !selectedTargetVisible) {
		return HIDDEN_MODEL
	}

	if (state.activeTool === "view") {
		return {
			visible: true,
			title: state.selectedFaceLabel ? `Face: ${state.selectedFaceLabel}` : `${selectedTargetLabel} Plane`,
			description: state.selectedFaceLabel ? "Start a sketch on the selected face." : `Start a sketch on the ${selectedTargetLabel.toLowerCase()} reference plane.`,
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
		title: `Sketch: ${selectedTargetLabel}`,
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
			...(state.canDimension
				? [
						{
							id: "dimension" as const,
							label: "Dimension"
						}
					]
				: []),
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
