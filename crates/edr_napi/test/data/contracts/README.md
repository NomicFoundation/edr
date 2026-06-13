# Solidity test fixtures

These `.sol` sources back the EIP-712 lazy-resolution integration tests in
`test/solidity-tests.ts`. Unlike most fixtures in this repo, the EIP-712
cheatcodes (`vm.eip712HashType`, `vm.eip712HashStruct`) parse the running test
contract's **source files from disk** on demand, so these sources must exist on
disk at the `sourceName` recorded in their compiled artifacts (resolved
relative to `projectRoot`, which the tests set to the `test/` directory).

## Files

- `Eip712LazyTest.t.sol` — defines `Person`/`Mail`/`Point` locally, imports
  `Asset` via a relative import and `Coupon` via a mapped (`@fixtures/...`)
  import. Compiled to `../artifacts/default/Eip712LazyTest.json`.
- `Eip712Imported.sol` — `Asset`, reached via a relative import.
- `external/Eip712External.sol` — `Coupon`, reached via a mapped import. The
  test maps `@fixtures/Eip712External.sol` to this file through the
  `eip712ImportMappings` runner config.
- `Eip712UnknownTest.t.sol` — references an undefined type to check that
  unresolvable lookups fail. Compiled to
  `../artifacts/default/Eip712UnknownTest.json`.

## Recompiling

The artifacts are committed pre-compiled (there is no build step in this
package's test run). They were produced with **solc 0.8.24** using the standard
JSON interface, with the remapping `@fixtures/=data/contracts/external/` and
source keys equal to each file's path relative to `test/` (e.g.
`data/contracts/Eip712LazyTest.t.sol`), so the artifact `sourceName` matches
where the provider reads the source from. After changing any `.sol` file here,
recompile and refresh the corresponding artifact JSON in
`../artifacts/default/`, keeping the `contractName`, `sourceName`, and
`solcVersion` ("0.8.24") fields.
