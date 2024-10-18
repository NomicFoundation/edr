## Solidity Test Runner Integratin Test Suite

A test suite that tests different aspects of the Solidity test runner.

### Structure

- [`core`](core): Tests for fundamental aspects of the Solidity test runner
- [`logs`](logs): Tests for logging capabilities
- [`cheats`](cheats): Tests for cheatcodes
- [`fuzz`](fuzz): Tests for the fuzzer
- [`trace`](trace): Tests for the tracer
- [`fork`](fork): Tests for the Solidity test runner's forking capabilities

---

### Running the tests using Hardhat

First, link your local checkout of hardhat v3 to node modules.

```bash
npm link $PATH_TO_HARDHAT/hardhat/v-next/hardhat
```

Then, run the tests using the following command:

```bash
npx hardhat test
```

Go to [hardhat.config.js](hardhat.config.js) to enable/disable test suites.
