function check(condition, message) {
  if (!condition) throw new Error(message);
}

check(Buffer.from("bap", "utf8").toString("hex") === "626170", "utf8->hex failed");
check(Buffer.from("626170", "hex").toString() === "bap", "hex->utf8 failed");
check(Buffer.from("YmFw", "base64").toString() === "bap", "base64 decode failed");
check(Buffer.from("bap").toString("base64") === "YmFw", "base64 encode failed");
check(Buffer.from([0x62, 0x61, 0x70]).toString("latin1") === "bap", "latin1 failed");
check(Buffer.isBuffer(Buffer.alloc(3)), "isBuffer/alloc failed");
check(Buffer.byteLength("héllo") === 6, "byteLength failed");
check(Buffer.concat([Buffer.from("a"), Buffer.from("bc")]).toString() === "abc", "concat failed");
check(Buffer.from("hello world").indexOf("world") === 6, "indexOf failed");
console.log("buffer encodings ok");
