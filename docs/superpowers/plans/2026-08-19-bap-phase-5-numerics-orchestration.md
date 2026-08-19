# Bap Phase 5: Numerics, Orchestration & Data Tooling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. SIMD strategy and the orchestration heuristics are re-verified against the current Rust toolchain at build time.

**Goal:** Ship the numeric fidelity stack (first-class interval types aligned with the ES 2026/2027 interval discussions, overflow-checked `int32`/`int64`, and `fixed<18,4>` with SIMD acceleration) as a Rust core exposed to JS/TS, plus a tree-structured task planner that chooses sequential vs parallel (and eventually CPU vs GPU) execution, and schema-aware data tooling with runtime validation, lineage metadata, and reproducible experiment bundles.

**Architecture:** `bap_math` implements the numeric core with property-based error-bound tests (proptest) and zero unsafe (vector lanes via the `wide` crate), so finance and scientific workloads get hardware-accelerated error bounds instead of ad-hoc BigInt/decimal libraries. `bap_orchestration` implements a tree-structured task planner: a partition tree that measures arithmetic intensity and data locality to decide sequential vs parallel per node and to fuse adjacent cheap tasks, executing the underlying dependency DAG over a rayon work-stealing pool; CPU vs GPU is a trait-sealed decision point with the GPU backend a documented deferral. `bap_ts` constant folding (Task 5) consumes `ModuleTypeInfo` to perform units-aware arithmetic with no runtime checks where the AST proves it — the spec's "type-integrated JIT" preview. Numerics cross the FFI boundary as plain JS values (`int64` as `BigInt`). A `bap:schema` module adds runtime validation, lineage metadata, and ARA-style experiment bundles executable as part of CI.

**Tech Stack:** `bap_math`: `proptest`, `wide` (f64x4/i64x4 SIMD lanes). `bap_orchestration`: `rayon`, `petgraph` (DAG), `tracing` + a chrome/perfetto trace serializer. Schema: a small zod-like `bap:schema` JS module, `serde_json` for lineage and experiment manifests. Both Rust crates keep `#![forbid(unsafe_code)]`; the SIMD hot path moves to `bap_unsafe` only if `wide` proves insufficient (documented decision, Miri/ASAN gated per `docs/unsafe-policy.md`).

**Phase 5 scope guard:** This is a prototype of error-bounded numerics and adaptive orchestration, not a full numeric standard library. Interval arithmetic follows directed rounding (bounds are guaranteed, never best-effort); `fixed<18,4>` and `int64` are overflow-checked by default. GPU execution is designed for but not shipped unless a measured win exists.

---

### Task 1: `bap_math` — interval arithmetic

**Files:**
- Create: `crates/bap_math/Cargo.toml`, `crates/bap_math/src/lib.rs`, `crates/bap_math/src/interval.rs`

- [ ] **Step 1: Write `Interval { lo: f64, hi: f64 }`** with `new(lo, hi)` (asserts `lo <= hi`), `contains(x)`, and arithmetic:
  - `add`: lo = lo_a + lo_b (round down), hi = hi_a + hi_b (round up) using directed rounding via `f64::next_down/next_up`.
  - `sub`, `mul` (four-product corner case), `div` (guard against divisor containing zero).
- [ ] **Step 2: Property tests** — `proptest!` over random intervals: for each op, `result.lo <= true_value <= result.hi` (the "error bound" property); width monotonicity; `contains` closed at endpoints. Include degenerate (`lo == hi`) and negative-span cases.
- [ ] **Step 3: Verify and commit** `feat(math): interval arithmetic with directed rounding`.

### Task 2: `bap_math` — checked `int32`/`int64`, `fixed<18,4>`

**Files:**
- Modify: `crates/bap_math/src/lib.rs`, create `crates/bap_math/src/checked.rs`, `crates/bap_math/src/fixed.rs`

- [ ] **Step 1: Checked integer wrappers** — `CheckedI32`/`CheckedI64` around `i32`/`i64` using Rust's `checked_add/sub/mul`; overflow returns a typed `NumericError` instead of wrapping. Convert to/from JS (`int64` → `BigInt` via the op layer).
- [ ] **Step 2: `Fixed<18,4>`** — i64-scaled fixed point (4 fractional digits, 18 integer digits), `new`/`from_f64`/`to_f64`, `checked_add/sub/mul/div`, `scale`-aware rounding to nearest. Compile-time scale constant via const generics or a `const N: u32` param on the type.
- [ ] **Step 3: Property tests** — for random inputs, `to_f64(from_f64(x))` error is within `1e-4`; checked ops overflow exactly at `i64::MAX`/`MIN` boundaries; fixed-point multiplication error bound vs `f64` reference.
- [ ] **Step 4: SIMD batch ops** — batch add/mul over slices using `wide::f64x4` (and `i64x4` where sensible); measure vs scalar loop in a `#[test]`/bench, keep SIMD only where it wins.
- [ ] **Step 5: Verify and commit** `feat(math): checked integers and fixed point`.

