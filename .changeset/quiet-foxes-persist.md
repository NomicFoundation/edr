---
"@nomicfoundation/edr": minor
---

- Added parallel workers for fuzz tests, capped by the new `workers` fuzz option (defaults to the available threads). With more than one worker, the runs before a failure and the reported counterexample depend on scheduling; set `workers: 1` for full reproducibility. Inputs for a given `seed` differ from earlier versions.
- Fixed `vm.random*` repeating the same sequence in every fuzz run under a fixed `seed`.
- Changed fuzz failure persistence to one JSON counterexample per test at `<failurePersistDir>/<failurePersistFile>/<contract>/<test>` (`<test>-<selector>` for overloaded functions), replayed first on the next run. A leftover `proptest` seed file at `<failurePersistDir>/<failurePersistFile>` is removed when the test run starts.
- BREAKING CHANGE: `maxTestRejects = 0` now disables the `vm.assume` reject limit instead of failing on the first rejected input, and requires a fuzz `timeout`.
