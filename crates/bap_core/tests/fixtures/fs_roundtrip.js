const path = Bap.args[0];
const written = "hello bap\n";
await bap.writeFile(path, written);
const data = await bap.readFile(path);
if (String.fromCharCode(...data) !== written) throw new Error("round trip mismatch");
const st = await bap.stat(path);
if (st.size !== written.length) throw new Error("stat size mismatch");
if (st.modified_ms <= 0) throw new Error("stat modified missing");