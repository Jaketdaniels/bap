const failed = await bap
  .fetch("http://127.0.0.1:1/")
  .then(() => false, () => true);
if (!failed) throw new Error("fetch must fail when refused");