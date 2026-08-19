# Bap Phase 6: Tail Latency & Benchmarking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Benchmark methodology and the V8 pointer-compression claims are re-verified against current upstream data at build time.

**Goal:** Make tail latency auditable to the spec's targets — cold start ≤8 ms, SSR p50 ≤20 ms, SSR p99 ≤150 ms, npm compatibility ≥99% — with per-request profiles, p50/p99 histograms across isolates, tiered JIT warmup, pinned SSR lanes, an integrated `bap bench` command, and a harness that reproduces the Node/Bun/Deno Next.js SSR latency comparison with Bap's numbers against the published baselines (Bun ~246 ms p50 / ~974 ms p99, Node ~20 ms p50 / ~174 ms p99, Deno ~12 ms p50 / ~101 ms p99).

**Architecture:** A `bap_bench` crate plus a `bap bench` CLI command measure p50/p99 with `hdrhistogram` on top of the Phase 2 per-op histograms. SSR latency work uses a synthetic route-render workload on Bap's own runtime (Bap will not run Next.js itself in this phase; the harness compares route-handler latency to the published figures as reference, labeled honestly). Warmup, isolate pinning ("SSR lanes"), and a trie route-matching cache are runtime options on `BapRuntime`. Pointer compression is a V8 build-time flag, not a runtime toggle — Phase 6 validates the spec claims (enabled by default, ~halves heap memory, 2–4% mean latency overhead, ~7% p99 improvement) and ships the default unless a measured win justifies a custom V8 build via `RUSTY_V8_ARCHIVE`.

**Tech Stack:** `hdrhistogram`, `criterion` (bench harness), `tokio`, existing `bap_metrics` histograms, Phase 3's minimal `bap bench`, Phase 4's compat harness. Node/Bun installed locally; Deno added only if the comparison demands it.

**Phase 6 scope guard:** The benchmark harness measures Bap's synthetic SSR route handler, not a real Next.js app. Warmup/isolation experiments are structured, published comparisons, not untested tweaks. CI runs a 20-iteration smoke benchmark but does not hard-gate on latency targets.

---

### Task 1: Per-request profile capture

**Files:**
- Create: `crates/bap_bench/Cargo.toml`, `crates/bap_bench/src/lib.rs`, `crates/bap_bench/src/profile.rs`
- Modify: `crates/bap_metrics/src/metrics.rs`

- [ ] **Step 1: Request-scoped profiling** — wrap each synthetic route invocation with a `ProfileScope { route, started, isolate_id, ops: BTreeMap<op, OpStat>, gc: GcStats }`; capture per-request op counts and durations plus V8 heap stats before/after.
- [ ] **Step 2: Isolate inventory** — `bap_bench` spawns N isolates, labels each `isolate_id`, and records warm vs cold invocations separately (first call on an isolate is "cold").
- [ ] **Step 3: Histograms** — feed every route-latency sample into an `hdrhistogram::Histogram<u64>`; expose `p50`, `p99`, `max`, `count` per route and per isolate.
- [ ] **Step 4: Tests** — feed a known distribution, assert p50/p99 within tolerance; assert warm-vs-cold classification is correct.
- [ ] **Step 5: Verify and commit** `feat(bench): per-request profiles and histograms`.

### Task 2: SSR benchmark harness

**Files:**
- Create: `bench/ssr/route.js`, `bench/ssr/harness.mjs`, `bench/ssr/README.md`, `crates/bap_bench/src/ssr.rs`

- [ ] **Step 1: Synthetic route workload** — a route renderer that mirrors Next.js SSR shape: parse URL params, fetch a small data payload (Phase 2 `fetch` to a local fixture server), interpolate into a template string, return HTML. Kept dependency-free so it runs under Bap, Node, and Bun identically.
- [ ] **Step 2: Harness** — `harness.mjs` runs the same workload N times against an invoked runtime (`bap`, `node`, `bun`, optionally `deno`), measures per-request latency, prints p50/p99/max in a shared table format. Bap runs through `bap run`; Node/Bun/Deno through their CLIs.
- [ ] **Step 3: Reference table** — a `bench/ssr/README.md` table pre-filled with the published 2026 figures (Bun ~246 ms p50 / ~974 ms p99; Node ~20 ms p50 / ~174 ms p99; Deno ~12 ms p50 / ~101 ms p99), Bap's target row (≤20 / ≤150), and empty rows for the local runs.
- [ ] **Step 4: Tests** — the harness parses Bap's JSON profile output; a CI-light smoke run of 20 iterations completes and prints a table (not gated on the 150 ms target, to avoid flaky CI).
- [ ] **Step 5: Verify and commit** `feat(bench): ssr latency harness with published reference`.

### Task 3: Tiered JIT warmup

**Files:**
- Modify: `crates/bap_core/src/lib.rs`, `crates/bap_cli/src/cli.rs`

