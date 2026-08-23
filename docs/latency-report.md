# Bap latency & compatibility report

Measured 2026-08-23 on an Apple M-series machine (macOS arm64), release build,
warm page cache unless stated. Methodology per measurement below.

**Scope honesty:** the SSR workload is Bap's own synthetic route
(`bench/ssr/route.js`), not a Next.js application. Published Next.js figures
are quoted as context only.

## Spec targets vs measured

| metric | spec target | measured | verdict |
|--------|-------------|----------|---------|
| cold start (steady, warm cache) | <= 8 ms | 4.8 - 6.2 ms (median ~5.4) | pass |
| SSR p50, synthetic route | <= 20 ms | 0.16 ms | pass |
| SSR p99, synthetic route | <= 150 ms | 0.36 - 0.55 ms | pass |
| npm compatibility | >= 99% | 100% of pinned local corpus; remote corpus pending package manager | deferred gate |

## Cold start

`target/release/bap --metrics json run examples/hello.ts`, five consecutive
runs after one warm-up invocation: 8.1, 6.2, 5.5, 5.4, 4.8 ms. The first
invocation after a fresh boot pays a page-cache miss on the ~30 MB V8 dylib
(20-42 ms observed in Phase 1); steady state sits comfortably under the 8 ms
ceiling. `cold_start_ms` is recorded at isolate construction, excluding CLI
startup and module load.

## Synthetic-route SSR

One warm process per runtime; each request renders `/users/:id` from a fetched
JSON payload into HTML (`bench/ssr/harness.mjs`, 200 requests after 10 warmup):

| runtime | p50 | p99 | max | requests |
|---------|-----|-----|-----|----------|
| bap | 0.16 ms | 0.36 ms | 0.40 ms | 200 |
| node 22 | 1.47 ms | 2.13 ms | 2.39 ms | 200 |
| bun 1.3 | 0.06 ms | 0.50 ms | 0.68 ms | 200 |

Published 2026 Next.js figures for context: Bun ~246/974 ms, Node ~20/174 ms,
Deno ~12/101 ms (p50/p99). Real frameworks do orders of magnitude more work
per request than this harness; the useful readout is Bap's trend across
releases, re-measured via `node bench/ssr/harness.mjs`.

## Lanes and warmup

`bap bench bench/ssr/request.mjs --warmup 10 --iterations 300 --lanes 3`
spreads requests across three long-lived isolates (~1.1 MiB heap each) and
completes in ~36 ms wall; single-lane 200 iterations take ~59 ms. Warmup runs
are captured separately from measured samples and excluded from percentiles.

## Pointer compression

Not enabled in this build; enabling it requires a V8 source build because no
prebuilt ptrcomp archive exists for this target. Decision and evidence:
[pointer-compression.md](pointer-compression.md).

## npm compatibility

`bap report compat`: 5/5 pinned local corpus packages pass (100%), exercising
the node: assert/path/fs/util/events/buffer surface through CommonJS requires.
The >= 99% gate activates once the corpus includes remotely installed packages,
which waits on the package-manager milestone.

## Variance notes

- Percentile histograms use three significant digits (hdrhistogram), so
  sub-microsecond readings round to bucket boundaries.
- Cross-run variance on loopback HTTP is roughly +/-30% at p50 for sub-ms
  workloads; compare trend lines, not single digits.
- CI runs a 20-iteration smoke bench report-only; latency targets are not
  hard-gated to avoid flaky infrastructure.
