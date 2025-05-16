export default {
  solidity: {
    compilers: [
      {
        version: "0.7.6",
      },
      {
        version: "0.8.24",
        settings: { evmVersion: "cancun" },
      },
    ],
  },
  paths: {
    sources: "./contracts",
    tests: "./test-contracts",
  },
};
