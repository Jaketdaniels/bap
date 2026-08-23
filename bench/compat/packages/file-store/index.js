const fs = require("node:fs");
const path = require("node:path");

exports.save = (directory, name, contents) => {
  fs.mkdirSync(directory);
  const target = path.join(directory, name);
  fs.writeFileSync(target, contents);
  return target;
};
exports.load = async (target) => fs.promises.readFile(target, "utf8");
