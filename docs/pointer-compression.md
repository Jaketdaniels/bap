# Pointer compression study

The Phase 0 spec claims V8 ships pointer compression enabled by default on
64-bit platforms, halving heap memory at a 2-4% mean latency cost and a ~7%
p99 improvement. This page records what Bap actually ships, what the claim is
worth here, and the decision.

## What the build actually does

- Upstream V8 defaults `v8_enable_pointer_compression=true` on 64-bit targets.
- The Rust packaging differs: `rusty_v8` (v8 crate 150.4.0) exposes pointer
  compression as an **opt-in cargo feature** (`v8_enable_pointer_compression`,
  also implied by `v8_enable_sandbox`), and `deno_v8` 0.2 re-exports that
  feature to embedders.
- Bap's workspace enables none of those features, so Bap currently builds V8
  **without** pointer compression. The spec's assumption does not hold for our
  binary.

## Why we did not measure on-vs-off this round

Enabling compression requires the matching prebuilt static archive
(`librusty_v8_ptrcomp_...`). For v8 150.4.0 on `aarch64-apple-darwin` no such
archive is published; the download fails and the fallback is a full
`V8_FROM_SOURCE=1` build (GN + ninja toolchain, tens of minutes per config,
doubled for the A/B pair). That cost is not justified while the feature cannot
ship in CI or release artifacts anyway.

For reference, the without-compression baseline on the SSR harness
(2026-08-23, Apple M-series, 200 requests after 10 warmup):

| metric | value |
|--------|-------|
| route p50 | 0.168 ms |
| route p99 | 0.454 ms |
| isolate heap used | ~1.11 MiB |
| cold start (release, warm cache) | ~5-7 ms steady |

## Decision

Ship the default (compression off). Revisit when either of these lands:

1. `denoland/rusty_v8` publishes ptrcomp archives for our target triple at the
   pinned v8 version - then flip it with a one-line feature unification in
   `crates/bap_core/Cargo.toml`
   (`deno_v8 = { version = "0.2", features = ["v8_enable_pointer_compression"] }`)
   and rerun this study.
2. A workload appears where isolate heap size dominates real memory pressure;
   halved pointers would then be worth building V8 from source.

Until then, the spec's "enabled by default" claim is recorded as
not-applicable-to-Bap, and its latency/memory numbers as unverified.
