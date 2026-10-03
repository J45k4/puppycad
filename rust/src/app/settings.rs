//! User settings (`settings.json` under the XDG config dir), the color
//! palette for the dark and light themes, display units and the SETTINGS
//! panel.

use super::*;

/// Display unit for lengths; the model always stores millimeters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Units {
    #[default]
    Mm,
    Cm,
    In,
}

impl Units {
    pub(crate) const ALL: [Units; 3] = [Units::Mm, Units::Cm, Units::In];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Units::Mm => "mm",
            Units::Cm => "cm",
            Units::In => "in",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|unit| unit.name() == name)
    }

    /// Millimeters per display unit (shared with the expression parser).
    pub(crate) fn mm_per_unit(self) -> f32 {
        sketch_value::evaluate_dimension(
            &format!("1{}", self.name()),
            sketch_value::ValueKind::Length,
            &[],
        )
        .map_or(1.0, |mm| mm as f32)
    }

    pub(crate) fn mm_to_display(self, mm: f32) -> f32 {
        mm / self.mm_per_unit()
    }

    pub(crate) fn display_to_mm(self, value: f32) -> f32 {
        value * self.mm_per_unit()
    }

    pub(crate) fn decimals(self) -> usize {
        match self {
            Units::Mm => 1,
            Units::Cm => 2,
            Units::In => 3,
        }
    }

    /// `12.7 mm` / `1.27 cm` / `0.500 in`.
    pub(crate) fn format(self, mm: f32) -> String {
        format!(
            "{:.*} {}",
            self.decimals(),
            self.mm_to_display(mm),
            self.name()
        )
    }

    fn next(self) -> Self {
        let index = Self::ALL.iter().position(|unit| *unit == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    pub(crate) fn palette(self) -> &'static ThemePalette {
        match self {
            Theme::Dark => &DARK,
            Theme::Light => &LIGHT,
        }
    }
}

/// Every UI color, in both themes. The `const` colors used by the painting
/// code are the `DARK` fields; the painter substitutes the active palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ThemePalette {
    pub bg: Color,
    pub panel: Color,
    pub panel_light: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub shape: Color,
    pub fully_constrained: Color,
    pub conflict: Color,
    pub redundant: Color,
    pub dimension: Color,
    pub face_hover: Color,
    /// Active button / selected row fill.
    pub active: Color,
    pub active_row: Color,
    pub grid: Color,
    /// Select-mode hover pre-highlight.
    pub hover: Color,
    /// Inference hints while placing.
    pub hint: Color,
}

pub(crate) const DARK: ThemePalette = ThemePalette {
    bg: [0.055, 0.070, 0.096, 1.0],
    panel: [0.090, 0.111, 0.145, 1.0],
    panel_light: [0.124, 0.150, 0.190, 1.0],
    border: [0.20, 0.24, 0.29, 1.0],
    text: [0.88, 0.91, 0.94, 1.0],
    muted: [0.50, 0.58, 0.65, 1.0],
    accent: [0.32, 0.78, 0.93, 1.0],
    shape: [0.96, 0.72, 0.38, 1.0],
    fully_constrained: [0.46, 0.84, 0.52, 1.0],
    conflict: [0.96, 0.36, 0.36, 1.0],
    redundant: [0.78, 0.56, 0.96, 1.0],
    dimension: [0.62, 0.80, 0.98, 1.0],
    face_hover: [1.0, 0.86, 0.30, 1.0],
    active: [0.13, 0.31, 0.40, 1.0],
    active_row: [0.13, 0.29, 0.37, 1.0],
    grid: [0.13, 0.16, 0.20, 1.0],
    hover: [0.66, 0.92, 1.0, 1.0],
    hint: [0.98, 0.84, 0.42, 1.0],
};

pub(crate) const LIGHT: ThemePalette = ThemePalette {
    bg: [0.95, 0.96, 0.97, 1.0],
    panel: [0.89, 0.91, 0.93, 1.0],
    panel_light: [0.98, 0.98, 0.99, 1.0],
    border: [0.70, 0.74, 0.78, 1.0],
    text: [0.10, 0.12, 0.15, 1.0],
    muted: [0.38, 0.43, 0.49, 1.0],
    accent: [0.05, 0.47, 0.66, 1.0],
    shape: [0.78, 0.42, 0.05, 1.0],
    fully_constrained: [0.12, 0.55, 0.22, 1.0],
    conflict: [0.80, 0.12, 0.12, 1.0],
    redundant: [0.52, 0.25, 0.78, 1.0],
    dimension: [0.12, 0.38, 0.72, 1.0],
    face_hover: [0.85, 0.60, 0.0, 1.0],
    active: [0.74, 0.88, 0.95, 1.0],
    active_row: [0.80, 0.90, 0.96, 1.0],
    grid: [0.86, 0.88, 0.91, 1.0],
    hover: [0.02, 0.58, 0.82, 1.0],
    hint: [0.72, 0.46, 0.02, 1.0],
};

