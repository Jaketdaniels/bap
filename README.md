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
