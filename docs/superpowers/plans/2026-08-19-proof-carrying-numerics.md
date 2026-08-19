# Proof-Carrying Numerics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver direct TypeScript execution, audited numeric primitives, and a cached TypeScript-checker proof pass that safely specializes unit-aware constants before V8 execution.

**Architecture:** `bap_ts` uses `deno_ast` for transpilation and a validated content-addressed cache; `bap_math` provides V8-free interval, checked-integer, and fixed-point semantics; `tools/ts-proof` uses exact-pinned TypeScript 7.0.2 to emit evidence that Rust independently validates before applying source-width-preserving edits. Ordinary TypeScript execution never requires Node.

**Tech Stack:** Rust 1.96, `deno_core` 0.410, `deno_ast` 0.53.3, `serde`, `sha2`, `proptest`, Node 22, TypeScript 7.0.2.

---

### Task 1: TypeScript transpiler and validated artifact cache

**Files:**
- Create: `crates/bap_ts/Cargo.toml`
- Create: `crates/bap_ts/src/lib.rs`
- Create: `crates/bap_ts/src/transpile.rs`
- Create: `crates/bap_ts/src/cache.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Write failing transpilation tests**

```rust
#[test]
fn strips_types_and_preserves_value_imports() {
    let output = transpile("file:///entry.ts", "import { x } from './x.ts'; const n: number = x;").unwrap();
    assert!(!output.code.contains(": number"));
    assert!(output.code.contains("from './x.ts'"));
    assert!(output.source_map.contains("entry.ts"));
}
```

- [ ] **Step 2: Verify the test fails because `bap_ts` does not exist**

Run: `cargo test -p bap_ts transpile::tests -- --nocapture`

Expected: package-not-found failure.

- [ ] **Step 3: Add `bap_ts` and implement `deno_ast` transpilation**

Use exact `deno_ast = { version = "=0.53.3", features = ["transpiling"] }`. Parse `.ts`/`.mts` with `parse_module`, emit ESM using `TranspileOptions::default()`, and request a separate source map with inline sources. JavaScript is returned byte-for-byte.

- [ ] **Step 4: Write cache tests before cache implementation**

```rust
#[test]
fn corrupt_artifact_is_rebuilt() {
    let cache = Cache::new(tempdir().unwrap().path()).unwrap();
    let first = cache.get_or_transpile(&fixture()).unwrap();
    std::fs::write(first.path, b"not json").unwrap();
    let second = cache.get_or_transpile(&fixture()).unwrap();
    assert_eq!(second.code, first.code);
    assert!(!second.cache_hit);
}
```

- [ ] **Step 5: Verify the cache test fails, then implement `ArtifactV1`**

Key artifacts by SHA-256 over a length-prefixed schema tag, canonical specifier, source bytes, compiler version, and stable owned options fingerprint. Validate schema, key, source hash, emitted-code hash, source-map JSON, and a 64 MiB size limit on every hit. Write with a unique sibling plus `sync_all` and atomic rename; derived filenames contain lowercase hex only.

- [ ] **Step 6: Run the slice tests**

Run: `cargo test -p bap_ts`

Expected: all transpiler and cache tests pass.

### Task 2: Mixed-graph TypeScript module loading

**Files:**
- Create: `crates/bap_ts/src/loader.rs`
- Create: `crates/bap_ts/tests/loader.rs`
- Modify: `crates/bap_core/Cargo.toml`
- Modify: `crates/bap_core/src/lib.rs`
- Modify: `crates/bap_cli/Cargo.toml`
- Modify: `crates/bap_cli/src/run.rs`
- Modify: `crates/bap_cli/tests/run_script.rs`
- Create: `crates/bap_cli/tests/fixtures/ts/entry.ts`
- Create: `crates/bap_cli/tests/fixtures/ts/dep.ts`
- Create: `crates/bap_cli/tests/fixtures/ts/mixed.js`
- Create: `examples/hello.ts`

- [ ] **Step 1: Write failing loader and CLI tests**

```rust
#[test]
fn js_entry_imports_typescript_dependency() {
    let out = command().args(["run", fixture("ts/mixed.js")]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "typed 42\n");
}
```

- [ ] **Step 2: Confirm RED**

Run: `cargo test -p bap_cli --test run_script js_entry_imports_typescript_dependency -- --exact`

Expected: V8 rejects TypeScript syntax from the dependency.

- [ ] **Step 3: Implement `TsModuleLoader` and reusable runtime constructor**

Delegate resolution and non-TypeScript loads to `FsModuleLoader`. Transpile TypeScript, append its source map as a base64 data URL, and return `ModuleSource::new(ModuleType::JavaScript, ...)`. Add `BapRuntime::with_args_and_loader`; install `TsModuleLoader` for every CLI file run so mixed graphs work.

- [ ] **Step 4: Verify TypeScript behavior**

Run: `cargo test -p bap_ts --test loader && cargo test -p bap_cli --test run_script`

Expected: `.ts`, `.mts`, and JS-to-TS graphs execute; existing JavaScript tests remain green.

### Task 3: Interval arithmetic

**Files:**
- Create: `crates/bap_math/Cargo.toml`
- Create: `crates/bap_math/src/lib.rs`
- Create: `crates/bap_math/src/error.rs`
- Create: `crates/bap_math/src/interval.rs`
- Create: `crates/bap_math/tests/interval_props.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Write constructor and edge-case tests**

