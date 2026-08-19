# Unsafe Policy

Phase 1 has zero unsafe Rust: every crate declares `#![forbid(unsafe_code)]`.

## Rules (binding from Phase 4 onward)

1. All `unsafe` lives in the `bap_unsafe` crate only. `bap_core`, `bap_math`,
   `bap_ops`, `bap_metrics`, `bap_cli` keep `#![forbid(unsafe_code)]`.
2. Unsafe lines are capped at 5,000 across the codebase.
3. Every `unsafe {}` block carries a `// SAFETY:` invariant statement.
4. CI runs `cargo miri test` (nightly toolchain) and an ASAN build on the
   unsafe crate for every change touching it.
5. AI-generated patches to unsafe code are rejected at review.
