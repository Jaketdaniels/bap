const path = require("node:path");
exports.joinSafe = (...parts) => path.normalize(path.join(...parts));
exports.baseName = (file) => path.basename(file, path.extname(file));
