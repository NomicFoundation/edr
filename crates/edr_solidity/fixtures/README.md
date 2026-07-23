# Fixtures

Committed compiler inputs and outputs used by the `edr_solidity` unit tests (mostly the DWARF tests in `src/debug_info/dwarf.rs`).

Don't edit the JSON files by hand — each pair is the output of a build flow. To update one, run its flow below and commit the result. If a freshly regenerated file differs from the committed one when you didn't expect it to, something in the toolchain moved (solx release, hardhat-solx settings, …); track that down instead of tweaking the JSON until it fits.

| Fixture | What it is | How to regenerate |
| --- | --- | --- |
| `solx_compiler_{input,output}.json` | solx compile of `sources/Counter.sol` | `cargo run -p edr_tool_cli -- gen-solx-fixtures <path-to-solx>` (regenerates both solx pairs) |
| `solx_compiler_{input,output}_stack_trace_scenarios.json` | solx compile of `sources/StackTraceScenarios{,Base}.sol`, the stack-trace scenario corpus | Same `gen-solx-fixtures` run |
| `compiler_{input,output}.json` | Minimal solc pair (a single inline `literal.sol`) for the artifact-parsing unit tests | Hand-maintained (predates this index) |

Conventions:

- Solidity source text lives once, in `sources/`. The committed solx inputs have `"content": ""`, and the tests fill the source back in when loading the fixture.
- Tests pin line numbers (and a few PCs) in the scenario sources — append new scenarios rather than shifting existing lines. After a regen that changes codegen (solx bump, optimizer change), re-derive moved PC anchors with a temporary probe next to the failing test: iterate `decode_deployed_for(...)`, filter for the opcode you're after (`REVERT`, `INVALID`), print each candidate's `pc` and resolved line under `cargo test -p edr_solidity -- --nocapture`, and pick the instruction whose line matches the test's intent.
- To see which solx built an output, check the bytecode's trailing CBOR metadata: the `solcx` key holds e.g. `solx:0.1.6;solc:0.8.34`.

Known issue: solx embeds the build directory in `debugInfo` ([solx#594](https://github.com/NomicFoundation/solx/issues/594)), so those bytes differ across checkouts even on identical toolchains — expect a diff there when regenerating from a different directory.