impl ThemePalette {
    fn colors(&self) -> [Color; 18] {
        [
            self.bg,
            self.panel,
            self.panel_light,
            self.border,
            self.text,
            self.muted,
            self.accent,
            self.shape,
            self.fully_constrained,
            self.conflict,
            self.redundant,
            self.dimension,
            self.face_hover,
            self.active,
            self.active_row,
            self.grid,
            self.hover,
            self.hint,
        ]
    }

    /// Painter substitutions turning `DARK` colors into this palette.
    pub(crate) fn remap(&self) -> Vec<(Color, Color)> {
        DARK.colors()
            .into_iter()
            .zip(self.colors())
            .filter(|(from, to)| from != to)
            .collect()
    }
}

/// UI scale choices; `None` in settings means "follow the monitor".
pub(crate) const UI_SCALES: [f32; 3] = [1.0, 1.5, 2.0];

/// Monitor scale factor rounded to 1.0 / 1.5 / 2.0.
pub(crate) fn round_ui_scale(factor: f64) -> f32 {
    if factor < 1.25 {
        1.0
    } else if factor < 1.75 {
        1.5
    } else {
        2.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WindowGeometry {
    /// Logical inner size.
    pub width: f32,
    pub height: f32,
    /// Physical outer position.
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Settings {
    pub units: Units,
    /// Grid snap spacing in millimeters.
    pub grid_mm: f32,
    pub theme: Theme,
    /// `None`: monitor scale factor rounded to 1.0/1.5/2.0.
    pub ui_scale: Option<f32>,
    pub window: Option<WindowGeometry>,
    /// Show the NEXT quick actions section in the Properties column.
    pub quick_actions: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            units: Units::Mm,
            grid_mm: 1.0,
            theme: Theme::Dark,
            ui_scale: None,
            window: None,
            quick_actions: true,
        }
    }
}

pub(crate) const GRID_STEPS: [f32; 6] = [0.5, 1.0, 2.0, 5.0, 10.0, 25.4];

impl Settings {
    /// `$PUPPYCAD_CONFIG_DIR`, else `$XDG_CONFIG_HOME/puppycad`, else
    /// `~/.config/puppycad`.
    pub(crate) fn dir() -> Option<PathBuf> {
        if let Some(dir) = std::env::var_os("PUPPYCAD_CONFIG_DIR").filter(|d| !d.is_empty()) {
            return Some(PathBuf::from(dir));
        }
        if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
            return Some(PathBuf::from(dir).join("puppycad"));
        }
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/puppycad"))
    }

    pub(crate) fn path() -> Option<PathBuf> {
        Self::dir().map(|dir| dir.join("settings.json"))
    }

    pub(crate) fn load() -> Self {
        Self::path().map_or_else(Self::default, |path| Self::load_from(&path))
    }

    /// Missing or unreadable files give the defaults.
    pub(crate) fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .map(|text| Self::from_json(&text))
            .unwrap_or_default()
    }

    pub(crate) fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path().ok_or("no config directory")?)
    }

    pub(crate) fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(path, self.to_json()).map_err(|error| error.to_string())
    }

    /// Unknown or invalid fields fall back to their defaults.
    pub(crate) fn from_json(text: &str) -> Self {
        let mut settings = Self::default();
        let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(text)
        else {
            return settings;
        };
        if let Some(units) = map
            .get("units")
            .and_then(|v| v.as_str())
            .and_then(Units::from_name)
        {
            settings.units = units;
        }
        if let Some(grid) = map.get("grid_mm").and_then(|v| v.as_f64())
            && grid.is_finite()
            && grid > 0.0
        {
            settings.grid_mm = grid as f32;
        }
        settings.theme = match map.get("theme").and_then(|v| v.as_str()) {
            Some("light") => Theme::Light,
            _ => Theme::Dark,
        };
        settings.ui_scale = map
            .get("ui_scale")
            .and_then(|v| v.as_f64())
            .map(round_ui_scale);
        if let Some(quick_actions) = map.get("quick_actions").and_then(|v| v.as_bool()) {
            settings.quick_actions = quick_actions;
        }
        if let Some(window) = map.get("window").and_then(|v| v.as_object()) {
            let number = |key: &str| window.get(key).and_then(|v| v.as_f64());
            if let (Some(width), Some(height), Some(x), Some(y)) =
                (number("width"), number("height"), number("x"), number("y"))
                && width >= 200.0
                && height >= 200.0
            {
                settings.window = Some(WindowGeometry {
                    width: width as f32,
                    height: height as f32,
                    x: x as i32,
                    y: y as i32,
                });
            }
        }
        settings
    }

    pub(crate) fn to_json(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("units".into(), self.units.name().into());
        map.insert("grid_mm".into(), serde_json::json!(self.grid_mm));
        map.insert("theme".into(), self.theme.name().into());
        map.insert(
            "ui_scale".into(),
            self.ui_scale
                .map_or(serde_json::Value::Null, |scale| serde_json::json!(scale)),
        );
        map.insert("quick_actions".into(), self.quick_actions.into());
        if let Some(window) = self.window {
            map.insert(
                "window".into(),
                serde_json::json!({
                    "width": window.width,
                    "height": window.height,
                    "x": window.x,
                    "y": window.y,
                }),
            );
        }
        let mut text =
            serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap_or_default();
        text.push('\n');
        text
    }
}

