---
"@nomicfoundation/edr": minor
---

Added Solidity stack traces for build infos with `compilerType: "slang"`. The compiler output must include the per-source `debugSymbols` output selection; without it, the build info contributes no contracts to stack traces and a warning is logged.
