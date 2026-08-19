# Bap Phase 3: TypeScript Pipeline & Tooling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. API pins verified at build time against the current `deno_core`/`swc` releases before starting.

**Goal:** Make Bap TypeScript-native: run `.ts` files directly through an owned "TS in, JS bytecode out" pipeline that preserves source maps and per-module type metadata for later JIT hints, then grow the single `bap` binary into the toolchain (test runner, hot reload, minimal bench runner, `bpm` install wrapper, bundler). This is the spec's "TS-first execution" and "TS-native DX": no separate `ts-node`/Babel, zero-config TS, and one command for install, run, test, and bundle.

**Architecture:** A `bap_ts` crate implements a `deno_core::ModuleLoader` (`TsModuleLoader`) that resolves `.ts`/`.js`/`.mjs` specifiers, strips types with SWC, and hands emitted JS plus inline source maps to the runtime. Type metadata is harvested into a per-module side-table (`ModuleTypeInfo`) — the seam Phase 6's tiered JIT consumes. The pipeline owns the "TS in, JS bytecode out" path so it can later feed both engines (V8 now, the JSC hybrid engine later) from the same surface. Tooling commands (`test`, `watch`, `bench`, `bundle`, `bpm install`) live in `bap_cli`, each a thin subcommand.

**Tech Stack:** `swc_ecma_parser`, `swc_ecma_transforms_typescript` (type strip), `swc_common` for source maps, `deno_core::ModuleLoader` trait, `glob`/`globset` for the test runner, `notify` for the watch loop, `clap` subcommands, `serde`/`serde_json`. Bundler is esbuild-binary-backed initially, `swc_bundler` later.

**Phase 3 scope guard:** No "full Bun alternative" ambitions. The pipeline supports running and testing real `.ts` files; the package manager wraps `npm`; the bundler is a convenience. Full Node API compat and the CJS loader are deferred to Phase 4.

---

### Task 1: `bap_ts` crate — type strip module loader

**Files:**
- Create: `crates/bap_ts/Cargo.toml`, `crates/bap_ts/src/lib.rs`, `crates/bap_ts/src/loader.rs`, `crates/bap_ts/src/strip.rs`, `crates/bap_ts/src/metadata.rs`

- [ ] **Step 1: Write `strip.rs`** — a function `strip_types(source: &str, specifier: &str) -> Result<(String, sourcemap::SourceMap), AnyError>` using `swc_ecma_parser` (parse TS) + `swc_ecma_transforms_typescript` (strip), emitting JS with an inline source map. Start with parse+strip only; no transforms for decorators/legacy syntax in this phase.
- [ ] **Step 2: Write `metadata.rs`** — define `ModuleTypeInfo { functions: Vec<FunctionInfo> }` with `FunctionInfo { name, params: Vec<TypeHint>, return_type: Option<TypeHint> }` where `TypeHint` is an enum (`Number`, `String`, `Boolean`, `BigInt`, `Unit(String)`, `Unknown`). Harvest from the TS AST during strip (single pass). This table is serialized as JSON next to the emitted JS for Phase 5.
- [ ] **Step 3: Write `loader.rs`** — implement `deno_core::ModuleLoader` for `TsModuleLoader`:
  - `resolve` — delegate to `FsModuleLoader` resolution semantics.
  - `load` — read the file, detect `.ts` by extension, call `strip_types`, return `ModuleSource { code: emitted_js, module_type: JavaScript, source_map: Some(inline) }`, and stash `ModuleTypeInfo` into the `OpState` side-table keyed by specifier.
- [ ] **Step 4: Wire into `bap_core`** — add `BapRuntime::with_ts_loader()` that swaps `FsModuleLoader` for `TsModuleLoader`, and a `type_info(specifier)` accessor.
- [ ] **Step 5: Tests** — `strip_types` on a sample with interfaces/annotations yields valid JS and a non-empty source map; `TsModuleLoader` round-trip loads a `.ts` file whose type-only imports vanish and whose value imports remain; a runtime test evaluates a `.ts` module with `console.log`.
- [ ] **Step 6: Verify and commit** `feat(ts): swc-backed type-strip module loader`.

### Task 2: `bap run` accepts `.ts`

**Files:**
- Modify: `crates/bap_cli/src/run.rs`, `crates/bap_cli/src/main.rs`

- [ ] **Step 1: Detect the loader by extension** — `.ts`/`.mts` files use `with_ts_loader()`; everything else keeps `FsModuleLoader`.
- [ ] **Step 2: Example** — `examples/hello.ts` with typed function + `console.log`, run via `bap run examples/hello.ts`.
- [ ] **Step 3: Test** — integration test spawning `bap run examples/hello.ts` asserting output and exit code.
- [ ] **Step 4: Verify and commit** `feat(cli): run typescript files`.

### Task 3: `bap test` runner

**Files:**
- Create: `crates/bap_cli/src/test.rs`, `crates/bap_ops/src/js/test.js` (bap:test module), modify `crates/bap_cli/src/cli.rs`, `crates/bap_cli/src/main.rs`

