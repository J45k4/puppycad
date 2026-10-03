//! Command palette (Ctrl+K / Ctrl+Shift+P): a searchable list generated
//! from the command registry.

use super::*;

/// Dimmed text for commands that cannot run in the current state.
pub(super) const DIMMED: Color = [0.34, 0.38, 0.43, 1.0];
const PALETTE_ROW_HEIGHT: f32 = 24.0;
const PALETTE_INPUT_HEIGHT: f32 = 34.0;
const PALETTE_MAX_ROWS: usize = 12;

#[derive(Default)]
pub(super) struct Palette {
    pub(super) query: String,
    /// Index into [`App::palette_entries`].
    pub(super) selected: usize,
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle.chars().all(|ch| chars.any(|other| other == ch))
}

/// Match quality of `query` against a command (lower is better), or `None`
/// when it does not match. The label weighs more than category and
/// shortcut; a subsequence ("mrr" → "Mirror") is the weakest match.
pub(super) fn palette_score(command: &Command, query: &str) -> Option<u8> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let label = command.label.to_lowercase();
    let haystack = format!(
        "{label} {} {}",
        command.category.name().to_lowercase(),
        command.shortcut.to_lowercase()
    );
    let exact_shortcut = command
        .shortcut
        .split(" / ")
        .any(|chord| chord.to_lowercase() == query);
    if label.starts_with(&query) || exact_shortcut {
        Some(0)
    } else if label
        .split(|ch: char| !ch.is_alphanumeric())
        .any(|word| word.starts_with(&query))
    {
        Some(1)
    } else if label.contains(&query) {
        Some(2)
    } else if haystack.contains(&query) {
        Some(3)
    } else if is_subsequence(&query.replace(' ', ""), &label) {
        Some(4)
    } else if is_subsequence(&query.replace(' ', ""), &haystack) {
        Some(5)
    } else {
        None
    }
}

/// Registry commands matching `query`, best first, ties in registry order.
pub(super) fn palette_matches(query: &str) -> Vec<&'static Command> {
    let mut matches: Vec<(u8, usize, &'static Command)> = COMMANDS
        .iter()
        .enumerate()
        .filter_map(|(index, command)| {
            palette_score(command, query).map(|score| (score, index, command))
        })
        .collect();
    matches.sort_by_key(|(score, index, _)| (*score, *index));
    matches.into_iter().map(|(_, _, command)| command).collect()
}

/// Command palette box centered over the viewport.
pub(super) fn palette_rect(layout: Layout) -> Rect {
    let w = 560.0_f32.min(layout.viewport.w - 24.0).max(200.0);
    let rows = palette_visible_rows(layout) as f32;
    Rect {
        x: layout.viewport.x + (layout.viewport.w - w) * 0.5,
        y: layout.viewport.y + 36.0,
        w,
        h: PALETTE_INPUT_HEIGHT + rows * PALETTE_ROW_HEIGHT + 34.0,
    }
}

fn palette_visible_rows(layout: Layout) -> usize {
    (((layout.viewport.h - 140.0) / PALETTE_ROW_HEIGHT).max(3.0) as usize).min(PALETTE_MAX_ROWS)
}

fn palette_row(layout: Layout, visible: usize) -> Rect {
    let rect = palette_rect(layout);
    Rect {
        x: rect.x + 4.0,
        y: rect.y + PALETTE_INPUT_HEIGHT + 6.0 + visible as f32 * PALETTE_ROW_HEIGHT,
        w: rect.w - 8.0,
        h: PALETTE_ROW_HEIGHT - 2.0,
    }
}

impl App {
    pub(super) fn open_palette(&mut self) {
        self.more_open = false;
        self.shortcut_overlay = false;
        self.palette = Some(Palette::default());
        self.palette_select_from(0, 1);
    }

    /// Filtered palette rows with their availability.
    pub(super) fn palette_entries(&self) -> Vec<(&'static Command, bool)> {
        let query = self.palette.as_ref().map_or("", |palette| &palette.query);
        palette_matches(query)
            .into_iter()
            .map(|command| (command, self.command_available(command)))
            .collect()
    }

    /// Select the first available row at or after `start` going in
    /// `direction`; keeps the old selection when there is none.
    fn palette_select_from(&mut self, start: isize, direction: isize) {
        let entries = self.palette_entries();
        let mut index = start;
        while index >= 0 && (index as usize) < entries.len() {
            if entries[index as usize].1 {
                if let Some(palette) = &mut self.palette {
                    palette.selected = index as usize;
                }
                return;
            }
            index += direction;
        }
        if let Some(palette) = &mut self.palette
            && !entries.get(palette.selected).is_some_and(|entry| entry.1)
        {
            palette.selected = 0;
        }
    }

    pub(super) fn palette_move(&mut self, direction: isize) {
        let Some(selected) = self.palette.as_ref().map(|palette| palette.selected) else {
            return;
        };
        self.palette_select_from(selected as isize + direction, direction);
    }

    pub(super) fn palette_type(&mut self, text: &str) {
        if let Some(palette) = &mut self.palette {
            for ch in text.chars() {
                if !ch.is_control() && palette.query.chars().count() < 40 {
                    palette.query.push(ch);
                }
            }
        }
        self.palette_select_from(0, 1);
    }

    pub(super) fn palette_backspace(&mut self) {
        if let Some(palette) = &mut self.palette {
            palette.query.pop();
        }
        self.palette_select_from(0, 1);
    }

