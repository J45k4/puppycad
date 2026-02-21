use std::process::Command;

#[test]
fn validate_puppybot_via_cli() {
	let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
	let puppybot = project_root.join("../examples/puppybot.pcad");
	let project_root = project_root
		.canonicalize()
		.expect("failed to canonicalize puppycad manifest directory");
	let repo_root = project_root.join("../");
	let puppycad_bin = std::env::var("CARGO_BIN_EXE_puppycad").ok();

	let output = if let Some(puppycad_bin) = puppycad_bin {
		Command::new(puppycad_bin)
			.arg("validate")
			.arg(&puppybot)
			.output()
			.expect("failed to run puppycad validate subcommand")
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
			.arg("validate")
			.arg(&puppybot)
			.output()
			.expect("failed to run puppycad validate via cargo")
	};

	assert!(output.status.success(), "validate command failed for puppybot.pcad");
	assert!(
		String::from_utf8_lossy(&output.stdout).contains("pcad file is valid"),
		"expected success output to include validation confirmation"
	);
}
