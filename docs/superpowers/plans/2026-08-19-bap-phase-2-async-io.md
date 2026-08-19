# Bap Phase 2: Async Runtime & I/O Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. API pins verified at build time against the current `deno_core` release before starting.

**Goal:** Add a Tokio-backed async I/O layer to Bap: timers, file I/O, TCP, and a WHATWG-style `fetch`, mapping JS promises to Rust futures through `deno_core` ops, while keeping the I/O surface small and spec-like so a separate compute pool can be added later without breaking semantics.

**Architecture:** Extend `bap_ops` with `#[op2(async)]` ops that return Tokio futures (deno_core resolves them into JS promises automatically). A `bap:runtime.js` ESM extension becomes the single JS glue module exposing `globalThis.setTimeout`, `globalThis.fetch`, and a `bap` namespace for fs/net. All ops already sit behind the `bap_metrics` middleware pattern, so p50/p99 per-op timings are free.

**Tech Stack:** `tokio 1` (already a deno_core dependency; used directly in ops), `reqwest` (client for `fetch`), `deno_web` (WHATWG Streams) with a minimal internal `ReadableStream` fallback, `bytes`, `http`. Reuse the existing `extension!` + `state` + `metrics` wiring from Phase 1.

**Phase 2 scope guard:** Do not add arbitrary Node APIs. The surface is deliberately tiny and spec-like (timers, fs basics, TCP echo, fetch). Everything else waits for Phase 4's Node shim.

**Dual-runtime concurrency note:** I/O and latency-sensitive work stay on this Tokio event loop bound to the single V8 isolate. Compute-heavy numeric and orchestration work (Phase 5) runs on a separate work-stealing compute pool (`rayon`) with its own threads and, when the hybrid JSC engine lands, its own GC heaps — never sharing the V8 isolate across pool threads, so scheduler pauses can't stall I/O. This phase keeps the I/O surface spec-shaped so that split is a mechanical addition rather than a redesign.

---

### Task 1: Timer ops

**Files:**
- Modify: `crates/bap_ops/src/timers.rs` (new), `crates/bap_ops/src/lib.rs` (register ops), `crates/bap_ops/src/js/runtime.js` (new glue)

- [ ] **Step 1: Write `setTimeout`/`clearTimeout` ops** using `#[op2(async)]` returning a future that completes after the delay, plus a `clear` op removing a pending timer by id from a `HashMap` held in `OpState`. Timer ids returned to JS as numbers.

```rust
#[op2(async)]
pub async fn op_timer_set_timeout(#[smi] delay_ms: u64) -> Result<(), JsErrorBox> {
    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    Ok(())
}
```

