# Bap Phase 1: Minimal Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A small, well-instrumented Rust-hosted JavaScript runtime (`bap`) that can run one script and print logs, plus an interactive REPL, with structured logging and op-level metrics wired from day one.

**Architecture:** Rust 2024 workspace of four crates with a strict dependency direction `bap_cli → bap_core → {bap_ops, bap_metrics}`. `bap_core` embeds V8 through `deno_core`'s `JsRuntime` (no raw V8 API usage) and owns the human-reviewed engine surface. `bap_ops` defines the `extension!` with console ops; `bap_metrics` owns structured logging (tracing) and a thread-safe `MetricsCollector`. Every crate is `#![forbid(unsafe_code)]` — Phase 1 has zero unsafe.

**Tech Stack:** Rust 1.96 (edition 2024), `deno_core 0.410` (+ `include_icudata`), `deno_ops 0.286`, `serde_v8 0.319`, `tokio 1`, `clap 4`, `rustyline 18`, `tracing`/`tracing-subscriber`, `serde`/`serde_json`.

**API facts verified from current `deno_core` 0.410 docs/source (Aug 2026):**
- `extension!(name, ops = [...], js = [dir "js", "file.js"], config = { field: Type }, state = |state, config| state.put(config.field))` generates `name::init(config_field)` returning an `Extension`. The `state` closure receives a generated `Config` struct; read fields off it.
- Ops use `#[op2]` (deno_ops); plain ops take `state: &mut OpState` first; `#[string]` args map to `String`; return `Result<(), deno_core::error::JsErrorBox>` for void ops.
- JS calls ops via `Deno.core.ops.op_<name>(...)` (confirmed by the `hello_world.rs` example).
- Module running: `resolve_path(path, parent) → load_main_es_module → mod_evaluate → run_event_loop(PollEventLoopOptions::default())`.
- REPL eval: `execute_script("<anon>", code)` returns `v8::Global<Value>`; `deno_core::scope!(scope, runtime)` then `serde_v8::from_v8::<serde_json::Value>(scope, local)`.
- Heap stats: `runtime.v8_isolate().get_heap_statistics(&mut v8::HeapStatistics)`.

**Verification loop for every task:** `cargo fmt --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`. First full build downloads the prebuilt V8 binary via `rusty_v8` (network needed; if blocked, set `RUSTY_V8_ARCHIVE` to a mirror).

---

### Task 1: Scaffold workspace and governance

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `rust-toolchain.toml`
- Create: `.gitignore`
- Create: `LICENSE`
- Create: `README.md`
- Create: `CONTRIBUTING.md`
- Create: `docs/governance.md`
- Create: `docs/unsafe-policy.md`
- Create: `.github/workflows/ci.yml`
- Create: empty crate dirs `crates/bap_core`, `crates/bap_ops`, `crates/bap_metrics`, `crates/bap_cli`

- [ ] **Step 1: Write the workspace root `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = [
  "crates/bap_core",
  "crates/bap_ops",
  "crates/bap_metrics",
  "crates/bap_cli",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
license = "MIT"
repository = "https://github.com/sirwillchris/kyx-link-generator"

[workspace.dependencies]
deno_core = { version = "0.410", features = ["include_icudata"] }
deno_ops = "0.286"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_v8 = "0.319"
tracing = "0.1"
anyhow = "1"
```

- [ ] **Step 2: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.96.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 3: Write `.gitignore`**

```gitignore
/target
.DS_Store
*.local
.dev.vars
```

- [ ] **Step 4: Write `LICENSE` (MIT)**

```text
MIT License

Copyright (c) 2026 Bap Runtime contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 5: Write `README.md`**

```markdown
# Bap Runtime

A TypeScript-native JavaScript runtime that makes advanced numerics and
sub-150 ms tail latency auditable by design.

Phase 1 (this milestone): a small, well-instrumented JS runtime that runs one
script and prints logs, plus a REPL, with structured logging and op-level
metrics wired around every syscall.

## Quick start

```sh
cargo build --workspace
./target/debug/bap run examples/hello.js
./target/debug/bap repl
./target/debug/bap --metrics json run examples/hello.js
```

## Layout

