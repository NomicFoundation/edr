// @ts-check

/** @type {import("syncpack").RcFile} */
const config = {
  versionGroups: [
    // smock only works with ethers v5
    {
      packages: ["hardhat-edr-smock-test"],
      dependencies: ["ethers"],
      // latest ethers v5 version
      pinVersion: "5.7.2",
    },
    {
      packages: ["**"],
      dependencies: [
        "@ignored/edr",
        "@nomicfoundation/edr-helpers",
        "hardhat-solidity-tests",
      ],
      dependencyTypes: ["local"],
    },
    // These packages use HH v2
    {
      packages: ["@ignored/edr", "hardhat-edr-tests", "hardhat-edr-smock-test"],
      dependencies: ["hardhat"],
    },
    // These packages use HH v3
    {
      packages: ["@nomicfoundation/edr-helpers", "solidity-tests", "benchmark"],
      dependencies: ["hardhat"],
    },
  ],
  semverGroups: [
    {
      dependencies: ["typescript"],
      range: "~",
    },
  ],
};

module.exports = config;
