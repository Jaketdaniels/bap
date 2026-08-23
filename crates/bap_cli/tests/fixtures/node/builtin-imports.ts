import { join, basename } from "node:path";
import { EventEmitter } from "node:events";
import util from "node:util";

const emitter = new EventEmitter();
let received = "";
emitter.once("ping", (message: string) => {
  received = message;
});
emitter.emit("ping", "pong");

if (received !== "pong") throw new Error(`emitter failed: ${received}`);
if (join("/a", "b/c") !== "/a/b/c") throw new Error("path.join mismatch");
if (basename("/x/y.ts") !== "y.ts") throw new Error("path.basename mismatch");
console.log(util.format("%s=%d", "answer", 42));
