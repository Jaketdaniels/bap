# SSR latency harness

A dependency-free synthetic route renderer (`route.js`) runs identically on
bap, Node, Bun, and Deno: parse a `/users/:id` path, fetch a JSON payload from
a local fixture server, interpolate an HTML template, return the page.
`harness.mjs` drives `driver.js` inside one warm process per runtime and
reports p50/p99/max.

**Honest framing:** this workload measures Bap's runtime shape against its own
synthetic route. It is not a Next.js application; the published figures below
are context, not a like-for-like comparison.

## Published reference (2026, Next.js SSR)

| runtime | p50 | p99 |
|---------|-----|-----|
| Bun | ~246 ms | ~974 ms |
| Node | ~20 ms | ~174 ms |
| Deno | ~12 ms | ~101 ms |
| Bap target | <=20 ms | <=150 ms |

## Local results

Machine: Apple M-series, macOS arm64, warm page cache, 200 requests after 10
warmup per runtime. Fill both tables from one run of:

```sh
BAP_BIN=target/release/bap node bench/ssr/harness.mjs
```

### Synthetic route (this harness)

| runtime | p50 | p99 | max | requests |
|---------|-----|-----|-----|----------|
| bap | 0.16 ms | 0.48 ms | 1.80 ms | 200 |
| node | 1.46 ms | 2.24 ms | 2.34 ms | 200 |
| bun | 0.06 ms | 0.60 ms | 2.76 ms | 200 |

The published Next.js numbers sit orders of magnitude above these because a
real framework render does orders of magnitude more work. The comparison that
matters here is Bap's own trend line across releases; run the harness before
and after a change to see movement.
