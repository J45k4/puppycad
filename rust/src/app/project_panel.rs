use super::*;

pub(super) const CRUMB_X: f32 = 172.0;
pub(super) const CRUMB_Y: f32 = 17.0;
pub(super) const CRUMB_SCALE: f32 = 1.4;
pub(super) const CRUMB_SEPARATOR: &str = " / ";
pub(super) const CRUMB_MAX_CHARS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Crumb {
    Projects,
    Project,
    Part,
    Sketch,
    Solid,
    Shape,
}

/// Lays out breadcrumb labels left to right, truncating long names and
/// dropping segments that would run past `max_x`.
pub(super) fn layout_breadcrumbs(
    segments: Vec<(Crumb, String)>,
    max_x: f32,
) -> Vec<(Crumb, String, Rect)> {
    let char_width = 8.0 * CRUMB_SCALE;
    let separator_width = CRUMB_SEPARATOR.chars().count() as f32 * char_width;
    let mut x = CRUMB_X;
    let mut crumbs = Vec::new();
    for (crumb, label) in segments {
        let label = if label.chars().count() > CRUMB_MAX_CHARS {
            format!(
                "{}...",
                label.chars().take(CRUMB_MAX_CHARS - 3).collect::<String>()
            )
        } else {
            label
        };
        let w = label.chars().count() as f32 * char_width;
        if x + w > max_x {
            break;
        }
        crumbs.push((
            crumb,
            label,
            Rect {
                x,
                y: CRUMB_Y - 4.0,
                w,
                h: 8.0 * CRUMB_SCALE + 8.0,
            },
        ));
        x += w + separator_width;
    }
    crumbs
}

pub(super) const PROJECT_ROW_TOP: f32 = 176.0;
pub(super) const PROJECT_ROW_STEP: f32 = 58.0;
pub(super) const PROJECT_ROW_HEIGHT: f32 = 52.0;
pub(super) const PROJECT_COLUMN_MAX: f32 = 960.0;

/// Left edge and width of the centered projects page column.
pub(super) fn project_page_column(layout: Layout) -> (f32, f32) {
    let w = (layout.top.w - 40.0).clamp(1.0, PROJECT_COLUMN_MAX);
    ((layout.top.w - w) * 0.5, w)
}

/// Row `index` on screen of the projects page (after scrolling).
pub(super) fn project_page_row(layout: Layout, index: usize) -> Rect {
    let (x, w) = project_page_column(layout);
    Rect {
        x,
        y: PROJECT_ROW_TOP + index as f32 * PROJECT_ROW_STEP,
        w,
        h: PROJECT_ROW_HEIGHT,
    }
}

pub(super) fn project_page_visible_rows(layout: Layout) -> usize {
    ((layout.bottom.y - 12.0 - PROJECT_ROW_TOP + PROJECT_ROW_STEP - PROJECT_ROW_HEIGHT)
        / PROJECT_ROW_STEP)
        .floor()
        .max(1.0) as usize
}

/// Shortens `text` to `max` characters, keeping the end (for paths) or the start.
/// Project panel rows start below the PROJECT heading; rows scrolled
/// above this are hidden and not clickable.
pub(super) const PANEL_ROWS_TOP: f32 = 40.0;
/// Pixels one wheel notch scrolls the Project panel (one row).
pub(super) const PANEL_ROW_STEP: f32 = 35.0;

pub(super) fn clip_text(text: &str, max: usize, keep_end: bool) -> String {
    let count = text.chars().count();
    if count <= max || max < 4 {
        return text.to_string();
    }
    if keep_end {
        format!(
            "...{}",
            text.chars().skip(count + 3 - max).collect::<String>()
        )
    } else {
        format!("{}...", text.chars().take(max - 3).collect::<String>())
    }
}

pub(super) struct ProjectsPage {
    pub(super) dir: PathBuf,
    pub(super) entries: Vec<ProjectSummary>,
    pub(super) scroll: usize,
    pub(super) selected: usize,
    /// Project the user picked once while the open project had unsaved changes.
    pub(super) pending_discard: Option<PathBuf>,
}

impl App {
    /// Project panel rows below the parts: sketches, features + ROLLBACK, shapes.
    fn panel_row_total(&self) -> usize {
        self.part_rows().len()
            + self.sketch_count().max(1)
            + self.tree_row_count()
            + self.document.shapes.len()
    }

    /// How far the Project panel can scroll before its last row is at the bottom.
    pub(super) fn max_panel_scroll(&self, layout: Layout) -> f32 {
        let content = 43.0 + self.panel_row_total() as f32 * PANEL_ROW_STEP;
        (content - (layout.bottom.y - layout.left.y) + 8.0).max(0.0)
    }

