use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::process::ExitCode;

use crate::args::{UiArgs, RenderArgs};
use crate::run_render;

pub fn run_ui(args: UiArgs) -> ExitCode {
    if let Some(path) = &args.inspect {
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pcad"))
        {
            return run_render(RenderArgs {
                input: Some(path.clone()),
                headless: false,
                iterations: None,
                camera: None,
                look_at: None,
                output: None,
                output_dir: None,
            });
        }
    }

    let Some((manifest, editor_package)) = resolve_pge_editor_manifest() else {
        eprintln!(
			"unable to locate pge editor manifest. Ensure the dependency is available in Cargo checkout."
		);
        return ExitCode::FAILURE;
    };

    let mut command = ProcessCommand::new("cargo");
    command
        .current_dir(manifest.parent().expect("pge manifest directory"))
        .arg("run")
        .arg("--manifest-path")
        .arg(&manifest);
    if let Some(package) = editor_package {
        command.arg("--package").arg(package);
    }

    if let Some(path) = args.inspect {
        command.arg("--").arg("inspect").arg(path);
    }

    match command.status() {
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
            None => ExitCode::FAILURE,
        },
        Err(err) => {
            eprintln!("failed to launch editor via cargo: {err}");
            ExitCode::FAILURE
        }
    }
}

fn resolve_pge_editor_manifest() -> Option<(PathBuf, Option<String>)> {
    if let Some(override_manifest) = resolve_pge_editor_manifest_override() {
        return Some((override_manifest, Some(String::from("pge_editor"))));
    }

    if let Some(local) = resolve_local_pge_manifest() {
        return Some((local, Some(String::from("pge_editor"))));
    }

    if let Some((manifest, package)) = resolve_from_cargo_metadata() {
        return Some((manifest, package));
    }

    resolve_from_cargo_checkouts()
}

fn resolve_pge_editor_manifest_override() -> Option<PathBuf> {
    let manifest_path = env::var_os("PGE_EDITOR_MANIFEST_PATH")?;
    let path = normalize_manifest_path(manifest_path.to_string_lossy().as_ref());
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn resolve_local_pge_manifest() -> Option<PathBuf> {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .to_path_buf();
    let local_editor_manifest = workspace_root.join("../pge/editor/Cargo.toml");
    if local_editor_manifest.exists() {
        return Some(local_editor_manifest);
    }
    None
}

fn resolve_from_cargo_metadata() -> Option<(PathBuf, Option<String>)> {
    let workspace_manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("Cargo.toml");
    let metadata_output = ProcessCommand::new("cargo")
        .current_dir(workspace_manifest.parent()?)
        .arg("metadata")
        .arg("--format-version")
        .arg("1")
        .output()
        .ok()?;

    if !metadata_output.status.success() {
        return None;
    }

    let metadata = serde_json::from_slice::<serde_json::Value>(&metadata_output.stdout).ok()?;
    let packages = metadata.get("packages")?.as_array()?;

    for package in packages {
        let name = package.get("name")?.as_str()?;
        if name == "pge_editor" {
            if let Some(manifest_path) = package
                .get("manifest_path")
                .and_then(|value| value.as_str())
            {
                let path = normalize_manifest_path(manifest_path);
                if path.exists() {
                    return Some((path, Some(String::from(name))));
                }
            }
        }
    }

    let mut pge_manifest = None;
    for package in packages {
        let name = package.get("name")?.as_str()?;
        let manifest_path = package
            .get("manifest_path")
            .and_then(|value| value.as_str())
            .map(normalize_manifest_path)?;
        let manifest_str = package.get("manifest_path")?.as_str()?;
        let is_editor_manifest =
            manifest_str.contains("/editor/") || manifest_str.contains("\\editor\\");
        if is_editor_manifest && manifest_path.exists() {
            return Some((manifest_path.to_path_buf(), Some(String::from(name))));
        }
        if name == "pge" {
            pge_manifest = Some(manifest_path);
        }
    }

    let pge_manifest = pge_manifest?;
    let editor_manifest = pge_manifest.parent()?.join("editor/Cargo.toml");
    (editor_manifest.exists()).then_some((editor_manifest, Some(String::from("pge_editor"))))
}

fn resolve_from_cargo_checkouts() -> Option<(PathBuf, Option<String>)> {
    let base = cargo_git_checkouts_dir()?;
    let mut fallback_manifest = None;

    for top in fs::read_dir(base).ok()? {
        let Ok(top_entry) = top else {
            continue;
        };
        let top_name = top_entry.file_name();
        let top_name = top_name.to_string_lossy().to_ascii_lowercase();
        if !top_name.starts_with("pge-") {
            continue;
        }

        for version in fs::read_dir(top_entry.path()).ok()? {
            let Ok(version_entry) = version else {
                continue;
            };
            let version_path = version_entry.path();
            if !version_path.is_dir() {
                continue;
            }

            let editor_manifest = version_path.join("editor/Cargo.toml");
            if editor_manifest.exists() {
                return Some((editor_manifest, Some(String::from("pge_editor"))));
            }

            let root_manifest = version_path.join("Cargo.toml");
            if root_manifest.exists() {
                fallback_manifest = Some(root_manifest);
            }
        }
    }

    fallback_manifest.map(|manifest| (manifest, Some(String::from("pge_editor"))))
}

fn cargo_git_checkouts_dir() -> Option<PathBuf> {
    if let Ok(cargo_home) = env::var("CARGO_HOME") {
        return Some(PathBuf::from(cargo_home).join("git").join("checkouts"));
    }

    env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".cargo")
            .join("git")
            .join("checkouts")
    })
}

fn normalize_manifest_path(raw: &str) -> PathBuf {
    let path_str = raw
        .strip_prefix("file://")
        .unwrap_or(raw)
        .replace("%20", " ");
    PathBuf::from(path_str)
}
