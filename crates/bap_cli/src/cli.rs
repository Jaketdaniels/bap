use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MetricsFormat {
    Pretty,
    Json,
}

#[derive(Parser)]
#[command(
    name = "bap",
    version,
    about = "Bap Runtime: a TypeScript-native JavaScript runtime with auditable numerics"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, global = true, value_enum, default_value_t = MetricsFormat::Pretty, help = "metrics report format")]
    pub metrics: MetricsFormat,

    #[arg(long, global = true, help = "tracing filter, e.g. debug")]
    pub log_level: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run a JavaScript file
    Run {
        file: String,
        #[arg(help = "arguments passed to the script")]
        script_args: Vec<String>,
    },
    /// Start an interactive REPL
    Repl,
}
