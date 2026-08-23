#!/usr/bin/env node
// Runs the pinned compatibility corpus under bap and records results.json.
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const bin = process.env.BAP_BIN ?? join(root, "target/debug/bap");
const corpus = JSON.parse(readFileSync(join(root, "bench/compat/corpus.json"), "utf8"));

mkdirSync(join(root, "bench/compat"), { recursive: true });
const results = [];
for (const entry of corpus.packages) {
  if (entry.enabled === false) {
    results.push({ name: entry.name, status: "disabled", durationMs: 0 });
    continue;
  }
  if (entry.source !== "local") {
    results.push({
      name: entry.name,
      status: "unsupported",
      reason: `${entry.source} sources need the package-manager milestone`,
      durationMs: 0,
    });
    continue;
  }
  const script = join(root, "bench/compat/packages", entry.name, entry.script);
  const started = Date.now();
  const run = spawnSync(bin, ["run", script], {
    encoding: "utf8",
    timeout: 30_000,
    env: { ...process.env, BAP_TS_CACHE_DIR: join(root, "bench/compat/.cache") },
  });
  const durationMs = Date.now() - started;
  const passed = run.status === 0;
  results.push({
    name: entry.name,
    status: passed ? "passed" : "failed",
    durationMs,
    ...(passed ? {} : { stderr: String(run.stderr ?? "").slice(-2000) }),
  });
}

const counted = results.filter((entry) => entry.status !== "disabled");
const passed = counted.filter((entry) => entry.status === "passed");
const report = {
  generatedAt: new Date().toISOString(),
  binary: bin,
  total: counted.length,
  passed: passed.length,
  ratio: counted.length === 0
    ? "100%"
    : `${Math.round((passed.length * 100) / counted.length)}%`,
  results,
};
const destination = join(root, "bench/compat/results.json");
writeFileSync(destination, `${JSON.stringify(report, null, 2)}\n`);
console.log(`wrote ${destination} (${report.passed}/${report.total} passed, ${report.ratio})`);
