const path = require("node:path");
const { install } = require("./install");

install({ root: path.resolve(__dirname, "..") });
