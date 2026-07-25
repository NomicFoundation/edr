# Fixtures

Committed compiler inputs and outputs used by the `edr_solidity` unit tests (mostly the DWARF tests in `src/debug_info/dwarf.rs`).

Don't edit the JSON files by hand — each pair is the output of a build flow. To update one, run its flow below and commit the result. If a freshly regenerated file differs from the committed one when you didn't expect it to, something in the toolchain moved (solx release, hardhat-solx settings, …); track that down instead of tweaking the JSON until it fits.

| Fixture | What it is | How to regenerate |
| --- | --- | --- |
| `solx_compiler_{input,output}_scenarios.json` | solx compile of `sources/Scenarios.t.sol`, produced by the parity-sweep project | `pnpm regen-fixtures` in [`js/integration-tests/solx-parity-sweep`](../../../js/integration-tests/solx-parity-sweep/README.md) |
| `solx_compiler_{input,output}.json` | solx compile of `sources/Counter.sol` | `cargo run -p edr_tool_cli -- gen-solx-fixtures <path-to-solx>` (regenerates both `gen-solx-fixtures` rows) |
| `solx_compiler_{input,output}_stack_trace_scenarios.json` | solx compile of `sources/StackTraceScenarios.sol`, the provider-path stack-trace scenarios | Same `gen-solx-fixtures` run |
| `solx_compiler_{input,output}_stack_trace_scenarios_mode3.json` | The same source at optimizer mode 3 — the only committed artifacts that reach the inference's declaration-attributed/unmapped-revert compat paths | Same `gen-solx-fixtures` run |
| `compiler_{input,output}.json` | Minimal solc pair (a single inline `literal.sol`) for the artifact-parsing unit tests | Hand-maintained (predates this index) |

Conventions:

- Solidity source text lives once, in `sources/`. The committed solx inputs have `"content": ""`, and the tests fill the source back in when loading the fixture. `Scenarios.t.sol` is also the sweep's test corpus, so between regenerations it may only be appended to — enforced by `scenarios_source_is_append_only` in `src/debug_info/dwarf.rs`.
- To see which solx built an output, check the bytecode's trailing CBOR metadata: the `solcx` key holds e.g. `solx:0.1.4;solc:0.8.34`.

Known issue: solx embeds the build directory in `debugInfo` ([solx#594](https://github.com/NomicFoundation/solx/issues/594)), so those bytes differ across checkouts even on identical toolchains — expect a diff there when regenerating from a different directory.
