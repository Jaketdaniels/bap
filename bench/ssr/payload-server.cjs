// Payload fixture hosted in its own process so the harness can spawnSync.
const { createServer } = require("node:http");

const port = Number(process.argv[2]);
const items = Array.from({ length: 12 }, (_, index) => ({
  name: `item-${index}`,
  score: 4 + ((index * 37) % 100) / 100,
}));
const body = JSON.stringify({ title: "Profile for user", items });

const server = createServer((_request, response) => {
  response.writeHead(200, { "content-type": "application/json" });
  response.end(body);
});
server.listen(port, "127.0.0.1", () => {
  // Report the BOUND port; callers passing 0 get an ephemeral one.
  console.log(JSON.stringify({ ready: true, port: server.address().port }));
});
