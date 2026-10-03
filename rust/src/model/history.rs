//! Undo step labels and ordering stamps for the sketch history.
//!
//! Every snapshot pushed on [`Document`]'s undo stack carries a short
//! human-readable label ("Draw line", "Set length 40 mm") and a global
//! ordering stamp from [`next_step_stamp`]. The app's solid steps draw stamps
//! from the same counter, so the two stacks can be interleaved in time order
//! for the history panel and for cross-view undo (`app/history.rs`).

use super::{DimensionField, Document, DocumentState, Shape};
use std::sync::atomic::{AtomicU64, Ordering};

/// Returns a new, globally increasing undo step stamp. Shared by sketch and
/// solid steps so their relative order is known.
pub fn next_step_stamp() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// One entry of the sketch undo or redo stack, as shown in the history panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryStep {
    pub label: String,
    pub stamp: u64,
}

/// "1 shape" / "3 shapes".
pub fn count_label(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// Short number for labels: integers without decimals, otherwise up to two.
pub fn number_label(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded}")
    }
}

/// "Parallel" from "parallel".
pub fn title_label(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// "Set length 40 mm" style label for a driving dimension edit.
pub fn dimension_label(field: DimensionField, value: f32) -> String {
    let (name, unit) = match field {
        DimensionField::LineLength => ("length", " mm"),
        DimensionField::RectangleWidth => ("width", " mm"),
        DimensionField::RectangleHeight => ("height", " mm"),
        DimensionField::CircleDiameter => ("diameter", " mm"),
        DimensionField::CurveRadius => ("radius", " mm"),
        DimensionField::ArcSweep => ("sweep", " deg"),
        DimensionField::Rotation => ("rotation", " deg"),
    };
    format!("Set {name} {}{unit}", number_label(value))
}

/// "Draw line" for a freshly drawn shape.
pub fn draw_label(shape: &Shape) -> String {
    format!("Draw {}", shape.name().to_lowercase())
}

impl Document {
    /// Push `state` (taken before the edit) as an undo step named `label`.
    pub(super) fn record_undo(&mut self, mut state: DocumentState, label: impl Into<String>) {
        state.label = label.into();
        state.stamp = next_step_stamp();
        self.history.push(state);
    }

    /// Label of the most recent undo step, if any.
    pub(super) fn last_undo_label(&self) -> Option<&str> {
        self.history.last().map(|state| state.label.as_str())
    }

    /// Undo stack, oldest first.
    pub fn undo_steps(&self) -> Vec<HistoryStep> {
        self.history.iter().map(DocumentState::step).collect()
    }

    /// Redo stack, next step to redo first.
    pub fn redo_steps(&self) -> Vec<HistoryStep> {
        self.future.iter().rev().map(DocumentState::step).collect()
    }

    /// Stamp of the step [`Document::undo`] would revert.
    pub fn undo_stamp(&self) -> Option<u64> {
        self.history.last().map(|state| state.stamp)
    }

    /// Stamp of the step [`Document::redo`] would reapply.
    pub fn redo_stamp(&self) -> Option<u64> {
        self.future.last().map(|state| state.stamp)
    }
}

impl DocumentState {
    fn step(&self) -> HistoryStep {
        HistoryStep {
            label: self.label.clone(),
            stamp: self.stamp,
        }
    }
}
