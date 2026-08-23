// Drives render() N times inside one warm runtime and prints one JSON line.
import { render } from "./route.js";

const iterations = Number(process.env.SSR_ITERATIONS ?? "200");
const warmup = Number(process.env.SSR_WARMUP ?? "10");
const samples = [];
for (let i = 0; i < warmup; i++) {
  await render("/users/42");
}
for (let i = 0; i < iterations; i++) {
  const started = performance.now();
  await render(`/users/${i + 1}`);
  samples.push(performance.now() - started);
}
samples.sort((a, b) => a - b);
const at = (q) => samples[Math.min(samples.length - 1, Math.floor(q * samples.length))];
process.stdout.write(
  JSON.stringify({
    total: samples.length,
    p50Ms: at(0.5),
    p99Ms: at(0.99),
    maxMs: samples[samples.length - 1],
  }) + "\n",
);
