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

/// Module format pin accepted by `bap init` and `bap.json`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum InitFormat {
    Auto,
    Esm,
    Cjs,
}

impl InitFormat {
    /// Converts to the config representation; `Auto` means no pin.
    pub fn into_module_format(self) -> Option<crate::config::ModuleFormat> {
        match self {
            Self::Auto => None,
            Self::Esm => Some(crate::config::ModuleFormat::Esm),
            Self::Cjs => Some(crate::config::ModuleFormat::Cjs),
        }
    }
}

/// Builds the root clap command for generators (completions, man pages).
pub fn root_command() -> clap::Command {
    <Cli as clap::CommandFactory>::command()
}

/// Parsed `bap` command-line interface.
#[derive(Parser)]
#[command(
    name = "bap",
    version,
    about = "Bap Runtime: a V8 JavaScript and TypeScript runtime",
    after_help = "Run `bap <command> --help` for details. New here? Try `bap init` then `bap run`."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

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
    /// Run a JavaScript or TypeScript file (bare `bap run` picks one)
    #[command(
        after_help = "Examples:\n  bap run server.js -- port 8080\n  bap run            # runs the only script in this directory\n  bap run --require-proofs route.ts"
    )]
    Run {
        /// Path to a JavaScript or TypeScript file.
        file: Option<String>,
        /// Never consult the TypeScript proof backend.
        #[arg(long, conflicts_with = "require_proofs")]
        no_proofs: bool,
        /// Fail when a recognized proof candidate does not prove.
        #[arg(long)]
        require_proofs: bool,
        /// Arguments passed to the script.
        #[arg(help = "arguments passed to the script (use `--` before dash-args)")]
        script_args: Vec<String>,
    },
    /// Start an interactive REPL
    #[command(
        after_help = "Dot commands inside the REPL:\n  .help    list commands\n  .metrics toggle timing output\n  .clear   clear the screen\n  .exit    leave the REPL (Ctrl-D also works)"
    )]
    Repl,
    /// Scaffold a bap.json for this project
    #[command(after_help = "Examples:\n  bap init\n  bap init --format cjs --force")]
    Init {
        /// Overwrite an existing bap.json.
        #[arg(long)]
        force: bool,
        /// Pin the module format instead of auto-detecting per file.
        #[arg(long, value_enum)]
        format: Option<InitFormat>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Configure this machine: completions and man pages
    #[command(
        after_help = "Examples:\n  bap install             # write completions + man pages, print PATH advice\n  bap install --dry-run   # show what would change"
    )]
    Install {
        /// Show what would be written without writing anything.
        #[arg(long)]
        dry_run: bool,
        /// Also append an fpath snippet to ~/.zshrc for zsh completions.
        #[arg(long)]
        zshrc: bool,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Update bap to the latest GitHub release
    Upgrade {
        /// Upgrade to this exact tag instead of latest.
        #[arg(long)]
        version: Option<String>,
        /// Show what would happen without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Diagnose a project's compatibility with Bap
    Doctor {
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Measure script latency across pinned isolates
    #[command(
        after_help = "Examples:\n  bap bench bench/ssr/request.mjs --warmup 10 --iterations 200 --lanes 3\n  bap bench unit.mjs --trace trace.json"
    )]
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
        #[arg(help = "arguments passed to the script (use `--` before dash-args)")]
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
    /// Emit a shell completion script for bap
    Complete {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<String>,
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
