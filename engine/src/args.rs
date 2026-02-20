use clap::Parser as _;
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[command(name = "pcad", version, about = "PuppyCAD parser CLI")]
pub struct Cli {
    #[arg(value_name = "FILE", help = "Input .pcad file. Reads stdin when omitted.")]
    pub input: Option<PathBuf>,
    #[arg(long, help = "Print parsed AST using Rust debug format.")]
    pub ast: bool,
}

pub fn parse_args() -> Cli {
    Cli::parse()
}
