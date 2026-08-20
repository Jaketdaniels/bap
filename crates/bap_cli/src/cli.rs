//! Clap definitions for the `bap` command-line interface.

use clap::{Parser, Subcommand, ValueEnum};

/// Metrics output format: human-readable lines or JSON.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MetricsFormat {
    Pretty,
    Json,
}

/// Parsed `bap` command-line interface.
#[derive(Parser)]
#[command(
    name = "bap",
    version,
    about = "Bap Runtime: a V8 JavaScript and TypeScript runtime"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Metrics report format.
    #[arg(long, global = true, value_enum, default_value_t = MetricsFormat::Pretty, help = "metrics report format")]
    pub metrics: MetricsFormat,

    /// Tracing filter, e.g. debug.
    #[arg(long, global = true, help = "tracing filter, e.g. debug")]
    pub log_level: Option<String>,
}

/// Subcommands accepted by `bap`.
#[derive(Subcommand)]
pub enum Command {
    /// Run a JavaScript or TypeScript file
    Run {
        /// Path to a JavaScript or TypeScript file.
        file: String,
        /// Arguments passed to the script.
        #[arg(help = "arguments passed to the script")]
        script_args: Vec<String>,
    },
    /// Start an interactive REPL
    Repl,
}