/// Rows of the SETTINGS panel.
pub(crate) const SETTINGS_ROWS: [&str; 5] = ["UNITS", "GRID", "THEME", "UI SCALE", "CLOSE"];

impl Layout {
    /// The SETTINGS panel, centered in the viewport.
    pub(crate) fn settings_panel(self) -> Rect {
        let w = 340.0;
        let h = 60.0 + SETTINGS_ROWS.len() as f32 * 36.0;
        Rect {
            x: self.viewport.x + (self.viewport.w - w) * 0.5,
            y: self.viewport.y + 40.0,
            w,
            h,
        }
    }

    pub(crate) fn settings_row(self, index: usize) -> Rect {
        let panel = self.settings_panel();
        Rect {
            x: panel.x + 12.0,
            y: panel.y + 44.0 + index as f32 * 36.0,
            w: panel.w - 24.0,
            h: 32.0,
        }
    }
}

impl App {
    pub(super) fn units(&self) -> Units {
        self.settings.units
    }

    /// A length in the display unit with its suffix.
    pub(super) fn len(&self, mm: f32) -> String {
        self.units().format(mm)
    }

    /// Viewport dimension label: plain lengths are shown in display units.
    pub(super) fn display_label(&self, key: DimensionKey, text: &str) -> String {
        if self.units() == Units::Mm || Document::dimension_is_angle(key) {
            return text.to_string();
        }
        let split = text
            .find(|c: char| c.is_ascii_digit() || c == '-' || c == '.')
            .unwrap_or(text.len());
        let (prefix, number) = text.split_at(split);
        match number.parse::<f32>() {
            Ok(mm) => format!(
                "{prefix}{:.*}{}",
                self.units().decimals(),
                self.units().mm_to_display(mm),
                self.units().name()
            ),
            Err(_) => text.to_string(),
        }
    }

    /// Converts a typed value to millimeters before it reaches the model:
    /// plain numbers for length fields are in display units; expressions
    /// (with their own units) are kept for dimension fields.
    pub(super) fn entry_to_mm(&self, field: EditField, buffer: &str) -> String {
        let units = self.units();
        if units == Units::Mm {
            return buffer.to_string();
        }
        let length_field = matches!(
            field,
            EditField::ExtrusionDepth
                | EditField::SlotWidth
                | EditField::MoveX
                | EditField::MoveY
                | EditField::CenterX
                | EditField::CenterY
                | EditField::SplineX
                | EditField::SplineY
                | EditField::PointX
                | EditField::PointY
                | EditField::RadialDifference
                | EditField::FilletRadius
                | EditField::ChamferFirst
                | EditField::OffsetCreate
                | EditField::OffsetDistance
        );
        let dimension_key = match field {
            EditField::Dimension(field) => self
                .document
                .selected
                .map(|id| DimensionKey::Field(id, field)),
            EditField::Label(key) => Some(key),
            _ => None,
        };
        let plain = buffer.trim().parse::<f32>().ok();
        if let Some(key) = dimension_key {
            if !Document::dimension_is_angle(key) && plain.is_some() {
                // The expression parser applies the unit.
                return format!("{}{}", buffer.trim(), units.name());
            }
            return buffer.to_string();
        }
        match plain {
            Some(value) if length_field => format!("{}", units.display_to_mm(value)),
            _ => buffer.to_string(),
        }
    }

    pub(super) fn apply_settings(&mut self) {
        self.document.grid_step = self.settings.grid_mm;
    }

    pub(super) fn save_settings(&mut self) {
        if let Err(error) = self.settings.save() {
            self.status_message = format!("Settings not saved: {error}");
        }
    }

    pub(super) fn open_settings(&mut self) {
        self.more_open = false;
        self.settings_open = !self.settings_open;
        self.status_message = if self.settings_open {
            "Settings: click a row to change it  |  Esc or Ctrl+, to close".into()
        } else {
            String::new()
        };
    }

