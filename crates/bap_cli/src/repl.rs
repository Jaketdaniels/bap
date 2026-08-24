//! Interactive and piped REPL loop.

use crate::cli::MetricsFormat;
use crate::run::report;
use crate::style;
use bap_core::BapRuntime;
use bap_metrics::MetricsCollector;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::io::{BufRead, IsTerminal};
use std::iter::Peekable;
use std::str::Chars;
use std::time::Instant;

/// Runs the REPL, selecting interactive or piped mode from the stdin tty state.
/// Prints the metrics report to stderr before returning.
pub fn run(
    format: MetricsFormat,
    metrics: MetricsCollector,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = BapRuntime::new(metrics.clone());
    if std::io::stdin().is_terminal() {
        let version = env!("CARGO_PKG_VERSION");
        println!(
            "{} {version} - {}",
            style::bold("bap"),
            style::dim("type .help for commands, Ctrl-D to quit")
        );
        run_interactive(&mut runtime, &metrics)?;
    } else {
        let errored = run_piped(&mut runtime, &metrics)?;
        if errored {
            return Err("scripted input contained evaluation errors".into());
        }
    }
    report(format, &metrics.snapshot());
    Ok(())
}

/// Evaluates `code`, prints its value, and records `repl.eval` timing and heap.
/// Returns false when evaluation errors. `verbose` gates the timing line.
fn eval_line(
    runtime: &mut BapRuntime,
    metrics: &MetricsCollector,
    code: &str,
    verbose: bool,
) -> bool {
    let started = Instant::now();
    let ok = match runtime.eval(code) {
        Ok(Some(value)) => {
            println!("{value}");
            true
        }
        Ok(None) => {
            println!("undefined");
            true
        }
        Err(err) => {
            eprintln!("{} {err}", style::bad("error:"));
            false
        }
    };
    metrics.record_op("repl.eval", started.elapsed());
    let (used, total) = runtime.sample_heap();
    metrics.record_heap(used, total);
    if verbose {
        println!("  ~{:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
    }
    ok
}

/// Returns true when `code` has balanced delimiters outside strings and comments.
/// Unterminated strings, templates, and block comments count as incomplete.
fn is_complete(code: &str) -> bool {
    let mut depth: i32 = 0;
    let mut chars = code.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' | '\'' => {
                if !skip_string(&mut chars, ch) {
                    return false;
                }
            }
            '`' => {
                if !skip_template(&mut chars) {
                    return false;
                }
            }
            '/' if matches!(chars.peek(), Some('/') | Some('*')) => {
                let starter = *chars.peek().expect("peeked above");
                if !skip_comment(&mut chars, starter) {
                    return false;
                }
            }
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            _ => {}
        }
    }
    depth <= 0
}

/// Consumes a quoted string after `quote`, honoring backslash escapes.
/// Returns false when the string reaches end of input unterminated.
fn skip_string(chars: &mut Peekable<Chars<'_>>, quote: char) -> bool {
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                chars.next();
            }
            _ if ch == quote => return true,
            _ => {}
        }
    }
    false
}

/// Consumes a line or block comment; `starter` is the char after the slash.
/// Returns false when a block comment reaches end of input unterminated.
fn skip_comment(chars: &mut Peekable<Chars<'_>>, starter: char) -> bool {
    match starter {
        '/' => {
            for ch in chars.by_ref() {
                if ch == '\n' {
                    break;
                }
            }
            true
        }
        '*' => {
            while let Some(ch) = chars.next() {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    return true;
                }
            }
            false
        }
        _ => unreachable!("comment starter is slash or star"),
    }
}

/// Consumes a template literal, tracking `${}` interpolation depth.
/// Returns false when the template reaches end of input unterminated.
fn skip_template(chars: &mut Peekable<Chars<'_>>) -> bool {
    let mut interp_depth: i32 = 0;
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                chars.next();
            }
            '$' if chars.peek() == Some(&'{') => {
                chars.next();
                interp_depth += 1;
            }
            '{' if interp_depth > 0 => interp_depth += 1,
            '}' if interp_depth > 0 => interp_depth -= 1,
            '`' if interp_depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Handles a dot command; returns false when the session should end.
