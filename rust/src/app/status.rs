//! Status bar fields (cursor coordinates, tool, DOF, snap state, zoom),
//! toolbar/MORE tooltips and the toolbar's pixel-art glyphs.

use super::*;
use std::time::{Duration, Instant};

/// Delay before a hovered toolbar button shows its tooltip.
pub(crate) const TOOLTIP_DELAY: Duration = Duration::from_millis(400);

/// The right side of the status bar; recomputed per frame but only
/// rebuilt into text when a field changes (`StatusCache`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StatusInfo {
    /// Cursor in world millimeters, `None` outside the sketch viewport.
    pub cursor: Option<(f32, f32)>,
    pub units: Units,
    pub tool: &'static str,
    pub dof: Option<String>,
    pub snap_geometry: bool,
    pub snap_grid: bool,
    pub zoom_percent: i32,
}

impl StatusInfo {
    /// Two lines: coordinates, then tool / DOF / snap / zoom.
    pub(crate) fn lines(&self) -> [String; 2] {
        let units = self.units;
        let coords = match self.cursor {
            Some((x, y)) => format!(
                "X {:.*} Y {:.*} {}",
                units.decimals(),
                units.mm_to_display(x),
                units.decimals(),
                units.mm_to_display(y),
                units.name().to_uppercase()
            ),
            None => format!("X - Y - {}", units.name().to_uppercase()),
        };
        let mut second = format!("{}  ", self.tool);
        if let Some(dof) = &self.dof {
            second.push_str(dof);
            second.push_str("  ");
        }
        // Zoom is drawn at the far right of the status bar already.
        second.push_str(match (self.snap_geometry, self.snap_grid) {
            (true, true) => "SNAP GEOM+GRID",
            (true, false) => "SNAP GEOM",
            (false, true) => "SNAP GRID",
            (false, false) => "SNAP OFF",
        });
        [coords, second]
    }
}

/// Caches the rendered status text; `update` reports whether it changed.
#[derive(Default)]
pub(crate) struct StatusCache {
    info: Option<StatusInfo>,
    lines: [String; 2],
}

impl StatusCache {
    pub(crate) fn update(&mut self, info: StatusInfo) -> bool {
        if self.info.as_ref() == Some(&info) {
            return false;
        }
        self.lines = info.lines();
        self.info = Some(info);
        true
    }

    pub(crate) fn lines(&self) -> &[String; 2] {
        &self.lines
    }
}

pub(crate) fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => "SELECT",
        Tool::Line => "LINE",
        Tool::Rectangle => "RECT",
        Tool::CenterRectangle => "CTR RECT",
        Tool::ThreePointRectangle => "3PT RECT",
        Tool::ThreePointCenterRectangle => "3PT CTR RECT",
        Tool::Circle => "CIRCLE",
        Tool::ThreePointCircle => "3PT CIRCLE",
        Tool::Ellipse => "ELLIPSE",
        Tool::FitSpline => "FIT SPLINE",
        Tool::ControlSpline => "CTRL SPLINE",
        Tool::Arc => "ARC",
        Tool::CenterArc => "CTR ARC",
        Tool::Point => "POINT",
        Tool::Dimension => "DIMENSION",
        Tool::Polygon => "POLYGON",
        Tool::Slot => "SLOT",
        Tool::TangentArc => "TAN ARC",
    }
}

/// A toolbar button or MORE menu row under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HoverTarget {
    Tool(usize),
    More(usize),
}

/// Tooltips (command name, shortcut) for toolbar buttons 0..=9. A local
/// table: the command registry (`app/commands.rs`) is not on this base.
pub(crate) const TOOL_TIPS: [(&str, &str); 10] = [
    ("Select", "V"),
    ("Rectangle", "R"),
    ("Circle", "C"),
    ("Line", "L"),
    ("Ellipse", "E"),
    ("New sketch on Top plane", "1"),
    ("New sketch on Front plane", "2"),
    ("New sketch on Right plane", "3"),
    ("Three-point arc", "A"),
    ("More tools", "M"),
];

