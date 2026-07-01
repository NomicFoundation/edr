import type { HardhatUserConfig } from "hardhat/config";
import HardhatSolx from "@nomicfoundation/hardhat-solx";

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
