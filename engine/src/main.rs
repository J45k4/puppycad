mod args;
mod codegen;
mod parser;
mod parse_cmd;
mod eval;
mod types;
mod api;

use args::{parse_args, Command};
use std::io::{IsTerminal, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
	let cli = parse_args();
	match cli.command {
		Command::Parse(args) => parse_cmd::run_parse(args),
		Command::Validate(args) => parse_cmd::run_validate(args),
		Command::Render(args) => parse_cmd::run_render(args),
		Command::Api(args) => api::run_api(args),
	}
}

fn read_source(path: Option<&std::path::Path>) -> Result<String, String> {
	if let Some(path) = path {
		return std::fs::read_to_string(path).map_err(|err| {
			format!(
				"failed to read '{}': {err}",
				path.to_string_lossy()
			)
		});
	}

	if std::io::stdin().is_terminal() {
		return Err("no input provided; pass FILE or pipe .pcad content via stdin".to_owned());
	}

	let mut input = String::new();
	std::io::stdin()
		.read_to_string(&mut input)
		.map_err(|err| format!("failed to read stdin: {err}"))?;

	if input.trim().is_empty() {
		return Err("no input provided; pass FILE or pipe .pcad content via stdin".to_owned());
	}

	Ok(input)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn reads_source_from_missing_stdin_as_error() {
		let err = read_source(None).expect_err("stdin should be empty in test");
		assert!(err.contains("no input provided"));
	}

	#[test]
	fn compiles_puppybot_example_to_json() {
		let source = include_str!("../../examples/puppybot.pcad");
		let file = parser::parse_pcad(source).expect("should parse puppybot example");
		let output = codegen::compile_to_three_json(&file).expect("should serialize puppybot to json");
		assert!(output.contains("\"finalId\": \"final\""));
		assert!(output.contains("\"kind\": \"solid\""));
		assert!(output.contains("\"op\": \"hole\""));
	}
}
