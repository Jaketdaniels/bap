(async () => {
  const { save, load } = require("./index.js");
  const target = save("/tmp/bap-compat-filestore", "note.txt", "stored");
  if ((await load(target)) !== "stored") throw new Error("roundtrip broken");
  console.log("file-store ok");
})();
