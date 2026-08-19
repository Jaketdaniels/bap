# Contributing

Bap follows a manual-first policy for core engine code.

- `crates/bap_core` and `crates/bap_math` (future) are **human-review only**:
  every patch requires an approving review by a maintainer. Bulk AI-driven
  rewrites of core subsystems are disallowed.
- AI assistance is welcome in tests, docs, examples, and non-critical utilities
  (`bap_ops` glue, `bap_metrics`, tooling).
- Rust rules: `#![forbid(unsafe_code)]` unless the crate is `bap_unsafe`; explicit
  types; `cargo fmt`, `cargo clippy -- -D warnings`, and tests must pass.
- Commits use Conventional Commits (`feat`, `fix`, `chore`, `test`, `refactor`).