/// Tooltips for MORE menu rows, in `paint_more_menu` order.
pub(crate) const MORE_TIPS: [(&str, &str); 22] = [
    ("Point", "P"),
    ("Polygon", "G"),
    ("Slot", "S"),
    ("Extrude selected profile", "X"),
    ("New part", "B"),
    ("Three-point circle", "-"),
    ("Center-point arc", "-"),
    ("Center rectangle", "-"),
    ("Three-point rectangle", "-"),
    ("Three-point center rectangle", "-"),
    ("Join endpoints", "Shift+J"),
    ("Fit-point spline", "-"),
    ("Control-point spline", "-"),
    ("Mirror selection", "Shift+I"),
    ("Fillet corner", "Shift+W"),
    ("Chamfer corner", "Shift+H"),
    ("Offset", "Shift+U"),
    ("Tangent arc", ","),
    ("Linear pattern", "Shift+Z"),
    ("Circular pattern", ";"),
    ("Sketch on hovered face", "4"),
    ("Settings", "Ctrl+,"),
];

pub(crate) fn tooltip_text(target: HoverTarget) -> Option<String> {
    let (name, key) = match target {
        HoverTarget::Tool(index) => TOOL_TIPS.get(index)?,
        HoverTarget::More(index) => MORE_TIPS.get(index)?,
    };
    Some(if *key == "-" {
        name.to_string()
    } else {
        format!("{name}  [{key}]")
    })
}

/// 16x16 glyphs, one string per row ('#' set). Drawn before toolbar labels.
pub(crate) fn tool_glyph(index: usize) -> [&'static str; 16] {
    match index {
        // Select arrow.
        0 => [
            "..#.............",
            "..##............",
            "..###...........",
            "..####..........",
            "..#####.........",
            "..######........",
            "..#######.......",
            "..########......",
            "..#########.....",
            "..#####.........",
            "..##.###........",
            "..#..###........",
            "......###.......",
            "......###.......",
            ".......##.......",
            "................",
        ],
        // Rectangle.
        1 => [
            "................",
            "................",
            "................",
            ".##############.",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            ".##############.",
            "................",
            "................",
            "................",
        ],
        // Circle.
        2 => [
            "................",
            ".....######.....",
            "....#......#....",
            "...#........#...",
            "..#..........#..",
            ".#............#.",
            ".#............#.",
            ".#.....##.....#.",
            ".#.....##.....#.",
            ".#............#.",
            ".#............#.",
            "..#..........#..",
            "...#........#...",
            "....#......#....",
            ".....######.....",
            "................",
        ],
        // Line with endpoints.
        3 => [
            "................",
            "............###.",
            "............###.",
            "...........####.",
            "..........#.....",
            ".........#......",
            "........#.......",
            ".......#........",
            "......#.........",
            ".....#..........",
            "....#...........",
            "...#............",
            ".###............",
            ".###............",
            ".###............",
            "................",
        ],
        // Ellipse.
        4 => [
            "................",
            "................",
            "................",
            ".....######.....",
            "...##......##...",
            "..#..........#..",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            "..#..........#..",
            "...##......##...",
            ".....######.....",
            "................",
            "................",
            "................",
        ],
        // Three planes (used by TOP/FRONT/RIGHT).
        5..=7 => [
            "................",
            ".....##########.",
            "....#.........#.",
            "...#.........##.",
            "..##########.#.#",
            "..#........#.#.#",
            "..#........#.#.#",
            "..#........#.#.#",
            "..#........#.#.#",
            "..#........#.#.#",
            "..#........#.#.#",
            "..#........#.##.",
            "..#........#.#..",
            "..#........##...",
            "..##########....",
            "................",
        ],
        // Arc.
        8 => [
            "................",
            "................",
            "................",
            "......####......",
            "....##....##....",
            "...#........#...",
            "..#..........#..",
            "..#..........#..",
            ".#............#.",
            ".#............#.",
            ".#............#.",
            "###..........###",
            "###..........###",
            "................",
            "................",
            "................",
        ],
        // More "...".
        _ => [
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
            ".##...##...##...",
            ".##...##...##...",
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
            "................",
        ],
    }
}

/// Paint a glyph with its top-left corner at (x, y), one logical px per cell.
pub(crate) fn paint_glyph(painter: &mut Painter, x: f32, y: f32, index: usize, color: Color) {
    for (row, bits) in tool_glyph(index).iter().enumerate() {
        // Merge horizontal runs into one rect each.
        let bytes = bits.as_bytes();
        let mut col = 0;
        while col < bytes.len() {
            if bytes[col] == b'#' {
                let start = col;
                while col < bytes.len() && bytes[col] == b'#' {
                    col += 1;
                }
                painter.rect(
                    Rect {
                        x: x + start as f32,
                        y: y + row as f32,
                        w: (col - start) as f32,
                        h: 1.0,
                    },
                    color,
                );
            } else {
                col += 1;
            }
        }
    }
}

