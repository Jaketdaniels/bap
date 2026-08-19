console.log("started at", Date.now());
setTimeout(() => console.log("timer fired after 10ms"), 10);
const res = await bap.fetch("http://127.0.0.1:9876/");
const body = await res.text();
await bap.writeFile("/tmp/bap-async-demo.txt", "fetched " + body.length + " bytes\n");
console.log("fetch status", res.status, "body bytes", body.length);