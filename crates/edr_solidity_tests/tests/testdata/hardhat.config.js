const config = {
  paths: {
    sources: {
      solidity: [
        "cancun",
        "default/cheats",
        "default/core",
        /**
         * The default/fork tests always fail with the following error:
         *
         * Error: Invalid hex bytecode for contract
         */
        // "default/fork",
        "default/fs",
        "default/fuzz",
        /**
         * The default/linking tests always fail with the following error:
         *
         * Error: Invalid hex bytecode for contract
         */
        // "default/linking",
        "default/logs",
        "default/repros",
        "default/spec",
        "default/trace",
        /**
         * The multi-version tests pass on their own; however, when executed together
         * with either the fuzz or repros tests, they fail with the following error:
         *
         * thread '<unnamed>' panicked at crates/foundry/cheatcodes/src/fs.rs:387:61:
         * index out of bounds: the len is 0 but the index is 0
         * note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
         */
        // "multi-version",
      ],
    },
  },
  solidity: {
    profiles: {
      default: {
        compilers: [
          { version: "0.8.25" },
          { version: "0.8.20" },
          { version: "0.8.18" },
          { version: "0.8.17" },
          { version: "0.8.15" },
          { version: "0.6.12" },
        ],
      },
    },
    remappings: ["ds-test/=lib/ds-test/src/"],
  },
};

export default config;
