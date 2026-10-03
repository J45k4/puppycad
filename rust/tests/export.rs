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