    /// Enter: run the selected command when it is available.
    pub(super) fn palette_run(&mut self) {
        let Some(selected) = self.palette.as_ref().map(|palette| palette.selected) else {
            return;
        };
        let entry = self.palette_entries().get(selected).copied();
        self.palette = None;
        match entry {
            Some((command, true)) => (command.run)(self),
            Some((command, false)) => {
                self.status_message = format!("{} is not available here", command.label);
            }
            None => self.status_message = "No matching command".into(),
        }
    }

    /// Keys while the palette is open. Returns true when handled.
    pub(super) fn palette_key(&mut self, code: KeyCode, text: Option<&str>) -> bool {
        if self.palette.is_none() {
            return false;
        }
        let ctrl = self.modifiers.control_key() || self.modifiers.super_key();
        match code {
            KeyCode::Escape => self.palette = None,
            KeyCode::KeyK if ctrl => self.palette = None,
            KeyCode::KeyP if ctrl && self.modifiers.shift_key() => self.palette = None,
            KeyCode::Enter | KeyCode::NumpadEnter => self.palette_run(),
            KeyCode::ArrowDown => self.palette_move(1),
            KeyCode::ArrowUp => self.palette_move(-1),
            KeyCode::Backspace => self.palette_backspace(),
            _ if !ctrl => {
                if let Some(text) = text {
                    self.palette_type(text);
                }
            }
            _ => {}
        }
        true
    }

    /// Click while the palette or the cheat sheet is open: a palette row
    /// runs, a click outside the palette or anywhere on the cheat sheet
    /// closes it. Returns true when handled.
    pub(super) fn overlay_click(&mut self, layout: Layout, point: Point) -> bool {
        if self.shortcut_overlay {
            self.shortcut_overlay = false;
            return true;
        }
        if self.palette.is_none() {
            return false;
        }
        let first = self.palette_first_visible(layout);
        let count = self.palette_entries().len();
        for visible in 0..palette_visible_rows(layout) {
            let index = first + visible;
            if index < count && palette_row(layout, visible).contains(point.x, point.y) {
                if let Some(palette) = &mut self.palette {
                    palette.selected = index;
                }
                self.palette_run();
                return true;
            }
        }
        if !palette_rect(layout).contains(point.x, point.y) {
            self.palette = None;
        }
        true
    }

    fn palette_first_visible(&self, layout: Layout) -> usize {
        let rows = palette_visible_rows(layout);
        let selected = self.palette.as_ref().map_or(0, |palette| palette.selected);
        (selected + 1).saturating_sub(rows)
    }

    pub(super) fn paint_palette(&self, painter: &mut Painter, layout: Layout) {
        let Some(palette) = &self.palette else {
            return;
        };
        let rect = palette_rect(layout);
        painter.rect(rect, PANEL);
        painter.outline(rect, 2.0, ACCENT);
        let input = Rect {
            x: rect.x + 6.0,
            y: rect.y + 6.0,
            w: rect.w - 12.0,
            h: PALETTE_INPUT_HEIGHT - 10.0,
        };
        painter.rect(input, PANEL_LIGHT);
        painter.outline(input, 1.0, BORDER);
        let max_chars = ((input.w - 40.0) / 8.0) as usize;
        let query = clip_text(&palette.query, max_chars, true);
        painter.text(input.x + 8.0, input.y + 8.0, ">", 1.0, ACCENT);
        if palette.query.is_empty() {
            painter.text(
                input.x + 24.0,
                input.y + 8.0,
                "Type a command...",
                1.0,
                MUTED,
            );
            painter.rect(
                Rect {
                    x: input.x + 24.0,
                    y: input.y + 6.0,
                    w: 2.0,
                    h: 12.0,
                },
                TEXT,
            );
        } else {
            painter.text(input.x + 24.0, input.y + 8.0, &query, 1.0, TEXT);
            painter.rect(
                Rect {
                    x: input.x + 24.0 + query.chars().count() as f32 * 8.0 + 1.0,
                    y: input.y + 6.0,
                    w: 2.0,
                    h: 12.0,
                },
                TEXT,
            );
        }
        let entries = self.palette_entries();
        let first = self.palette_first_visible(layout);
        let category_x = rect.w * 0.52;
        let label_chars = ((category_x - 20.0) / 8.0) as usize;
        for visible in 0..palette_visible_rows(layout) {
            let index = first + visible;
            let Some((command, available)) = entries.get(index) else {
                break;
            };
            let row = palette_row(layout, visible);
            if index == palette.selected && *available {
                painter.rect(row, [0.13, 0.31, 0.40, 1.0]);
                painter.outline(row, 1.0, ACCENT);
            }
            let (label_color, meta_color, key_color) = if *available {
                (TEXT, MUTED, SHAPE)
            } else {
                (DIMMED, DIMMED, DIMMED)
            };
            painter.text(
                row.x + 10.0,
                row.y + 7.0,
                &clip_text(command.label, label_chars, false),
                1.0,
                label_color,
            );
            painter.text(
                rect.x + category_x,
                row.y + 7.0,
                command.category.name(),
                1.0,
                meta_color,
            );
            let keys = command.shortcut;
            painter.text(
                row.x + row.w - 10.0 - keys.chars().count() as f32 * 8.0,
                row.y + 7.0,
                keys,
                1.0,
                key_color,
            );
        }
        if entries.is_empty() {
            painter.text(
                rect.x + 14.0,
                rect.y + PALETTE_INPUT_HEIGHT + 13.0,
                "No matching command",
                1.0,
                MUTED,
            );
        }
        let available = entries.iter().filter(|entry| entry.1).count();
        painter.text(
            rect.x + 12.0,
            rect.y + rect.h - 20.0,
            &format!(
                "Up/Down: move  Enter: run  Esc: close   {available}/{} available",
                entries.len()
            ),
            1.0,
            MUTED,
        );
    }
}
