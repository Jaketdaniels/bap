# Bap Runtime Roadmap

Bap is a TypeScript-native JavaScript runtime built on V8 via `deno_core`, targeting
advanced numerics and sub-150 ms p99 tail latency. Growth is phased and each phase
produces working, testable software before the next starts.

| Phase | Title | Deliverable | Status | Plan |
|-------|-------|-------------|--------|------|
| 1 | Minimal core | Engine + REPL + console + metrics, zero unsafe | Current | [plan](superpowers/plans/2026-08-19-bap-phase-1-minimal-core.md) |
| 2 | Async runtime & I/O | Tokio ops: timers, fs, TCP, fetch | Next | [plan](superpowers/plans/2026-08-19-bap-phase-2-async-io.md) |
| 3 | TS pipeline & tooling | Native TS pipeline, hot reload, test/bench runners, `bpm`, bundler | Planned | [plan](superpowers/plans/2026-08-19-bap-phase-3-ts-pipeline.md) |
| 4 | Node compatibility & adoption | CJS loader, Node-core shim, adoption pathway, LTS/semver | Planned | [plan](superpowers/plans/2026-08-19-bap-phase-4-node-compat-adoption.md) |
| 5 | Numerics & orchestration | Interval/fixed/int64 core, tree-structured planner, data tooling | Planned | [plan](superpowers/plans/2026-08-19-bap-phase-5-numerics-orchestration.md) |
| 6 | Tail latency & benchmarking | Profiles, warmup, SSR harness vs Node/Bun/Deno | Planned | [plan](superpowers/plans/2026-08-19-bap-phase-6-tail-latency.md) |
| Future | Hybrid V8 + JSC engine | JSC for profile-selected SSR/numeric kernels | Deferred | — |

## Guiding rules

- **Layers, not a rewrite.** Each phase builds on the last; Phase 1 stays a small,
  well-instrumented runtime that runs one script and prints logs. The hybrid
  V8 + JavaScriptCore engine (JSC for hot, profile-selected SSR and numeric
  kernels) arrives only after the V8 architecture is solid.
- **Governance from day one.** Core engine code is human-reviewed; AI assistance is
  confined to tests, docs, and utilities. Unsafe Rust is capped, tagged, and gated
  by Miri/ASAN in CI. See `governance.md` and `unsafe-policy.md`.
- **Spec-like I/O surface.** The async layer stays small and standard so a
  work-stealing compute runtime with its own thread pools and GC heaps can isolate
  pauses from latency-sensitive I/O without breaking semantics.
- **Measured, not marketed.** Every latency, memory, and compatibility claim gets a
  reproducible benchmark and a published result table.
- **Adoption-friendly.** Bap is a drop-in runtime option, not a rewrite: Node/Bun
  apps migrate by running test suites on the Node shim, then flipping per-service
  runtime configs while keeping `package.json` and tooling unchanged. Overlapping
  18-month LTS lines align with major Node LTS cycles.