// Demonstrates the Node-compatibility surface: require, node: shims, Buffer, process.
const path = require("node:path");
const fs = require("fs");
const { EventEmitter } = require("events");

const emitter = new EventEmitter();
emitter.on("greet", (name) => console.log(`hello ${name}`));
emitter.emit("greet", "from events");

const joined = path.join("/tmp", "bap-demo", "file.txt");
console.log(`path.join -> ${joined}`);

fs.writeFileSync("/tmp/bap-node-style.txt", Buffer.from("written by bap\n"));
const contents = fs.readFileSync("/tmp/bap-node-style.txt", "utf8");
console.log(`fs roundtrip -> ${contents.trim()}`);

const decoded = Buffer.from("62617000", "hex").toString();
console.log(`buffer hex -> ${decoded.replace("\0", "")} (${Buffer.byteLength(contents)} bytes read)`);

console.log(`process.platform=${process.platform} arch=${process.arch} argv=${JSON.stringify(process.argv.slice(2))}`);
if (process.env.BAP_DEMO_TOKEN) {
  console.log(`env token present: ${process.env.BAP_DEMO_TOKEN.length} chars`);
}
