mod args;
mod api;

fn main() -> std::process::ExitCode {
	let args = args::parse_args();
	api::run_api(args)
}
