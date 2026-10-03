//! Headless PuppyCAD core: project I/O, sketch model, splines and STL export.
//! The windowed app in `main.rs` builds on this library.

pub mod assembly;
pub mod model;
pub mod project;
pub mod sketch_value;
pub mod solver;
pub mod spline;
pub mod stl;
pub mod threemf;

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

/// Exports every supported body of `input`'s active part.
/// `out.stl` writes binary STL per body (`out.stl`, `out-2.stl`, …);
/// `out.3mf` writes one package. Existing files at those paths are replaced.
pub fn export(input: &Path, output: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    if !input.is_file() {
        return Err(format!("{} does not exist", input.display()));
    }
    let extension = output
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("stl" | "3mf")) {
        return Err(format!(
            "{}: output must end in .stl or .3mf",
            output.display()
        ));
    }
    let (project, _) = project::ProjectFile::open(input)?;
    let bodies = stl::project_bodies(&project)?;
    if bodies.is_empty() {
        return Err("No supported solid to export".into());
    }
    let write = |path: std::path::PathBuf, bytes: &[u8]| {
        std::fs::write(&path, bytes)
            .map(|()| path.clone())
            .map_err(|error| format!("write {}: {error}", path.display()))
    };
    if extension.as_deref() == Some("3mf") {
        let bytes = threemf::package(project.part_name(), &bodies)?;
        return Ok(vec![write(output.to_path_buf(), &bytes)?]);
    }
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("puppycad");
    bodies
        .iter()
        .enumerate()
        .map(|(index, body)| {
            let path = if index == 0 {
                output.to_path_buf()
            } else {
                output.with_file_name(format!("{stem}-{}.stl", index + 1))
            };
            let header = format!("{} / {}", project.part_name(), body.name);
            write(path, &stl::binary_stl(&header, &body.triangles))
        })
        .collect()
}