    /// Wheel over the Project panel: `rows` positive scrolls down.
    pub(super) fn scroll_panel(&mut self, rows: f32) {
        let max = self.max_panel_scroll(self.layout());
        self.panel_scroll = (self.layout().panel_scroll + rows * PANEL_ROW_STEP).clamp(0.0, max);
    }

    /// True when a Project panel row at this rect is drawn (not scrolled
    /// under the heading or past the status bar).
    pub(super) fn panel_row_shown(layout: Layout, row: Rect) -> bool {
        row.y >= layout.left.y + PANEL_ROWS_TOP && row.y + row.h < layout.bottom.y
    }

    /// After a key press: when the selected feature (3D view) or the active
    /// sketch changed, scroll its row into view.
    pub(super) fn follow_panel_selection(&mut self) {
        let active_sketch = self.project.as_ref().map_or(0, ProjectFile::active_sketch);
        let key = (self.solid_preview, self.selected_solid, active_sketch);
        if key == self.panel_followed {
            return;
        }
        self.panel_followed = key;
        let layout = self.layout();
        let row = if self.solid_preview && self.tree_row_count() > 0 {
            self.feature_row(layout, self.selected_solid)
        } else {
            layout.sketch_row(active_sketch, self.part_rows().len())
        };
        let top = layout.left.y + PANEL_ROWS_TOP;
        let bottom = layout.bottom.y - 4.0;
        let max = self.max_panel_scroll(layout);
        if row.y < top {
            self.panel_scroll = (layout.panel_scroll - (top - row.y)).clamp(0.0, max);
        } else if row.y + row.h > bottom {
            self.panel_scroll = (layout.panel_scroll + (row.y + row.h - bottom)).clamp(0.0, max);
        }
    }

    pub(super) fn breadcrumbs(&self, layout: Layout) -> Vec<(Crumb, String, Rect)> {
        let mut segments = vec![(Crumb::Projects, "Projects".to_string())];
        if self.projects_page.is_some() {
            return layout_breadcrumbs(segments, layout.top.w - 20.0);
        }
        let project_name = self
            .project
            .as_ref()
            .and_then(|project| project.path().file_stem())
            .and_then(|name| name.to_str())
            .unwrap_or("UNTITLED");
        segments.push((Crumb::Project, project_name.to_string()));
        if let Some(project) = &self.project {
            segments.push((Crumb::Part, project.part_name().to_string()));
        }
        if self.solid_preview {
            // The selected tree feature (extrusion, revolve, loft or finish).
            let name = self.project.as_ref().and_then(|project| {
                project
                    .feature_node(self.selected_solid)
                    .and_then(|node| node["name"].as_str().map(str::to_string))
                    .or_else(|| {
                        project
                            .extrusion_previews()?
                            .into_iter()
                            .nth(self.selected_solid)
                            .map(|solid| solid.name)
                    })
            });
            if let Some(name) = name {
                segments.push((Crumb::Solid, name));
            }
        } else {
            let sketch_name = self
                .project
                .as_ref()
                .and_then(|project| project.sketch_name(project.active_sketch()))
                .unwrap_or("Sketch");
            segments.push((Crumb::Sketch, sketch_name.to_string()));
            if let Some(shape) = self
                .document
                .selected
                .and_then(|id| self.document.shapes.iter().find(|shape| shape.id() == id))
            {
                segments.push((Crumb::Shape, format!("{} {}", shape.name(), shape.id())));
            }
        }
        layout_breadcrumbs(
            segments,
            if self.solid_preview {
                layout.top.w - 270.0
            } else {
                layout.top.w - 20.0
            },
        )
    }

    pub(super) fn open_projects_page(&mut self) {
        self.document.finish_drag();
        self.editing = None;
        self.name_edit = None;
        self.more_open = false;
        self.solid_drag = None;
        self.split_circle_start = None;
        let open_path = self
            .project
            .as_ref()
            .map(|project| project.path().to_path_buf());
        let dir = match open_path.as_deref().and_then(Path::parent) {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        self.show_folder(&dir, open_path.as_deref());
    }

    /// Lists `dir` on the projects page, selecting `focus` when it is listed there.
    pub(super) fn show_folder(&mut self, dir: &Path, focus: Option<&Path>) {
        let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let entries = match list_projects(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                self.status_message = format!("Projects: {error}");
                return;
            }
        };
        let focus = focus.and_then(|focus| focus.canonicalize().ok());
        let selected = entries
            .iter()
            .position(|entry| {
                focus.is_some()
                    && entry.kind != EntryKind::Parent
                    && entry.path.canonicalize().ok() == focus
            })
            .or_else(|| {
                entries
                    .iter()
                    .position(|entry| entry.kind == EntryKind::Project)
            })
            .unwrap_or(0);
        self.status_message =
            "Click or Enter: open  |  Up/Down: select  |  Backspace: parent folder  |  Esc: back"
                .into();
        self.projects_page = Some(ProjectsPage {
            dir,
            entries,
            scroll: 0,
            selected,
            pending_discard: None,
        });
        if let Some(window) = &self.window {
            let size = window.inner_size();
            self.reveal_selected_project(Layout::new(size.width as f32, size.height as f32));
        }
    }