- [ ] **Step 2: Write the JS glue** in `runtime.js` mapping `setTimeout(fn, ms)` to the op + `queueMicrotask`/promise completion, and `clearTimeout` to the clear op. Use `Deno.core.ops.op_timer_*`.
- [ ] **Step 3: Tests** — `crates/bap_core` integration test: evaluate `await new Promise(r => setTimeout(() => r("ok"), 5))` inside a module and assert it resolves under `run_event_loop`; assert timer ordering (two timers fire in order). Use `tokio::test` with `time::pause` for determinism where possible.
- [ ] **Step 4: Verify** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -p bap_core`; commit `feat(ops): timers`.

### Task 2: File I/O ops

**Files:**
- Modify: `crates/bap_ops/src/fs.rs` (new), `crates/bap_ops/src/lib.rs`, `crates/bap_ops/src/js/runtime.js`

- [ ] **Step 1: Write `readFile`, `writeFile`, `readDir`, `stat` ops** via `tokio::fs`. Signature: `readFile(path) -> Uint8Array|string`, `writeFile(path, data)`, `readDir(path) -> array of {name, isFile, isDir}`, `stat(path) -> {size, modified}`. Errors surface as JS `Error` with a readable message.
- [ ] **Step 2: Expose in `runtime.js`** as `bap.readFile`, `bap.writeFile`, `bap.readDir`, `bap.stat`.
- [ ] **Step 3: Tests** — fs round-trip to a temp file (write then read back), readDir on the workspace `examples/` dir, stat on a known file. Run against a real tempdir (`std::env::temp_dir()` + uuid).
- [ ] **Step 4: Verify and commit** `feat(ops): file io`.

### Task 3: TCP ops (echo server + client)

**Files:**
- Modify: `crates/bap_ops/src/net.rs` (new), `crates/bap_ops/src/lib.rs`, `crates/bap_ops/src/js/runtime.js`

- [ ] **Step 1: Write `listen`, `connect`, and connection handlers** using `tokio::net`. Ops: `bap.listen(port) -> server handle`, `bap.connect(host, port) -> connection`. Connection lifecycle handled with an async op per accepted socket; data flows via `Uint8Array` chunks. Keep it minimal: an echo protocol for tests only, no HTTP parsing here.
- [ ] **Step 2: Tests** — bind an ephemeral port (port 0), evaluate a script that connects and echoes a buffer, assert the round-trip bytes match, then close. Guard with a tokio timeout to avoid hangs.
- [ ] **Step 3: Verify and commit** `feat(ops): tcp`.

### Task 4: `fetch` (WHATWG-style)

**Files:**
- Modify: `crates/bap_ops/src/fetch.rs` (new), `crates/bap_ops/src/lib.rs`, `crates/bap_ops/src/js/runtime.js`, `crates/bap_ops/Cargo.toml` (add `reqwest`)

- [ ] **Step 1: Write `op_fetch`** using `reqwest::Client` with a response type carrying status, headers, and body bytes. Return a JS object from the op; body exposure via `Response.text()`/`Response.arrayBuffer()` in the glue.
- [ ] **Step 2: Wire WHATWG Streams** — prefer `deno_web` for a spec-compliant `ReadableStream` on `response.body`; if the crate version is incompatible with the pinned `deno_core`, fall back to a minimal internal `ReadableStream` implemented in `runtime.js` over a body-chunk op. Document the choice in the commit body.
- [ ] **Step 3: Tests** — spin up a `tokio` HTTP fixture (e.g. `axum` or a manual `tokio::net` listener) in the test, fetch `http://127.0.0.1:<port>/`, assert status 200 and body content; assert an error surfaces for a refused connection.
- [ ] **Step 4: Verify and commit** `feat(ops): fetch`.

### Task 5: Per-op latency metrics

**Files:**
- Modify: `crates/bap_ops/src/lib.rs`, `crates/bap_metrics/src/metrics.rs`

- [ ] **Step 1: Centralize op timing** — replace per-op `Instant` calls with a small `record_op` helper invoked at each op boundary (sync ops time inline; async ops time before `.await` resolution by wrapping the future). Confirm every registered op in the extension is instrumented.
- [ ] **Step 2: Add p50/p99 histograms** to `bap_metrics` using `hdrhistogram` for async ops (timers, fs, net, fetch), so Phase 5 can report per-op tail latency. Keep sync ops on `{calls, total_ns, max_ns}`.
- [ ] **Step 3: Tests** — assert histogram percentiles update after a batch of mocked durations; assert `record_op` wraps both sync and async paths.
- [ ] **Step 4: Verify and commit** `feat(metrics): op latency histograms`.

---

## Verification

`cargo fmt --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`, then manual: `bap run examples/async.js` demonstrating `setTimeout` + `fetch` + file write with `--metrics json`.

## Risks

- `deno_web` may not align with the pinned `deno_core` 0.410; the internal-streams fallback is the designated escape hatch (Task 4 Step 2).
- Async ops returning complex shapes (fetch response) need `#[serde]` payloads; keep op return types JSON-serializable to reuse `serde_v8`.
- `reqwest` pulls a heavy dep tree; if it conflicts with `deno_core`'s `http` version, pin `reqwest = "0.12"` and resolve at build time.

## Self-review

- Spec: Phase 2 roadmap items covered (Tokio I/O runtime, ops mapping JS promises to futures, small spec-like surface). "Swap implementations later without breaking semantics" is satisfied by the thin `runtime.js` glue boundary.
- No placeholders: every task names concrete files, ops, and test assertions. The only adaptive points are explicit API-verification steps.
- Type consistency: reuse the Phase 1 `MetricsCollector` + `extension!` pattern unchanged.