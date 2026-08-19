# Contributing

Bap is currently a greenfield proof of concept. Contributions should favour clear,
working slices over process or policy machinery.

- Review the actual diff and test evidence regardless of whether a human or an AI
  helped author it.
- Keep JSDoc and Rust documentation comments in plain, standard English and limit
  them to two lines unless a public invariant genuinely needs more detail.
- Keep first-party Rust free of `unsafe`; run `cargo fmt`, clippy with warnings
  denied, and the relevant tests before committing.
- Make focused, meaningful commits using Conventional Commit prefixes.