    pub(super) fn show_parent_folder(&mut self) {
        let Some(page) = &self.projects_page else {
            return;
        };
        if let Some(parent) = page.dir.parent().map(Path::to_path_buf) {
            let from = page.dir.clone();
            self.show_folder(&parent, Some(&from));
        }
    }

    pub(super) fn close_projects_page(&mut self) {
        if self.projects_page.take().is_some() {
            self.status_message = match &self.project {
                Some(project) => format!("Project: {}", project.path().display()),
                None => String::new(),
            };
        }
    }

    pub(super) fn has_unsaved_work(&self) -> bool {
        match &self.project {
            Some(project) => project.has_unsaved_changes(&self.document),
            None => !self.document.shapes.is_empty(),
        }
    }

    pub(super) fn is_open_project(&self, path: &Path) -> bool {
        self.project.as_ref().is_some_and(|project| {
            project.path() == path
                || project
                    .path()
                    .canonicalize()
                    .ok()
                    .zip(path.canonicalize().ok())
                    .is_some_and(|(open, listed)| open == listed)
        })
    }

    pub(super) fn open_listed_project(&mut self, index: usize) {
        let Some(entry) = self
            .projects_page
            .as_ref()
            .and_then(|page| page.entries.get(index))
            .cloned()
        else {
            return;
        };
        if let Some(page) = &mut self.projects_page {
            page.selected = index;
        }
        match entry.kind {
            EntryKind::Parent => {
                self.show_parent_folder();
                return;
            }
            EntryKind::Folder => {
                self.show_folder(&entry.path, None);
                return;
            }
            EntryKind::Project => {}
        }
        if let Some(error) = &entry.error {
            self.status_message = format!("Cannot open {}: {error}", entry.name);
            return;
        }
        if self.is_open_project(&entry.path) {
            self.close_projects_page();
            return;
        }
        let confirmed = self
            .projects_page
            .as_ref()
            .and_then(|page| page.pending_discard.as_deref())
            == Some(entry.path.as_path());
        if self.has_unsaved_work() && !confirmed {
            if let Some(page) = &mut self.projects_page {
                page.pending_discard = Some(entry.path.clone());
            }
            self.status_message = format!(
                "Unsaved changes  |  Ctrl+S to save, or open {} again to discard them",
                entry.name
            );
            return;
        }
        self.load_project(&entry.path);
    }

    pub(super) fn select_project_row(&mut self, step: isize, layout: Layout) {
        if let Some(page) = &mut self.projects_page {
            if page.entries.is_empty() {
                return;
            }
            page.selected = page
                .selected
                .saturating_add_signed(step)
                .min(page.entries.len() - 1);
        }
        self.reveal_selected_project(layout);
    }

    pub(super) fn reveal_selected_project(&mut self, layout: Layout) {
        if let Some(page) = &mut self.projects_page {
            let visible = project_page_visible_rows(layout);
            if page.selected < page.scroll {
                page.scroll = page.selected;
            } else if page.selected >= page.scroll + visible {
                page.scroll = page.selected + 1 - visible;
            }
        }
    }

    /// Replaces the open project with `path`, keeping the window and input state.
    pub(super) fn load_project(&mut self, path: &Path) {
        match App::with_project(path) {
            Ok(mut next) => {
                next.window = self.window.take();
                next.renderer = self.renderer.take();
                next.cursor = self.cursor;
                next.modifiers = self.modifiers;
                *self = next;
            }
            Err(error) => self.status_message = format!("Open failed: {error}"),
        }
    }

    pub(super) fn save_project(&mut self) {
        self.status_message = match &mut self.project {
            Some(project) => match project.save(&self.document) {
                Ok(()) => format!("Saved {}", project.path().display()),
                Err(error) => format!("Save failed: {error}"),
            },
            None => "Start with: puppycad FILE.pcad to save".into(),
        };
        if let Some(page) = &mut self.projects_page {
            page.pending_discard = None;
        }
    }

