const assert = require("node:assert");
const { joinSafe, baseName } = require("./index.js");
assert.equal(joinSafe("/a", "b//c"), "/a/b/c");
assert.equal(baseName("photo.jpeg"), "photo");
console.log("tiny-path-utils ok");
