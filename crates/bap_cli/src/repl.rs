use crate::cli::MetricsFormat;
use crate::run::report;
use bap_core::BapRuntime;
use bap_metrics::MetricsCollector;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::io::{BufRead, IsTerminal};
use std::iter::Peekable;
use std::str::Chars;
use std::time::Instant;

pub fn run(
    format: MetricsFormat,
    metrics: MetricsCollector,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = BapRuntime::new(metrics.clone());
    if std::io::stdin().is_terminal() {
        println!("bap {} — .exit to quit", env!("CARGO_PKG_VERSION"));
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
            eprintln!("{err}");
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

fn run_interactive(runtime: &mut BapRuntime, metrics: &MetricsCollector) -> rustyline::Result<()> {
    let mut editor = DefaultEditor::new()?;
    let mut pending = String::new();
    loop {
        let prompt = if pending.is_empty() { "bap> " } else { "bap* " };
        match editor.readline(prompt) {
            Ok(line) => {
                if line.trim() == ".exit" {
                    break;
                }
                pending.push_str(&line);
                if !is_complete(&pending) {
                    continue;
                }
                let code = std::mem::take(&mut pending);
                eval_line(runtime, metrics, &code, true);
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn run_piped(runtime: &mut BapRuntime, metrics: &MetricsCollector) -> std::io::Result<bool> {
    let stdin = std::io::stdin();
    let mut errored = false;
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim() == ".exit" {
            break;
        }
        if !line.trim().is_empty() && !eval_line(runtime, metrics, &line, false) {
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