    /// Cycle one setting row; saved and applied immediately.
    pub(super) fn cycle_setting(&mut self, index: usize) {
        match index {
            0 => self.settings.units = self.settings.units.next(),
            1 => {
                let current = GRID_STEPS
                    .iter()
                    .position(|step| (*step - self.settings.grid_mm).abs() < 1e-4);
                self.settings.grid_mm =
                    GRID_STEPS[current.map_or(1, |index| (index + 1) % GRID_STEPS.len())];
            }
            2 => {
                self.settings.theme = match self.settings.theme {
                    Theme::Dark => Theme::Light,
                    Theme::Light => Theme::Dark,
                }
            }
            3 => {
                self.settings.ui_scale = match self.settings.ui_scale {
                    None => Some(UI_SCALES[0]),
                    Some(scale) => UI_SCALES
                        .iter()
                        .position(|s| *s == scale)
                        .and_then(|index| UI_SCALES.get(index + 1).copied()),
                };
                self.ui_scale = self.settings.ui_scale.unwrap_or(self.monitor_scale);
            }
            _ => {
                self.settings_open = false;
                self.status_message = String::new();
                return;
            }
        }
        self.apply_settings();
        self.status_message = format!("Settings: {}", self.setting_value(index));
        self.save_settings();
    }

    pub(super) fn setting_value(&self, index: usize) -> String {
        match index {
            0 => format!("UNITS {}", self.settings.units.name().to_uppercase()),
            1 => format!("GRID {}", self.len(self.settings.grid_mm).to_uppercase()),
            2 => format!("THEME {}", self.settings.theme.name().to_uppercase()),
            3 => match self.settings.ui_scale {
                None => format!("UI SCALE AUTO ({:.1}X)", self.monitor_scale),
                Some(scale) => format!("UI SCALE {scale:.1}X"),
            },
            _ => "CLOSE [ESC]".into(),
        }
    }

    /// Click inside the open SETTINGS panel; returns whether it was handled.
    pub(super) fn settings_click(&mut self, layout: Layout, point: Point) -> bool {
        if !self.settings_open {
            return false;
        }
        if let Some(index) = (0..SETTINGS_ROWS.len())
            .find(|index| layout.settings_row(*index).contains(point.x, point.y))
        {
            self.cycle_setting(index);
            return true;
        }
        if layout.settings_panel().contains(point.x, point.y) {
            return true;
        }
        self.settings_open = false;
        false
    }

    /// Keyboard: Esc closes the panel (Ctrl+, is the registry's Settings).
    pub(super) fn settings_key(&mut self, code: KeyCode) -> bool {
        if self.settings_open && code == KeyCode::Escape {
            self.settings_open = false;
            self.status_message = String::new();
            return true;
        }
        false
    }

    pub(super) fn paint_settings(&self, painter: &mut Painter, layout: Layout) {
        if !self.settings_open {
            return;
        }
        let panel = layout.settings_panel();
        painter.rect(panel, PANEL);
        painter.outline(panel, 1.0, ACCENT);
        painter.text(panel.x + 14.0, panel.y + 14.0, "SETTINGS", 1.6, ACCENT);
        painter.text(panel.x + 200.0, panel.y + 18.0, "CTRL+,", 1.0, MUTED);
        for index in 0..SETTINGS_ROWS.len() {
            let row = layout.settings_row(index);
            painter.rect(row, PANEL_LIGHT);
            painter.outline(row, 1.0, BORDER);
            painter.text(
                row.x + 10.0,
                row.y + 11.0,
                &self.setting_value(index),
                1.2,
                TEXT,
            );
        }
    }

    /// Settings applied once the window exists (scale, saved geometry).
    pub(super) fn window_attributes(&self) -> winit::window::WindowAttributes {
        let mut attributes = Window::default_attributes().with_title("PuppyCAD");
        attributes = match self.settings.window {
            Some(window) => attributes
                .with_inner_size(LogicalSize::new(window.width, window.height))
                .with_position(winit::dpi::PhysicalPosition::new(window.x, window.y)),
            None => attributes.with_inner_size(LogicalSize::new(1280.0, 800.0)),
        };
        attributes
    }

    pub(super) fn init_window_scale(&mut self, window: &Window) {
        self.monitor_scale = round_ui_scale(window.scale_factor());
        self.ui_scale = self.settings.ui_scale.unwrap_or(self.monitor_scale);
    }

    /// Remember the window geometry for the next start.
    pub(super) fn save_window_geometry(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let size = window.inner_size().to_logical::<f32>(window.scale_factor());
        let position = window.outer_position().unwrap_or_default();
        self.settings.window = Some(WindowGeometry {
            width: size.width,
            height: size.height,
            x: position.x,
            y: position.y,
        });
        let _ = self.settings.save();
    }
}
