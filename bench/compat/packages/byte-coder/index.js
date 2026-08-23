function toBase64(text) {
  return Buffer.from(text, "utf8").toString("base64");
}
function fromHex(hex) {
  return Buffer.from(hex, "hex").toString();
}
module.exports = { toBase64, fromHex };
