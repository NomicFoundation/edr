# Solidity test fixtures

These `.sol` sources back the EIP-712 type-resolution integration tests in
`test/solidity-tests.ts`. Unlike most fixtures here, these files are read at
run time rather than only compiled ahead of it: the EIP-712 cheatcodes
(`vm.eip712HashType`, `vm.eip712HashStruct`) resolve type names by parsing the
running test contract's **source files from disk**, eagerly at runner
creation. The sources are read from the absolute paths supplied through the
`testSourcePaths` runner config, keyed by the `sourceName` recorded in their
compiled artifacts (the tests point each entry into this directory).

## Files

- `Eip712ResolveTest.t.sol` — defines `Person`/`Mail`/`Point` locally, imports
  `Asset` via a relative import and `Coupon` via a mapped (`@fixtures/...`)
  import. Compiled to `../artifacts/default/Eip712ResolveTest.json`.
- `Eip712Imported.sol` — `Asset`, reached via a relative import.
- `external/Eip712External.sol` — `Coupon`, reached via a mapped import. The
  test maps `@fixtures/Eip712External.sol` to this file through the
  `importMappings` runner config.
- `Eip712UnknownTest.t.sol` — references an undefined type to check that
  unresolvable lookups fail. Compiled to
  `../artifacts/default/Eip712UnknownTest.json`.
- `Eip712SyntaxError.sol` — deliberately broken (never compiled); a test
  points a suite's `testSourcePaths` entry at it to check that an unparseable
  source rejects the run up front.

## Recompiling

The artifacts are committed pre-compiled (there is no build step in this
package's test run). They were produced with **solc 0.8.24** using the standard
JSON interface, with the remapping `@fixtures/=data/contracts/external/` and
source keys equal to each file's path relative to `test/` (e.g.
`data/contracts/Eip712ResolveTest.t.sol`), so the artifact `sourceName` matches
the keys the tests use in `testSourcePaths`. After changing any `.sol` file here,
recompile and refresh the corresponding artifact JSON in
`../artifacts/default/`, keeping the `contractName`, `sourceName`, and
`solcVersion` ("0.8.24") fields.
