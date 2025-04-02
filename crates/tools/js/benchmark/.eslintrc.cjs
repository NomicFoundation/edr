module.exports = {
  extends: [`${__dirname}/../../../../config/eslint/eslintrc.js`],
  parserOptions: {
    project: `${__dirname}/tsconfig.json`,
    sourceType: "module",
  },
  ignorePatterns: ["forge-std/**"],
  rules: {
    "@typescript-eslint/restrict-template-expressions": "off",
    "import/no-extraneous-dependencies": [
      "error",
      {
        devDependencies: true,
      },
    ],
    "prefer-template": "off",
  },
};
