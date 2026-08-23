const util = require("node:util");

exports.describe = (name, value) => util.format("%s=%j", name, value);
exports.show = (value) => util.inspect(value);
