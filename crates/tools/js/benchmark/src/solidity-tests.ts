// Baseline
// foundryup --commit 0a5b22f07
// forge test --no-match-contract 'StdChainsTest|StdCheatsTest|MockERC721Test|MockERC20Test|StdCheatsForkTest|StdJsonTest|StdUtilsForkTest|StdTomlTest'

import fs from "fs";
import { execSync } from "child_process";
import path from "path";
import simpleGit from "simple-git";
import { runAllSolidityTests } from "@nomicfoundation/edr-helpers";

const EXCLUDED_TEST_SUITES = new Set([
  "StdChainsTest",
  "StdCheatsTest",
  "MockERC721Test",
  "MockERC20Test",
  "StdCheatsForkTest",
  "StdJsonTest",
  "StdUtilsForkTest",
  "StdTomlTest",
]);
const EXPECTED_RESULTS = 7;

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

  const artifacts = listFilesRecursively(artifactsDir)
    .filter((p) => !p.endsWith(".dbg.json") && !p.includes("build-info"))
    .map((artifactPath) => loadArtifact(hardhatConfig, artifactPath));

  const testSuiteIds = artifacts
    .filter(
      (a) =>
        a.id.source.includes(".t.sol") && !EXCLUDED_TEST_SUITES.has(a.id.name)
    )
    .map((a) => a.id);

  const configs = {
    projectRoot: forgeStdRepoPath,
    fuzz: {
      failurePersistDir: path.join(forgeStdRepoPath, "failures"),
    },
  };
  const results = await runAllSolidityTests(artifacts, testSuiteIds, configs);

  console.error("elapsed (s)", computeElapsedSec(start));

  if (results.length !== EXPECTED_RESULTS) {
    console.log(results.map((r: any) => r.name));
    throw new Error(
      `Expected ${EXPECTED_RESULTS} results, got ${results.length}`
    );
  }

  const failed = new Set();
  for (const res of results) {
    for (const r of res.testResults) {
      if (r.status !== "Success") {
        failed.add(`${res.id.name} ${r.name} ${r.status}`);
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
  const compiledContract = require(artifactPath);

  const artifactId = {
    name: compiledContract.contractName,
    solcVersion: hardhatConfig.solidity.version,
    source: compiledContract.sourceName,
  } as { name: string; solcVersion: string; source: string };

  const testContract = {
    abi: JSON.stringify(compiledContract.abi),
    bytecode: compiledContract.bytecode,
    deployedBytecode: compiledContract.deployedBytecode,
  };

  return {
    id: artifactId,
    contract: testContract,
  };
}
