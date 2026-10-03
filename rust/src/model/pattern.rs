//! Linked linear (grid) and circular sketch patterns, ported from
//! `src/sketch-pattern.ts` and `src/sketch-circular-pattern.ts` (web
//! `linearPattern` / `circularPattern` relations).
//!
//! Instances are regenerated from their sources after every solve (the same
//! fallback the Mirror tool uses for rectangles, polygons and slots), so
//! editing a source updates every instance while instances themselves follow
//! the pattern.

use super::*;

/// Most shapes a pattern may contain, including the source cell (web limit).
pub const MAX_PATTERN_CELLS: usize = 32;
/// Most copies a pattern may create (web solver limit).
pub const MAX_PATTERN_COPIES: usize = 64;

#[derive(Clone, Debug, PartialEq)]
pub enum PatternKind {
    /// `columns` cells along `step`, rows along `row_step`.
    Linear {
        step: Point,
        columns: usize,
        row_step: Point,
    },
    /// Copies rotated about `center` (or the anchor `center_anchor`) over
    /// `angle` degrees.
    Circular {
        center: Point,
        center_anchor: Option<(u32, usize)>,
        angle: f32,
    },
}

/// A linked pattern: `instances[i][j]` is the copy of `sources[j]` in cell
/// `i + 1` (cell 0 is the source).
#[derive(Clone, Debug, PartialEq)]
pub struct SketchPattern {
    pub sources: Vec<u32>,
    pub instances: Vec<Vec<u32>>,
    pub kind: PatternKind,
}

impl SketchPattern {
    /// Total cells including the source.
    pub fn count(&self) -> usize {
        self.instances.len() + 1
    }

    /// Rows of a linear pattern (1 for circular ones).
    pub fn rows(&self) -> usize {
        match self.kind {
            PatternKind::Linear { columns, .. } => self.count() / columns.max(1),
            PatternKind::Circular { .. } => 1,
        }
    }

    /// Translation of linear cell `index` (web `patternCellOffset`).
    pub fn cell_offset(&self, index: usize) -> Point {
        match self.kind {
            PatternKind::Linear {
                step,
                columns,
                row_step,
            } => {
                let columns = columns.max(1);
                let (col, row) = ((index % columns) as f32, (index / columns) as f32);
                Point {
                    x: step.x * col + row_step.x * row,
                    y: step.y * col + row_step.y * row,
                }
            }
            PatternKind::Circular { .. } => Point { x: 0.0, y: 0.0 },
        }
    }

    /// Rotation of circular cell `index` in degrees (web
    /// `circularPatternAngle`): full circles exclude the duplicate end cell.
    pub fn cell_angle(&self, index: usize) -> f32 {
        match self.kind {
            PatternKind::Circular { angle, .. } => {
                let count = self.count() as f32;
                let divisor = if (angle.abs() - 360.0).abs() < 1e-6 {
                    count
                } else {
                    count - 1.0
                };
                angle * index as f32 / divisor
            }
            PatternKind::Linear { .. } => 0.0,
        }
    }

    pub fn contains(&self, id: u32) -> bool {
        self.sources.contains(&id) || self.instances.iter().flatten().any(|&i| i == id)
    }
}

fn check_point(p: Point, message: &str) -> Result<(), String> {
    if p.x.is_finite() && p.y.is_finite() && (p.x as f64).hypot(p.y as f64) > 1e-9 {
        Ok(())
    } else {
        Err(message.into())
    }
}

impl Document {
    fn pattern_shape(&self, id: u32) -> Option<&Shape> {
        self.shapes.iter().find(|shape| shape.id() == id)
    }

    /// Index of the pattern that `id` is a source or instance of.
    pub fn pattern_of(&self, id: u32) -> Option<usize> {
        self.patterns.iter().position(|p| p.contains(id))
    }

    /// Whether `id` is a pattern copy (its geometry follows the pattern).
    pub fn is_pattern_instance(&self, id: u32) -> bool {
        self.patterns
            .iter()
            .any(|p| p.instances.iter().flatten().any(|&i| i == id))
    }

