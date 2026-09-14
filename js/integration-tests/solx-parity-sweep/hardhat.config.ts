import type { HardhatUserConfig } from "hardhat/config";
import HardhatSlangSolx from "@nomicfoundation/hardhat-slang-solx";

export default {
  plugins: [HardhatSlangSolx],
  solidity: {
    profiles: {
      default: { version: "0.8.34" },
      // The plugin requires this exact profile name. It maps each Solidity
      // version it sees to a matching solx release; the source stays 0.8.34.
      "slang-solx": { type: "slang-solx", version: "0.8.34" },
    },
  },
  paths: {
    sources: "./contracts",
    tests: "./contracts",
  },
} as unknown as HardhatUserConfig;
