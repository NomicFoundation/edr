# Solidity test fixtures

These `.sol` sources back the EIP-712 type-resolution integration tests in `test/solidity-tests.ts`. Unlike most fixtures here, these files are read at run time rather than only compiled ahead of it. The EIP-712 cheatcodes (`vm.eip712HashType`, `vm.eip712HashStruct`) resolve type names by parsing the running test contract's sources from disk. That parsing happens eagerly when a run starts, over the sources of the suites the run selected.

The sources are read from the absolute paths supplied through the `testSourcePaths` runner config. Each path is keyed by the `sourceName` recorded in its compiled artifact, and the tests point every entry into this directory.

## Recompiling

The artifacts are committed pre-compiled, because this package's test run has no build step. They were produced with solc 0.8.24 through the standard JSON interface, using the remapping `@fixtures/=data/contracts/external/`. Each source key is the file's path relative to `test/`, for example `data/contracts/Eip712ResolveTest.t.sol`. An artifact's `sourceName` therefore matches the key the tests use in `testSourcePaths`.

After changing any `.sol` file here, recompile and refresh the corresponding artifact JSON in `../artifacts/default/`, keeping the `contractName`, `sourceName`, and `solcVersion` ("0.8.24") fields.
