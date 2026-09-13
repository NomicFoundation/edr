import type { HardhatUserConfig } from "hardhat/config";
import HardhatSlangSolx from "@nomicfoundation/hardhat-slang-solx";

export default {
  plugins: [HardhatSlangSolx],
  solidity: {
    profiles: {
      default: { version: "0.8.34" },
      // hardhat-slang-solx maps each Solidity version it sees to a matching
      // solx release; we keep the 0.8.34 source unchanged.
      solx: { type: "slang-solx", version: "0.8.34" },
    },
  },
  paths: {
    sources: "./contracts",
    tests: "./contracts",
  },
} as unknown as HardhatUserConfig;
