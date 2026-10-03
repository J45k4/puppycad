//! Load → save → reload → save for every fixture; the two saves must be byte-identical.

use std::{fs, path::PathBuf};

#[test]
fn fixtures_roundtrip_stably() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let out = std::env::temp_dir().join(format!("puppycad-roundtrip-{}", std::process::id()));
    fs::create_dir_all(&out).unwrap();
    let mut paths: Vec<PathBuf> = fs::read_dir(&fixtures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "pcad"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures in {}", fixtures.display());
    let mut failures = Vec::new();
    for path in &paths {
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        let first = out.join(format!("{name}.first.pcad"));
        let second = out.join(format!("{name}.second.pcad"));
        let result = puppycad::roundtrip(path, &first)
            .and_then(|()| puppycad::roundtrip(&first, &second))
            .and_then(|()| {
                let a = fs::read(&first).map_err(|e| e.to_string())?;
                let b = fs::read(&second).map_err(|e| e.to_string())?;
                if a == b {
                    Ok(())
                } else {
                    Err("second save differs from first".into())
                }
            });
        if let Err(error) = result {
            failures.push(format!("{name}: {error}"));
        }
    }
    let _ = fs::remove_dir_all(&out);
    assert!(
        failures.is_empty(),
        "round-trip failures:\n{}",
        failures.join("\n")
    );
}

#[test]
fn roundtrip_rejects_missing_input() {
    let missing = std::env::temp_dir().join("puppycad-does-not-exist.pcad");
    let out = std::env::temp_dir().join("puppycad-missing-out.pcad");
    assert!(puppycad::roundtrip(&missing, &out).is_err());
}
