let failed = false;
try {
  await bap.connect("127.0.0.1", 1);
} catch (e) {
  failed = String(e.message).includes("refused");
}
if (!failed) throw new Error("connect must fail when refused");