fn handle_dot_command(command: &str, verbose: &mut bool) -> bool {
    match command {
        ".help" => {
            println!("{}", style::bold("commands:"));
            println!(
                "  {}  evaluate JavaScript; multi-line input continues until balanced",
                style::accent("<code>")
            );
            println!("  {}  show this list", style::accent(".help"));
            println!(
                "  {}  toggle per-evaluation timing output (currently {})",
                style::accent(".metrics"),
                if *verbose { "on" } else { "off" }
            );
            println!("  {} clear the screen", style::accent(".clear"));
            println!("  {} leave the REPL", style::accent(".exit"));
        }
        ".metrics" => {
            *verbose = !*verbose;
            println!(
                "timing output {}",
                if *verbose {
                    style::good("on")
                } else {
                    style::warn("off")
                }
            );
        }
        ".clear" => {
            print!("\u{1b}[2J\u{1b}[H");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
        ".exit" => return false,
        other => {
            eprintln!(
                "{} unknown command {other}; try .help",
                style::bad("error:")
            );
        }
    }
    true
}

/// Readline loop with `bap>` and `bap*` prompts; `.exit` ends the session.
fn run_interactive(runtime: &mut BapRuntime, metrics: &MetricsCollector) -> rustyline::Result<()> {
    let mut editor = DefaultEditor::new()?;
    let mut pending = String::new();
    let mut verbose = true;
    loop {
        let prompt = if pending.is_empty() {
            style::accent("bap> ")
        } else {
            style::accent("bap* ")
        };
        match editor.readline(&prompt) {
            Ok(line) => {
                let _ = editor.add_history_entry(line.trim());
                let trimmed = line.trim();
                if trimmed.starts_with('.') && pending.is_empty() && is_dot_command(trimmed) {
                    if !handle_dot_command(trimmed, &mut verbose) {
                        break;
                    }
                    continue;
                }
                pending.push_str(&line);
                if !is_complete(&pending) {
                    continue;
                }
                let code = std::mem::take(&mut pending);
                eval_line(runtime, metrics, &code, verbose);
            }
            Err(ReadlineError::Interrupted) => {
                pending.clear();
            }
            Err(ReadlineError::Eof) => break,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn is_dot_command(line: &str) -> bool {
    matches!(line, ".help" | ".metrics" | ".clear" | ".exit") || line.starts_with(".metrics ")
}

/// Evaluates each non-empty stdin line without timing output.
/// Dot commands behave as in interactive mode.
/// Returns true when any line errored.
fn run_piped(runtime: &mut BapRuntime, metrics: &MetricsCollector) -> std::io::Result<bool> {
    let stdin = std::io::stdin();
    let mut errored = false;
    let mut verbose = false;
    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed == ".exit" {
            break;
        }
        if !trimmed.is_empty() && is_dot_command(trimmed) {
            if !handle_dot_command(trimmed, &mut verbose) {
                break;
            }
            continue;
        }
        if !trimmed.is_empty() && !eval_line(runtime, metrics, &line, verbose) {
            errored = true;
        }
    }
    Ok(errored)
}

#[cfg(test)]
mod tests {
    use super::is_complete;

    #[test]
    fn complete_single_expression() {
        assert!(is_complete("1 + 1"));
    }

    #[test]
    fn incomplete_unclosed_delimiter() {
        assert!(!is_complete("{"));
        assert!(!is_complete("("));
        assert!(!is_complete("["));
    }

    #[test]
    fn brace_in_string_is_literal() {
        assert!(is_complete("var s = \"{\";"));
        assert!(is_complete("var s = '{';"));
        assert!(is_complete("var s = \"\\\"\";"));
    }

    #[test]
    fn brace_in_template_is_literal() {
        assert!(is_complete("var s = `{`;"));
        assert!(is_complete("var s = `a ${1 + 2} b`;"));
    }

    #[test]
    fn comments_do_not_affect_depth() {
        assert!(is_complete("// }"));
        assert!(is_complete("/* } */"));
        assert!(!is_complete("/* } */ {"));
        assert!(is_complete("// } {\n"));
        assert!(!is_complete("/* unterminated"));
    }

    #[test]
    fn template_interpolation_stays_open() {
        assert!(!is_complete("var s = `${"));
        assert!(!is_complete("var s = `a${1 +"));
    }
}
