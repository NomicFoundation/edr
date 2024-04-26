import { resetHardhatContext } from "hardhat/internal/reset";
import { HardhatRuntimeEnvironment } from "hardhat/types";

declare module "mocha" {
  interface Context {
    env: HardhatRuntimeEnvironment;
  }
}

export function useEnvironment(configPath?: string) {
  beforeEach("Load environment", function () {
    if (configPath !== undefined) {
      process.env.HARDHAT_CONFIG = configPath;
    }
    this.env = require("hardhat/internal/lib/hardhat-lib");
  });

  afterEach("reset hardhat context", function () {
    delete process.env.HARDHAT_CONFIG;
    resetHardhatContext();
  });
}
