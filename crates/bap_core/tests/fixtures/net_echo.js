const server = await bap.listen(0);
const roundTrip = (async () => {
  const conn = await bap.accept(server.id);
  const data = await bap.read(conn);
  await bap.write(conn, data);
  await bap.close(conn);
})();
const client = await bap.connect("127.0.0.1", server.port);
await bap.write(client, "ping");
const echoed = await bap.read(client);
await bap.close(client);
await roundTrip;
await bap.closeListener(server.id);
if (String.fromCharCode(...echoed) !== "ping") throw new Error("echo mismatch");