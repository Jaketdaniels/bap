// Synthetic SSR route renderer, identical under bap/node/bun/deno.
// Renders an HTML page for /users/:id using a fetched payload.

export async function render(path) {
  const { id } = parseParams("/users/:id", path);
  const payload = await fetchPayload(id);
  const rows = payload.items
    .map((item) => `      <li>${escapeHtml(item.name)} &middot; ${item.score.toFixed(1)}</li>`)
    .join("\n");
  return `<!doctype html>
<html>
  <head><title>User ${id}</title></head>
  <body>
    <h1>${payload.title} ${id}</h1>
    <ul>
${rows}
    </ul>
    <footer>rendered ${new Date().toISOString()}</footer>
  </body>
</html>
`;
}

function parseParams(pattern, path) {
  const patternParts = pattern.split("/").filter(Boolean);
  const pathParts = path.split("/").filter(Boolean);
  const params = {};
  patternParts.forEach((segment, index) => {
    if (segment.startsWith(":")) params[segment.slice(1)] = pathParts[index];
  });
  return params;
}

function escapeHtml(text) {
  return String(text)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

async function fetchPayload(id) {
  const port = process.env.SSR_PORT;
  if (!port) throw new Error("SSR_PORT is not set");
  const response = await fetch(`http://127.0.0.1:${port}/payload?user=${id}`);
  if (!response.ok) throw new Error(`payload fetch failed: ${response.status}`);
  return response.json();
}
