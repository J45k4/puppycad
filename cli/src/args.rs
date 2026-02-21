use clap::{Args, Parser as _};
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[command(name = "puppycad", version, about = "PuppyCAD parser CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
	Parse(ParseArgs),
	Validate(ValidateArgs),
	Render(RenderArgs),
	Ui(UiArgs),
}

#[derive(Args, Debug)]
#[command(name = "parse", about = "Parse a .pcad file")]
pub struct ParseArgs {
    #[arg(value_name = "FILE", help = "Input .pcad file. Reads stdin when omitted.")]
    pub input: Option<PathBuf>,
    #[arg(long, help = "Print parsed AST using Rust debug format.")]
    pub ast: bool,
    #[arg(long, help = "Emit compact JSON model for renderer tooling.")]
    pub json: bool,
}

#[derive(Args, Debug)]
#[command(name = "validate", about = "Validate .pcad structure and semantics")]
pub struct ValidateArgs {
    #[arg(value_name = "FILE", help = "Input .pcad file. Reads stdin when omitted.")]
    pub input: Option<PathBuf>,
}

#[derive(Args, Debug)]
#[command(name = "render", about = "Render a .pcad file using PGE")]
pub struct RenderArgs {
    #[arg(value_name = "FILE", help = "Input .pcad file. Reads stdin when omitted.")]
    pub input: Option<PathBuf>,
    #[arg(long, help = "Run in headless mode (no window).")]
    pub headless: bool,
    #[arg(
        long,
        value_name = "N",
        help = "Iterations/frame count. Defaults to 1 when running in headless or screenshot mode."
    )]
    pub iterations: Option<u64>,
    #[arg(
        long,
        num_args = 3,
        value_name = "X Y Z",
        value_parser = clap::value_parser!(f32),
        help = "Camera position in world coordinates (x y z)."
    )]
    pub camera: Option<Vec<f32>>,
    #[arg(
        long,
        num_args = 3,
        value_name = "X Y Z",
        value_parser = clap::value_parser!(f32),
        help = "Camera look-at point in world coordinates (x y z)."
    )]
    pub look_at: Option<Vec<f32>>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Write the selected frame to this file path."
    )]
    pub output: Option<PathBuf>,
    #[arg(
        long,
        value_name = "DIR",
        help = "Write the selected frame to this directory (defaults to workdir/screenshots)."
    )]
	pub output_dir: Option<PathBuf>,
}

#[derive(Args, Debug)]
#[command(name = "ui", about = "Open the PuppyCAD editor UI")]
pub struct UiArgs {
	#[arg(value_name = "FILE", help = "Optional .pcad file to inspect on launch.")]
	pub inspect: Option<PathBuf>,
}

pub fn parse_args() -> Cli {
    Cli::parse()
}