```rust
#[test]
fn division_rejects_zero_spanning_interval() {
    let a = Interval::new(1.0, 2.0).unwrap();
    let zero = Interval::new(-0.0, 1.0).unwrap();
    assert_eq!(a.checked_div(zero), Err(NumericError::Discontinuous));
}
```

- [ ] **Step 2: Confirm RED**

Run: `cargo test -p bap_math interval -- --nocapture`

Expected: package-not-found or missing-type failure.

- [ ] **Step 3: Implement interval semantics**

Reject NaN and reversed bounds; accept infinities. Outward-round finite endpoints with `next_down`/`next_up`; keep overflow infinities. Return `Indeterminate` for opposing-infinity addition/subtraction, define `0 * infinity` as zero for interval corner evaluation, and return `Discontinuous` for any divisor containing signed zero.

- [ ] **Step 4: Add property tests using exact rational oracles**

```rust
proptest! {
    #[test]
    fn addition_contains_sample(a in finite_interval(), b in finite_interval()) {
        let result = a.checked_add(b).unwrap();
        prop_assert!(result.contains(a.lo() + b.lo()));
        prop_assert!(result.contains(a.hi() + b.hi()));
    }
}
```

- [ ] **Step 5: Run interval gates**

Run: `PROPTEST_CASES=256 cargo test -p bap_math interval`

Expected: targeted and property tests pass.

### Task 4: Checked integers and `Fixed<18,4>`

**Files:**
- Create: `crates/bap_math/src/checked.rs`
- Create: `crates/bap_math/src/fixed.rs`
- Create: `crates/bap_math/tests/checked_props.rs`
- Create: `crates/bap_math/tests/fixed_props.rs`

- [ ] **Step 1: Write failing boundary and decimal tests**

```rust
#[test]
fn fixed_18_4_uses_total_precision_and_half_even() {
    assert_eq!("99999999999999.9999".parse::<Fixed18_4>().unwrap().to_string(), "99999999999999.9999");
    assert!("100000000000000.0000".parse::<Fixed18_4>().is_err());
    assert_eq!("1.23445".parse::<Fixed18_4>().unwrap().to_string(), "1.2344");
    assert_eq!("1.23455".parse::<Fixed18_4>().unwrap().to_string(), "1.2346");
}
```

- [ ] **Step 2: Confirm RED**

Run: `cargo test -p bap_math fixed -- --nocapture`

Expected: missing-type failure.

- [ ] **Step 3: Implement checked wrappers and fixed point**

Use private-field `CheckedI32`/`CheckedI64` wrappers with checked add/sub/mul/div/neg. Implement `Fixed<P,S>` over `i128`, allow `1 <= P <= 18` and `S <= P`, parse strict decimal strings, round excess fractional digits half-to-even, enforce the coefficient range, and never expose a primary `f64` constructor.

- [ ] **Step 4: Add arithmetic properties and run gates**

Run: `PROPTEST_CASES=256 cargo test -p bap_math`

Expected: no wrapping, exact boundary enforcement, canonical display, and half-even arithmetic pass.

### Task 5: Branded scalar and batch numeric APIs