    pub(super) fn scroll_projects_page(&mut self, rows: isize, layout: Layout) {
        if let Some(page) = &mut self.projects_page {
            let max = page
                .entries
                .len()
                .saturating_sub(project_page_visible_rows(layout));
            page.scroll = page.scroll.saturating_add_signed(rows).min(max);
        }
    }

    pub(super) fn projects_page_entry_at(&self, layout: Layout, point: Point) -> Option<usize> {
        let page = self.projects_page.as_ref()?;
        (0..project_page_visible_rows(layout))
            .find(|row| project_page_row(layout, *row).contains(point.x, point.y))
            .map(|row| row + page.scroll)
            .filter(|index| *index < page.entries.len())
    }

    /// Returns the ancestor breadcrumb under `point`; the last (current) crumb is not a link.
    pub(super) fn breadcrumb_at(&self, layout: Layout, point: Point) -> Option<Crumb> {
        let crumbs = self.breadcrumbs(layout);
        let last = crumbs.len().saturating_sub(1);
        crumbs
            .into_iter()
            .take(last)
            .find(|(_, _, rect)| rect.contains(point.x, point.y))
            .map(|(crumb, _, _)| crumb)
    }

    pub(super) fn open_breadcrumb(&mut self, crumb: Crumb) {
        if crumb == Crumb::Projects {
            self.open_projects_page();
            return;
        }
        self.document.finish_drag();
        self.editing = None;
        self.split_circle_start = None;
        self.document.selected = None;
        self.document.selected_spline_point = None;
        self.sync_sketch_to_project();
        let has_solids = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .is_some();
        self.status_message = match (crumb, &self.project) {
            (Crumb::Project, Some(project)) => {
                format!("Project: {}", project.path().display())
            }
            (Crumb::Project, None) => "Untitled sketch  |  open a .pcad project to save".into(),
            (Crumb::Part, Some(project)) => {
                self.solid_preview = has_solids;
                if has_solids {
                    format!("Part: {}  |  Tab: sketch", project.part_name())
                } else {
                    format!("Part: {}", project.part_name())
                }
            }
            (Crumb::Sketch, _) => {
                self.solid_preview = false;
                format!(
                    "Editing {}",
                    self.project
                        .as_ref()
                        .and_then(|project| project.sketch_name(project.active_sketch()))
                        .unwrap_or("Sketch")
                )
            }
            _ => self.status_message.clone(),
        };
    }

    pub(super) fn paint_breadcrumbs(&self, painter: &mut Painter, layout: Layout) {
        let crumbs = self.breadcrumbs(layout);
        let last = crumbs.len().saturating_sub(1);
        for (index, (_, label, rect)) in crumbs.iter().enumerate() {
            let hovered = index < last && rect.contains(self.cursor.x, self.cursor.y);
            let color = if hovered {
                ACCENT
            } else if index == last {
                TEXT
            } else {
                MUTED
            };
            painter.text(rect.x, CRUMB_Y, label, CRUMB_SCALE, color);
            if hovered {
                painter.line(
                    rect.x,
                    CRUMB_Y + 8.0 * CRUMB_SCALE + 2.0,
                    rect.x + rect.w,
                    CRUMB_Y + 8.0 * CRUMB_SCALE + 2.0,
                    1.0,
                    ACCENT,
                );
            }
            if index < last {
                painter.text(
                    rect.x + rect.w,
                    CRUMB_Y,
                    CRUMB_SEPARATOR,
                    CRUMB_SCALE,
                    MUTED,
                );
            }
        }
    }

    pub(super) fn paint_projects_page(
        &self,
        painter: &mut Painter,
        layout: Layout,
        page: &ProjectsPage,
    ) {
        painter.rect(layout.top, PANEL);
        painter.rect(layout.bottom, PANEL);
        painter.line(0.0, 94.0, layout.top.w, 94.0, 1.0, BORDER);
        painter.line(
            0.0,
            layout.bottom.y,
            layout.top.w,
            layout.bottom.y,
            1.0,
            BORDER,
        );
        painter.text(20.0, 13.0, "PUPPYCAD", 2.0, TEXT);
        self.paint_breadcrumbs(painter, layout);

        let (column_x, column_w) = project_page_column(layout);
        painter.text(column_x, 114.0, "PROJECTS", 2.0, TEXT);
        let project_count = page
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Project)
            .count();
        let count = format!(
            "{project_count} PROJECT{}",
            if project_count == 1 { "" } else { "S" }
        );
        painter.text(
            column_x + column_w - count.chars().count() as f32 * 8.8,
            118.0,
            &count,
            1.1,
            MUTED,
        );
        painter.text(
            column_x,
            144.0,
            &clip_text(
                &page.dir.display().to_string(),
                (column_w / 8.8) as usize,
                true,
            ),
            1.1,
            MUTED,
        );

