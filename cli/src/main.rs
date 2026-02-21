use std::process::ExitCode;

use puppycad::{parse_args, Command, run_parse, run_render, run_ui, run_validate};

fn main() -> ExitCode {
	let cli = parse_args();
	match cli.command {
		Command::Parse(args) => run_parse(args),
		Command::Validate(args) => run_validate(args),
		Command::Render(args) => run_render(args),
		Command::Ui(args) => run_ui(args),
	}
}
