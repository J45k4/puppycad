//! Linear and circular pattern tools: Properties prompts for count, spacing
//! and angle, then (circular) a click for the center.

use super::*;
use puppycad::model::PatternKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PatternTool {
    Linear,
    Circular,
}

/// Prompt steps; values are collected in [`PatternPrompt::values`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PatternStep {
    Count,
    StepX,
    StepY,
    Rows,
    RowX,
    RowY,
    Angle,
}

impl PatternStep {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Count => "COUNT",
            Self::StepX => "SPACING X",
            Self::StepY => "SPACING Y",
            Self::Rows => "ROWS",
            Self::RowX => "ROW SPACING X",
            Self::RowY => "ROW SPACING Y",
            Self::Angle => "ANGLE",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Self::Count | Self::Rows => "",
            Self::Angle => " deg",
            _ => " mm",
        }
    }

    /// Whether the value may be negative.
    pub(super) fn signed(self) -> bool {
        matches!(
            self,
            Self::StepX | Self::StepY | Self::RowX | Self::RowY | Self::Angle
        )
    }
}

#[derive(Clone, Debug)]
pub(super) struct PatternPrompt {
    pub(super) tool: PatternTool,
    pub(super) sources: Vec<u32>,
    /// Existing pattern being edited.
    pub(super) existing: Option<usize>,
    pub(super) step: PatternStep,
    pub(super) values: PatternValues,
}

/// Values the prompts start from (and remember).
#[derive(Clone, Copy, Debug)]
pub(super) struct PatternValues {
    pub(super) count: usize,
    pub(super) step: Point,
    pub(super) rows: usize,
    pub(super) row_step: Point,
    pub(super) circular_count: usize,
    pub(super) angle: f32,
}

impl Default for PatternValues {
    fn default() -> Self {
        Self {
            count: 3,
            step: Point { x: 20.0, y: 0.0 },
            rows: 1,
            row_step: Point { x: 0.0, y: 20.0 },
            circular_count: 6,
            angle: 360.0,
        }
    }
}

impl PatternValues {
    fn get(&self, tool: PatternTool, step: PatternStep) -> f32 {
        match step {
            PatternStep::Count if tool == PatternTool::Circular => self.circular_count as f32,
            PatternStep::Count => self.count as f32,
            PatternStep::StepX => self.step.x,
            PatternStep::StepY => self.step.y,
            PatternStep::Rows => self.rows as f32,
            PatternStep::RowX => self.row_step.x,
            PatternStep::RowY => self.row_step.y,
            PatternStep::Angle => self.angle,
        }
    }
}

impl App {
    /// MORE → LINEAR/CIRCULAR PATTERN or Shift+Z / Shift+U. With a pattern
    /// member selected, edit that pattern; otherwise pattern the selection.
    pub(super) fn begin_pattern(&mut self, tool: PatternTool) {
        self.more_open = false;
        if self.read_only() {
            self.status_message = "READ ONLY: this sketch has unsupported native features".into();
            return;
        }
        let selection = self.document.selected_ids();
        if selection.is_empty() {
            self.status_message = "Select geometry to pattern first".into();
            return;
        }
        self.document.finish_drag();
        let existing = selection
            .iter()
            .find_map(|&id| self.document.pattern_of(id))
            .filter(|&index| {
                matches!(
                    (&self.document.patterns[index].kind, tool),
                    (PatternKind::Linear { .. }, PatternTool::Linear)
                        | (PatternKind::Circular { .. }, PatternTool::Circular)
                )
            });
        if let Some(index) = existing {
            let pattern = &self.document.patterns[index];
            match pattern.kind {
                PatternKind::Linear {
                    step,
                    columns,
                    row_step,
                } => {
                    self.pattern_values.count = columns;
                    self.pattern_values.step = step;
                    self.pattern_values.rows = pattern.rows();
                    self.pattern_values.row_step = row_step;
                }
                PatternKind::Circular { angle, .. } => {
                    self.pattern_values.circular_count = pattern.count();
                    self.pattern_values.angle = angle;
                }
            }
        } else if selection
            .iter()
            .any(|&id| self.document.pattern_of(id).is_some())
        {
            self.status_message = "Pattern: selection is already part of a pattern".into();
            return;
        }
        self.pattern_prompt = Some(PatternPrompt {
            tool,
            sources: selection,
            existing,
            step: PatternStep::Count,
            values: self.pattern_values,
        });
        self.open_pattern_step(PatternStep::Count);
    }

    fn open_pattern_step(&mut self, step: PatternStep) {
        let Some(prompt) = &mut self.pattern_prompt else {
            return;
        };
        prompt.step = step;
        let fallback = prompt.values.get(prompt.tool, step);
        let name = match prompt.tool {
            PatternTool::Linear => "Linear pattern",
            PatternTool::Circular => "Circular pattern",
        };
        self.editing = Some(DimensionEdit {
            field: EditField::PatternValue,
            buffer: String::new(),
        });
        self.status_message = format!(
            "{name} {} (Enter keeps {fallback}{})  |  Esc to cancel",
            step.label().to_lowercase(),
            step.unit()
        );
    }

    /// Prompt text shown in the Properties field.
    pub(super) fn pattern_prompt_text(&self, buffer: &str) -> String {
        match &self.pattern_prompt {
            Some(prompt) => format!("{} > {buffer}_{}", prompt.step.label(), prompt.step.unit()),
            None => format!("> {buffer}_"),
        }
    }

