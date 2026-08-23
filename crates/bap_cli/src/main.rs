//! bap command-line entry point.
//! Parses arguments and dispatches to `run`, `repl`, or `inspect`.

#![forbid(unsafe_code)]

mod cli;
mod inspect;
mod repl;
mod run;

use bap_metrics::MetricsCollector;
use bap_ts::ProofMode;
use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    bap_metrics::logging::init(cli.log_level.as_deref().unwrap_or("info"));
    let metrics = MetricsCollector::new();
    let code = match cli.command {
        cli::Command::Run {
            file,
            no_proofs,
            require_proofs,
            script_args,
        } => {
            let proof_mode = if no_proofs {
                ProofMode::Disabled
            } else if require_proofs {
                ProofMode::Required
            } else {
                ProofMode::Normal
            };
            run::run_file(&file, script_args, proof_mode, cli.metrics, metrics)
        }
        cli::Command::Repl => match repl::run(cli.metrics, metrics) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("bap: {err}");
                1
            }
        },
        cli::Command::Inspect { command } => inspect::inspect(command),
    };
    std::process::exit(code);
}
