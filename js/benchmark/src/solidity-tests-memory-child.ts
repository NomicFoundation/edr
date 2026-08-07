/**
 * Internal entry point for the `solidity-tests-memory` benchmark, spawned by
 * `runSolidityTestsMemoryBenchmark` — one process per measurement, because
 * `maxRSS` is a per-process high-water mark that never decreases.
 *
 * Parameters arrive as a single JSON argument, and the result leaves as a
 * `MEMORY_RESULT`-prefixed JSON line on stdout.
 */

import {
  EdrContext,
  L1_CHAIN_TYPE,
  l1SolidityTestRunnerFactory,
} from "@nomicfoundation/edr";

import {
  compileSolidityTestsInput,
  runSolidityTestsMemoryChild,
  printMemoryResult,
  MemoryChildParams,
} from "./solidity-tests.js";

const rawParams = process.argv[2];
if (rawParams === undefined) {
  throw new Error(
    "Missing parameters argument. This script is spawned by the " +
      "solidity-tests-memory benchmark driver and is not meant to be run directly."
  );
}
const params: MemoryChildParams = JSON.parse(rawParams);

if (params.compileOnly === true) {
  await compileSolidityTestsInput(params.repoPath);
} else {
  const context = new EdrContext();
  await context.registerSolidityTestRunnerFactory(
    L1_CHAIN_TYPE,
    l1SolidityTestRunnerFactory()
  );

  const result = await runSolidityTestsMemoryChild(
    context,
    L1_CHAIN_TYPE,
    params.repo,
    params.repoPath,
    params.verbosity
  );
  printMemoryResult(result);
}
