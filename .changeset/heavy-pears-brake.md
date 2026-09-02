---
"@nomicfoundation/edr": minor
---

Reduced the memory footprint of the Solidity test runner. Call trace arenas that nothing will consume are now freed as soon as each test finishes instead of surviving until the whole suite completes, and the recorded EVM steps (one entry per executed opcode) are stripped from the arenas that are retained. Invariant runs no longer collect traces beyond the gas report's sample budget, nor retain the arenas of a passing test's replayed run when no result will surface them, and a suite's `setUp()` traces are freed once nothing can surface them and otherwise shared by its test results rather than copied into each of them.

This fixes the out-of-memory failures at Hardhat verbosity `-vvvv` on large test suites, and lowers peak memory at the verbosities that record EVM steps (`-vvv` and above) — by 73% on an invariant-heavy suite at `-vvv`.

Changed invariant campaigns that fail without executing any EVM call (for example an ABI error, or too many `vm.assume` rejections) to report no stack trace when stack traces are collected on every run (`CollectStackTraces.Always`), rather than a heuristic failure derived from the unrelated `setUp()` trace. The result's `reason` already explains such failures.

Fixed Solidity test stack traces for tests carrying an inline-configuration `isolate` or `evmVersion` directive: when a stack trace is computed only for failing tests (`CollectStackTraces.OnFailure`, the default), the failing test is re-executed to compute it, and that re-run applied the directive before `setUp()` instead of after it as the original run does. An `evmVersion` older than the suite's could therefore make a `setUp()` that had succeeded fail in the re-run, replacing the stack trace with a "Test setup unexpectedly failed during execution with revert reason: …" error, and `isolate` could shift the re-run's deployments away from the original run's.

BREAKING CHANGE: Renamed the `IncludeTraces` enum to `IncludeCallTraces`, and the `includeTraces` property of `SolidityTestRunnerConfigArgs` to `includeCallTraces`, matching the `ObservabilityConfig.includeCallTraces` provider option that already used this enum. The type of `ObservabilityConfig.includeCallTraces` changes with the rename. `IncludeCallTraces` controls whether call traces are included in the results; `CollectStackTraces` separately controls whether a stack trace is computed for failing tests.