    /// Point, line end or curve center anchor within `tolerance` of `at`.
    pub fn anchor_near(&self, at: Point, tolerance: f32) -> Option<(u32, usize)> {
        self.shapes
            .iter()
            .rev()
            .filter_map(|shape| self.nearest_symmetry_anchor(shape.id(), at))
            .filter_map(|anchor| {
                let distance = self.symmetry_anchor_point(anchor)?.distance(at);
                (distance <= tolerance).then_some((anchor, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(anchor, _)| anchor)
    }

    /// Resolved rotation center of a circular pattern.
    pub fn pattern_center(&self, pattern: &SketchPattern) -> Option<Point> {
        match pattern.kind {
            PatternKind::Circular {
                center_anchor: Some(anchor),
                ..
            } => self.symmetry_anchor_point(anchor),
            PatternKind::Circular { center, .. } => Some(center),
            PatternKind::Linear { .. } => None,
        }
    }

    /// Expected geometry of cell `cell` (1-based) for `source`.
    fn pattern_cell_shape(
        &self,
        pattern: &SketchPattern,
        source: &Shape,
        cell: usize,
    ) -> Option<Shape> {
        match pattern.kind {
            PatternKind::Linear { .. } => Some(source.translated(pattern.cell_offset(cell))),
            PatternKind::Circular { .. } => {
                source.rotated_about(pattern.cell_angle(cell), self.pattern_center(pattern)?)
            }
        }
    }

    fn validate_pattern_sources(&self, ids: &[u32]) -> Result<Vec<u32>, String> {
        let mut sources = Vec::new();
        for &id in ids {
            if !sources.contains(&id) {
                sources.push(id);
            }
        }
        if sources.is_empty() {
            return Err("Select source geometry for the pattern".into());
        }
        for &id in &sources {
            if self.pattern_shape(id).is_none() {
                return Err("Selected shape is missing".into());
            }
            if self.pattern_of(id).is_some() {
                return Err("Shape is already part of a pattern".into());
            }
        }
        Ok(sources)
    }

    fn validate_pattern_size(cells: usize, sources: usize) -> Result<(), String> {
        if !(2..=MAX_PATTERN_CELLS).contains(&cells) {
            return Err(format!(
                "Pattern needs 2 to {MAX_PATTERN_CELLS} instances including the source"
            ));
        }
        if (cells - 1) * sources > MAX_PATTERN_COPIES {
            return Err(format!(
                "A pattern supports at most {MAX_PATTERN_COPIES} copies"
            ));
        }
        Ok(())
    }

    /// Linear pattern tool: `columns` cells `step` apart and `rows` rows
    /// `row_step` apart (the source counts as the first cell). One undo step.
    /// Returns the pattern index.
    pub fn linear_pattern(
        &mut self,
        ids: &[u32],
        columns: usize,
        step: Point,
        rows: usize,
        row_step: Point,
    ) -> Result<usize, String> {
        let sources = self.validate_pattern_sources(ids)?;
        let pattern = SketchPattern {
            sources,
            instances: Vec::new(),
            kind: PatternKind::Linear {
                step,
                columns,
                row_step,
            },
        };
        self.add_pattern(pattern, columns.saturating_mul(rows.max(1)))
    }

    /// Circular pattern tool: `count` cells (source included) over `angle`
    /// degrees about `center`, or about the anchor `center_anchor` when given.
    pub fn circular_pattern(
        &mut self,
        ids: &[u32],
        count: usize,
        angle: f32,
        center: Point,
        center_anchor: Option<(u32, usize)>,
    ) -> Result<usize, String> {
        let sources = self.validate_pattern_sources(ids)?;
        let pattern = SketchPattern {
            sources,
            instances: Vec::new(),
            kind: PatternKind::Circular {
                center,
                center_anchor,
                angle,
            },
        };
        self.add_pattern(pattern, count)
    }

    fn validate_pattern(&self, pattern: &SketchPattern, cells: usize) -> Result<(), String> {
        Self::validate_pattern_size(cells, pattern.sources.len())?;
        match pattern.kind {
            PatternKind::Linear {
                step,
                columns,
                row_step,
            } => {
                if columns == 0 || !cells.is_multiple_of(columns) {
                    return Err("Pattern columns and rows must be positive integers".into());
                }
                check_point(step, "Pattern spacing must be finite and nonzero")?;
                check_point(row_step, "Row spacing must be finite and nonzero")?;
            }
            PatternKind::Circular {
                center,
                center_anchor,
                angle,
            } => {
                if !angle.is_finite() || angle.abs() < 1e-9 || angle.abs() > 360.0 {
                    return Err(
                        "Pattern angle must be nonzero and between -360 and 360 degrees".into(),
                    );
                }
                if !center.x.is_finite() || !center.y.is_finite() {
                    return Err("Pattern center must be finite".into());
                }
                if let Some(anchor) = center_anchor {
                    if pattern.instances.iter().flatten().any(|&i| i == anchor.0) {
                        return Err("A pattern center cannot reference its own copy".into());
                    }
                    if self.symmetry_anchor_point(anchor).is_none() {
                        return Err("Pick a point, line end or center as pattern center".into());
                    }
                }
            }
        }
        Ok(())
    }

    fn add_pattern(&mut self, pattern: SketchPattern, cells: usize) -> Result<usize, String> {
        self.validate_pattern(&pattern, cells)?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.patterns.push(pattern);
        let index = self.patterns.len() - 1;
        self.resize_pattern_cells(index, cells);
        self.selected_spline_point = None;
        self.solve_or_rollback(&[])?;
        Ok(index)
    }

    /// Grow or shrink pattern `index` to `cells` cells, creating copies for
    /// new cells and deleting copies of removed ones (with their relations).
    fn resize_pattern_cells(&mut self, index: usize, cells: usize) {
        let pattern = self.patterns[index].clone();
        let keep = cells - 1;
        let removed: Vec<u32> = pattern
            .instances
            .iter()
            .skip(keep)
            .flatten()
            .copied()
            .collect();
        let mut instances: Vec<Vec<u32>> = pattern.instances.into_iter().take(keep).collect();
        while instances.len() < keep {
            let mut group = Vec::new();
            for &source in &pattern.sources {
                let Some(shape) = self.pattern_shape(source).cloned() else {
                    continue;
                };
                let id = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                self.shapes.push(shape.with_id(id));
                if self.construction.contains(&source) {
                    self.construction.insert(id);
                }
                group.push(id);
            }
            instances.push(group);
        }
        self.patterns[index].instances = instances;
        for id in removed {
            self.remove_shape_and_relations(id);
        }
        let selection: Vec<u32> = self.patterns[index].sources.clone();
        self.set_selection(&selection);
        self.propagate_patterns();
    }

    /// Change count / spacing of linear pattern `index` (web
    /// `resizeLinearSketchPattern`, keeping copies of surviving cells).
    pub fn edit_linear_pattern(
        &mut self,
        index: usize,
        columns: usize,
        step: Point,
        rows: usize,
        row_step: Point,
    ) -> Result<(), String> {
        let pattern = self.patterns.get(index).ok_or("Missing linear pattern")?;
        let PatternKind::Linear {
            columns: old_columns,
            ..
        } = pattern.kind
        else {
            return Err("Missing linear pattern".into());
        };
        let cells = columns.saturating_mul(rows.max(1));
        let mut next = pattern.clone();
        next.kind = PatternKind::Linear {
            step,
            columns,
            row_step,
        };
        self.validate_pattern(&next, cells)?;
        // Re-map surviving cells by (row, column).
        let old_columns = old_columns.max(1);
        let mut by_cell: HashMap<(usize, usize), Vec<u32>> = pattern
            .instances
            .iter()
            .enumerate()
            .map(|(i, group)| {
                (
                    ((i + 1) / old_columns, (i + 1) % old_columns),
                    group.clone(),
                )
            })
            .collect();
        self.history.push(self.snapshot());
        self.future.clear();
        let mut instances = Vec::new();
        let mut fresh = Vec::new();
        for cell in 1..cells {
            let key = (cell / columns, cell % columns);
            match by_cell.remove(&key) {
                Some(group) => instances.push(group),
                None => {
                    fresh.push(instances.len());
                    instances.push(Vec::new());
                }
            }
        }
        let removed: Vec<u32> = by_cell.into_values().flatten().collect();
        for slot in fresh {
            for &source in &next.sources {
                let Some(shape) = self.pattern_shape(source).cloned() else {
                    continue;
                };
                let id = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                self.shapes.push(shape.with_id(id));
                if self.construction.contains(&source) {
                    self.construction.insert(id);
                }
                instances[slot].push(id);
            }
        }
        next.instances = instances;
        self.patterns[index] = next;
        for id in removed {
            self.remove_shape_and_relations(id);
        }
        self.propagate_patterns();
        self.solve_or_rollback(&[])
    }

    /// Change count / angle / center of circular pattern `index`.
    pub fn edit_circular_pattern(
        &mut self,
        index: usize,
        count: usize,
        angle: f32,
        center: Point,
    ) -> Result<(), String> {
        let pattern = self.patterns.get(index).ok_or("Missing circular pattern")?;
        let PatternKind::Circular { center_anchor, .. } = pattern.kind else {
            return Err("Missing circular pattern".into());
        };
        let mut next = pattern.clone();
        next.kind = PatternKind::Circular {
            center,
            center_anchor,
            angle,
        };
        self.validate_pattern(&next, count)?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.patterns[index].kind = next.kind;
        self.resize_pattern_cells(index, count);
        self.solve_or_rollback(&[])
    }

    /// Add a pattern read from a project file (no undo step). Copies must
    /// exist, be distinct and match their source's shape kind.
    pub fn load_pattern(&mut self, pattern: SketchPattern) -> Result<(), String> {
        self.validate_pattern(&pattern, pattern.count())?;
        let mut seen = HashSet::new();
        for &id in pattern
            .sources
            .iter()
            .chain(pattern.instances.iter().flatten())
        {
            if !seen.insert(id) || self.pattern_of(id).is_some() {
                return Err("Pattern shapes must be distinct".into());
            }
        }
        for (cell, group) in pattern.instances.iter().enumerate() {
            if group.len() != pattern.sources.len() {
                return Err("Pattern copies must match their sources".into());
            }
            for (&source, &target) in pattern.sources.iter().zip(group) {
                let source = self
                    .pattern_shape(source)
                    .ok_or("Pattern source is missing")?;
                let target = self
                    .pattern_shape(target)
                    .ok_or("Pattern copy is missing")?;
                let expected = self
                    .pattern_cell_shape(&pattern, source, cell + 1)
                    .ok_or("Invalid pattern copy")?;
                if std::mem::discriminant(&expected) != std::mem::discriminant(target) {
                    return Err("Pattern copy type must match its source".into());
                }
            }
        }
        self.patterns.push(pattern);
        Ok(())
    }

    /// Regenerate every pattern copy from its source.
    pub(super) fn propagate_patterns(&mut self) {
        for p in 0..self.patterns.len() {
            let pattern = self.patterns[p].clone();
            if let PatternKind::Circular {
                center_anchor: Some(_),
                angle,
                ..
            } = pattern.kind
                && let Some(center) = self.pattern_center(&pattern)
            {
                self.patterns[p].kind = PatternKind::Circular {
                    center,
                    center_anchor: match pattern.kind {
                        PatternKind::Circular { center_anchor, .. } => center_anchor,
                        _ => None,
                    },
                    angle,
                };
            }
            for (cell, group) in pattern.instances.iter().enumerate() {
                for (&source, &target) in pattern.sources.iter().zip(group) {
                    let Some(next) = self
                        .pattern_shape(source)
                        .and_then(|shape| self.pattern_cell_shape(&pattern, shape, cell + 1))
                    else {
                        continue;
                    };
                    if let Some(index) = self.shapes.iter().position(|s| s.id() == target) {
                        self.shapes[index] = next.with_id(target);
                    }
                }
            }
        }
    }

    /// Shapes whose geometry follows a pattern.
    pub(super) fn pattern_followers(&self) -> impl Iterator<Item = u32> + '_ {
        self.patterns
            .iter()
            .flat_map(|p| p.instances.iter().flatten().copied())
    }

    /// Drop every pattern that refers to shape `id` (as source, copy or
    /// center anchor). Like the web app the remaining copies stay as free
    /// geometry.
    pub(super) fn forget_patterns(&mut self, id: u32) {
        self.patterns.retain(|p| {
            !p.contains(id)
                && !matches!(p.kind, PatternKind::Circular { center_anchor: Some((anchor, _)), .. } if anchor == id)
        });
    }
}
