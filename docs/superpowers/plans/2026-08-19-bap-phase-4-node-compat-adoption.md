# Bap Phase 4: Node Compatibility & Adoption Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. `deno_node` version alignment and SWC commonjs output are verified at build time.

**Goal:** Make Bap a drop-in runtime option for existing Node and Bun apps: ESM-first with a compatibility CJS loader and a Node-core polyfill shim, an incremental adoption pathway (run test suites on the Node shim, then flip per-service runtime configs while keeping `package.json` and tooling unchanged), and overlapping 18-month LTS lines with strict semver and a published deprecation matrix.

**Architecture:** Extend the Phase 3 module loader with a CJS loader that wraps CommonJS modules so `require`/`module.exports` interop with ESM. A `node:` resolution layer maps `node:fs`, `node:path`, `node:util`, `node:events`, `node:stream`, and `node:buffer` to Bap ops and JS shims, and installs `process`, `Buffer`, and `globalThis` basics. Adoption tooling lives in `bap_cli`: a `bap.json` per-service runtime config, `bap doctor` reporting shim gaps, and `bap report compat`. Release process: `cargo-semver-checks` in CI, `docs/deprecation-matrix.md`, LTS lines aligned with major Node LTS cycles. This phase is what the spec's "Standard module surface" and "Adoption Pathway" sections describe.

**Tech Stack:** `swc_ecma_transforms_commonjs` (CJS compile) or a hand-rolled `__bap_require` shim, `deno_node` crate (only if aligned with the pinned `deno_core`; in-house minimal shim otherwise), `semver` crate, `cargo-semver-checks` (CI), `serde`/`serde_json` for `bap.json`. Reuses Phase 2 fs/path ops under the `node:` modules.

**Phase 4 scope guard:** Compat is bounded to what real Node/Bun test suites need: CJS loading, the core `node:` modules, and `process`/`Buffer`/`globalThis`. This is not a full Node reimplementation — unsupported modules surface a clear "not implemented on Bap" error, and npm compatibility is measured and reported (target ≥99% of the sample corpus), never assumed.

---

### Task 1: CJS loader

**Files:**
- Create: `crates/bap_ts/src/cjs.rs`, modify `crates/bap_ts/src/loader.rs`, `crates/bap_ts/src/lib.rs`

- [ ] **Step 1: Detect CJS** — a module is CJS if it has a `.cjs` extension, or the nearest `package.json` lacks `"type": "module"`, or the source uses `require`/`module`/`exports` without ESM import/export syntax. Return a `ModuleKind` alongside the stripped JS.
- [ ] **Step 2: Compile CJS** — wrap the source in a function invoked with `(module, exports, require, __dirname, __filename)`; `require` resolves relative paths and `node:` specifiers through the same module loader; support a basic `__esModule`-aware interop so `import foo from "./x.cjs"` yields `module.exports` as default and named imports work best-effort.
- [ ] **Step 3: Tests** — a `.cjs` fixture exporting via `module.exports` is importable from an ESM `.ts` file under `bap run`; a require cycle of two CJS files terminates and produces correct values; `require("node:path")` inside CJS resolves through the shim (Task 2).
- [ ] **Step 4: Verify and commit** `feat(ts): commonjs loader with esm interop`.

### Task 2: Node-core polyfill shim

**Files:**
- Create: `crates/bap_ops/src/js/node/path.js`, `crates/bap_ops/src/js/node/fs.js`, `crates/bap_ops/src/js/node/util.js`, `crates/bap_ops/src/js/node/events.js`, `crates/bap_ops/src/js/node/process.js`, modify `crates/bap_ops/src/lib.rs`, `crates/bap_ts/src/loader.rs`

- [ ] **Step 1: `node:` resolver** — the module loader maps `node:<name>` specifiers to the shim modules. Prefer `deno_node` if its API aligns with the pinned `deno_core`; otherwise implement in-house shims backed by Phase 2 ops (fs/path) and pure JS for util/events.
- [ ] **Step 2: Globals** — install `process` (`env`, `argv`, `cwd`, `exit`, `version`), `Buffer` (over Phase 2 bytes), and `globalThis` compatibility via the extension's `global_object_middleware`.
- [ ] **Step 3: Tests** — a Node-style script using `require("path").join`, `require("fs").readFileSync` (via the sync-op or a wrapped async), `process.env.HOME`, and `Buffer.from` runs under `bap run`; a `.ts` module imports from `node:path`/`node:events`.
- [ ] **Step 4: Verify and commit** `feat(ops): node-core polyfill shim`.

