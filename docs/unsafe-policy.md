# Unsafe Rust

The proof of concept uses `#![forbid(unsafe_code)]` in every first-party crate.
There is no first-party unsafe Rust today.

If a measured feature eventually requires unsafe code, its invariants, tests, and
appropriate Miri or sanitizer coverage will be designed with that feature. Bap does
not carry a speculative unsafe-code policy or claim CI checks that do not run.
