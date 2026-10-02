---
"@nomicfoundation/edr": minor
---

Fuzz tests now drive their runs directly instead of through the `proptest` test runner. Fuzz failures are persisted as one JSON counterexample per test at `<failurePersistDir>/<failurePersistFile>/<contract name>/<test name>` and replayed first on the next run; a leftover `proptest` seed file at `<failurePersistDir>/<failurePersistFile>` from earlier versions is removed at the start of the test run.

BREAKING CHANGE: A `maxTestRejects` of `0` now disables the `vm.assume` reject limit instead of failing the test on the first rejected input.
