# Bap Runtime

An experimental V8-backed JavaScript runtime building toward fast TypeScript,
error-bounded numerics, and proof-backed specialization.

The current prototype runs ES modules and a REPL with timers, file I/O, TCP,
fetch, structured logging, and operation-level latency metrics.

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

See `docs/roadmap.md` for the six-phase plan. Planning docs live in
`docs/superpowers/plans/`.

## Project practice

The proof of concept uses a compact quality gate and meaningful main-branch commits.
See `CONTRIBUTING.md` and `docs/governance.md`.