### Task 3: Expose numerics to JS/TS

**Files:**
- Create: `crates/bap_ops/src/numeric.rs`, `crates/bap_ops/src/js/numeric.js`, `crates/bap_ops/src/js/numeric.d.ts`, modify `crates/bap_ops/src/lib.rs`

- [ ] **Step 1: Ops** — `bap.interval(lo, hi)` → `{lo, hi}` plain object; `bap.intervalAdd(a, b)` etc. (interval ops take/return the plain-object shape). `bap.i64(n)`/`bap.i64Add(a, b)` using `BigInt`; `bap.fixed18_4(x)`/`bap.fixedAdd(a, b)`.
- [ ] **Step 2: TS typings** — `numeric.d.ts` defining `Interval` (as a value type with `lo`/`hi`), `Fixed<18,4>` (nominal type), `CheckedI64`; so the Phase 3 pipeline type-checks user code against these.
- [ ] **Step 3: Tests** — JS evaluation tests in `bap_core` asserting `intervalAdd` bounds and `i64Add` overflow throws; a `.ts` fixture using the typings passes `bap test`.
- [ ] **Step 4: Verify and commit** `feat(ops): numerics over js/ts boundary`.

### Task 4: `bap_orchestration` — tree-structured task planner

**Files:**
- Create: `crates/bap_orchestration/Cargo.toml`, `crates/bap_orchestration/src/lib.rs`, `crates/bap_orchestration/src/graph.rs`, `crates/bap_orchestration/src/plan.rs`, `crates/bap_orchestration/src/executor.rs`, `crates/bap_orchestration/src/trace.rs`

- [ ] **Step 1: `graph.rs`** — the dependency layer: `TaskGraph { nodes: Vec<Task>, edges: Vec<Edge> }`, `Task { id, cost_estimate, input_size, kind }` where `kind ∈ {Compute, Io, Fused}`. Built from numeric batch descriptors or explicit closures.
- [ ] **Step 2: `plan.rs`** — the tree-structured planner: recursively decompose work into a partition tree, measuring arithmetic intensity (`cost_estimate / input_size`) and data locality (cache-friendly grouping) at each node. Decide per node: sequential vs parallel (`intensity * input_size > threshold` and `available_cores > 1`), fuse adjacent cheap nodes into one `Fused` node to cut context switches, and select a `Backend` (`Cpu` now; `Gpu` is a trait-sealed decision point, not shipped unless a measured win exists).
- [ ] **Step 3: `executor.rs`** — schedule the planned tree over the rayon work-stealing pool, respecting the DAG's topological order for dependencies; inline cheap subtrees on the current thread.
- [ ] **Step 4: `trace.rs`** — emit `tracing` spans per node (`start/end`, kind, decision: sequential|parallel|fused, backend) and serialize a chrome-trace JSON file (`bap trace --out trace.json`) viewable in Perfetto. Trace output is a first-class deliverable, matching "emits traces so you can iterate toward adaptive orchestration."
- [ ] **Step 5: Tests** — deterministic small graphs: assert execution order respects topological edges; threshold decisions differ for tiny vs large inputs; fusion happens for adjacent cheap nodes; the trace JSON round-trips.
- [ ] **Step 6: Wire a demo op** — `bap.sumReduce(batch)` splits a numeric batch across the planner and reports the trace; an example script demonstrates parallel vs sequential on input size.
- [ ] **Step 7: Verify and commit** `feat(orchestration): tree-structured task planner with traces`.

### Task 5: Type-integrated constant folding (preview)

**Files:**
- Modify: `crates/bap_ts/src/metadata.rs`, `crates/bap_ts/src/strip.rs`

