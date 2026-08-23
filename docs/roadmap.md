# Bap Runtime Roadmap

Bap is an experimental JavaScript runtime built on V8 through `deno_core`. It grows
through working vertical slices; planned capabilities are not presented as shipped.

| Phase | Title | Deliverable | Status | Plan |
|-------|-------|-------------|--------|------|
| 1 | Minimal core | Engine, REPL, console, metrics | Complete | [plan](superpowers/plans/2026-08-19-bap-phase-1-minimal-core.md) |
| 2 | Async runtime & I/O | Tokio timers, files, TCP, fetch | Complete | [plan](superpowers/plans/2026-08-19-bap-phase-2-async-io.md) |
| 3 | Proof-carrying numerics | TypeScript loader/cache, numeric core, proof-backed specialization | Complete | [plan](superpowers/plans/2026-08-19-proof-carrying-numerics.md) |
| 4 | Ecosystem compatibility | CJS loader, node: shims, adoption tooling, compat report | Complete | [plan](superpowers/plans/2026-08-19-bap-phase-4-node-compat-adoption.md) |
| 5 | Tail latency | Bench harness, lanes/warmup, measured report | Complete | [plan](superpowers/plans/2026-08-19-bap-phase-6-tail-latency.md) |
| Future | Hybrid V8 + JSC engine | JSC for profile-selected SSR/numeric kernels | Deferred | — |

## Guiding rules

- **Layers, not a rewrite.** Each phase builds on the last; Phase 1 stays a small,
  well-instrumented runtime that runs one script and prints logs. The hybrid
  V8 + JavaScriptCore engine (JSC for hot, profile-selected SSR and numeric
  kernels) arrives only after the V8 architecture is solid.
- **Prototype first.** Keep CI strategic and commits reviewable. Add production
  governance only when the runtime has a stable proof of concept.
- **Spec-like I/O surface.** The async layer stays small and standard so a
  work-stealing compute runtime with its own thread pools and GC heaps can isolate
  pauses from latency-sensitive I/O without breaking semantics.
- **Measured, not marketed.** Every latency, memory, and compatibility claim gets a
  reproducible benchmark and a published result table.
- **Adoption-friendly.** Compatibility claims require a representative corpus,
  native-addon coverage, and real application tests rather than selected examples.
