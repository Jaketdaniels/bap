#![forbid(unsafe_code)]

mod cli;
mod repl;
mod run;

use bap_metrics::MetricsCollector;
use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    bap_metrics::logging::init(cli.log_level.as_deref().unwrap_or("info"));
    let metrics = MetricsCollector::new();
    let code = match cli.command {
        cli::Command::Run { file, script_args } => {
            run::run_file(&file, script_args, cli.metrics, metrics)
        }
        cli::Command::Repl => match repl::run(cli.metrics, metrics) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("bap: {err}");
                1
            }
        },
    };
    std::process::exit(code);
}