- [ ] **Step 1: `--warmup <routes>`** — at startup, pre-execute the listed hot routes once per pinned isolate so V8's TurboFan profile is hot before the first real request. Route list parsed from a small config (`--warmup` flag or `bap.json`).
- [ ] **Step 2: `--lanes <n>`** — spawn `n` long-lived isolates (`BapRuntime::new` per lane) instead of one; requests round-robin across lanes; lanes are never destroyed mid-run. These are the spec's "SSR lanes" (long-lived isolates).
- [ ] **Step 3: Route-matching cache** — a static trie built once at startup matching request paths to handler modules, so per-request routing does no string matching.
- [ ] **Step 4: Tests** — assert a warmup run executes the route before the measured loop; assert `--lanes 3` distributes requests across 3 isolates (checked via `isolate_id` in profiles); assert trie match correctness for prefix/param routes.
- [ ] **Step 5: Verify and commit** `feat(core): warmup, pinned lanes, route trie`.

### Task 4: Pointer-compression validation

**Files:**
- Create: `docs/pointer-compression.md`

- [ ] **Step 1: Verify the claim** — determine whether the `rusty_v8`-bundled V8 build has pointer compression enabled (V8 defaults it on for 64-bit in recent releases). A runtime toggle does not exist; confirm at the build level by inspecting `rusty_v8`'s bundled V8 config or `V8_USE_POINTER_COMPRESSION`.
- [ ] **Step 2: Validate the spec numbers** — measure, on the SSR harness with compression on vs off (custom archive via `RUSTY_V8_ARCHIVE` if needed): heap-memory reduction (~half), mean latency overhead (spec: 2–4%), and p99 movement (spec: ~7% improvement). Record the measured values next to the claims in `docs/pointer-compression.md`.
- [ ] **Step 3: Decision** — ship the default unless the measurement shows a p99 or heap win worth the build complexity; document the decision and evidence.
- [ ] **Step 4: Commit** `docs: pointer compression study`.

### Task 5: `bap bench` command

**Files:**
- Create: `crates/bap_cli/src/bench.rs`, modify `crates/bap_cli/src/cli.rs`, `crates/bap_cli/src/main.rs`

- [ ] **Step 1: Integrate the bench runner** — upgrade Phase 3's minimal `bap bench <file>` to run N iterations, sample per-iteration latency, and print p50/p99/max using the `bap_bench` histograms and per-request profiles (Phase 6 Task 1). `bap bench bench/ssr/route.js --warmup 10 --iterations 1000`.
- [ ] **Step 2: Trace output** — `bap bench --trace out.json` emits the per-request chrome-trace profile from the orchestrator spans (Phase 5).
- [ ] **Step 3: Tests** — assert the command runs the fixture, parses, and prints the histogram row with count ≥ iterations.
- [ ] **Step 4: Verify and commit** `feat(cli): bap bench with histograms`.

### Task 6: Target evaluation vs spec

**Files:**
- Modify: `README.md`, create `docs/latency-report.md`

- [ ] **Step 1: Cold start** — verify `bap run examples/hello.js` cold start ≤8 ms in release on the bench machine (Phase 1 already reports this via `--metrics json`).
- [ ] **Step 2: SSR p50/p99** — run the harness across all four runtimes on the same machine; produce the filled comparison table. Bap's synthetic route targets: p50 ≤20 ms, p99 ≤150 ms.
- [ ] **Step 3: npm compatibility** — pull the ≥99% ratio from the Phase 4 compat harness (`bap report compat`) into the report.
- [ ] **Step 4: Report** — write `docs/latency-report.md` with methodology (machine, N, warmup, GC flags), a results table matching the spec's "Performance & Compatibility" shape (cold start / p50 / p99 / npm compat for Bap, Bun, Node, Deno), and variance notes. Honest framing: Bap's workload is synthetic, not a real Next.js app.
- [ ] **Step 5: Commit** `docs: phase 6 latency report`.

---

## Verification

`cargo fmt --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace` → manual `bap bench bench/ssr/route.js` with the filled table. CI runs the 20-iteration smoke benchmark but does not hard-gate on latency targets.

## Risks

- Real Next.js cannot run on Bap yet; all SSR numbers are synthetic-route comparisons. The report explicitly labels this and defers real-app SSR to a post-Phase-6 milestone.
- `--lanes` multiplies memory (V8 heap per isolate); the isolate inventory report must surface heap per lane.
- Pointer compression is build-time; the study may conclude "ship default," which is an acceptable outcome documented with evidence, not a failure.
- The pointer-compression spec claims (heap halving, 2–4% overhead, ~7% p99 gain) may not reproduce on the synthetic harness; the report records measured values with the claims for comparison rather than forcing a match.

## Self-review

- Spec: "Tail-latency strategy" (tiered JIT warmup for Next.js routes, pinned SSR lanes, precomputed route-matching caches, p99 <150 ms) and the pointer-compression claim are covered in Tasks 3–4; the "Performance & Compatibility" table targets (cold start ≤8, p50 ≤20, p99 ≤150, npm ≥99%) are evaluated in Tasks 2 and 6; the integrated test/bench runner DX item lands in Task 5.
- No placeholders: every task names concrete files, flags, and assertions; the open questions (pointer compression outcome, real-app SSR) are explicit decisions with defined evidence gates.
- Type consistency: `isolate_id`, `route`, and histogram accessor names are defined in Task 1 and reused by Tasks 2–6 and the report; the compat ratio comes from the Phase 4 `bap report compat` surface unchanged.