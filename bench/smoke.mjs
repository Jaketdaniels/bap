// Self-contained per-request unit for the CI smoke benchmark: no network,
// no globals beyond ECMAScript builtins.
function parseParam(query, key) {
  for (const pair of query.split("&")) {
    const [k, v] = pair.split("=");
    if (k === key) return decodeURIComponent(v ?? "");
  }
  return null;
}

const user = parseParam("user=42&verbose=1", "user");
const verbose = parseParam("user=42&verbose=1", "verbose");
const items = Array.from({ length: 12 }, (_, i) => ({
  name: `item-${i}`,
  score: 4 + ((i * 37) % 100) / 100,
}));
const rows = items.map((item) => `<li>${item.name} - ${item.score.toFixed(1)}</li>`).join("\n");
const html = `<!doctype html>
<html><body><h1>user ${user}</h1><ul>${rows}</ul></body></html>`;
if (!html.includes("</html>") || verbose !== "1") {
  throw new Error("smoke render failed");
}
