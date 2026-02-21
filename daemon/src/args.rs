use clap::Parser;

#[derive(clap::Args, Debug)]
#[command(name = "daemon", about = "Start the PuppyCAD HTTP API daemon")]
pub struct ApiArgs {
    #[arg(long, default_value = "127.0.0.1", value_name = "HOST", help = "Bind address (localhost by default)")]
    pub host: String,
    #[arg(long, default_value_t = 18790, value_name = "PORT", help = "Bind port")]
    pub port: u16,
    #[arg(long, help = "Allow binding outside localhost")]
    pub allow_remote: bool,
}

#[derive(clap::Parser, Debug)]
#[command(name = "puppycadd", version, about = "PuppyCAD daemon CLI")]
pub struct Cli {
    #[command(flatten)]
    pub args: ApiArgs,
}

pub fn parse_args() -> ApiArgs {
    Cli::parse().args
}
