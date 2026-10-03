//! `puppycad --export` writes binary STL per body or one 3MF package.

use std::{ffi::OsStr, fs, path::PathBuf, process::Command};

fn run(args: &[&OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_puppycad"))
        .args(args)
        .output()
        .unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("puppycad-export-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(name)
}

#[test]
fn cli_exports_binary_stl() {
    let dir = scratch("stl");
    let out = dir.join("block.stl");
    let input = fixture("extruded-rectangle.pcad");
    let result = run(&["--export".as_ref(), input.as_os_str(), out.as_os_str()]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&out).unwrap();
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(count, 12);
    assert_eq!(bytes.len(), 84 + count * 50);
    assert!(bytes.starts_with(b"PuppyCAD "));
}

#[test]
fn cli_exports_3mf() {
    let dir = scratch("3mf");
    let out = dir.join("block.3mf");
    let input = fixture("extruded-rectangle.pcad");
    let result = run(&["--export".as_ref(), input.as_os_str(), out.as_os_str()]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&out).unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");
}

#[test]
fn cli_export_fails_on_bad_input() {
    let dir = scratch("bad");
    let missing = dir.join("missing.pcad");
    let out = dir.join("x.stl");
    assert!(
        !run(&["--export".as_ref(), missing.as_os_str(), out.as_os_str()])
            .status
            .success()
    );
    let input = fixture("extruded-rectangle.pcad");
    let wrong = dir.join("x.obj");
    assert!(
        !run(&["--export".as_ref(), input.as_os_str(), wrong.as_os_str()])
            .status
            .success()
    );
    assert!(!run(&["--export".as_ref()]).status.success());
}

/// A copy of `fixtures/assembly.pcad` whose selected document is its
/// assembly.
fn assembly_selected(dir: &std::path::Path) -> PathBuf {
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture("assembly.pcad")).unwrap()).unwrap();
    let index = value["items"]
        .as_array()
        .unwrap()
        .iter()
        .position(|item| item["type"] == "assembly")
        .unwrap();
    value["selectedPath"] = serde_json::json!([index]);
    let path = dir.join("robot.pcad");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

fn stl_triangles(path: &std::path::Path) -> Vec<[[f32; 3]; 3]> {
    let bytes = fs::read(path).unwrap_or_else(|_| panic!("{}", path.display()));
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 84 + count * 50);
    (0..count)
        .map(|t| {
            std::array::from_fn(|v| {
                std::array::from_fn(|c| {
                    let at = 84 + t * 50 + 12 + v * 12 + c * 4;
                    f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
                })
            })
        })
        .collect()
}

#[test]
fn cli_exports_selected_assembly_as_stl_per_instance() {
    let dir = scratch("asm-stl");
    let input = assembly_selected(&dir);
    let out = dir.join("arm.stl");
    let result = run(&["--export".as_ref(), input.as_os_str(), out.as_os_str()]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!out.exists(), "assemblies write one file per instance");
    for name in ["Base", "Upper-arm", "Forearm"] {
        assert_eq!(
            stl_triangles(&dir.join(format!("arm-{name}.stl"))).len(),
            12
        );
    }
    // The upper arm sits on the base: solved placement, not the part origin.
    let min_z = stl_triangles(&dir.join("arm-Upper-arm.stl"))
        .iter()
        .flatten()
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min);
    assert!((min_z - 10.0).abs() < 1e-3, "{min_z}");
}

#[test]
fn cli_exports_named_assembly_as_3mf() {
    let dir = scratch("asm-3mf");
    let input = fixture("assembly.pcad");
    let out = dir.join("arm.3mf");
    let result = run(&[
        "--export".as_ref(),
        input.as_os_str(),
        out.as_os_str(),
        "--assembly".as_ref(),
        "Arm assembly".as_ref(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let mut zip = zip::ZipArchive::new(fs::File::open(&out).unwrap()).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("3D/3dmodel.model").unwrap(), &mut xml).unwrap();
    assert_eq!(xml.matches("<object ").count(), 3);
    assert!(xml.contains("name=\"Upper arm\""), "{xml}");
    let missing = run(&[
        "--export".as_ref(),
        input.as_os_str(),
        out.as_os_str(),
        "--assembly".as_ref(),
        "Nope".as_ref(),
    ]);
    assert!(!missing.status.success());
}
