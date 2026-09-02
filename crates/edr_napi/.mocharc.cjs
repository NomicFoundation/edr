const { noStripTypesFlag } = require("../../config/mocha.cjs");

module.exports = {
  require: "ts-node/register/transpile-only",
  timeout: 25000,
  // `expose-gc` lets the provider thread-reclamation tests drive GC.
  "node-option": ["max-old-space-size=8192", "expose-gc", noStripTypesFlag].filter(
    Boolean
  ),
};
