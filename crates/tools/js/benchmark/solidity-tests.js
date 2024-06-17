// Baseline
// foundryup --commit 0a5b22f07
// forge test --no-match-contract 'StdChainsTest|StdCheatsTest|MockERC721Test|MockERC20Test|StdCheatsForkTest|StdJsonTest|StdUtilsForkTest|StdTomlTest'

const fs = require("fs");
const { execSync } = require("child_process");
const path = require("path");
const simpleGit = require("simple-git");
const { SolidityTestRunner } = require("@nomicfoundation/edr");
const { mkdtemp } = require("fs/promises");
const { tmpdir } = require("os");

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
  const tmpDir = await mkdtemp(path.join(tmpdir(), "solidity-tests-"));
  const gasReport = false;

  const start = performance.now();

  const artifactsDir = path.join(forgeStdRepoPath, "artifacts");
  const hardhatConfig = require(
    path.join(forgeStdRepoPath, "hardhat.config.js"),
  );

  const testSuites = listFilesRecursively(artifactsDir)
    .filter((p) => !p.endsWith(".dbg.json") && p.includes(".t.sol"))
    .map(loadContract.bind(null, hardhatConfig))
    .filter((ts) => !EXCLUDED_TEST_SUITES.has(ts.id.name));

  const runner = new SolidityTestRunner(tmpDir, gasReport, (...args) => {
    console.error(`${args[1].name} took ${elapsedSec(start)} seconds`);
  });
  const results = await runner.runTests(testSuites);
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

// Load a contract built with Hardhat into a test suite
function loadContract(hardhatConfig, artifactPath) {
  const compiledContract = require(artifactPath);

  const artifactId = {
    // Artifact cache path is ignored
    artifactCachePath: "./none",
    name: compiledContract.contractName,
    solcVersion: hardhatConfig.solidity.version,
    source: compiledContract.sourceName,
  };

  const testContract = {
    abi: JSON.stringify(compiledContract.abi),
    bytecode: compiledContract.bytecode,
    libsToDeploy: [],
    libraries: [],
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
