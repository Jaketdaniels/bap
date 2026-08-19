let failed = false;
try {
  await bap.readFile(Bap.args[0]);
} catch (e) {
  failed = String(e.message).includes("no-such-bap-file");
}
if (!failed) throw new Error("readFile must fail on missing file");