**Files:**
- Create: `crates/bap_ops/src/numeric.rs`
- Create: `crates/bap_ops/src/js/numeric.js`
- Create: `crates/bap_ops/src/js/numeric.d.ts`
- Create: `crates/bap_core/tests/numeric.rs`
- Create: `crates/bap_ops/tests/types/numeric.ts`
- Create: `examples/numeric.ts`
- Modify: `crates/bap_ops/Cargo.toml`
- Modify: `crates/bap_ops/src/lib.rs`

- [ ] **Step 1: Write failing scalar, branding, and error tests**

```rust
#[tokio::test]
async fn forged_interval_is_rejected() {
    let err = run("bap.numeric.intervalAdd({lo:0,hi:1}, bap.numeric.interval(1,2))").await.unwrap_err();
    assert!(err.to_string().contains("BAP_NUMERIC_INVALID_ARGUMENT"));
}
```

- [ ] **Step 2: Confirm RED**

Run: `cargo test -p bap_core --test numeric -- --nocapture`

Expected: `bap.numeric` is undefined.

- [ ] **Step 3: Implement scalar ops and frozen runtime branding**

Map numeric-domain failures to `RangeError` with stable `code`; validation failures are `TypeError`. Build result objects in JavaScript, assign non-enumerable brands, track authentic objects in private `WeakSet`s, and freeze objects and namespace. Send raw validated fields through Rust ops.

- [ ] **Step 4: Write failing batch tests, then implement one-crossing batches**

Test equal lengths, maximum 1,000,000 elements, atomic overflow failure, frozen result arrays, and one recorded Rust op. Use one typed scratch buffer per call, compute into a Rust temporary, and copy results only after full success. SIMD remains excluded until it beats this scalar baseline with identical results.

- [ ] **Step 5: Verify JavaScript and TypeScript surfaces**

Run: `cargo test -p bap_ops numeric && cargo test -p bap_core --test numeric && cargo run -p bap_cli -- run examples/numeric.ts`

Expected: scalar and batch APIs work, forged values fail, and errors retain stable codes.

### Task 6: TypeScript proof producer

**Files:**
- Create: `tools/ts-proof/package.json`
- Create: `tools/ts-proof/package-lock.json`
- Create: `tools/ts-proof/proof.mjs`
- Create: `tools/ts-proof/bap-units.d.ts`
- Create: `tools/ts-proof/test/proof.test.mjs`
- Create: `tools/ts-proof/manifest.schema.json`

- [ ] **Step 1: Write Node tests for the trusted helper vocabulary**

```javascript
test("rejects mismatched units with TS2345", async () => {
  const result = await prove(`
    const length = Bap.numeric.unit("metre", 2);
    const time = Bap.numeric.unit("second", 3);
    export const bad = Bap.numeric.unitAdd(length, time);
  `);
  assert.equal(result.diagnostics[0].code, 2345);
});
```

- [ ] **Step 2: Confirm RED and pin the compiler**

Run: `npm --prefix tools/ts-proof test`

Expected: missing package/script failure. Add only exact `typescript = 7.0.2`, generate and commit the lockfile.

- [ ] **Step 3: Implement the bounded proof subset**

Create a strict `ts.createProgram`, include the bundled ambient declaration, run `getPreEmitDiagnostics`, and resolve helper symbols back to that declaration. Prove only top-level consts containing numeric literals, unary signs, parentheses, scalar arithmetic, const references, and trusted `unit`, `unitAdd`, `unitSub`, `unitMul`, or `unitDiv` calls. Add/sub require identical units; mul/div require `dimensionless`. Reject cycles, shadowing, division by zero, nonfinite values, depth above 64, more than 256 evidence nodes per expression, or more than 1,024 edits.

- [ ] **Step 4: Emit a versioned evidence manifest and run tests**

Store canonical specifier, SHA-256 source and dependency hashes, UTF-8 byte ranges, slice hashes, closed evidence nodes, unit name, and exact `f64::to_bits()` hex. Convert TypeScript UTF-16 offsets to UTF-8 byte offsets explicitly.

Run: `npm --prefix tools/ts-proof ci && npm --prefix tools/ts-proof test`

Expected: matching units prove; mismatches retain TypeScript diagnostics; Unicode, negative zero, cycles, limits, and spoofed helpers are covered.

### Task 7: Rust proof validation, specialization, and CLI controls