| Crate             | Responsibility                                   |
|-------------------|--------------------------------------------------|
| `bap_core`        | V8 via `deno_core` `JsRuntime`, module loading   |
| `bap_ops`         | Console ops and the `extension!` definition      |
| `bap_metrics`     | Structured logging + `MetricsCollector`          |
| `bap_cli`         | `bap` binary: `run`, `repl`, `--metrics`         |

## Roadmap

See `docs/roadmap.md` for the five-phase plan. Planning docs live in
`docs/superpowers/plans/`.

## Governance

Core engine/runtime code is human-reviewed and hand-written; AI assistance is
confined to tests, docs, and non-critical utilities. See `CONTRIBUTING.md` and
`docs/governance.md`.
```

- [ ] **Step 6: Write `CONTRIBUTING.md`**

```markdown
# Contributing

Bap follows a manual-first policy for core engine code.

- `crates/bap_core` and `crates/bap_math` (future) are **human-review only**:
  every patch requires an approving review by a maintainer. Bulk AI-driven
  rewrites of core subsystems are disallowed.
- AI assistance is welcome in tests, docs, examples, and non-critical utilities
  (`bap_ops` glue, `bap_metrics`, tooling).
- Rust rules: `#![forbid(unsafe_code)]` unless the crate is `bap_unsafe`; explicit
  types; `cargo fmt`, `cargo clippy -- -D warnings`, and tests must pass.
- Commits use Conventional Commits (`feat`, `fix`, `chore`, `test`, `refactor`).
```

- [ ] **Step 7: Write `docs/governance.md`**

```markdown
# Governance

## Human-first review

Core engine, scheduler, and GC-adjacent paths are hand-written and hand-reviewed.
AI assistance is confined to tests, docs, and non-critical utilities. Every patch
to `bap_core` requires an approving maintainer review. This is public policy:
early adopters must be able to trust the runtime.

## Unsafe and AI policy

- Unsafe Rust is capped at <=5,000 lines in the ~500 kLOC codebase, isolated in
  the `bap_unsafe` crate, each block tagged with a `// SAFETY:` reason.
- Miri and ASAN run in CI on every change to unsafe modules.
- Bulk AI-driven rewrites of core subsystems are disallowed.

See `docs/unsafe-policy.md` for details. Phase 1 ships with zero unsafe Rust.
```

- [ ] **Step 8: Write `docs/unsafe-policy.md`**

```markdown
# Unsafe Policy

Phase 1 has zero unsafe Rust: every crate declares `#![forbid(unsafe_code)]`.

## Rules (binding from Phase 4 onward)

1. All `unsafe` lives in the `bap_unsafe` crate only. `bap_core`, `bap_math`,
   `bap_ops`, `bap_metrics`, `bap_cli` keep `#![forbid(unsafe_code)]`.
2. Unsafe lines are capped at 5,000 across the codebase.
3. Every `unsafe {}` block carries a `// SAFETY:` invariant statement.
4. CI runs `cargo miri test` (nightly toolchain) and an ASAN build on the
   unsafe crate for every change touching it.
5. AI-generated patches to unsafe code are rejected at review.
```

- [ ] **Step 9: Write `.github/workflows/ci.yml`**

```yaml
name: ci
on:
  push:
    branches: ["main"]
  pull_request:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace

  miri:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: miri
      - uses: Swatinem/rust-cache@v2
      - run: cargo miri test -p bap_metrics
