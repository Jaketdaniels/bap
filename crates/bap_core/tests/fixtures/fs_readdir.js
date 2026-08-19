const dir = Bap.args[0];
const entries = await bap.readDir(dir);
const fileEntry = entries.find((e) => e.name === "fs_roundtrip.js");
if (!fileEntry || !fileEntry.isFile || fileEntry.isDir) throw new Error("readDir entry wrong");