**Files:**
- Create: `crates/bap_ts/src/proof/mod.rs`
- Create: `crates/bap_ts/src/proof/backend.rs`
- Create: `crates/bap_ts/src/proof/manifest.rs`
- Create: `crates/bap_ts/src/proof/specialize.rs`
- Create: `crates/bap_cli/src/inspect.rs`
- Create: `crates/bap_cli/tests/proofs.rs`
- Create: `crates/bap_cli/tests/fixtures/proofs/unit_fold.ts`
- Create: `crates/bap_cli/tests/fixtures/proofs/unit_mismatch.ts`
- Modify: `crates/bap_ts/src/cache.rs`
- Modify: `crates/bap_ts/src/loader.rs`
- Modify: `crates/bap_cli/src/cli.rs`
- Modify: `crates/bap_cli/src/main.rs`
- Modify: `crates/bap_cli/src/run.rs`

- [ ] **Step 1: Write manifest rejection tests**

```rust
#[test]
fn rejects_overlapping_edits() {
    let manifest = manifest_with_ranges([(10, 20), (19, 30)]);
    assert_eq!(manifest.validate(&source()).unwrap_err().code(), "BAP_PROOF_INVALID_MANIFEST");
}
```

- [ ] **Step 2: Confirm RED**

Run: `cargo test -p bap_ts proof:: -- --nocapture`

Expected: proof modules do not exist.

- [ ] **Step 3: Implement independent Rust validation**

Validate protocol/compiler versions, canonical specifier, source/dependency/options hashes, sorted nonoverlapping UTF-8 ranges, slice hashes, trusted AST helper nodes, evidence structure, unit rules, depth/count limits, finite values, and exact recomputed bits. Apply edits highest-offset-first with ASCII padding so byte length and source-map positions remain unchanged.

- [ ] **Step 4: Implement bounded backend and proof modes**

Use request/output files, poll `Child::try_wait`, kill after 5 seconds, and reject output over 16 MiB. `--no-proofs` never starts Node; normal mode starts it only for a proof candidate on a cache miss; `--require-proofs` fails recognized but unproven candidates. Cache hits validate dependency hashes and never launch Node.

- [ ] **Step 5: Add `bap inspect proofs` and integration tests**

Run: `cargo test -p bap_cli --test proofs`

Expected: proven addition prints `15`; inspection reports `unit.add`; mismatch exits with `TS2345`; unavailable backend plus `--no-proofs` still runs; source edits and corruption rebuild; cache hits do not relaunch Node.

### Task 8: POC quality gates and accurate project documentation

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `.gitignore`
- Modify: `README.md`
- Modify: `docs/roadmap.md`
- Modify: `docs/superpowers/plans/2026-08-19-bap-phase-3-ts-pipeline.md`
- Modify: `docs/superpowers/plans/2026-08-19-bap-phase-5-numerics-orchestration.md`
- Modify: `docs/superpowers/plans/2026-08-19-bap-phase-6-tail-latency.md`
- Modify: `Cargo.toml`

- [ ] **Step 1: Keep CI strategic**

Install Node dependencies with `npm --prefix tools/ts-proof ci`, then run Node tests, formatting, clippy with warnings denied, and the Rust test suite. Do not add speculative Miri, sanitizer, release, or approval machinery during the proof-of-concept phase.

- [ ] **Step 2: Correct claims and roadmap state**

Describe implemented TypeScript and numerics precisely; label proof specialization AOT; report cold-start observations as warm-cache versus first invocation; replace synthetic Next.js target language with a future real-application gate; add native-addon translation and representative-corpus requirements to Phase 4. Correct the workspace repository URL or omit it until a real upstream exists. Keep JSDoc and Rust documentation comments in plain English and no longer than two lines unless an invariant requires more detail.

- [ ] **Step 3: Run final verification**

```sh
npm --prefix tools/ts-proof ci
npm --prefix tools/ts-proof test
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
PROPTEST_CASES=256 cargo test --workspace
cargo build --release -p bap_cli
target/release/bap run examples/hello.ts
target/release/bap run examples/numeric.ts
target/release/bap inspect proofs --json crates/bap_cli/tests/fixtures/proofs/unit_fold.ts
git diff --check
```

Expected: every command exits zero; tests cover cache corruption, arithmetic boundaries, proof spoofing, and mixed module graphs; documentation contains no unqualified unimplemented claims.
