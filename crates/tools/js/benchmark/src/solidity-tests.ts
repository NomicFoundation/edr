// Baseline
// foundryup --commit 0a5b22f07
// forge test --no-match-test "test_ChainBubbleUp()|test_DeriveRememberKey()"

import fs from "fs";
import { execSync } from "child_process";
import path from "path";
import simpleGit from "simple-git";
import { runAllSolidityTests } from "@nomicfoundation/edr-helpers";
import {
  SolidityTestRunnerConfigArgs,
  FsAccessPermission,
} from "@nomicfoundation/edr";

const EXPECTED_RESULTS = 15;

// Hack: since EDR currently doesn't support filtering certain tests in test suites, we run them, but ignore their failures.
const EXCLUDED_TESTS = new Set([
  // This relies on environment variable interpolation in the `rpcEndpoints` config which is not supported by EDR.
  "test_ChainBubbleUp()",
  // This relies on the `deriveKey` and `rememberKey` cheatcodes which are not supported by EDR.
  "test_DeriveRememberKey()",
]);

const REPO_DIR = "forge-std";
const REPO_URL = "https://github.com/NomicFoundation/forge-std.git";
const BRANCH_NAME = "js-benchmark-config";

export async function setupForgeStdRepo() {
  const repoPath = path.join(__dirname, REPO_DIR);
  // Ensure directory exists
  if (!fs.existsSync(repoPath)) {
    await simpleGit().clone(REPO_URL, repoPath);
  }

  const git = simpleGit(repoPath);
  await git.fetch();
  await git.checkout(BRANCH_NAME);
  await git.pull();

  // Run npx hardhat compile
  execSync("npx hardhat compile", {
    cwd: repoPath,
    stdio: "inherit",
  });

  return repoPath;
}

export async function runForgeStdTests(forgeStdRepoPath: string) {
  const start = performance.now();

  const artifactsDir = path.join(forgeStdRepoPath, "artifacts");
  const hardhatConfig = require(
    path.join(forgeStdRepoPath, "hardhat.config.js")
  );

  let artifacts = listFilesRecursively(artifactsDir)
    .filter((p) => !p.endsWith(".dbg.json") && !p.includes("build-info"))
    .map((artifactPath) => loadArtifact(hardhatConfig, artifactPath))
    .filter((a) => a !== undefined);

  const testSuiteIds = artifacts
    .filter(
      (a) =>
        a.contract !== undefined &&
        a.contract.abi !== undefined &&
        a.contract.abi.some(
          (item: { type: string; name: string }) =>
            item.type === "function" && item.name.startsWith("test")
        )
    )
    .map((a) => a.id);

  artifacts = artifacts.map((a) => {
    a.contract.abi = JSON.stringify(a.contract.abi);
    return a;
  });

  const configs: SolidityTestRunnerConfigArgs = {
    projectRoot: forgeStdRepoPath,
    // TODO cache this in CI
    rpcCachePath: "./forge-std-rpc-cache",
    fsPermissions: [
      { path: forgeStdRepoPath, access: FsAccessPermission.ReadWrite },
    ],
    testFail: true,
    rpcEndpoints: {
      // These are hardcoded in the `forge-std` foundry.toml
      mainnet:
        "https://eth-mainnet.alchemyapi.io/v2/WV407BEiBmjNJfKo9Uo_55u0z0ITyCOX",
      optimism_sepolia: "https://sepolia.optimism.io/",
      arbitrum_one_sepolia: "https://sepolia-rollup.arbitrum.io/rpc/",
    },
    fuzz: {
      seed: "0x1234567890123456789012345678901234567890",
    },
  };

  const results = await runAllSolidityTests(artifacts, testSuiteIds, configs);

  console.error("elapsed (s)", computeElapsedSec(start));

  if (results.length !== EXPECTED_RESULTS) {
    throw new Error(
      `Expected ${EXPECTED_RESULTS} results, got ${results.length}`
    );
  }

  const failed = new Set();
  for (const res of results) {
    for (const r of res.testResults) {
      if (r.status !== "Success" && !EXCLUDED_TESTS.has(r.name)) {
        failed.add(`${res.id.name} ${r.name} ${r.status} reason:\n${r.reason}`);
      }
    }
  }
  if (failed.size !== 0) {
    console.error(failed);
    throw new Error(`Some tests failed`);
  }
}

function computeElapsedSec(since: number) {
  const elapsedSec = (performance.now() - since) / 1000;
  return Math.round(elapsedSec * 1000) / 1000;
}

function listFilesRecursively(dir: string, fileList: string[] = []): string[] {
  const files = fs.readdirSync(dir);

  files.forEach((file) => {
    const filePath = path.join(dir, file);
    if (fs.statSync(filePath).isDirectory()) {
      listFilesRecursively(filePath, fileList);
    } else {
      fileList.push(filePath);
    }
  });

  return fileList;
}

// Load a contract built with Hardhat
function loadArtifact(hardhatConfig: any, artifactPath: string) {
  let compiledContract;
  try {
    compiledContract = require(artifactPath);
  } catch (e) {
    console.error("Failed to load artifact", artifactPath);
    return undefined;
  }

  const artifactId = {
    name: compiledContract.contractName,
    solcVersion: hardhatConfig.solidity.version,
    source: compiledContract.sourceName,
  } as { name: string; solcVersion: string; source: string };

  const contract = {
    abi: compiledContract.abi,
    bytecode: compiledContract.bytecode,
    deployedBytecode: compiledContract.deployedBytecode,
  };

  return {
    id: artifactId,
    contract: contract,
  };
}
