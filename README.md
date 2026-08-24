# Bap

Bap is a small V8-based JavaScript and TypeScript runtime written in Rust. It gives
scripts a focused runtime surface for files, timers, TCP, HTTP fetches, metrics,
audited numerics, and proof-specialized constants.

## Build

You need Rust 1.96 or newer and Node 22 for the proof-tool tests.

```sh
cargo build --workspace
```

The binary is `target/debug/bap`.

## Run a script

```sh
target/debug/bap run examples/hello.js
target/debug/bap run examples/hello.ts
```

The TypeScript loader strips types, keeps source maps, and caches compiled modules
under the current user's cache directory. A JavaScript entry point can import a
`.ts` module.

Pass arguments after the file name. They appear in `Bap.args`.

```sh
target/debug/bap run examples/hello.js one two
```

```js
console.log(Bap.args);
```

## Built-in runtime APIs

The lowercase `bap` global exposes the current runtime helpers.

```js
const text = await bap.readFile("notes.txt");
await bap.writeFile("/tmp/bap-output.txt", text);

const response = await bap.fetch("https://example.com");
console.log(response.status, await response.text());
```

It also provides `readDir`, `stat`, `listen`, `accept`, `connect`, `read`, `write`,
`close`, and `closeListener`. `setTimeout` and `clearTimeout` use the runtime event
loop. The TCP helpers bind to localhost.

## Node compatibility

Bap runs CommonJS and ESM side by side. `.cjs` files (and plain `.js` without
import/export markers) load through a synchronous `require` with Node-style
resolution, cycle handling, JSON requires, and best-effort named exports for
`import { x } from "./lib.cjs"`.

Implemented builtins: `node:assert`, `node:path`, `node:fs`, `node:util`,
`node:events`, `node:buffer`, and `node:process`. Anything else fails at load
time with a clear message instead of misbehaving later.

```sh
target/debug/bap run examples/node-style.js
```

Projects adopt Bap per service through `bap.json` (`runtime`, optional
`moduleFormat`, and bare-specifier `aliases`); `package.json` stays untouched.
`bap doctor` scans a project's imports and reports what is supported, what is
not implemented, and what would need an alias — see [docs/adoption.md](docs/adoption.md).
Measured compatibility over the pinned corpus ships via
`bench/compat/run.sh` and `target/debug/bap report compat`.

## Benchmarking

```sh
target/release/bap bench bench/ssr/request.mjs --warmup 10 --iterations 200 --lanes 3
```

Measures per-request latency across long-lived isolates with p50/p99
histograms and optional chrome-trace output (`--trace out.json`). The
cross-runtime comparison harness lives in `bench/ssr`; current numbers and
methodology are in [docs/latency-report.md](docs/latency-report.md).

## REPL and metrics

```sh
target/debug/bap repl
target/debug/bap --metrics json run examples/hello.ts
target/debug/bap --log-level debug run examples/hello.js
```

The REPL accepts JavaScript expressions and exits with `.exit`. Metrics go to stderr,
so a script can keep stdout for its own output. JSON metrics include cold-start,
heap samples, operation counts, and async p50/p99 measurements.

## Numeric core

`bap.numeric` exposes the V8-free Rust core to scripts. The same namespace is
mirrored on the capital `Bap` global for the proof vocabulary.

```js
const range = bap.numeric.interval(1, 2);
const product = bap.numeric.intervalMul(range, bap.numeric.interval(0, 10));
console.log(product.lo, product.hi); // outward-rounded bounds

bap.numeric.i64Add(9223372036854775806n, 1n); // exact 64-bit math
bap.numeric.fixedAdd("12.3456", "0.0001"); // "12.3457", half-even rounding
```

Infinite interval bounds are valid (interval arithmetic allows `[1, +Infinity)`),
but NaN bounds throw. Intervals and unit values are frozen objects branded by
`bap.numeric`; passing forged lookalikes throws
`TypeError: BAP_NUMERIC_INVALID_ARGUMENT`. Numeric
failures surface as `RangeError` with stable codes such as
`BAP_NUMERIC_OVERFLOW` and `BAP_NUMERIC_DISCONTINUOUS`. Batch helpers
(`intervalAddBatch`, `i64AddBatch`, ...) cross into Rust once per call and
reject atomically on any element failure.

## TypeScript proofs

`bap run file.ts` consults the proof tool for sources that use the trusted
`Bap.numeric.*` vocabulary. Proven constant initializers are replaced with
folded literals before transpilation; every manifest is revalidated in Rust
(hashes, ranges, unit rules, exact bits) before an edit is applied. Validated
manifests are cached, so warm runs never launch Node.

```sh
target/debug/bap run --require-proofs script.ts # recognized candidates must prove
target/debug/bap run --no-proofs script.ts      # never start the backend
target/debug/bap inspect proofs script.ts       # report evidence (add --json)
```

The standalone tool can also be driven directly:

```sh
npm --prefix tools/ts-proof ci
npm --prefix tools/ts-proof test
node tools/ts-proof/proof.mjs request.json response.json
```

The tool uses TypeScript 7.0.2, keeps source locations in UTF-8 byte offsets, and
rejects mismatched units, dynamic global mutation, non-finite results, and oversized
proof requests.

## What is next

Phase 4 connects the Node surface: a CommonJS loader, a Node-core polyfill shim,
`bap.json` project config, and an npm-compatibility harness. Real application
latency tests follow in Phase 5.

Read the [documentation site](docs/index.html), [roadmap](docs/roadmap.md), and
[readability audit](docs/readability-audit.md) for the current project shape.
