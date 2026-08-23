const { toBase64, fromHex } = require("./index.js");
if (toBase64("bap") !== "YmFw") throw new Error("base64 encode broken");
if (fromHex("72 75 6e 74 69 6d 65".replace(/ /g, "")) !== "runtime") throw new Error("hex decode broken");
console.log("byte-coder ok");
