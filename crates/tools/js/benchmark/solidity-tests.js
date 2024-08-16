// Baseline
// foundryup --commit 0a5b22f07
// forge test --no-match-contract 'StdChainsTest|StdCheatsTest|MockERC721Test|MockERC20Test|StdCheatsForkTest|StdJsonTest|StdUtilsForkTest|StdTomlTest'

const fs = require("fs");
const { execSync } = require("child_process");
const path = require("path");
const simpleGit = require("simple-git");
const { runSolidityTests } = require("@nomicfoundation/edr");

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

async function setupForgeStdRepo() {
  const repoPath = path.join(__dirname, REPO_DIR);
  // Ensure directory exists
  if (!fs.existsSync(repoPath)) {
    const git = simpleGit();
    await git.clone(REPO_URL, repoPath);
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

async function runForgeStdTests(forgeStdRepoPath) {
  const start = performance.now();

  const artifactsDir = path.join(forgeStdRepoPath, "artifacts");
  const hardhatConfig = require(
    path.join(forgeStdRepoPath, "hardhat.config.js"),
  );

  const artifacts = listFilesRecursively(artifactsDir)
    .filter((p) => !p.endsWith(".dbg.json") && !p.includes("build-info"))
    .map(loadArtifact.bind(null, hardhatConfig));

  const testSuiteIds = artifacts
    .filter(
      (a) =>
        a.id.source.includes(".t.sol") && !EXCLUDED_TEST_SUITES.has(a.id.name),
    )
    .map((a) => a.id);

  const results = await new Promise(async (resolve, reject) => {
    const resultsFromCallback = [];
    const configs = {
      projectRoot: forgeStdRepoPath,
      fuzz: {
        failurePersistDir: path.join(forgeStdRepoPath, "failures"),
      },
    };

    runSolidityTests(artifacts, testSuiteIds, configs, (result) => {
      console.error(`${result.id.name} took ${elapsedSec(start)} seconds`);

      resultsFromCallback.push(result);
      if (resultsFromCallback.length === artifacts.length) {
        resolve(resultsFromCallback);
      }
    }).catch(reject);
  });
  console.error("elapsed (s)", elapsedSec(start));

  if (results.length !== EXPECTED_RESULTS) {
    console.log(results.map((r) => r.name));
    throw new Error(
      `Expected ${EXPECTED_RESULTS} results, got ${results.length}`,
    );
  }

  const failed = new Set();
  for (let res of results) {
    for (let r of res.testResults) {
      if (r.status !== "Success") {
        failed.add(`${res.name} ${r.name} ${r.status}`);
      }
    }
  }
  if (failed.size !== 0) {
    console.error(failed);
    throw new Error(`Some tests failed`);
  }
}

function elapsedSec(since) {
  const elapsedSec = (performance.now() - since) / 1000;
  return Math.round(elapsedSec * 1000) / 1000;
}

function listFilesRecursively(dir, fileList = []) {
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
function loadArtifact(hardhatConfig, artifactPath) {
  const compiledContract = require(artifactPath);

  const artifactId = {
    name: compiledContract.contractName,
    solcVersion: hardhatConfig.solidity.version,
    source: compiledContract.sourceName,
  };

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

module.exports = {
  runForgeStdTests,
  setupForgeStdRepo,
};
