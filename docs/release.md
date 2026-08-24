# Release process

Bap ships overlapping LTS lines so libraries can support several Bap versions
alongside Node, Bun, and Deno in the same release cycle.

## Versioning

- Strict [semver](https://semver.org). Breaking changes land only in major
  versions; deprecations are announced at least one minor before removal.
- Conventional Commits (`feat`, `fix`, `chore`, `docs`, `refactor`, `test`)
  drive version decisions; `feat` bumps minor, `fix` patches.
- No bap crate currently publishes to crates.io (distribution is GitHub
  Releases), so the crates are marked `publish = false` and
  `cargo-semver-checks` is not wired into CI. If a crate ever publishes,
  re-enable the check with that crate's registry baseline before cutting
  releases; until then API stability is enforced by review against the
  [deprecation matrix](deprecation-matrix.md).

## LTS lines

- Each major version receives an **18-month** support window: 12 months of bug
  fixes followed by 6 months of security fixes only.
- Lines align with Node.js LTS majors so adopters can pair a Bap line with the
  Node line their service already tracks. Overlap is intentional: two Bap LTS
  lines are always open, mirroring how libraries juggle Node and Bun today.
- A new LTS line opens when a major ships; the previous line enters its final
  6-month security window at that point.

## Deprecations

Deprecated APIs are recorded in the [deprecation matrix](deprecation-matrix.md)
with the version they were deprecated in, the version they will be removed in,
and the migration note. Removals happen in the major stated by the matrix — no
exceptions, no surprise removals.

## Checklist for cutting a release

1. `cargo fmt --check`, `cargo clippy -- -D warnings`, full test suite green.
2. `cargo-semver-checks` clean (or the diff carries an explicit major bump).
3. Update `docs/deprecation-matrix.md` if any API was newly deprecated.
4. Tag the release; the changelog is generated from Conventional Commits.
