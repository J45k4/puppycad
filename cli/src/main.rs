use std::process::ExitCode;

use puppycad::{Command, UiArgs, parse_args, run_parse, run_render, run_ui, run_validate};

fn main() -> ExitCode {
    let cli = parse_args();
    match cli.command {
        Some(Command::Parse(args)) => run_parse(args),
        Some(Command::Validate(args)) => run_validate(args),
        Some(Command::Render(args)) => run_render(args),
        Some(Command::Ui(args)) => run_ui(args),
        None => run_ui(UiArgs {
            inspect: cli.inspect,
        }),
    }
}
