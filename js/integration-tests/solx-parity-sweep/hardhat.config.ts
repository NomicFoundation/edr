import type { HardhatUserConfig } from "hardhat/config";
import HardhatSolx from "@nomicfoundation/hardhat-solx";

// `as HardhatUserConfig` tells TypeScript to accept the `type: "solx"`
// solidity profile — the hardhat-solx plugin augments the config types
// at runtime via module declaration merging, which the TypeScript
// compiler doesn't always pick up across plugin boundaries.
export default {
  plugins: [HardhatSolx],
  solidity: {
    profiles: {
      default: { version: "0.8.34" },
      // hardhat-solx maps each Solidity version it sees to a matching
      // solx release; we keep the 0.8.34 source unchanged.
      solx: { type: "solx", version: "0.8.34" },
    },
  },
  paths: {
    sources: "./contracts",
    tests: "./contracts",
  },
} as unknown as HardhatUserConfig;