impl Layout {
    /// Status-bar block left of the filter toggles holding `StatusInfo`.
    pub(crate) fn status_info(self) -> Rect {
        Rect {
            x: self.top.w - 420.0 - 236.0,
            y: self.bottom.y + 2.0,
            w: 230.0,
            h: self.bottom.h - 4.0,
        }
    }

    /// Toolbar button or MORE row at a point (MORE rows only when open).
    pub(crate) fn hover_target(self, point: Point, more_open: bool) -> Option<HoverTarget> {
        if more_open
            && let Some(index) =
                (0..MORE_TIPS.len()).find(|index| self.more_item(*index).contains(point.x, point.y))
        {
            return Some(HoverTarget::More(index));
        }
        (0..TOOL_TIPS.len())
            .find(|index| self.tool_button(*index).contains(point.x, point.y))
            .map(HoverTarget::Tool)
    }
}

impl App {
    pub(super) fn status_info(&self, layout: Layout) -> StatusInfo {
        let cursor = (!self.solid_preview
            && layout.viewport.contains(self.cursor.x, self.cursor.y))
        .then(|| {
            let world = self
                .document
                .screen_to_world(self.cursor, self.viewport_origin());
            (world.x, world.y)
        });
        let dof = (!self.solid_preview).then(|| self.sketch_analysis().label());
        StatusInfo {
            cursor,
            units: self.units(),
            tool: if self.solid_preview {
                "3D"
            } else {
                tool_name(self.document.tool)
            },
            dof,
            snap_geometry: self.snap_geometry,
            snap_grid: self.snap_grid,
            zoom_percent: (if self.solid_preview {
                self.solid_view.zoom
            } else {
                self.document.zoom
            } * 100.0)
                .round() as i32,
        }
    }

    pub(super) fn paint_status_info(&self, painter: &mut Painter, layout: Layout) {
        let info = self.status_info(layout);
        let mut cache = self.status_cache.borrow_mut();
        cache.update(info);
        let rect = layout.status_info();
        painter.rect(rect, PANEL);
        let [first, second] = cache.lines();
        let fit = |text: &str, max: f32| {
            max.min((rect.w - 8.0) / (text.chars().count().max(1) as f32 * 8.0))
        };
        painter.text(rect.x + 4.0, rect.y + 3.0, first, fit(first, 1.0), TEXT);
        painter.text(rect.x + 4.0, rect.y + 15.0, second, fit(second, 0.9), MUTED);
    }

    /// Track the hovered toolbar button; returns whether it changed.
    pub(super) fn update_hover(&mut self) -> bool {
        let target = if self.projects_page.is_some() {
            None
        } else {
            self.layout().hover_target(self.cursor, self.more_open)
        };
        if self.hover.map(|(current, _)| current) == target {
            return false;
        }
        self.hover = target.map(|target| (target, Instant::now()));
        true
    }

    /// When the pending tooltip becomes due (for the event loop timer).
    pub(super) fn tooltip_deadline(&self) -> Option<Instant> {
        self.hover
            .map(|(_, since)| since + TOOLTIP_DELAY)
            .filter(|deadline| *deadline > Instant::now())
    }

    pub(super) fn paint_tooltip(&self, painter: &mut Painter, layout: Layout) {
        let Some((target, since)) = self.hover else {
            return;
        };
        if since.elapsed() < TOOLTIP_DELAY {
            return;
        }
        let Some(text) = tooltip_text(target) else {
            return;
        };
        let anchor = match target {
            HoverTarget::Tool(index) => layout.tool_button(index),
            HoverTarget::More(index) => layout.more_item(index),
        };
        let w = text.chars().count() as f32 * 8.0 + 12.0;
        let (x, y) = match target {
            HoverTarget::Tool(_) => (anchor.x, anchor.y + anchor.h + 4.0),
            HoverTarget::More(_) => (anchor.x - w - 4.0, anchor.y + 6.0),
        };
        let x = x.clamp(0.0, (layout.top.w - w).max(0.0));
        let tip = Rect { x, y, w, h: 20.0 };
        painter.rect(tip, PANEL_LIGHT);
        painter.outline(tip, 1.0, ACCENT);
        painter.text(tip.x + 6.0, tip.y + 6.0, &text, 1.0, TEXT);
    }
}
