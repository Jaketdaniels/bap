const assert = require("node:assert");
const { describe, show } = require("./index.js");
assert.equal(describe("answer", 42), 'answer=42');
assert.ok(show({ a: 1 }).includes("a"));
console.log("str-format ok");
