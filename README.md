# Bap

Bap is a small V8-based JavaScript and TypeScript runtime written in Rust. It gives
scripts a focused runtime surface for files, timers, TCP, HTTP fetches, and metrics.
The project is a proof of concept. The Rust numeric core and TypeScript proof tool
are useful today; their JavaScript integration is still being built.

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

`bap_math` is a V8-free Rust crate. It currently provides:

- outward-rounded `Interval` arithmetic;
- checked `i32` and `i64` operations;
- `Fixed<18,4>` decimal arithmetic with half-even rounding.

Use the crate from Rust while the JavaScript bindings are under construction:

```rust
use bap_math::{Fixed18_4, Interval};
use std::str::FromStr;

let price = Fixed18_4::from_str("12.3456")?;
let range = Interval::new(1.0, 2.0)?;
```

## TypeScript proof tool

The standalone proof tool checks a small unit-aware TypeScript subset and emits a
JSON evidence manifest. It does not change runtime execution yet.

```sh
npm --prefix tools/ts-proof ci
npm --prefix tools/ts-proof test
node tools/ts-proof/proof.mjs request.json response.json
```

The tool uses TypeScript 7.0.2, keeps source locations in UTF-8 byte offsets, and
rejects mismatched units, dynamic global mutation, non-finite results, and oversized
proof requests.

## What is next

The next integration slice connects the numeric API to JavaScript and lets `bap run`
validate and apply proof manifests. Node compatibility, native add-ons, real
application latency tests, and a package manager are later milestones.

Read the [documentation site](docs/index.html), [roadmap](docs/roadmap.md), and
[readability audit](docs/readability-audit.md) for the current project shape.
