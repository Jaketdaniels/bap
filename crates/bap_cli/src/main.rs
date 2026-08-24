use clap::Parser as _;

// bap binary entry point.

fn main() {
    bap_cli::configure();
    let args = bap_cli::cli::Cli::parse();
    std::process::exit(bap_cli::entry(
        args.command,
        args.metrics,
        args.log_level.as_deref(),
    ));
}
