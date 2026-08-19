# Proof-Carrying Numerics Design

## Objective

Bap's next milestone delivers two coherent differentiators: error-bounded numeric
primitives implemented in Rust and a TypeScript compilation path that uses real
type-checker evidence to specialize safe constant expressions. The milestone must
remain honest about what is implemented: specialization is an ahead-of-time proof
pass feeding V8, not a custom JIT tier.

## Scope

The milestone includes direct `.ts`/`.mts` execution, cached TypeScript proof
artifacts, interval arithmetic, checked `int32`/`int64`, `Fixed<18,4>`, branded
TypeScript APIs, batched numeric operations, source maps, and reproducible tests and
benchmarks.

Compatibility shims, native addons, package management, general-purpose bundling,
adaptive orchestration, schemas, GPU dispatch, JavaScriptCore, and Next.js latency
claims remain separate milestones. The README and roadmap must distinguish current
features from targets.

## Architecture

### Fast TypeScript path

`bap_ts` implements the `deno_core::ModuleLoader` boundary. It resolves local
`.js`, `.mjs`, `.ts`, and `.mts` modules, transpiles TypeScript with a mature
open-source parser/transpiler, preserves inline source maps, and returns JavaScript
to V8. JavaScript remains a zero-transform path.

Transpilation is content-addressed by a digest of source bytes, canonical specifier,
compiler version, and options. Cached artifacts contain emitted JavaScript, source
maps, and optional proof metadata. Cache corruption or version mismatch is a miss,
never executable input.

### Proof pass

The proof backend invokes the installed TypeScript compiler API as a bounded build
step. It is not started on cache hits and is not required for ordinary unannotated
TypeScript. The backend type-checks a module and emits JSON records only for
supported, closed expressions:

```json
{
  "sourceHash": "...",
  "typescriptVersion": "...",
  "constants": [
    { "start": 120, "end": 135, "kind": "unit", "unit": "metre", "value": 12 }
  ]
}
```

The Rust loader validates the source hash, compiler version, byte ranges, expression
kind, and finite numeric value before applying a substitution. Unsupported or
unproven expressions remain unchanged. Type errors fail compilation with the
original TypeScript diagnostic and source location. A `--no-proofs` option disables
the pass without disabling TypeScript execution.

The initial proof vocabulary is intentionally small: numeric literals, unary signs,
parentheses, and `+`, `-`, `*`, and `/` over compile-time constants branded as
`Unit<Name>`. Addition and subtraction require identical units; multiplication and
division are folded only for dimensionless operands in this milestone. This gives
sound behavior without claiming support for the entire TypeScript type system.

### Numeric core

`bap_math` contains no V8 dependencies and forbids unsafe code.

- `Interval` stores finite or infinite `f64` bounds and rejects NaN or `lo > hi`.
  Arithmetic widens results with outward rounding. Division by an interval spanning
  zero returns `NumericError::Discontinuous`; it never invents a single finite range.
- `CheckedI32` and `CheckedI64` wrap Rust checked operations and return
  `NumericError::Overflow` rather than wrapping.
- `Fixed<const PRECISION: u32, const SCALE: u32>` uses an `i128` coefficient.
  `Fixed<18,4>` means 18 total decimal digits, four after the decimal point, and
  therefore at most 14 before it. Parsing is decimal-string based; floating-point
  construction is explicitly lossy and excluded from the primary API.

Scalar operations prioritize correctness. Batch interval and fixed-point operations
cross the V8/Rust boundary once per array. SIMD is retained only when a checked
benchmark demonstrates improvement without changing results.

### JavaScript and TypeScript surface

The extension exposes frozen constructors under `bap.numeric`:

```ts
type Interval = Readonly<{ readonly lo: number; readonly hi: number; readonly __brand: "bap.Interval" }>;
type Fixed18_4 = Readonly<{ readonly coefficient: bigint; readonly __brand: "bap.Fixed18_4" }>;
type CheckedI64 = Readonly<{ readonly value: bigint; readonly __brand: "bap.CheckedI64" }>;

bap.numeric.interval(lo, hi): Interval;
bap.numeric.intervalAdd(a, b): Interval;
bap.numeric.intervalBatchAdd(a, b): readonly Interval[];
bap.numeric.fixed18_4(decimal: string): Fixed18_4;
bap.numeric.fixedAdd(a, b): Fixed18_4;
bap.numeric.i64(value: bigint): CheckedI64;
bap.numeric.i64Add(a, b): CheckedI64;
```

Brand fields are created internally and cannot substitute for runtime validation;
every op validates incoming values. Numeric errors become JavaScript `RangeError`s
with stable codes such as `BAP_NUMERIC_OVERFLOW`.

## Data Flow

1. The CLI resolves the entry module and selects `TsModuleLoader` whenever the
   module graph contains TypeScript.
2. The loader checks the artifact cache.
3. On a miss it transpiles, optionally requests a TypeScript proof manifest,
   validates any proofs, applies safe substitutions, and atomically stores the
   artifact.
4. V8 executes emitted JavaScript and calls batched numeric ops through the existing
   `bap_ops` extension.
5. Existing metrics record cache hit/miss, transpile time, proof time, numeric op
   latency, and batch size without recording source text or values.

## Failure and Trust Model

- Invalid numeric inputs fail closed with stable error codes.
- Failed or unavailable proof tooling falls back to unoptimized transpilation only
  when the module has no type errors; `--require-proofs` makes failure fatal.
- Artifacts are written to a temporary sibling and atomically renamed.
- The cache never follows symlinks outside its configured root.
- All new Rust crates use `#![forbid(unsafe_code)]`; CI runs formatting, clippy,
  unit/integration tests, and Miri on `bap_math`.
- Generated proof manifests are inspectable with `bap inspect proofs <file>`.

## Verification Gates

- Property tests show sampled exact results remain inside every interval result,
  including infinities, signed zero, cancellation, and overflow-adjacent values.
- Fixed-point parsing and arithmetic are checked at all precision boundaries; no
  operation wraps or silently rounds beyond the documented half-even rule.
- A deliberately mismatched unit fails in the TypeScript checker; a matching-unit
  constant is proven, folded, and visible through `bap inspect proofs`.
- Editing source invalidates its artifact; corrupt artifacts are ignored and rebuilt.
- Direct `.ts` execution, mixed JS/TS imports, source-mapped diagnostics, and
  `--no-proofs` are integration tested.
- Benchmarks publish cold compile, cache-hit startup, scalar numeric, and batch
  numeric results. Targets are reported as observations, not guarantees.

## Delivery Phases

1. **TypeScript foundation:** direct execution, source maps, artifact cache, and
   current-state documentation.
2. **Numeric fidelity:** Rust primitives, property tests, Miri, JavaScript bindings,
   and batch operations.
3. **Proof integration:** TypeScript checker backend, proof validation,
   specialization, inspection command, and end-to-end benchmarks.

Each phase leaves the repository buildable and releasable. Phase status changes only
after its verification commands pass.
