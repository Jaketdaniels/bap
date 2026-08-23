const a = require("./cycle-a.cjs");

// During the cycle, a.exports is still partially filled.
exports.value = "b";
exports.sawAStarted = Boolean(a.startedWith);