### Task 3: Adoption pathway tooling

**Files:**
- Create: `crates/bap_cli/src/doctor.rs`, `docs/adoption.md`, modify `crates/bap_cli/src/cli.rs`, `crates/bap_cli/src/main.rs`, `crates/bap_cli/src/run.rs`

- [ ] **Step 1: `bap.json`** — per-service runtime config `{ "runtime": "bap", "moduleFormat": "esm"|"cjs", "aliases": {} }`; `bap run` and `bap test` read it from the project dir. `package.json` stays untouched.
- [ ] **Step 2: `bap doctor`** — scan the project's imports/requires, check each against the shim surface, and print a report of supported, missing, and "not implemented on Bap" modules plus the measured compat ratio.
- [ ] **Step 3: Migration doc** — `docs/adoption.md` captures the spec's pathway: run existing npm test suites under `bap test` → fix shim gaps with `bap doctor` → flip per-service runtime configs → keep tooling unchanged.
- [ ] **Step 4: Tests** — a fixture project with `bap.json` runs under `bap run`; `bap doctor` reports the supported ratio for a fixture with `node:path` + `node:crypto` (where crypto is missing) and exit code 0 with the gap listed.
- [ ] **Step 5: Verify and commit** `feat(cli): bap.json config and doctor`.

### Task 4: LTS & semver

**Files:**
- Create: `docs/release.md`, `docs/deprecation-matrix.md`, modify `.github/workflows/ci.yml`, `Cargo.toml`

- [ ] **Step 1: Release process** — `docs/release.md`: 18-month LTS lines aligned with major Node LTS cycles, overlapping lines so libraries can support multiple Bap versions plus Node/Bun/Deno in parallel; strict semver, no breaking changes outside majors.
- [ ] **Step 2: Deprecation matrix** — `docs/deprecation-matrix.md` is the published, versioned table of deprecated APIs and their removal versions.
- [ ] **Step 3: CI semver gate** — add a `cargo-semver-checks` job on the lib crates; Conventional Commits drive version decisions.
- [ ] **Step 4: Verify and commit** `chore(ci): semver checks and release docs`.

### Task 5: npm compatibility harness (target ≥99%)

**Files:**
- Create: `bench/compat/corpus.json`, `bench/compat/run.sh`, `crates/bap_cli/src/report.rs`, modify `crates/bap_cli/src/cli.rs`, `.github/workflows/ci.yml`

- [ ] **Step 1: Corpus** — `bench/compat/corpus.json` lists representative npm packages (utility, framework-adjacent, CLI) pinned to versions; each package's own test suite runs under `bap test` via the Node shim.
- [ ] **Step 2: Runner** — `run.sh` installs each corpus package with `bpm`, runs its test suite, and records pass/fail per package.
- [ ] **Step 3: `bap report compat`** — prints the compatibility table (package, tests run, pass/fail, ratio) and the aggregate npm-compatibility percentage against the spec's ≥99% target.
- [ ] **Step 4: CI** — the compat job reports the ratio; gate on ≥99% once the corpus is stable and green (report-only until then, to avoid a flaky gate).
- [ ] **Step 5: Verify and commit** `feat(cli): npm compatibility report`.

---

## Verification

`cargo fmt --check` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`, then manual: `bap run examples/node-style.js`, `bap doctor`, `bap report compat`, and the corpus run under `bench/compat/run.sh`.

## Risks

- `deno_node` may not align with the pinned `deno_core`; the in-house shim is the designated fallback (Task 2 Step 1).
- CJS/ESM interop edge cases (default vs named exports, cycles, dynamic `require`) are unbounded; the plan ships a documented best-effort interop and treats full Node require semantics as out of scope.
- The npm corpus is version-pinned; compatibility drift across releases is tracked by the report, not assumed.

## Self-review

- Spec: "Standard module surface" (ESM-first, CJS loader, Node-core shim, package manager + test runner + bundler in one toolchain) and "Adoption Pathway" (test suites on the Node shim, per-service config flips, unchanged `package.json`/tooling, overlapping 18-month LTS lines) are fully covered across Tasks 1–5.
- No placeholders: concrete modules, fixtures, and commands per task; the corpus selection is an explicit pinned list.
- Type consistency: `ModuleKind` and the `node:` resolver are defined in Task 1 and reused by Tasks 2–3; `bap.json` shape is fixed in Task 3 and consumed by `bap run`/`bap test`/`bap doctor`.