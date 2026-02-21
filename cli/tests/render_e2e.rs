use std::{fs, process::Command, time::{SystemTime, UNIX_EPOCH}};

#[test]
fn render_one_frame_from_puppybot_to_folder() {
	let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
	let puppybot = project_root.join("../examples/puppybot.pcad");
	let project_root = project_root
		.canonicalize()
		.expect("failed to canonicalize puppycad manifest directory");
	let repo_root = project_root.join("../");

	let captures = std::env::temp_dir().join("puppycad-render-e2e");
	fs::create_dir_all(&captures).expect("failed to create temporary render directory");
	let output_dir = captures.join(format!(
		"puppybot-frame-dir-{}",
		SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
	));
	fs::create_dir_all(&output_dir).expect("failed to create temporary output directory");
	let expected = output_dir.join("frame_0.png");

	let output = if let Some(puppycad_bin) = std::env::var("CARGO_BIN_EXE_puppycad").ok() {
		Command::new(puppycad_bin)
			.arg("render")
			.arg("--headless")
			.arg("--iterations")
			.arg("1")
			.arg("--camera")
			.arg("0")
			.arg("0")
			.arg("10")
			.arg("--look-at")
			.arg("0")
			.arg("0")
			.arg("0")
			.arg("--output-dir")
			.arg(&output_dir)
			.arg(&puppybot)
			.output()
			.expect("failed to run puppycad render with installed binary")
	} else {
		let cargo_bin = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
		Command::new(cargo_bin)
			.arg("run")
			.arg("--manifest-path")
			.arg(repo_root.join("Cargo.toml"))
			.arg("-p")
			.arg("puppycad")
			.arg("--quiet")
			.arg("--")
			.arg("render")
			.arg("--headless")
			.arg("--iterations")
			.arg("1")
			.arg("--camera")
			.arg("0")
			.arg("0")
			.arg("10")
			.arg("--look-at")
			.arg("0")
			.arg("0")
			.arg("0")
			.arg("--output-dir")
			.arg(&output_dir)
			.arg(&puppybot)
			.output()
			.expect("failed to run puppycad render via cargo")
	};

	let stdout = String::from_utf8_lossy(&output.stdout);
	let stderr = String::from_utf8_lossy(&output.stderr);

	if !output.status.success() {
		if stderr.contains("Failed to find an appropriate adapter")
			|| stderr.contains("no suitable GPU adapter")
			|| stderr.contains("no appropriate adapter")
			|| stderr.contains("No compatible GPU adapter found")
		{
			eprintln!(
				"render e2e skipped: environment does not support GPU adapter for screenshot path ({stderr})"
			);
			return;
		}

		panic!(
			"render command failed for puppybot.pcad (stdout: {stdout}, stderr: {stderr})"
		);
	}

	assert!(expected.exists(), "render output file was not written");
	let meta = fs::metadata(&expected).expect("failed to inspect output file metadata");
	assert!(meta.len() > 0, "render output file is empty");

	let _ = fs::remove_file(&expected);
	let _ = fs::remove_dir_all(&output_dir);
}
