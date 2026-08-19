# Project Practice

Bap is a proof of concept. Its audit trail is the main-branch commit history, and
each commit should describe one meaningful, tested change.

The current quality gate is deliberately small: format the code, deny clippy
warnings, and run relevant tests. Authorship does not replace inspection; assess
the diff and evidence whether the work was written by a person, an AI, or both.

Formal maintainer approval rules, LTS policy, release governance, Miri matrices,
and sanitizer infrastructure are deferred until the runtime has a stable prototype
worth governing. First-party crates continue to forbid unsafe Rust in the meantime.
