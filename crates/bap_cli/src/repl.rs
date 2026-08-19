use crate::cli::MetricsFormat;
use crate::run::report;
use bap_core::BapRuntime;
use bap_metrics::MetricsCollector;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{BufRead, IsTerminal};
use std::time::Instant;

pub fn run(format: MetricsFormat, metrics: MetricsCollector) -> Result<(), Box<dyn std::error::Error>> {
    let mut runtime = BapRuntime::new(metrics.clone());
    println!("bap {} — .exit to quit", env!("CARGO_PKG_VERSION"));
    if std::io::stdin().is_terminal() {
        run_interactive(&mut runtime, &metrics)?;
    } else {
        run_piped(&mut runtime, &metrics)?;
    }
    report(format, &metrics.snapshot());
    Ok(())
}

fn eval_line(runtime: &mut BapRuntime, metrics: &MetricsCollector, code: &str) {
    let started = Instant::now();
    match runtime.eval(code) {
        Ok(value) => println!("{value}"),
        Err(err) => eprintln!("{err}"),
    }
    metrics.record_op("repl.eval", started.elapsed());
    let (used, total) = runtime.sample_heap();
    metrics.record_heap(used, total);
    println!("  ~{} ms", started.elapsed().as_millis());
}

fn is_complete(code: &str) -> bool {
    let mut depth: i32 = 0;
    for ch in code.chars() {
        match ch {
            '{' | '(' | '[' => depth += 1,
            '}' | ')' | ']' => depth -= 1,
            _ => {}
        }
    }
    depth <= 0
}

fn run_interactive(
    runtime: &mut BapRuntime,
    metrics: &MetricsCollector,
) -> rustyline::Result<()> {
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
                eval_line(runtime, metrics, &code);
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn run_piped(runtime: &mut BapRuntime, metrics: &MetricsCollector) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim() == ".exit" {
            break;
        }
        if !line.trim().is_empty() {
            eval_line(runtime, metrics, &line);
        }
    }
    Ok(())
}
