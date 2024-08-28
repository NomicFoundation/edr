import {
  Artifact,
  ArtifactId,
  ContractData,
} from "@nomicfoundation/edr";
import { task } from "hardhat/config";

task("test:solidity").setAction(async (_: any, hre: any) => {
  await hre.run("compile", { quiet: true });
  const { runAllSolidityTests } = await import("@nomicfoundation/edr-helpers");
  const { spec } = require("node:test/reporters");

  const specReporter = new spec();

  specReporter.pipe(process.stdout);

  let totalTests = 0;
  let failedTests = 0;

  const artifacts: Artifact[] = [];
  const testSuiteIds: ArtifactId[] = [];
  const fqns = await hre.artifacts.getAllFullyQualifiedNames();

  for (const fqn of fqns) {
    const artifact = hre.artifacts.readArtifactSync(fqn);
    const buildInfo = hre.artifacts.getBuildInfoSync(fqn);

    const id = {
      name: artifact.contractName,
      solcVersion: buildInfo.solcVersion,
      source: artifact.sourceName,
    };

    const contract: ContractData = {
      abi: JSON.stringify(artifact.abi),
      bytecode: artifact.bytecode,
      deployedBytecode: artifact.deployedBytecode,
    };

    artifacts.push({ id, contract });

    const sourceName = artifact.sourceName;
    const isTestFile =
      sourceName.endsWith(".t.sol") &&
      sourceName.startsWith("contracts/") &&
      !sourceName.startsWith("contracts/forge-std/") &&
      !sourceName.startsWith("contracts/ds-test/");

    if (isTestFile) {
      testSuiteIds.push(id);
    }
  }

  const config = {
    projectRoot: hre.config.paths.root,
  };

  await runAllSolidityTests(
    artifacts,
    testSuiteIds,
    config,
    (suiteResult, testResult) => {
      let name = suiteResult.id.name + " | " + testResult.name;
      if ("runs" in testResult?.kind) {
        name += ` (${testResult.kind.runs} runs)`;
      }

      let failed = testResult.status === "Failure";
      totalTests++;
      if (failed) {
        failedTests++;
      }

      specReporter.write({
        type: failed ? "test:fail" : "test:pass",
        data: {
          name,
        },
      });
    }
  );

  console.log(`\n${totalTests} tests found, ${failedTests} failed`);

  if (failedTests > 0) {
    process.exit(1);
  }
  process.exit(0);
});
