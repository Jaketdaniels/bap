//! Clap definitions for the `bap` command-line interface.

use clap::{Parser, Subcommand, ValueEnum};

/// Metrics output format: human-readable lines or JSON.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MetricsFormat {
    Pretty,
    Json,
}

/// Bench output format.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BenchFormat {
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
        /// Never consult the TypeScript proof backend.
        #[arg(long, conflicts_with = "require_proofs")]
        no_proofs: bool,
        /// Fail when a recognized proof candidate does not prove.
        #[arg(long)]
        require_proofs: bool,
        /// Arguments passed to the script.
        #[arg(help = "arguments passed to the script")]
        script_args: Vec<String>,
    },
    /// Start an interactive REPL
    Repl,
    /// Diagnose a project's compatibility with Bap
    Doctor {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Measure script latency across pinned isolates
    Bench {
        /// Script executed once per request.
        file: String,
        /// Unmeasured executions per lane before measuring.
        #[arg(long, default_value_t = 0)]
        warmup: u32,
        /// Measured requests distributed round-robin across lanes.
        #[arg(long, default_value_t = 100)]
        iterations: u32,
        /// Long-lived isolates; each owns a thread.
        #[arg(long, default_value_t = 1)]
        lanes: u32,
        /// Write a chrome-trace JSON document to this path.
        #[arg(long)]
        trace: Option<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = BenchFormat::Pretty)]
        format: BenchFormat,
        /// Arguments passed to the script.
        #[arg(help = "arguments passed to the script")]
        script_args: Vec<String>,
    },
    /// Print measured reports
    Report {
        #[command(subcommand)]
        command: ReportCommand,
    },
    /// Inspect runtime artifacts
    Inspect {
        #[command(subcommand)]
        command: InspectCommand,
    },
}

/// Report subcommands.
#[derive(Subcommand)]
pub enum ReportCommand {
    /// Summarize the npm-compatibility corpus run
    Compat {
        /// Path to results.json from bench/compat/run.mjs.
        #[arg(long)]
        input: Option<String>,
    },
}

/// Inspection subcommands.
#[derive(Subcommand)]
pub enum InspectCommand {
    /// Report the proof evidence a TypeScript file produces
    Proofs {
        /// Path to a TypeScript file.
        file: String,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}
