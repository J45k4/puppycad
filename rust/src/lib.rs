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
///
/// When the project's selected top-level document is an assembly, or
/// `assembly` names one, the solved assembly is exported instead: one STL per
/// instance (`out-<instance>.stl`) or one 3MF with an object per instance.
pub fn export(
    input: &Path,
    output: &Path,
    assembly: Option<&str>,
) -> Result<Vec<std::path::PathBuf>, String> {
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
    let (project, document) = project::ProjectFile::open(input)?;
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("puppycad")
        .to_string();
    let write = |path: std::path::PathBuf, bytes: &[u8]| {
        std::fs::write(&path, bytes)
            .map(|()| path.clone())
            .map_err(|error| format!("write {}: {error}", path.display()))
    };
    let assembly_item = match assembly {
        Some(name) => Some(
            project
                .assembly_named(name)
                .ok_or_else(|| format!("assembly \"{name}\" does not exist"))?,
        ),
        None => project.selected_assembly(),
    };
    if let Some(item) = assembly_item {
        let mut asm = project.assembly_at(item).ok_or("assembly does not exist")?;
        if asm.read_only_reason.is_none() {
            assembly::solver::apply(&mut asm)?;
        }
        let bodies = project.assembly_world_bodies(&asm, &document)?;
        if extension.as_deref() == Some("3mf") {
            let bytes = threemf::package(&asm.name, &bodies)?;
            return Ok(vec![write(output.to_path_buf(), &bytes)?]);
        }
        return bodies
            .iter()
            .map(|body| {
                let path = output.with_file_name(format!(
                    "{stem}-{}.stl",
                    assembly::file_stem_part(&body.name)
                ));
                let header = format!("{} / {}", asm.name, body.name);
                write(path, &stl::binary_stl(&header, &body.triangles))
            })
            .collect();
    }
    let bodies = stl::project_bodies(&project)?;
    if bodies.is_empty() {
        return Err("No supported solid to export".into());
    }
    if extension.as_deref() == Some("3mf") {
        let bytes = threemf::package(project.part_name(), &bodies)?;
        return Ok(vec![write(output.to_path_buf(), &bytes)?]);
    }
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
