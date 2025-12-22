# Test

Functionality in EDR is tested in two ways:

1. Rust unit & integration tests
2. End-to-end (E2E) tests of EDR in Hardhat

As EDR matures, we will gradually be moving over Hardhat E2E tests to granular unit & integration tests in EDR.

## EDR

Part of EDR's test suite requires a working internet connection. Those tests are marked with the `test-remote` feature flag. EDR uses both Alchemy and Infura as Ethereum mainnet providers for its remote tests. This requires their API URLs (including token) to be set in the `ALCHEMY_URL` and `INFURA_URL` environment variables (a free tier token should suffice for local development). Tests may use different chains, some of which may not be enabled by default when creating a new alchemy app API Key, make sure to enable them - i.e.: `base-sepolia`, `arb-mainnet` and `polygon-mainnet`.

To run all tests, including remote tests, execute:

```bash
cargo t --all-features
```

To only run local tests, execute:

```bash
cargo t --features serde,std,tracing
```

## Hardhat

To run Hardhat integration tests, execute:

```bash
cd hardhat-tests &&
pnpm test
```

Similar to EDR, Hardhat can be configured to run remote tests. This can be accomplished by setting environment variables for the API URL (including token) of Alchemy or Infura, respectively: `ALCHEMY_URL` and `INFURA_URL`.

### Filtering Tests

Specific tests can be executed by filtering with the `--grep` or `-g` flag. E.g.:

```bash
pnpm test -- -g "Reads from disk if available, not making any request a request"
```

will only run the test with this specific name.

Hierarchies of tests can be filtered by separating the levels with spaces. E.g. the test matching

```
Alchemy Forked provider
  hardhat_impersonateAccount
    hash collisions
```

can be run by using the following command:

```bash
pnpm test -- -g "Alchemy Forked provider hardhat_impersonateAccount hash collisions"
```
