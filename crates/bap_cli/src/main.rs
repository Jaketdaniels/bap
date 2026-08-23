//! bap command-line entry point.
//! Parses arguments and dispatches to `run`, `repl`, or `inspect`.

#![forbid(unsafe_code)]

mod cli;
mod config;
mod doctor;
mod inspect;
mod repl;
mod run;

use bap_metrics::MetricsCollector;
use bap_ts::{ProofMode, TsModuleLoader};
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
            match config::BapConfig::load(std::path::Path::new(".")) {
                Ok(project) => {
                    if project.runtime.as_deref().is_some_and(|runtime| runtime != "bap") {
                        eprintln!("bap: bap.json declares a non-'bap' runtime");
                        std::process::exit(2);
                    }
                    let loader = TsModuleLoader::for_user()
                        .with_proof_mode(proof_mode)
                        .with_aliases(&project.base_directory(), &project.aliases);
                    let format_override = project.module_format.map(|format| match format {
                        config::ModuleFormat::Esm => bap_ts::ModuleKind::Esm,
                        config::ModuleFormat::Cjs => bap_ts::ModuleKind::CommonJs,
                    });
                    run::run_file(
                        &file,
                        script_args,
                        loader,
                        project.aliases.clone(),
                        format_override,
                        cli.metrics,
                        metrics,
                    )
                }
                Err(error) => {
                    eprintln!("bap: {error}");
                    1
                }
            }
        }
        cli::Command::Repl => match repl::run(cli.metrics, metrics) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("bap: {err}");
                1
            }
        },
        cli::Command::Inspect { command } => inspect::inspect(command),
        cli::Command::Doctor { json } => doctor::doctor(json),
    };
    std::process::exit(code);
}
