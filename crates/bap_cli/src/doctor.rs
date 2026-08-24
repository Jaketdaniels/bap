//! `bap doctor`: report which imports and requires Bap can satisfy.

use crate::config::BapConfig;
use crate::style;
use bap_ts::shims::{IMPLEMENTED_NODE_BUILTINS, KNOWN_NODE_BUILTINS};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SCAN_EXTENSIONS: &[&str] = &["js", "cjs", "mjs", "ts", "mts"];
const SKIP_DIRECTORIES: &[&str] = &["node_modules", ".git", "target", "dist", "build"];

/// One unique specifier with its classification.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Dependency {
    /// An implemented `node:` builtin.
    Supported(String),
    /// A Node builtin Bap has not implemented.
    NotImplemented(String),
    /// A relative or absolute path import, or a mapped alias.
    Local,
    /// A bare npm package with no alias mapping.
    Missing(String),
}

impl Dependency {
    fn status(&self) -> &'static str {
        match self {
            Self::Supported(_) => "supported",
            Self::NotImplemented(_) => "not-implemented",
            Self::Local => "local",
            Self::Missing(_) => "missing",
        }
    }

    fn specifier(&self) -> String {
        match self {
            Self::Supported(name) | Self::NotImplemented(name) => format!("node:{name}"),
            Self::Local => "(relative path)".to_owned(),
            Self::Missing(name) => name.clone(),
        }
    }
}

/// Handles `bap doctor` and returns the process exit code (0 unless IO fails).
pub fn doctor(json_output: bool) -> i32 {
    let root = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let config = match BapConfig::load(&root) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("bap: {error}");
            return 1;
        }
    };
    let files = collect_sources(&root);
    let mut specifiers = BTreeSet::new();
    for file in &files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        for specifier in extract_specifiers(&source) {
            specifiers.insert(specifier);
        }
    }

    let report: Vec<Dependency> = specifiers
        .iter()
        .map(|specifier| classify(specifier, &config))
        .collect();
    let total = report.len();
    let supported = report
        .iter()
        .filter(|dependency| matches!(dependency, Dependency::Supported(_) | Dependency::Local))
        .count();

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "files": files.len(),
                "dependencies": total,
                "supported": supported,
                "ratio": ratio(supported, total),
                "entries": report.iter().map(|dependency| json!({
                    "specifier": dependency.specifier(),
                    "status": dependency.status(),
                })).collect::<Vec<_>>(),
            }))
            .expect("doctor json")
        );
        return 0;
    }

    println!("{}", style::bold("bap doctor"));
    println!("  scanned {} source files", files.len());
    println!(
        "  {total} unique dependencies; {} ({})",
        style::good(&format!("{supported} supported")),
        ratio(supported, total)
    );
    if total > 0 {
        println!("  {}", progress_bar(supported, total));
    }
    let mut gaps: Vec<&Dependency> = Vec::new();
    for dependency in &report {
        match dependency {
            Dependency::Supported(name) => {
                println!("  {} node:{name}", style::good("supported       "));
            }
            Dependency::NotImplemented(name) => {
                gaps.push(dependency);
                println!("  {} node:{name}", style::warn("NOT IMPLEMENTED "));
            }
            Dependency::Local => {
                println!("  {} (relative path)", style::dim("local           "));
            }
            Dependency::Missing(name) => {
                gaps.push(dependency);
                println!("  {} {name}", style::bad("missing         "));
            }
        }
    }
    print_hints(&gaps);
    0
}

/// Ten-segment bar showing the supported share.
fn progress_bar(supported: usize, total: usize) -> String {
    let filled = if total == 0 {
        10
    } else {
        (supported * 10).div_ceil(total)
    };
    let full = "\u{2588}".repeat(filled);
    let empty = "\u{2591}".repeat(10 - filled);
    format!("[{}{}]", style::good(&full), style::dim(&empty))
}

/// Next-step guidance per gap category; quiet when everything resolves.
fn print_hints(gaps: &[&Dependency]) {
    if gaps.is_empty() {
        println!(
            "  {}",
            style::good("everything this project imports is available")
        );
        return;
    }
    println!("  {}", style::bold("next steps:"));
    let missing: Vec<&str> = gaps
        .iter()
        .filter_map(|dependency| match dependency {
            Dependency::Missing(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let unimplemented: Vec<&str> = gaps
        .iter()
        .filter_map(|dependency| match dependency {
            Dependency::NotImplemented(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    if !missing.is_empty() {
        println!(
            "   - map npm packages onto local files in bap.json aliases: {}",
            missing.join(", ")
        );
    }
    if !unimplemented.is_empty() {
        println!(
            "   - builtins not implemented on Bap yet: {}",
            unimplemented.join(", ")
        );
        println!("     see docs/adoption.md for the migration pathway");
    }
}

fn ratio(supported: usize, total: usize) -> String {
    if total == 0 {
        return "100%".to_owned();
    }
    format!("{}%", (supported * 100 + total / 2) / total)
}

fn classify(specifier: &str, config: &BapConfig) -> Dependency {
    let name = specifier.strip_prefix("node:").unwrap_or(specifier);
    let is_bare = !specifier.starts_with("./")
        && !specifier.starts_with("../")
        && !specifier.starts_with('/');
    if is_bare && IMPLEMENTED_NODE_BUILTINS.contains(&name) {
        return Dependency::Supported(name.to_owned());
    }
    if is_bare && KNOWN_NODE_BUILTINS.contains(&name) {
        return Dependency::NotImplemented(name.to_owned());
    }
    if !is_bare || config.aliases.contains_key(specifier) {
        return Dependency::Local;
    }
    Dependency::Missing(specifier.to_owned())
}

fn collect_sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if !SKIP_DIRECTORIES.contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            let extension = path.extension().and_then(|extension| extension.to_str());
            if extension.is_some_and(|extension| SCAN_EXTENSIONS.contains(&extension)) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Extracts `import`/`export from`/`require`/dynamic-`import` specifiers.
fn extract_specifiers(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        for pattern in [
            "from \"",
            "from '",
            "require(\"",
            "require('",
            "import(\"",
            "import('",
        ] {
            // Every occurrence per line: `import a from "x"; import b from "y"`
            // must yield both specifiers.
            let mut cursor = 0usize;
            while let Some(offset) = trimmed[cursor..].find(pattern) {
                let start = cursor + offset + pattern.len();
                if let Some(specifier) = quoted_after(&trimmed[start..]) {
                    found.push(specifier);
                }
                cursor = start;
            }
        }
        if (trimmed.starts_with("import \"") || trimmed.starts_with("import '"))
            && let Some(specifier) = quoted_after(trimmed["import ".len()..].trim_start())
        {
            found.push(specifier);
        }
    }
    found
}

fn quoted_after(text: &str) -> Option<String> {
    let end = text.find(['"', '\''])?;
    Some(text[..end].to_owned())
}