- [ ] **Step 1: Units-aware arithmetic hint** — when `ModuleTypeInfo` records `Unit(String)` params, fold `len`-style typed constants at compile time in the pipeline (no runtime checks) for the subset of literal arithmetic the AST can prove, emitting the specialized folded JS. This is the "type-integrated JIT" preview: Bap harvests TS type-level computations and emits the specialized path without runtime checks where the proof exists.
- [ ] **Step 2: Tests** — a `.ts` fixture with unit phantom types folds to a constant; a mismatched-unit case is left unfolded and passes the normal path.
- [ ] **Step 3: Verify and commit** `feat(ts): unit-aware constant folding preview`.

### Task 6: Schema-aware data tooling

**Files:**
- Create: `crates/bap_ops/src/js/schema.js`, `crates/bap_ops/src/js/schema.d.ts`, `bench/experiment/` fixture, modify `crates/bap_ops/src/lib.rs`

- [ ] **Step 1: `bap:schema` module** — a zod-like `s.object({...})`, `s.number()`, `s.string()`, `s.interval()` with runtime validation; validation failures produce human-readable errors (this is the "first-class schemas with runtime validation" DX item).
- [ ] **Step 2: Lineage metadata** — validated values carry lineage (`source`, `derivation`, `schemaVersion`) so data workflows are traceable.
- [ ] **Step 3: Experiment bundles** — an ARA-style bundle: a directory with `data.json`, `schema.json`, `manifest.json` (inputs, parameters, lineage), and a `run.js`; `bap run bundle/manifest.json` reproduces it deterministically and it is executable as part of CI.
- [ ] **Step 4: Tests** — validation passes/fails on correct/incorrect data; lineage round-trips through a bundle run; a bundle runs under `bap test` in CI.
- [ ] **Step 5: Verify and commit** `feat(ops): schema validation, lineage, experiment bundles`.

### Task 7: Observability & numerics REPL

**Files:**
- Modify: `crates/bap_cli/src/repl.rs`, `crates/bap_ops/src/js/numeric.js`

- [ ] **Step 1: Numerics commands** — extend the Phase 1 REPL with `.interval <expr>`, `.fixed <expr>`, `.i64 <expr>` that evaluate and print interval ranges, fixed-point overflow status, and int64 values.
- [ ] **Step 2: Task-graph introspection** — `.task <request-id>` prints the task graph and per-node decisions (sequential/parallel/fused, backend) for a given request from the orchestrator trace store.
- [ ] **Step 3: Tests** — piped REPL tests for `.interval`, `.fixed`, and `.task` output shapes.
- [ ] **Step 4: Verify and commit** `feat(cli): observability and numerics repl`.

---

## Verification

`cargo fmt --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace` → manual `bap run examples/numeric.ts`, `bap trace --out /tmp/trace.json`, and REPL `.interval`/`.task` commands. If any SIMD work lands in `bap_unsafe`, CI adds `cargo miri test -p bap_unsafe` and an ASAN job per the unsafe policy.

## Risks

- `wide` may not expose `i64x4` on all targets; cap SIMD to `f64x4` for the fixed-point scale path and fall back to scalar for the checked-integer path.
- Interval division semantics (divisor spanning zero) must be decided explicitly — plan chooses "return split interval" as a documented `NumericError` in the prototype, refined later.
- `rayon` + `deno_core` (V8 isolate is `!Send`) must never share an isolate across pool threads — the compute pool only runs Rust closures with owned/`Send` data; the JS boundary stays single-isolate.
- The tree-structured planner's heuristics are empirical; the trace output is the instrument that tunes thresholds without code churn.

## Self-review

- Spec: "Numeric fidelity stack" (intervals aligned with ES 2026/2027 discussions, overflow-checked ints, `fixed<18,4>`, SIMD, error bounds), "Type-integrated JIT" preview (units-aware folding without runtime checks), "Adaptive orchestration layer" (tree-structured planner, arithmetic intensity + data locality, sequential/parallel, CPU vs GPU decision point, fusion), "Data- and schema-aware tooling" (runtime validation, lineage metadata, ARA-style experiment bundles in CI), and the "observability and numerics REPL" are all covered across Tasks 1–7.
- No placeholders: concrete ops, type shapes, planner structure, and property-test assertions per task; the documented decisions (split-interval on zero-span div, SIMD fallback, GPU deferral) are choices, not TODOs.
- Type consistency: `Interval` value shape `{lo, hi}` is defined in Task 3 and reused by ops, typings, and the REPL; `Task`/`TaskGraph`/`NodeKind` names are consistent across `graph.rs`/`plan.rs`/`executor.rs`/`trace.rs`.