    pub(super) fn pattern_step_signed(&self) -> bool {
        self.pattern_prompt
            .as_ref()
            .is_some_and(|prompt| prompt.step.signed())
    }

    /// Finish one pattern prompt step; returns the status message.
    pub(super) fn finish_pattern_edit(&mut self, buffer: &str) -> String {
        let Some(mut prompt) = self.pattern_prompt.take() else {
            return "Pattern: nothing to edit".into();
        };
        let step = prompt.step;
        let value = if buffer.is_empty() {
            prompt.values.get(prompt.tool, step)
        } else {
            match buffer.parse::<f32>() {
                Ok(value) if value.is_finite() => value,
                _ => return "Pattern error: enter a number".into(),
            }
        };
        let integer = |value: f32, min: usize| -> Result<usize, String> {
            if value.fract() == 0.0 && value >= min as f32 && value <= 64.0 {
                Ok(value as usize)
            } else {
                Err(format!(
                    "Pattern error: enter a whole number of at least {min}"
                ))
            }
        };
        let next = match (prompt.tool, step) {
            (PatternTool::Linear, PatternStep::Count) => match integer(value, 1) {
                Ok(count) => {
                    prompt.values.count = count;
                    Some(PatternStep::StepX)
                }
                Err(error) => return error,
            },
            (_, PatternStep::StepX) => {
                prompt.values.step.x = value;
                Some(PatternStep::StepY)
            }
            (_, PatternStep::StepY) => {
                prompt.values.step.y = value;
                Some(PatternStep::Rows)
            }
            (_, PatternStep::Rows) => match integer(value, 1) {
                Ok(rows) => {
                    prompt.values.rows = rows;
                    (rows > 1).then_some(PatternStep::RowX)
                }
                Err(error) => return error,
            },
            (_, PatternStep::RowX) => {
                prompt.values.row_step.x = value;
                Some(PatternStep::RowY)
            }
            (_, PatternStep::RowY) => {
                prompt.values.row_step.y = value;
                None
            }
            (PatternTool::Circular, PatternStep::Count) => match integer(value, 2) {
                Ok(count) => {
                    prompt.values.circular_count = count;
                    Some(PatternStep::Angle)
                }
                Err(error) => return error,
            },
            (_, PatternStep::Angle) => {
                prompt.values.angle = value;
                None
            }
        };
        self.pattern_values = prompt.values;
        if let Some(next) = next {
            self.pattern_prompt = Some(prompt);
            self.open_pattern_step(next);
            return self.status_message.clone();
        }
        let values = prompt.values;
        match (prompt.tool, prompt.existing) {
            (PatternTool::Linear, existing) => {
                let rows = values.rows.max(1);
                let result = match existing {
                    Some(index) => self.document.edit_linear_pattern(
                        index,
                        values.count,
                        values.step,
                        rows,
                        values.row_step,
                    ),
                    None => self
                        .document
                        .linear_pattern(
                            &prompt.sources,
                            values.count,
                            values.step,
                            rows,
                            values.row_step,
                        )
                        .map(|_| ()),
                };
                match result {
                    Ok(()) => format!(
                        "Linear pattern {}x{}  |  Ctrl+Z to undo  |  Ctrl+S to save",
                        values.count, rows
                    ),
                    Err(error) => format!("Linear pattern: {error}"),
                }
            }
            (PatternTool::Circular, Some(index)) => {
                let center = self
                    .document
                    .pattern_center(&self.document.patterns[index])
                    .unwrap_or(Point { x: 0.0, y: 0.0 });
                match self.document.edit_circular_pattern(
                    index,
                    values.circular_count,
                    values.angle,
                    center,
                ) {
                    Ok(()) => format!(
                        "Circular pattern x{}  |  Ctrl+Z to undo  |  Ctrl+S to save",
                        values.circular_count
                    ),
                    Err(error) => format!("Circular pattern: {error}"),
                }
            }
            (PatternTool::Circular, None) => {
                self.select_tool(Tool::Select);
                self.pick = Some(Pick::PatternCenter {
                    sources: prompt.sources,
                });
                "Circular pattern: click a point, line end or center (or the origin)  |  Esc to cancel"
                    .into()
            }
        }
    }

    /// Viewport click choosing a circular pattern center: the nearest
    /// point/line end/center anchor under the cursor becomes a referenced
    /// center; otherwise the clicked point (snapped) is used as-is.
    pub(super) fn pattern_center_click(&mut self, sources: Vec<u32>, world: Point) -> String {
        let tolerance = 7.0 / self.document.zoom.max(0.2);
        let anchor = self.document.anchor_near(world, tolerance);
        let center = match anchor {
            Some(anchor) => self.document.symmetry_anchor_point(anchor).unwrap_or(world),
            None if world.distance(Point { x: 0.0, y: 0.0 }) <= tolerance => {
                Point { x: 0.0, y: 0.0 }
            }
            None => world,
        };
        let values = self.pattern_values;
        match self.document.circular_pattern(
            &sources,
            values.circular_count,
            values.angle,
            center,
            anchor,
        ) {
            Ok(_) => format!(
                "Circular pattern x{} over {} deg  |  Ctrl+Z to undo  |  Ctrl+S to save",
                values.circular_count, values.angle
            ),
            Err(error) => format!("Circular pattern: {error}"),
        }
    }
}
