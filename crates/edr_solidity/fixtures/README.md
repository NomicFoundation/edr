# Fixtures

Committed compiler artifacts are outputs of documented flows. To understand or regenerate one, run its documented flow and diff against the committed file; if they diverge, the toolchain changed — find that change rather than reconstructing the artifact some other way.

| Fixture | What it is | Regeneration |
| --- | --- | --- |
| `solx_compiler_{input,output}_scenarios.json` | solx compile of `sources/Scenarios.t.sol` by the parity-sweep project | The provenance section of [the sweep README](../../../js/integration-tests/solx-parity-sweep/README.md) |
| `solx_compiler_{input,output}.json` | solx compile of `sources/Counter.sol` | Tooling in review ([#1552](https://github.com/NomicFoundation/edr/pull/1552), `edr_tool_cli gen-solx-fixtures`); until then, `solx --standard-json` with the committed input plus the spliced source |
| `compiler_{input,output}.json` | Minimal solc pair (single inline `literal.sol`) for the artifact-parsing unit tests | Hand-maintained; provenance predates this index |

Conventions:

- Solidity source text lives once, in `sources/`; the solx compiler inputs commit `content: ""` and the tests splice the source in at load time. `Scenarios.t.sol` doubles as the sweep's corpus — see its append-only rule (`scenarios_source_is_append_only` in `src/debug_info/dwarf.rs`).
- Which solx produced an output is stamped in each bytecode's trailing CBOR metadata (`solcx` key, e.g. `solx:0.1.4;solc:0.8.34`).
- solx `debugInfo` embeds the build directory ([solx#594](https://github.com/NomicFoundation/solx/issues/594)), so those bytes differ across checkouts even on identical toolchains.