- [ ] **Step 1: Write the `bap:test` module** (`test.js`) exposing `test(name, fn)`, `expect(value)` (with `toBe`, `toEqual`, `toThrow`), and `run()` — collecting results into a shared array; the runner reads results after evaluation.
- [ ] **Step 2: Write `test.rs`** — glob `**/*.test.{ts,js}` under a root (default cwd, `--root` to override), skip `node_modules`/`target`, run each file in a fresh isolate (Phase 1 `BapRuntime::new`), collect pass/fail/failure-message, print TAP13 to stdout.
- [ ] **Step 3: CLI** — `bap test [root]` subcommand; exit code 1 if any test fails.
- [ ] **Step 4: Tests** — a fixture `tests/fixtures/ok.test.ts` (passing) and `fail.test.ts` (failing); spawn `bap test` on the fixture dir; assert TAP output contains the expected pass/fail counts and correct exit codes.
- [ ] **Step 5: Verify and commit** `feat(cli): test runner`.

### Task 4: `bpm install` wrapper

**Files:**
- Create: `crates/bap_cli/src/bpm.rs`, modify `crates/bap_cli/src/cli.rs`, `crates/bap_cli/src/main.rs`

- [ ] **Step 1: Write `bpm install`** — read `package.json`, spawn `npm install` in the project dir (configurable `packageManager` field: npm default, pnpm/bun later), stream output, propagate exit code. Also `bap install` alias.
- [ ] **Step 2: Tests** — create a temp project with a dependency, run `bpm install`, assert `node_modules` exists and exit code 0; offline-marked fixtures only (no real network dependency in CI).
- [ ] **Step 3: Verify and commit** `feat(cli): bpm install wrapper`.

### Task 5: `bap bundle`

**Files:**
- Create: `crates/bap_cli/src/bundle.rs`, modify `crates/bap_cli/src/cli.rs`, `crates/bap_cli/src/main.rs`

- [ ] **Step 1: Write `bundle <entry> [-o out.js]`** — initially shell out to the `esbuild` binary (resolved via `npm exec esbuild`), targeting ESM output; document that `swc_bundler` replaces it in a later milestone. Output single-file ESM with source map.
- [ ] **Step 2: Tests** — bundle the `examples/hello.ts` entry, run the output through `bap run`, assert identical stdout.
- [ ] **Step 3: Verify and commit** `feat(cli): bundle`.

### Task 6: `bap run --watch` hot reload

**Files:**
- Modify: `crates/bap_cli/src/run.rs`, `crates/bap_cli/src/cli.rs`

- [ ] **Step 1: Watch loop** — `--watch` runs the entry module once, then watches its module graph (`notify` watcher on the entry file plus its imports via the loader's specifier map), restarts the run on change, and prints a clear `restarting after change: <path>` line. The Phase 2 `ModuleSourceCache` is the invalidation trigger.
- [ ] **Step 2: Tests** — a fixture with two modules: touch the imported file, assert the runner restarts and re-prints output with the new value (spawned via `--watch` in an integration test with a bounded timeout).
- [ ] **Step 3: Verify and commit** `feat(cli): hot reload with watch`.

### Task 7: `bap bench` minimal runner

**Files:**
- Create: `crates/bap_cli/src/bench.rs`, modify `crates/bap_cli/src/cli.rs`, `crates/bap_cli/src/main.rs`

- [ ] **Step 1: Minimal bench** — `bap bench <file>` runs the module N times (default 100, `--iterations` to override), samples per-iteration wall time, and prints mean/p50/p99/max. The full histogram instrumentation lands in Phase 6; this task ships the command surface the Phase 6 `bap bench` builds on.
- [ ] **Step 2: Tests** — a fixture that sleeps/loops a known amount; assert the printed sample count equals `--iterations` and the output shape is parseable.
- [ ] **Step 3: Verify and commit** `feat(cli): minimal bench runner`.

---

## Verification

`cargo fmt --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`, then manual: `bap run examples/hello.ts`, `bap test tests/fixtures`, `bpm install`, `bap bundle examples/hello.ts -o /tmp/hello.bundle.js && bap run /tmp/hello.bundle.js`, `bap run --watch examples/hello.ts`, `bap bench bench/loop.ts`.

## Risks

- `deno_core::ModuleLoader` trait shape may differ across versions — the `FsModuleLoader` implementation in the vendored crate is the reference; copy its pattern.
- SWC crate versions (parser/transforms/common) must be pinned together; `swc_ecma_transforms_typescript` output must remain parseable by V8 — verified by the runtime test in Task 1 Step 5.
- Decorators/parameter properties require `transforms_typescript` config beyond plain strip; Phase 3 ships plain type-strip only and records unsupported syntax as a clear error.
- `notify` on macOS fsevents can fire duplicate events; the watch loop debounces and keyed by file mtime before restarting.

## Self-review

- Spec: Phase 3 roadmap items covered (native TS pipeline via swc-as-library but owned bytecode path, type metadata for JIT, single `bap` binary with runner/REPL/bpm/test/bundle, plus the hot-reload and integrated test/bench runner DX items the spec names).
- No placeholders: concrete files, ops, and assertions per task; the esbuild dependency and SWC config are explicit decisions, not deferred TODOs.
- Type consistency: `ModuleTypeInfo`/`FunctionInfo`/`TypeHint` types are defined once in Task 1 and reused unchanged by the loader, the `bap_core` accessor, and (later) Phase 5; the Phase 6 `bap bench` command extends the `bap_cli::bench` surface from Task 7.