        let visible = project_page_visible_rows(layout);
        let shown = page.entries.len().saturating_sub(page.scroll).min(visible);
        for (index, entry) in page
            .entries
            .iter()
            .enumerate()
            .skip(page.scroll)
            .take(visible)
        {
            let rect = project_page_row(layout, index - page.scroll);
            let open = entry.kind == EntryKind::Project && self.is_open_project(&entry.path);
            let pending = page.pending_discard.as_deref() == Some(entry.path.as_path());
            let hovered = rect.contains(self.cursor.x, self.cursor.y);
            let selected = index == page.selected;
            painter.rect(
                rect,
                if selected {
                    [0.13, 0.29, 0.37, 1.0]
                } else if hovered {
                    PANEL_LIGHT
                } else {
                    PANEL
                },
            );
            painter.outline(
                rect,
                1.0,
                if pending {
                    SHAPE
                } else if selected || hovered {
                    ACCENT
                } else {
                    BORDER
                },
            );
            let icon_x = rect.x + 18.0;
            let icon_y = rect.y + 17.0;
            if entry.kind == EntryKind::Project {
                painter.outline(
                    Rect {
                        x: icon_x,
                        y: icon_y,
                        w: 18.0,
                        h: 18.0,
                    },
                    1.5,
                    ACCENT,
                );
                painter.rect(
                    Rect {
                        x: icon_x + 5.0,
                        y: icon_y + 5.0,
                        w: 8.0,
                        h: 8.0,
                    },
                    ACCENT,
                );
            } else {
                painter.rect(
                    Rect {
                        x: icon_x,
                        y: icon_y,
                        w: 9.0,
                        h: 4.0,
                    },
                    MUTED,
                );
                painter.outline(
                    Rect {
                        x: icon_x,
                        y: icon_y + 3.0,
                        w: 20.0,
                        h: 15.0,
                    },
                    1.5,
                    MUTED,
                );
            }
            let tag = if pending {
                "OPEN AGAIN TO DISCARD CHANGES"
            } else if open {
                "OPEN"
            } else {
                ""
            };
            let tag_w = tag.chars().count() as f32 * 9.6;
            let text_x = rect.x + 54.0;
            let name_chars = ((rect.x + rect.w - text_x - tag_w - 28.0) / 12.0).max(4.0) as usize;
            let (name, detail) = match entry.kind {
                EntryKind::Parent => (
                    "..".to_string(),
                    format!(
                        "UP TO {}",
                        entry
                            .path
                            .file_name()
                            .map_or_else(|| "/".into(), |name| name.to_string_lossy())
                    ),
                ),
                EntryKind::Folder => (entry.name.clone(), "FOLDER".to_string()),
                EntryKind::Project => (
                    entry.name.clone(),
                    match &entry.error {
                        Some(error) => format!("CANNOT READ: {error}"),
                        None => format!(
                            "{} PART{}  /  REVISION {}",
                            entry.parts,
                            if entry.parts == 1 { "" } else { "S" },
                            entry.revision
                        ),
                    },
                ),
            };
            painter.text(
                text_x,
                rect.y + 11.0,
                &clip_text(&name, name_chars, false),
                1.5,
                TEXT,
            );
            painter.text(
                text_x,
                rect.y + 33.0,
                &clip_text(&detail, name_chars * 3 / 2, false),
                1.0,
                if entry.error.is_some() { SHAPE } else { MUTED },
            );
            painter.text(
                rect.x + rect.w - 16.0 - tag_w,
                rect.y + 20.0,
                tag,
                1.2,
                if pending { SHAPE } else { ACCENT },
            );
        }
        if project_count == 0 {
            painter.text(
                column_x,
                project_page_row(layout, shown).y + 12.0,
                "No .pcad projects in this folder. Open a folder above to look elsewhere.",
                1.1,
                MUTED,
            );
        }
        if page.entries.len() > visible {
            let range = format!(
                "{}-{} OF {}",
                page.scroll + 1,
                (page.scroll + visible).min(page.entries.len()),
                page.entries.len()
            );
            painter.text(
                column_x + column_w - range.chars().count() as f32 * 8.8,
                144.0,
                &range,
                1.1,
                MUTED,
            );
        }
        painter.text(
            15.0,
            layout.bottom.y + 10.0,
            &self.status_message,
            1.2,
            MUTED,
        );
    }
}
