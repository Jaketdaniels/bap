const res = await bap.fetch(Bap.args[0]);
if (res.status !== 200 || !res.ok) throw new Error("bad status");
const text = await res.text();
if (text !== "bap fetch works") throw new Error("bad body: " + text);