module.exports = {
  extends: [`${__dirname}/../../config/eslint/eslintrc.js`],
  parserOptions: {
    project: `${__dirname}/tsconfig.json`,
    sourceType: "module",
  },
  rules: {
    "import/no-extraneous-dependencies": "off",
  },
};
