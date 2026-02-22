mod args;
mod parse_cmd;
mod ui;

pub use args::{parse_args, Command, ParseArgs, RenderArgs, ValidateArgs};
pub use parse_cmd::{run_parse, run_render, run_validate};
pub use ui::run_ui;
pub use args::UiArgs;

use std::io::{IsTerminal, Read};

pub fn read_source(path: Option<&std::path::Path>) -> Result<String, String> {
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
