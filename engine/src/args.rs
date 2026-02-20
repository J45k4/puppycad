use clap::{Args, Parser as _};
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[command(name = "pcad", version, about = "PuppyCAD parser CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(clap::Subcommand, Debug)]
pub enum Command {
    Parse(ParseArgs),
    Validate(ValidateArgs),
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

pub fn parse_args() -> Cli {
    Cli::parse()
}
