//! Machine-level setup behind `bap install`: completions and man pages.

use crate::cli::{Cli, root_command};
use clap_complete::Shell;
use serde_json::json;
use std::path::{Path, PathBuf};

/// One planned or completed filesystem change.
struct Action {
    path: PathBuf,
    description: &'static str,
}

/// Handles `bap setup`; returns the process exit code.
pub fn setup(dry_run: bool, zshrc: bool, json_output: bool) -> i32 {
    let home = home_dir();
    let data = data_dir(&home);
    let actions = plan(&home, &data);

    let mut applied = Vec::new();
    for action in &actions {
        let verb = if dry_run { "would write" } else { "wrote" };
        match perform(action, dry_run) {
            Ok(()) => applied.push(json!({
                "path": action.path.display().to_string(),
                "action": format!("{verb}"),
                "what": action.description,
            })),
            Err(error) => {
                eprintln!(
                    "bap: failed to {} {}: {error}",
                    if dry_run { "plan" } else { "write" },
                    action.path.display()
                );
                return 1;
            }
        }
    }

    if zshrc {
        match append_zshrc_snippet(&home, dry_run) {
            Ok(added) => {
                let rc = zshrc_path(&home);
                applied.push(json!({
                    "path": rc.display().to_string(),
                    "action": if added { if dry_run { "would append" } else { "appended" } } else { "already configured" },
                    "what": "zsh fpath snippet",
                }));
            }
            Err(error) => {
                eprintln!("bap: could not update ~/.zshrc: {error}");
                return 1;
            }
        }
    }

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({ "dryRun": dry_run, "actions": applied }))
                .expect("install json")
        );
    } else {
        println!(
            "{}",
            crate::style::bold(if dry_run {
                "bap setup (dry run)"
            } else {
                "bap setup"
            })
        );
        for action in &actions {
            println!(
                "  {} {}",
                crate::style::dim(verb_for(dry_run)),
                action.path.display()
            );
        }
        if zshrc {
            println!(
                "  {} {}",
                crate::style::dim(verb_for(dry_run)),
                zshrc_path(&home).display()
            );
        }
        println!("{}", path_advice());
    }
    0
}

fn verb_for(dry_run: bool) -> &'static str {
    if dry_run { "would write" } else { "wrote" }
}

/// Everything `bap install` maintains, derived from the live CLI definition.
fn plan(home: &Path, data: &Path) -> Vec<Action> {
    let mut actions = Vec::new();

    let completion_dir = data.join("completions");
    actions.push(Action {
        path: completion_dir.join("bap.bash"),
        description: "bash completions",
    });
    actions.push(Action {
        path: completion_dir.join("_bap"),
        description: "zsh completions (add to fpath)",
    });
    actions.push(Action {
        path: completion_dir.join("bap.fish"),
        description: "fish completions",
    });
    actions.push(Action {
        path: data.join("zfunc/_bap"),
        description: "zsh fpath copy",
    });

    // One man page per command page the CLI defines.
    let man1 = data.join("man/man1");
    let mut pending = vec![root_command()];
    while let Some(command) = pending.pop() {
        actions.push(Action {
            path: man1.join(format!("{}.1", command.get_name())),
            description: "man page",
        });
        for subcommand in command.get_subcommands() {
            if !subcommand.is_hide_set() {
                // clap borrows names; the leak is bounded by page count.
                let full_name: &'static str = Box::leak(
                    format!("{}-{}", command.get_name(), subcommand.get_name()).into_boxed_str(),
                );
                pending.push(subcommand.clone().name(full_name));
            }
        }
    }
    let _ = home;
    actions
}

/// Writes (or pretends to write) one action.
fn perform(action: &Action, dry_run: bool) -> std::io::Result<()> {
    if dry_run {
        return Ok(());
    }
    let contents = render_action(action)?;
    if let Some(parent) = action.path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&action.path, contents)
}

fn render_action(action: &Action) -> std::io::Result<Vec<u8>> {
    let name = action
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut buffer = Vec::new();
    match name.as_str() {
        "bap.bash" => generate_completion(Shell::Bash, &mut buffer),
        "_bap" | "zfunc/_bap" => generate_completion(Shell::Zsh, &mut buffer),
        "bap.fish" => generate_completion(Shell::Fish, &mut buffer),
        other if other.ends_with(".1") => {
            render_man_page(other.trim_end_matches(".1"), &mut buffer)?;
        }
        _ => return Err(std::io::Error::other("unknown setup artifact")),
    }
    Ok(buffer)
}

fn generate_completion(shell: Shell, out: &mut Vec<u8>) {
    let mut command = <Cli as clap::CommandFactory>::command();
    clap_complete::generate(shell, &mut command, "bap", out);
}

