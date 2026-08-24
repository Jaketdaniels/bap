//! Script discovery and friendly resolution for `bap run [file]`.

use crate::style;
use std::path::{Path, PathBuf};

/// Extensions treated as runnable scripts when discovering candidates.
const SCRIPT_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "ts", "mts"];
/// Directories never scanned for candidates.
const SKIP_DIRECTORIES: &[&str] = &["node_modules", ".git", "target", "dist", "build"];

/// Where a run target came from, for messaging.
pub enum Resolution {
    /// An explicitly given path that exists.
    Given(PathBuf),
    /// The only script in the current directory, picked automatically.
    OnlyCandidate(PathBuf),
    /// A user-selected candidate from the interactive picker.
    Picked(PathBuf),
}

impl Resolution {
    pub fn path(&self) -> &Path {
        match self {
            Self::Given(path) | Self::OnlyCandidate(path) | Self::Picked(path) => path,
        }
    }

    /// One-line note printed before running, so auto-picks are visible.
    pub fn note(&self) -> Option<String> {
        match self {
            Self::Given(_) => None,
            Self::OnlyCandidate(path) => Some(format!(
                "{} {} {}",
                style::dim("using the only script found:"),
                style::accent(&path.display().to_string()),
                style::dim("(pass a file to be explicit)")
            )),
            Self::Picked(path) => Some(format!(
                "{} {}",
                style::dim("running"),
                style::accent(&path.display().to_string())
            )),
        }
    }
}

/// Resolves the run target from an optional CLI argument.
pub fn resolve_script(explicit: Option<&str>) -> Result<Resolution, String> {
    match explicit {
        Some(path) => resolve_explicit(path),
        None => discover(),
    }
}

fn resolve_explicit(path: &str) -> Result<Resolution, String> {
    let candidate = PathBuf::from(path);
    if candidate.is_file() {
        return Ok(Resolution::Given(candidate));
    }
    let mut message = format!(
        "no such file: {}\nsearched the current directory",
        style::bad(path)
    );
    if let Some(suggestion) = closest_match(path) {
        message.push_str(&format!(
            "\n{} {}",
            style::warn("did you mean"),
            style::accent(&suggestion.display().to_string())
        ));
        message.push_str("?\n");
    } else {
        message.push_str(&format!(
            "\n{} `bap run` with no argument lists what is available",
            style::dim("tip:")
        ));
    }
    Err(message)
}

fn discover() -> Result<Resolution, String> {
    let mut candidates = collect_scripts();
    candidates.sort();
    match candidates.len() {
        0 => Err(format!(
            "no scripts found in the current directory\n{} supported extensions: {}\n{} create one, or pass a path: `bap run path/to/file.js`",
            style::dim("tip:"),
            SCRIPT_EXTENSIONS.join("/"),
            style::dim("or:")
        )),
        1 => Ok(Resolution::OnlyCandidate(candidates.remove(0))),
        _ => pick_interactively(candidates),
    }
}

fn collect_scripts() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(".") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let extension = path.extension().and_then(|extension| extension.to_str());
        if extension.is_some_and(|extension| SCRIPT_EXTENSIONS.contains(&extension))
            && !SKIP_DIRECTORIES.contains(&name.as_str())
        {
            // Strip the "./" that read_dir(".") prefixes for cleaner output.
            found.push(path.strip_prefix("./").unwrap_or(&path).to_path_buf());
        }
    }
    found
}

fn pick_interactively(mut candidates: Vec<PathBuf>) -> Result<Resolution, String> {
    use std::io::IsTerminal as _;
    if !std::io::stdin().is_terminal() {
        let listed = candidates
            .iter()
            .map(|path| format!("  {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(format!(
            "multiple scripts found; pass one explicitly:\n{listed}"
        ));
    }
    use std::io::Write as _;
    println!("{}", style::bold("which script should bap run?"));
    for (index, path) in candidates.iter().enumerate() {
        println!(
            "  {}) {}",
            index + 1,
            style::accent(&path.display().to_string())
        );
    }
    print!("{} 1-{} [1]: ", style::accent("?"), candidates.len());
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return Err("could not read a selection".to_owned());
    }
    let choice = line.trim();
    let index = if choice.is_empty() {
        1
    } else {
        match choice.parse::<usize>() {
            Ok(index) if (1..=candidates.len()).contains(&index) => index,
            _ => return Err(format!("'{choice}' is not one of 1-{}", candidates.len())),
        }
    };
    Ok(Resolution::Picked(candidates.remove(index - 1)))
}

/// Finds the closest script in the current directory by edit distance.
fn closest_match(requested: &str) -> Option<PathBuf> {
    let requested_name = Path::new(requested)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| requested.to_owned());
    let mut best: Option<(usize, PathBuf)> = None;
    for candidate in collect_scripts() {
        let name = candidate
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let distance = edit_distance(&requested_name, &name);
        let within_reach = distance <= 2
            || name
                .to_ascii_lowercase()
                .starts_with(&requested_name.to_ascii_lowercase());
        if within_reach
            && best
                .as_ref()
                .is_none_or(|(best_distance, _)| distance < *best_distance)
        {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, path)| path)
}

/// Classic Wagner-Fischer edit distance without the full matrix row history.
fn edit_distance(left: &str, right: &str) -> usize {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    for (i, l) in left.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, r) in right.iter().enumerate() {
            let substitute = previous[j] + usize::from(l != r);
            let insert = current[j] + 1;
            let delete = previous[j + 1] + 1;
            current.push(substitute.min(insert).min(delete));
        }
        previous = current;
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("srv.js", "server.js"), 3);
        assert_eq!(edit_distance("app.js", "app.js"), 0);
        assert_eq!(edit_distance("mai.js", "main.js"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    #[test]
    fn close_matches_are_in_reach_and_distant_ones_are_not() {
        // mai.js -> main.js is distance 1; totally different names exceed 2.
        assert_eq!(edit_distance("mai.js", "main.js"), 1);
        assert!(edit_distance("zzz.js", "main.js") > 2);
    }
}
