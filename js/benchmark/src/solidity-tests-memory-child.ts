/**
 * Internal entry point for the `solidity-tests-memory` benchmark, spawned by
 * `runSolidityTestsMemoryBenchmark` — one process per measurement, because
 * `maxRSS` is a per-process high-water mark that never decreases.
 *
 * The driver uses `spawn` rather than `fork` so the child can be wrapped in
 * `/usr/bin/time`; that leaves no IPC channel, so parameters arrive as a single
 * JSON argument and the result leaves as a `MEMORY_RESULT`-prefixed JSON line
 * on stdout.
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
const params = JSON.parse(rawParams) as Partial<MemoryChildParams>;
if (
  typeof params.repo !== "string" ||
  typeof params.repoPath !== "string" ||
  typeof params.verbosity !== "number"
) {
  throw new Error(`malformed memory-benchmark parameters: ${rawParams}`);
}

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
