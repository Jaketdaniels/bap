//! `bap init`: scaffold a project's `bap.json` and summarize compatibility.

use crate::config::{BapConfig, ModuleFormat};
use crate::style;
use serde_json::json;
use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;

/// Handles `bap init`; returns the process exit code.
pub fn init(force: bool, format: Option<ModuleFormat>, json_output: bool) -> i32 {
    let directory = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let path = directory.join("bap.json");

    if path.is_file() && !force {
        eprintln!(
            "bap: {} already exists; pass --force to overwrite",
            path.display()
        );
        return 1;
    }

    let module_format = match format {
        Some(format) => Some(format),
        None if std::io::stdin().is_terminal() => prompt_module_format(),
        None => None,
    };

    let config = BapConfig {
        runtime: Some("bap".to_owned()),
        module_format,
        aliases: Default::default(),
    };
    let rendered = render_config(&config);
    if let Err(error) = std::fs::write(&path, &rendered) {
        eprintln!("bap: failed to write {}: {error}", path.display());
        return 1;
    }

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "written": path.display().to_string(),
                "config": rendered.trim(),
            }))
            .expect("init json")
        );
    } else {
        println!("{} {}", style::good("created"), path.display());
        println!(
            "{}",
            style::dim("run `bap doctor` to see what your project can use")
        );
    }
    0
}

/// Renders the config with stable key order and no empty fields.
fn render_config(config: &BapConfig) -> String {
    let mut parts = vec![format!(
        "\"runtime\": \"{}\"",
        config.runtime.as_deref().unwrap_or("bap")
    )];
    if let Some(format) = config.module_format {
        let text = match format {
            ModuleFormat::Esm => "esm",
            ModuleFormat::Cjs => "cjs",
        };
        parts.push(format!("\"moduleFormat\": \"{text}\""));
    }
    format!("{{\n  {}\n}}\n", parts.join(",\n  "))
}

/// Interactive prompt for the module-format pin; Enter keeps auto-detection.
fn prompt_module_format() -> Option<ModuleFormat> {
    print!(
        "{} moduleFormat [auto/esm/cjs] (auto): ",
        style::accent("?")
    );
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).is_err() {
        return None;
    }
    match line.trim().to_ascii_lowercase().as_str() {
        "esm" => Some(ModuleFormat::Esm),
        "cjs" => Some(ModuleFormat::Cjs),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_renders_runtime_only() {
        assert_eq!(
            render_config(&BapConfig::default()),
            "{\n  \"runtime\": \"bap\"\n}\n"
        );
    }

    #[test]
    fn module_format_renders_camel_case() {
        let config = BapConfig {
            runtime: Some("bap".to_owned()),
            module_format: Some(ModuleFormat::Cjs),
            aliases: Default::default(),
        };
        assert!(render_config(&config).contains("\"moduleFormat\": \"cjs\""));
    }

    #[test]
    fn rendered_config_round_trips_through_the_parser() {
        let original = BapConfig {
            runtime: Some("bap".to_owned()),
            module_format: Some(ModuleFormat::Esm),
            aliases: Default::default(),
        };
        let parsed: BapConfig =
            serde_json::from_str(&render_config(&original)).expect("valid config json");
        assert_eq!(parsed.module_format, Some(ModuleFormat::Esm));
    }
}
