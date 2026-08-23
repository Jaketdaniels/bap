import local, { add, name } from "./local.cjs";

const sum = add(20, 22);
if (sum !== 42) throw new Error(`expected 42, got ${sum}`);
if (name !== "bap-cjs") throw new Error(`unexpected name ${name}`);
if (local.add !== add) throw new Error("default export should expose module.exports");
console.log(`cjs interop ${sum} ${name}`);
