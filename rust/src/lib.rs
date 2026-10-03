//! Headless PuppyCAD core: project I/O, sketch model, splines and STL export.
//! The windowed app in `main.rs` builds on this library.

pub mod model;
pub mod project;
pub mod solver;
pub mod spline;
pub mod stl;

use std::path::Path;

/// Loads `input` and writes it to `output` without bumping the revision.
/// Used by `puppycad --roundtrip` and the cross-loader tests.
pub fn roundtrip(input: &Path, output: &Path) -> Result<(), String> {
    if !input.is_file() {
        return Err(format!("{} does not exist", input.display()));
    }
    let (project, document) = project::ProjectFile::open(input)?;
    project.export_to(&document, output)
}
