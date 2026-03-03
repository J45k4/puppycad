use std::process::ExitCode;

use crate::args::UiArgs;
use crate::project_items::run_project_items_ui;

pub fn run_ui(args: UiArgs) -> ExitCode {
    run_project_items_ui(args.inspect)
}
