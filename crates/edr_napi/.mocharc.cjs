const { noStripTypesFlag } = require("../../config/mocha.cjs");

module.exports = {
  require: "ts-node/register/transpile-only",
  timeout: 25000,
  "node-option": ["max-old-space-size=8192", noStripTypesFlag].filter(Boolean),
};
