#!/usr/bin/env node
// Cross-runtime SSR harness: runs the same synthetic route on every runtime
// available and prints one shared p50/p99/max table.
// The payload fixture runs in a child process so spawnSync cannot block it.
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = fileURLToPath(new URL(".", import.meta.url));
const root = resolve(here, "../..");
const iterations = Number(process.env.SSR_ITERATIONS ?? "200");
const warmup = Number(process.env.SSR_WARMUP ?? "10");

const runtimes = [
  { name: "bap", bin: process.env.BAP_BIN ?? join(root, "target/release/bap"), args: ["run"] },
  { name: "node", bin: process.execPath, args: [] },
  { name: "bun", bin: "bun", args: ["run"], optional: true },
  { name: "deno", bin: "deno", args: ["run", "-A"], optional: true },
].filter((runtime) => !runtime.optional || isOnPath(runtime.bin));

const payloadServer = await startPayloadServer();
try {
  const rows = [];
  for (const runtime of runtimes) {
    const row = measure(runtime);
    printRow(row);
    rows.push(row);
  }
  console.log();
  console.log("| runtime | p50 | p99 | max | requests |");
  console.log("|---------|-----|-----|-----|----------|");
  for (const row of rows) {
    if (row.error) {
      console.log(`| ${row.name} | unavailable | - | - | - |`);
    } else {
      console.log(
        `| ${row.name} | ${row.p50Ms.toFixed(2)} ms | ${row.p99Ms.toFixed(2)} ms | ${row.maxMs.toFixed(2)} ms | ${row.total} |`,
      );
    }
  }
} finally {
  payloadServer.kill();
}

function measure({ name, bin, args }) {
  // Fresh cache dir per invocation so Bap's TS cache never skews runs.
  const cacheDir = mkdtempSync(join(tmpdir(), "bap-ssr-"));
  try {
    const started = Date.now();
    const result = spawnSync(bin, [...args, join(here, "driver.js")], {
      encoding: "utf8",
      timeout: 120_000,
      env: {
        ...process.env,
        SSR_PORT: String(payloadServer.port),
        SSR_ITERATIONS: String(iterations),
        SSR_WARMUP: String(warmup),
        BAP_TS_CACHE_DIR: cacheDir,
      },
      cwd: here,
    });
    const wallMs = Date.now() - started;
    if (result.status !== 0 && result.status !== null) {
      return { name, error: String(result.stderr).slice(-500), wallMs };
    }
    const line = String(result.stdout)
      .split("\n")
      .map((text) => text.trim())
      .filter((text) => text.startsWith("{"))
      .pop();
    if (!line) {
      return { name, error: "no JSON summary on stdout", wallMs };
    }
    return { name, ...JSON.parse(line), wallMs };
  } finally {
    rmSync(cacheDir, { recursive: true, force: true });
  }
}

function printRow(row) {
  if (row.error) {
    console.error(`[${row.name}] failed: ${row.error}`);
    console.log(`${row.name.padEnd(8)} unavailable (${row.wallMs} ms wall)`);
    return;
  }
  console.log(
    `${row.name.padEnd(8)} p50=${row.p50Ms.toFixed(2).padStart(8)} ms  p99=${row.p99Ms.toFixed(2).padStart(9)} ms  max=${row.maxMs.toFixed(2).padStart(9)} ms`,
  );
}

async function startPayloadServer() {
  const child = await new Promise((resolvePromise, rejectPromise) => {
    const process_ = spawn(process.execPath, [join(here, "payload-server.cjs"), "0"], {
      stdio: ["ignore", "pipe", "inherit"],
    });
    let buffer = "";
    process_.stdout.on("data", (chunk) => {
      buffer += chunk;
      const line = buffer.split("\n").find((text) => text.startsWith("{"));
      if (!line || !process_.port) {
        if (!line) return;
      }
      try {
        const parsed = JSON.parse(line);
        process_.port = parsed.port;
        resolvePromise(process_);
      } catch (error) {
        rejectPromise(error);
      }
    });
    process_.on("exit", () => rejectPromise(new Error("payload server exited early")));
  });
  return child;
}

function isOnPath(bin) {
  const result = spawnSync("which", [bin], { encoding: "utf8" });
  return result.status === 0;
}
