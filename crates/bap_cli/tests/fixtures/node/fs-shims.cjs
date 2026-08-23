const fs = require("node:fs");
const path = require("path");

const target = path.join(__dirname, "scratch", "shim.txt");
fs.mkdirSync(path.dirname(target));
fs.writeFileSync(target, Buffer.from("roundtrip"));
const asText = fs.readFileSync(target, "utf8");
const bytes = fs.readFileSync(target);
if (!bytes.equals(Buffer.from("roundtrip"))) throw new Error("binary mismatch");
if (!fs.existsSync(target)) throw new Error("existsSync failed");
console.log(asText);

(async () => {
  const text = await fs.promises.readFile(target, "utf8");
  if (text !== asText) throw new Error("promises.readFile mismatch");
  console.log(`promises ok (${text.length})`);
})();
