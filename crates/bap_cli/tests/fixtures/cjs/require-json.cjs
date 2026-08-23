const data = require("./data.json");

if (data.answer !== 42) throw new Error(`bad json require: ${data.answer}`);
console.log("json required");
