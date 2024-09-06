import { task } from "hardhat/config";
import fs from "node:fs";
import hre from "hardhat";
import path from "node:path";

// HACK: copy the libraries Rust integration tests into the Hardhat project source
task("copyLibraries", "Run pre-test script", async (taskArgs, hre) => {
  await fs.promises.copyFile(
    "../../../crates/foundry/testdata/lib/ds-test/src/test.sol",
    path.join(hre.config.paths.sources, "test.sol")
  );
  await fs.promises.copyFile(
    "../../../crates/foundry/testdata/cheats/Vm.sol",
    path.join(hre.config.paths.sources, "Vm.sol")
  );
});

task("test").setAction(async (taskArgs, hre, runSuper) => {
  // Run the pretest task before tests
  await hre.run("copyLibraries");
  // Run the actual tests
  await runSuper();
});

export default {
  solidity: "0.8.24",
};
