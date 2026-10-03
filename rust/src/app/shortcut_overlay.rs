//! Shortcut cheat sheet (? / F1): every registry shortcut grouped by
//! category, flowed into as many columns as the window fits.

use super::*;

/// One line of the shortcut cheat sheet.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum OverlayText {
    Header(&'static str),
    Entry { keys: String, label: String },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct OverlayLine {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) text: OverlayText,
}

pub(super) const OVERLAY_MARGIN: f32 = 24.0;
pub(super) const OVERLAY_TOP: f32 = 60.0;
pub(super) const OVERLAY_LINE: f32 = 14.0;
/// Room for a 15-character key column and a 33-character label.
const OVERLAY_COLUMN_MIN: f32 = 400.0;
const OVERLAY_KEY_CHARS: usize = 15;

/// Width of each cheat-sheet column for a window `width` wide.
pub(super) fn overlay_column_width(width: f32) -> f32 {
    let usable = width - 2.0 * OVERLAY_MARGIN;
    let columns = (usable / OVERLAY_COLUMN_MIN).floor().max(1.0);
    usable / columns
}

/// Category blocks (header first) of every command that has a shortcut.
fn overlay_blocks(label_chars: usize) -> Vec<Vec<OverlayText>> {
    let mut blocks = Vec::new();
    for category in Category::ALL {
        let entries: Vec<&Command> = COMMANDS
            .iter()
            .filter(|command| command.category == category && !command.shortcut.is_empty())
            .collect();
        if entries.is_empty() {
            continue;
        }
        let mut block = vec![OverlayText::Header(category.name())];
        for command in entries {
            // Long alternatives ("Ctrl+Y / Ctrl+Shift+Z") get a line each.
            let keys: Vec<&str> = if command.shortcut.len() < OVERLAY_KEY_CHARS {
                vec![command.shortcut]
            } else {
                command.shortcut.split(" / ").collect()
            };
            for (index, key) in keys.into_iter().enumerate() {
                block.push(OverlayText::Entry {
                    keys: key.to_string(),
                    label: if index == 0 {
                        clip_text(command.label, label_chars, false)
                    } else {
                        String::new()
                    },
                });
            }
        }
        blocks.push(block);
    }
    blocks
}

/// Cheat sheet lines grouped by category and balanced over as many columns
/// as fit `width`; a column only breaks at the bottom of the window or
/// before a category once it holds its share, so headers never end one.
pub(super) fn shortcut_overlay_lines(width: f32, height: f32) -> Vec<OverlayLine> {
    let column_width = overlay_column_width(width);
    let columns = ((width - 2.0 * OVERLAY_MARGIN) / column_width)
        .round()
        .max(1.0);
    let label_chars = ((column_width - OVERLAY_KEY_CHARS as f32 * 8.0 - 12.0) / 8.0) as usize;
    let bottom = height - OVERLAY_MARGIN;
    let gap = OVERLAY_LINE * 0.6;
    let blocks = overlay_blocks(label_chars);
    let total: f32 = blocks
        .iter()
        .map(|block| block.len() as f32 * OVERLAY_LINE + gap)
        .sum();
    let share = total / columns;
    let mut lines = Vec::new();
    let mut column = 0.0;
    let mut y = OVERLAY_TOP;
    for block in blocks {
        let block_height = block.len() as f32 * OVERLAY_LINE;
        // Start the category in the next column when this one already
        // holds its share and the category would mostly spill past it.
        if y > OVERLAY_TOP && column + 1.0 < columns && y - OVERLAY_TOP + block_height * 0.5 > share
        {
            column += 1.0;
            y = OVERLAY_TOP;
        }
        for (index, text) in block.into_iter().enumerate() {
            let needed = if index == 0 { 2.0 } else { 1.0 } * OVERLAY_LINE;
            if y + needed > bottom && y > OVERLAY_TOP {
                column += 1.0;
                y = OVERLAY_TOP;
            }
            lines.push(OverlayLine {
                x: OVERLAY_MARGIN + column * column_width,
                y,
                text,
            });
            y += OVERLAY_LINE;
        }
        y += gap;
    }
    lines
}

impl App {
    pub(super) fn toggle_shortcut_overlay(&mut self) {
        self.palette = None;
        self.more_open = false;
        self.shortcut_overlay = !self.shortcut_overlay;
    }

    /// Keys while the cheat sheet is open: ?, F1 or Esc close it, other keys
    /// are swallowed. Returns true when handled.
    pub(super) fn shortcut_overlay_key(&mut self, code: KeyCode) -> bool {
        if !self.shortcut_overlay {
            return false;
        }
        if matches!(code, KeyCode::Escape | KeyCode::F1)
            || (code == KeyCode::Slash && self.modifiers.shift_key())
        {
            self.shortcut_overlay = false;
        }
        true
    }

    pub(super) fn paint_shortcut_overlay(&self, painter: &mut Painter, layout: Layout) {
        if !self.shortcut_overlay {
            return;
        }
        let width = layout.top.w;
        let height = layout.bottom.y + layout.bottom.h;
        painter.rect(
            Rect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: height,
            },
            BG,
        );
        painter.text(
            OVERLAY_MARGIN,
            OVERLAY_MARGIN,
            "KEYBOARD SHORTCUTS",
            1.5,
            ACCENT,
        );
        painter.text(
            OVERLAY_MARGIN + 300.0,
            OVERLAY_MARGIN + 3.0,
            "? / F1 / Esc: close    Ctrl+K: command palette",
            1.0,
            MUTED,
        );
        let column_width = overlay_column_width(width);
        for line in shortcut_overlay_lines(width, height) {
            match &line.text {
                OverlayText::Header(name) => {
                    painter.text(line.x, line.y, &name.to_uppercase(), 1.0, ACCENT);
                    painter.rect(
                        Rect {
                            x: line.x,
                            y: line.y + 10.0,
                            w: column_width - 16.0,
                            h: 1.0,
                        },
                        BORDER,
                    );
                }
                OverlayText::Entry { keys, label } => {
                    painter.text(line.x, line.y, keys, 1.0, SHAPE);
                    painter.text(
                        line.x + OVERLAY_KEY_CHARS as f32 * 8.0,
                        line.y,
                        label,
                        1.0,
                        TEXT,
                    );
                }
            }
        }
    }
}
