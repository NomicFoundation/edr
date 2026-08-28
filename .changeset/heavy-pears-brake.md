---
"@nomicfoundation/edr": minor
---

Reduced the memory footprint of the Solidity test runner: trace arenas that
nothing will consume are now freed as soon as each test finishes, recorded EVM
steps are stripped from arenas retained for call traces, and invariant runs no
longer accumulate per-run traces when no gas report was requested. This fixes
out-of-memory failures at Hardhat verbosity `-vvv` and above on large test
suites.

BREAKING: the `IncludeTraces` enum was renamed to `IncludeCallTraces`, and the
`includeTraces` property of `SolidityTestRunnerConfigArgs` was renamed to
`includeCallTraces`, matching the provider's `includeCallTraces` observability
option. `IncludeCallTraces` controls whether call trace arenas are included in
test results; `CollectStackTraces` controls whether a stack trace is computed
for failing tests.
