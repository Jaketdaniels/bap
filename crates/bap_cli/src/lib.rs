//! bap command-line implementation library.
//!
//! `src/main.rs` is a thin wrapper that parses arguments and calls [`entry`].

#![forbid(unsafe_code)]

pub mod bench;
pub mod cli;
pub use cli::root_command;
pub mod config;
mod doctor;
pub mod init;
mod inspect;
pub mod repl;
mod report;
pub mod resolve;
pub mod run;
pub mod style;

use bap_metrics::MetricsCollector;
use bap_ts::{ProofMode, TsModuleLoader};

/// Configures process-global runtime defaults; call once before any isolate.
pub fn configure() {
    bap_ops::configure_v8_defaults();
}

/// Dispatches a parsed command and returns the process exit code.
pub fn entry(
    command: cli::Command,
    metrics_format: cli::MetricsFormat,
    log_level: Option<&str>,
) -> i32 {
    bap_metrics::logging::init(log_level.unwrap_or("info"));
    let metrics = MetricsCollector::new();
    match command {
        cli::Command::Run {
            file,
            no_proofs,
            require_proofs,
            script_args,
        } => {
            let script = match resolve::resolve_script(file.as_deref()) {
                Ok(resolution) => {
                    if let Some(note) = resolution.note() {
                        println!("{note}");
                    }
                    resolution.path().to_path_buf()
                }
                Err(message) => {
                    eprintln!("bap: {message}");
                    return 1;
                }
            };
            let proof_mode = if no_proofs {
                ProofMode::Disabled
            } else if require_proofs {
                ProofMode::Required
            } else {
                ProofMode::Normal
            };
            match config::BapConfig::load(std::path::Path::new(".")) {
                Ok(project) => {
                    if project
                        .runtime
                        .as_deref()
                        .is_some_and(|runtime| runtime != "bap")
                    {
                        eprintln!("bap: bap.json declares a non-'bap' runtime");
                        return 2;
                    }
                    let loader = TsModuleLoader::for_user()
                        .with_proof_mode(proof_mode)
                        .with_aliases(&project.base_directory(), &project.aliases);
                    let format_override = project.module_format.map(|format| match format {
                        config::ModuleFormat::Esm => bap_ts::ModuleKind::Esm,
                        config::ModuleFormat::Cjs => bap_ts::ModuleKind::CommonJs,
                    });
                    run::run_file(
                        &script.to_string_lossy(),
                        script_args,
                        loader,
                        project.aliases.clone(),
                        format_override,
                        metrics_format,
                        metrics,
                    )
                }
                Err(error) => {
                    eprintln!("bap: {error}");
                    1
                }
            }
        }
        cli::Command::Init {
            force,
            format,
            json,
        } => init::init(
            force,
            format.and_then(|format| format.into_module_format()),
            json,
        ),
        cli::Command::Repl => match repl::run(metrics_format, metrics) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("bap: {err}");
                1
            }
        },
        cli::Command::Bench {
            file,
            warmup,
            iterations,
            lanes,
            trace,
            format,
            script_args,
        } => bench::bench(file, warmup, iterations, lanes, script_args, trace, format),
        cli::Command::Doctor { json } => doctor::doctor(json),
        cli::Command::Report { command } => match command {
            cli::ReportCommand::Compat { input } => report::report(input),
        },
        cli::Command::Inspect { command } => inspect::inspect(command),
        cli::Command::Complete { shell, out } => complete(shell, out),
    }
}

/// Emits shell completion scripts for the parser definition.
fn complete(shell: clap_complete::Shell, out: Option<String>) -> i32 {
    use std::io::Write as _;
    let mut command = cli::root_command();
    match out {
        Some(path) => {
            let result = std::fs::File::create(&path).and_then(|mut file| {
                clap_complete::generate(shell, &mut command, "bap", &mut file);
                file.flush()
            });
            match result {
                Ok(()) => {
                    println!("wrote {path}");
                    0
                }
                Err(error) => {
                    eprintln!("bap: cannot write {path}: {error}");
                    1
                }
            }
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            clap_complete::generate(shell, &mut command, "bap", &mut stdout);
            0
        }
    }
}