fn render_man_page(page: &str, out: &mut Vec<u8>) -> std::io::Result<()> {
    let root = root_command();
    let mut queue = vec![root];
    while let Some(command) = queue.pop() {
        if command.get_name() == page {
            clap_mangen::Man::new(command).render(out)?;
            return Ok(());
        }
        for subcommand in command.get_subcommands() {
            if !subcommand.is_hide_set() {
                let full_name: &'static str = Box::leak(
                    format!("{}-{}", command.get_name(), subcommand.get_name()).into_boxed_str(),
                );
                queue.push(subcommand.clone().name(full_name));
            }
        }
    }
    Err(std::io::Error::other(format!("no command named {page}")))
}

/// Appends a guarded fpath block for zsh completions exactly once.
fn append_zshrc_snippet(home: &Path, dry_run: bool) -> Result<bool, String> {
    use std::io::Write as _;
    let rc = zshrc_path(home);
    let marker = "# >>> bap completions >>>";
    let block = format!(
        "{marker}\nfpath=(~/.local/share/bap/zfunc $fpath)\nautoload -Uz compinit && compinit -C\n# <<< bap completions <<<\n"
    );
    let existing = std::fs::read_to_string(&rc).unwrap_or_default();
    if existing.contains(marker) {
        return Ok(false);
    }
    if dry_run {
        return Ok(true);
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&rc)
        .map_err(|error| error.to_string())?;
    file.write_all(block.as_bytes())
        .map_err(|error| error.to_string())?;
    Ok(true)
}

fn zshrc_path(home: &Path) -> PathBuf {
    home.join(".zshrc")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn data_dir(home: &Path) -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"))
        .join("bap")
}

/// Human guidance about putting the binary directory on PATH.
fn path_advice() -> String {
    let Some(exe) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    else {
        return String::new();
    };
    let in_path = std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|entry| entry == exe));
    if in_path {
        format!(
            "{} on PATH: {}",
            crate::style::good("ready."),
            exe.display()
        )
    } else {
        format!(
            "{} add {} to your PATH to finish setup",
            crate::style::warn("action needed:"),
            crate::style::accent(&exe.display().to_string())
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_covers_completions_and_every_command_page() {
        let home = Path::new("/home/tester");
        let data = data_dir(home);
        let actions = plan(home, &data);

        let paths: Vec<String> = actions
            .iter()
            .map(|a| a.path.display().to_string())
            .collect();
        assert!(paths.iter().any(|p| p.ends_with("completions/bap.bash")));
        assert!(paths.iter().any(|p| p.ends_with("completions/_bap")));
        assert!(paths.iter().any(|p| p.ends_with("completions/bap.fish")));
        assert!(paths.iter().any(|p| p.ends_with("man/man1/bap.1")));
        assert!(paths.iter().any(|p| p.ends_with("man/man1/bap-run.1")));
        assert!(paths.iter().any(|p| p.ends_with("man/man1/bap-complete.1")));
    }

    #[test]
    fn every_planned_artifact_renders_nonempty_content() {
        let home = Path::new("/home/tester");
        let data = data_dir(home);
        for action in plan(home, &data) {
            let rendered = render_action(&action)
                .unwrap_or_else(|error| panic!("{}: {error}", action.path.display()));
            assert!(!rendered.is_empty(), "{} was empty", action.path.display());
        }
    }

    #[test]
    fn zshrc_snippet_is_idempotent() {
        let home = tempfile::tempdir().unwrap();
        assert!(append_zshrc_snippet(home.path(), false).unwrap());
        assert!(!append_zshrc_snippet(home.path(), false).unwrap());
        let rc = std::fs::read_to_string(zshrc_path(home.path())).unwrap();
        assert_eq!(rc.matches(">>> bap completions >>>").count(), 1);
    }

    #[test]
    fn dry_run_writes_nothing() {
        let home = tempfile::tempdir().unwrap();
        let data = home.path().join("data").join("bap");
        for action in plan(home.path(), &data) {
            perform(&action, true).unwrap();
            assert!(
                !action.path.exists(),
                "dry run wrote {}",
                action.path.display()
            );
        }
    }

    #[test]
    fn real_perform_writes_expected_content() {
        let home = tempfile::tempdir().unwrap();
        let data = home.path().join("data").join("bap");
        for action in plan(home.path(), &data) {
            perform(&action, false).unwrap();
            assert!(action.path.is_file(), "missing {}", action.path.display());
        }
        let bash = std::fs::read_to_string(data.join("completions/bap.bash")).unwrap();
        assert!(bash.contains("_bap()"));
        let man = std::fs::read_to_string(data.join("man/man1/bap-run.1")).unwrap();
        assert!(man.contains(".TH"));
    }
}