```

- [ ] **Step 10: Create empty crate directories, init git, first commit**

Run:
```sh
mkdir -p crates/bap_core/src crates/bap_ops/src crates/bap_metrics/src crates/bap_cli/src
git init -b main
git add -A
git commit -m "chore: scaffold bap workspace"
```
Expected: clean `git status`, commit created.

---

### Task 2: `bap_metrics` crate (logging + collector)

**Files:**
- Create: `crates/bap_metrics/Cargo.toml`
- Create: `crates/bap_metrics/src/lib.rs`
- Create: `crates/bap_metrics/src/metrics.rs`
- Create: `crates/bap_metrics/src/logging.rs`

- [ ] **Step 1: Write the failing tests in `crates/bap_metrics/src/metrics.rs`** (impl code comes later; write tests + module skeleton now)

```rust
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct OpStat {
    pub calls: u64,
    pub total_ns: u128,
    pub max_ns: u128,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HeapSample {
    pub elapsed_ms: u128,
    pub used_bytes: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MetricsSnapshot {
    pub cold_start_ms: f64,
    pub ops: HashMap<String, OpStat>,
    pub heap_samples: Vec<HeapSample>,
}

#[derive(Debug, Clone)]
pub struct MetricsCollector {
    started: Instant,
    inner: Arc<Mutex<MetricsSnapshot>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            inner: Arc::new(Mutex::new(MetricsSnapshot::default())),
        }
    }

    pub fn record_op(&self, name: &str, duration: Duration) {
        let mut snap = self.inner.lock().expect("metrics lock poisoned");
        let stat = snap.ops.entry(name.to_owned()).or_default();
        stat.calls += 1;
        stat.total_ns += duration.as_nanos();
        stat.max_ns = stat.max_ns.max(duration.as_nanos());
    }

    pub fn record_heap(&self, used_bytes: usize, total_bytes: usize) {
        let mut snap = self.inner.lock().expect("metrics lock poisoned");
        snap.heap_samples.push(HeapSample {
            elapsed_ms: self.started.elapsed().as_millis(),
            used_bytes,
            total_bytes,
        });
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let mut snap = self.inner.lock().expect("metrics lock poisoned").clone();
        snap.cold_start_ms = self.started.elapsed().as_secs_f64() * 1000.0;
        snap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_op_accumulates() {
        let collector = MetricsCollector::new();
        collector.record_op("console.log", Duration::from_millis(2));
        collector.record_op("console.log", Duration::from_millis(5));
        let snap = collector.snapshot();
        let op = &snap.ops["console.log"];
        assert_eq!(op.calls, 2);
        assert_eq!(op.total_ns, 7_000_000);
        assert_eq!(op.max_ns, 5_000_000);
        assert!(snap.cold_start_ms >= 0.0);
    }

    #[test]
    fn record_heap_saves_samples() {
        let collector = MetricsCollector::new();
        collector.record_heap(10, 100);
        collector.record_heap(20, 100);
        let snap = collector.snapshot();
        assert_eq!(snap.heap_samples.len(), 2);
        assert_eq!(snap.heap_samples[0].used_bytes, 10);
        assert!(snap.heap_samples[1].elapsed_ms >= snap.heap_samples[0].elapsed_ms);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```sh
cargo test -p bap_metrics
```
Expected: FAIL — crate does not exist yet (`error: package ID specification ... did not match`).

- [ ] **Step 3: Write `crates/bap_metrics/Cargo.toml`**

```toml
[package]
name = "bap_metrics"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
tracing.workspace = true
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 4: Write `crates/bap_metrics/src/lib.rs`**

```rust
#![forbid(unsafe_code)]

pub mod logging;
mod metrics;

pub use metrics::{HeapSample, MetricsCollector, MetricsSnapshot, OpStat};
```

- [ ] **Step 5: Write `crates/bap_metrics/src/logging.rs`**

```rust
use tracing_subscriber::{EnvFilter, fmt};

pub fn init(verbosity: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(verbosity));
    let _ = fmt().with_env_filter(filter).try_init();
}
```

- [ ] **Step 6: Run the tests, verify they pass**

Run:
```sh
cargo test -p bap_metrics
```
Expected: `2 passed`.

- [ ] **Step 7: Verify lint + fmt, then commit**

Run:
```sh
cargo fmt --check && cargo clippy -p bap_metrics --all-targets -- -D warnings
git add crates/bap_metrics
git commit -m "feat(metrics): structured logging and op metrics"
```

---

### Task 3: `bap_ops` crate (console ops)

**Files:**
- Create: `crates/bap_ops/Cargo.toml`
- Create: `crates/bap_ops/src/lib.rs`
- Create: `crates/bap_ops/src/console.rs`
- Create: `crates/bap_ops/src/js/console.js`

- [ ] **Step 1: Write the test in `crates/bap_ops/src/lib.rs`** (failing first)

```rust
#![forbid(unsafe_code)]

mod console;
pub use console::{op_console_error, op_console_log, op_console_warn};

use bap_metrics::MetricsCollector;
use deno_core::extension;

extension!(
    bap,
    ops = [op_console_log, op_console_error, op_console_warn],
    js = [dir "js", "console.js"],
    config = { metrics: MetricsCollector },
    state = |state, config| state.put(config.metrics),
    docs = "Bap builtin ops"
);

#[cfg(test)]
mod tests {
    use super::*;
    use bap_metrics::MetricsCollector;
    use std::sync::{Arc, Mutex};

    #[test]
    fn op_log_writes_stdout_and_metrics() {
        let mut state = deno_core::OpState::new();
        state.put(MetricsCollector::new());
        let sink = Arc::new(Mutex::new(Vec::new()));
        std::io::set_output_capture(Some(sink.clone()));
        op_console_log(&mut state, "hello from bap\n".to_string()).unwrap();
        let bytes = sink.lock().unwrap().clone();
        assert_eq!(String::from_utf8(bytes).unwrap(), "hello from bap\n");
        let metrics = state.borrow::<MetricsCollector>();
        assert_eq!(metrics.snapshot().ops["console.log"].calls, 1);
    }

    #[test]
    fn op_error_records_metrics() {
        let mut state = deno_core::OpState::new();
        state.put(MetricsCollector::new());
        op_console_error(&mut state, "boom\n".to_string()).unwrap();
        let metrics = state.borrow::<MetricsCollector>();
        assert_eq!(metrics.snapshot().ops["console.error"].calls, 1);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```sh
cargo test -p bap_ops
```
Expected: FAIL — package not found.

- [ ] **Step 3: Write `crates/bap_ops/Cargo.toml`**

```toml
[package]
name = "bap_ops"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
bap_metrics = { path = "../bap_metrics" }
deno_core.workspace = true
deno_ops.workspace = true
```

- [ ] **Step 4: Write `crates/bap_ops/src/console.rs`**

```rust
use bap_metrics::MetricsCollector;
use deno_core::error::JsErrorBox;
use deno_core::op2;
use std::io::Write;
use std::time::Instant;

fn emit(state: &mut deno_core::OpState, stream: &mut dyn Write, name: &str, msg: &str) {
    let started = Instant::now();
    let metrics = state.borrow::<MetricsCollector>();
    let _ = stream.write_all(msg.as_bytes());
    let _ = stream.flush();
    metrics.record_op(name, started.elapsed());
    tracing::debug!(target: "bap::ops", op = name, "console write");
}

#[op2]
pub fn op_console_log(
    state: &mut deno_core::OpState,
    #[string] msg: String,
) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stdout(), "console.log", &msg);
    Ok(())
}

#[op2]
pub fn op_console_error(
    state: &mut deno_core::OpState,
    #[string] msg: String,
) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stderr(), "console.error", &msg);
    Ok(())
}

#[op2]
pub fn op_console_warn(
    state: &mut deno_core::OpState,
    #[string] msg: String,
) -> Result<(), JsErrorBox> {
    emit(state, &mut std::io::stderr(), "console.warn", &msg);
    Ok(())
}
```

- [ ] **Step 5: Write `crates/bap_ops/src/js/console.js`**

```js
function formatArg(value) {
  if (value === null) return "null";
  if (value === undefined) return "undefined";
  if (typeof value === "string") return value;
  if (typeof value === "bigint") return value.toString() + "n";
  if (typeof value === "function") return value.toString();
  if (value instanceof Error) return value.stack || value.toString();
  try {
    return JSON.stringify(value, (_, val) =>
      typeof val === "bigint" ? val.toString() + "n" : val,
    );
  } catch {
    return String(value);
  }
}

function formatArgs(args) {
  return args.map(formatArg).join(" ");
}

globalThis.console = {
  log: (...args) => Deno.core.ops.op_console_log(formatArgs(args) + "\n"),
  error: (...args) => Deno.core.ops.op_console_error(formatArgs(args) + "\n"),
  warn: (...args) => Deno.core.ops.op_console_warn(formatArgs(args) + "\n"),
};
```

- [ ] **Step 6: Run the tests, verify they pass**

Run:
```sh
cargo test -p bap_ops
```
Expected: `2 passed`. If `OpState::new()` or `state.borrow` signatures differ in 0.410, read the vendored source at `~/.cargo/registry/src/*/deno_core-0.410.0/core/` and adjust the test (the op implementations themselves are unaffected).

- [ ] **Step 7: Verify lint + fmt, then commit**

Run:
```sh
cargo fmt --check && cargo clippy -p bap_ops --all-targets -- -D warnings
git add crates/bap_ops
git commit -m "feat(ops): console ops wired to metrics"
```

---

### Task 4: `bap_core` crate (V8 runtime wrapper)

**Files:**
- Create: `crates/bap_core/Cargo.toml`
- Create: `crates/bap_core/src/lib.rs`

- [ ] **Step 1: Write the failing tests in `crates/bap_core/src/lib.rs`**

```rust
#![forbid(unsafe_code)]

use bap_metrics::MetricsCollector;
use deno_core::{FsModuleLoader, JsRuntime, RuntimeOptions};
use serde_json::Value;

pub struct BapRuntime {
    runtime: JsRuntime,
    metrics: MetricsCollector,
}

impl BapRuntime {
    pub fn new(metrics: MetricsCollector) -> Self {
        let extensions = vec![bap_ops::bap::init(metrics.clone())];
        let runtime = JsRuntime::new(RuntimeOptions {
            module_loader: Some(std::rc::Rc::new(FsModuleLoader)),
            extensions,
            ..Default::default()
        });
        Self { runtime, metrics }
    }

    pub async fn run_script(&mut self, path: &str) -> Result<(), deno_core::error::AnyError> {
        let cwd = std::env::current_dir()?;
        let main_module = deno_core::resolve_path(path, &cwd)?;
        let mod_id = self.runtime.load_main_es_module(&main_module).await?;
        let result = self.runtime.mod_evaluate(mod_id);
        self.runtime.run_event_loop(Default::default()).await?;
        result.await?;
        Ok(())
    }

    pub fn eval(&mut self, code: &str) -> Result<Value, deno_core::error::AnyError> {
        let global = self.runtime.execute_script("<anon>", code)?;
        deno_core::scope!(scope, self.runtime);
        let local = deno_core::v8::Local::new(scope, global);
        Ok(serde_v8::from_v8::<Value>(scope, local)?)
    }

    pub fn sample_heap(&mut self) -> (usize, usize) {
        let isolate = self.runtime.v8_isolate();
        let mut stats = deno_core::v8::HeapStatistics::default();
        isolate.get_heap_statistics(&mut stats);
        (stats.used_heap_size(), stats.total_heap_size())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn eval_returns_json_value() {
        let mut runtime = BapRuntime::new(MetricsCollector::new());
        let value = runtime.eval("1 + 1").unwrap();
        assert_eq!(value, serde_json::json!(2));
    }

    #[test]
    fn console_log_flows_to_stdout_and_metrics() {
        let metrics = MetricsCollector::new();
        let mut runtime = BapRuntime::new(metrics.clone());
        let sink = Arc::new(Mutex::new(Vec::new()));
        std::io::set_output_capture(Some(sink.clone()));
        runtime.eval("console.log('hello', 42)").unwrap();
        let bytes = sink.lock().unwrap().clone();
        assert_eq!(String::from_utf8(bytes).unwrap(), "hello 42\n");
        assert_eq!(metrics.snapshot().ops["console.log"].calls, 1);
    }

    #[test]
    fn sample_heap_reports_nonzero() {
        let mut runtime = BapRuntime::new(MetricsCollector::new());
        let (used, total) = runtime.sample_heap();
        assert!(used > 0);
        assert!(total >= used);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:
```sh
cargo test -p bap_core
```
Expected: FAIL — package not found.

- [ ] **Step 3: Write `crates/bap_core/Cargo.toml`**

```toml
[package]
name = "bap_core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
bap_metrics = { path = "../bap_metrics" }
bap_ops = { path = "../bap_ops" }
deno_core.workspace = true
serde_json.workspace = true
serde_v8.workspace = true
```

- [ ] **Step 4: Write the implementation in `crates/bap_core/src/lib.rs`** (the exact file from Step 1, including its test module)

Expected API notes — resolve at build time from the vendored crate if the compiler objects:
- `bap_ops::bap::init(metrics)` — the `extension!` macro generates `init(config_fields...)` on a module named after the extension.
- `deno_core::error::AnyError` — check the error re-export path (`deno_core::error::AnyError`).
- `runtime.v8_isolate()` — returns `&mut v8::Isolate`.

- [ ] **Step 5: Run the tests, verify they pass**

Run:
```sh
cargo test -p bap_core
```
Expected: `3 passed`. First build downloads the prebuilt V8 binary (several minutes).

- [ ] **Step 6: Verify lint + fmt, then commit**

Run:
```sh
cargo fmt --check && cargo clippy -p bap_core --all-targets -- -D warnings
git add crates/bap_core
git commit -m "feat(core): v8 runtime with console extension"
```

---

### Task 5: `bap_cli` crate (run + REPL + metrics reporting)

**Files:**
- Create: `crates/bap_cli/Cargo.toml`
- Create: `crates/bap_cli/src/main.rs`
- Create: `crates/bap_cli/src/cli.rs`
- Create: `crates/bap_cli/src/run.rs`
- Create: `crates/bap_cli/src/repl.rs`
- Create: `examples/hello.js`
- Create: `crates/bap_cli/tests/run_script.rs`
- Create: `crates/bap_cli/tests/repl.rs`

- [ ] **Step 1: Write the failing integration tests**

`crates/bap_cli/tests/run_script.rs`:

```rust
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_bap");

fn hello_path() -> String {
    format!("{}/../../examples/hello.js", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn run_hello_prints_stdout() {
    let out = Command::new(BIN)
        .args(["run", &hello_path()])
        .output()
        .expect("spawn bap");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("hello from bap"));
    assert!(stdout.contains("sum: 3"));
}

#[test]
fn metrics_json_reports_console_ops() {
    let out = Command::new(BIN)
        .args(["--metrics", "json", "run", &hello_path()])
        .output()
        .expect("spawn bap");
    assert!(out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let snap: serde_json::Value =
        serde_json::from_str(&stderr).expect("metrics json written to stderr");
    assert!(snap["cold_start_ms"].as_f64().is_some());
    assert!(snap["ops"]["console.log"]["calls"].as_u64().unwrap() >= 2);
}
```

`crates/bap_cli/tests/repl.rs`:

```rust
use std::io::Write;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_bap");

#[test]
fn repl_evaluates_expression_from_pipe() {
    let mut child = Command::new(BIN)
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn bap repl");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"1 + 1\n.exit\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("2"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
```sh
cargo test -p bap_cli --test run_script --test repl
```
Expected: FAIL — `bap_cli` package and binary do not exist.

- [ ] **Step 3: Write `crates/bap_cli/Cargo.toml`**

```toml
[package]
name = "bap_cli"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "bap"
path = "src/main.rs"

[dependencies]
bap_core = { path = "../bap_core" }
bap_metrics = { path = "../bap_metrics" }
clap = { version = "4", features = ["derive"] }
rustyline = "18"
serde_json.workspace = true
tokio = { version = "1", features = ["full"] }
tracing.workspace = true
```

- [ ] **Step 4: Write `crates/bap_cli/src/cli.rs`**

```rust
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

    #[arg(long, value_enum, default_value_t = MetricsFormat::Pretty)]
    pub metrics: MetricsFormat,

    #[arg(long, help = "tracing filter, e.g. debug")]
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
```

- [ ] **Step 5: Write `crates/bap_cli/src/run.rs`**

```rust
use crate::cli::MetricsFormat;
use bap_core::BapRuntime;
use bap_metrics::{MetricsCollector, MetricsSnapshot};

pub fn run_file(
    path: &str,
    _script_args: Vec<String>,
    format: MetricsFormat,
    metrics: MetricsCollector,
) -> i32 {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let result = rt.block_on(async {
        let mut runtime = BapRuntime::new(metrics.clone());
        let result = runtime.run_script(path).await;
        let (used, total) = runtime.sample_heap();
        metrics.record_heap(used, total);
        result
    });
    match result {
        Ok(()) => {
            report(format, &metrics.snapshot());
            0
        }
        Err(err) => {
            eprintln!("bap: {err}");
            1
        }
    }
}

pub fn report(format: MetricsFormat, snap: &MetricsSnapshot) {
    match format {
        MetricsFormat::Pretty => {
            eprintln!("--- bap metrics ---");
            eprintln!("cold start: {:.3} ms", snap.cold_start_ms);
            eprintln!("heap samples: {}", snap.heap_samples.len());
            for (name, stat) in &snap.ops {
                eprintln!(
                    "op {name}: {} calls, {} us total, {} us max",
                    stat.calls,
                    stat.total_ns / 1_000,
                    stat.max_ns / 1_000
                );
            }
        }
        MetricsFormat::Json => {
            eprintln!(
                "{}",
                serde_json::to_string_pretty(snap).expect("metrics serialize to json")
            );
        }
    }
}
```

- [ ] **Step 6: Write `crates/bap_cli/src/repl.rs`**

```rust
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
```

- [ ] **Step 7: Write `crates/bap_cli/src/main.rs`**

```rust
mod cli;
mod repl;
mod run;

use bap_metrics::MetricsCollector;
use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    bap_metrics::logging::init(cli.log_level.as_deref().unwrap_or("info"));
    let metrics = MetricsCollector::new();
    let code = match cli.command {
        cli::Command::Run { file, script_args } => {
            run::run_file(&file, script_args, cli.metrics, metrics)
        }
        cli::Command::Repl => match repl::run(cli.metrics, metrics) {
            Ok(()) => 0,
            Err(err) => {
                eprintln!("repl error: {err}");
                1
            }
        },
    };
    std::process::exit(code);
}
```

- [ ] **Step 8: Write `examples/hello.js`**

```js
console.log("hello from bap");
console.log("sum:", 1 + 2);
console.error("warn goes to stderr");
```

- [ ] **Step 9: Run the tests, verify they pass**

Run:
```sh
cargo test -p bap_cli
```
Expected: `3 passed` (1 run test + 1 metrics test + 1 repl test).

- [ ] **Step 10: Manual smoke test**

Run:
```sh
cargo build --workspace
./target/debug/bap run examples/hello.js
./target/debug/bap --metrics json run examples/hello.js
./target/debug/bap repl
```
Expected: hello output on stdout; JSON metrics on stderr; REPL evaluates `1 + 1` to `2` and shows a `~N ms` timing line.

- [ ] **Step 11: Verify lint + fmt, then commit**

Run:
```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings
git add crates/bap_cli examples/hello.js
git commit -m "feat(cli): bap run and repl with metrics reporting"
```

This REPL is the seed for the observability and numerics REPL in Phase 5: the `.exit` / piped-line reader and per-eval metrics reporting shape (`eval_line` recording `repl.eval` op time) are exactly the seam `.interval`, `.fixed`, `.i64`, and `.task` commands extend.

---

### Task 6: Full verification and cold-start measurement

**Files:**
- Modify: `README.md` (record measured cold start if desired)

- [ ] **Step 1: Run the full verification loop**

Run:
```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: all green.

- [ ] **Step 2: Measure cold start in debug and release**

Run:
```sh
cargo build --release
./target/release/bap --metrics json run examples/hello.js 2>&1 >/dev/null
```
Expected: `cold_start_ms` reported; record the value in `notes.txt` (release target is ≤8 ms; debug is informational). The spec's ≤8 ms is a release-mode stretch goal — report, do not gate the build on it.

- [ ] **Step 3: Commit any remaining changes and tag the milestone**

Run:
```sh
git add -A
git commit -m "test: bap phase 1 green under fmt, clippy, tests"
git tag -a v0.1.0 -m "bap phase 1: minimal core"
```

---

## Self-review

- **Spec coverage:** roadmap Phase 1 items all covered — V8 via `deno_core` (`Task 4`), `console.log` + REPL + hello world (`Task 3`, `Task 5`), structured logging and metrics around every syscall (`Task 2`, `Task 3` emit), cold-start + heap measurement (`Task 5`, `Task 6`). Governance items (human-review, unsafe policy, Miri CI gate) land in `Task 1`.
- **Placeholders:** all file contents provided inline; the only adaptive notes are explicit API-verification steps with the fallback source path, not TBDs.
- **Type consistency:** `MetricsCollector` is `Clone` (required by `OpState::borrow`), created in `bap_cli` and cloned into the extension via `bap::init`; `MetricsFormat` is shared between `cli.rs`, `run.rs`, and `repl.rs`; `BapRuntime::{new, run_script, eval, sample_heap}` signatures match across `bap_core` and callers.