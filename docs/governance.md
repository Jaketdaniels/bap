# Governance

## Human-first review

Core engine, scheduler, and GC-adjacent paths are hand-written and hand-reviewed.
AI assistance is confined to tests, docs, and non-critical utilities. Every patch
to `bap_core` requires an approving maintainer review. This is public policy:
early adopters must be able to trust the runtime.

## Unsafe and AI policy

- Unsafe Rust is capped at <=5,000 lines in the ~500 kLOC codebase, isolated in
  the `bap_unsafe` crate, each block tagged with a `// SAFETY:` reason.
- Miri and ASAN run in CI on every change to unsafe modules.
- Bulk AI-driven rewrites of core subsystems are disallowed.

See `docs/unsafe-policy.md` for details. Phase 1 ships with zero unsafe Rust.
