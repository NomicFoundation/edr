---
"@nomicfoundation/edr": minor
---

Fuzz tests now run their runs on parallel workers, each with its own executor and RNG stream derived from the fuzz seed. The new optional `workers` fuzz configuration caps the number of workers per test (defaults to the number of available threads; every worker gets at least 64 runs). With more than one worker the number of runs before a failure and the reported counterexample depend on scheduling, even with a fixed seed; set `workers: 1` for fully reproducible results.
