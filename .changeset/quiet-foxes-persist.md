---
"@nomicfoundation/edr": minor
---

- Added parallel workers for fuzz tests, capped by the new `workers` fuzz option (defaults to the available threads). On fuzz-heavy suites this trades higher CPU time and peak memory for shorter wall time; lower `workers` to bound that cost. With more than one worker, the runs before a failure and the reported counterexample depend on scheduling; set `workers: 1` for full reproducibility. Inputs for a given `seed` differ from earlier versions.
- Fixed `vm.random*` repeating the same sequence in every fuzz run under a fixed `seed`.
- Changed fuzz failure persistence to one JSON counterexample per test at `<failurePersistDir>/failures/<contract>/<test>` (`<test>-<selector>` for overloaded functions), replayed first on the next run. A leftover `proptest` seed file at `<failurePersistDir>/failures` is removed when the test run starts.
- BREAKING CHANGE: Removed the `failurePersistFile` fuzz option; the failure directory under `failurePersistDir` is always named `failures`, as in Foundry.
- BREAKING CHANGE: `maxTestRejects = 0` now disables the `vm.assume` reject limit instead of failing on the first rejected input, and requires a fuzz `timeout`.
