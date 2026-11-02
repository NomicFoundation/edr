# Stack traces tests

> ⚠️ _This explanation was written on 2023-07-04, and can be out of date or incomplete. If you think there's something wrong, or that some important part of the explanation is missing, please open an issue about it._ ⚠️

This directory contains tests related to our tracing engine. They are mainly about the generated stack traces, but they also test things like inferred errors and console logs.

These tests are written with an ad-hoc mini framework, where each test corresponds to a directory under this one. Each of these directories has a `test.json` file describing the test, and Solidity files that are used by that test. The `edr_stack_trace_tests` crate then consumes these files to generate and execute the corresponding tests.

## Example

One simple example of a test is `0_8/call-message/external-calls/non-contract-account-called`. This test checks that, when a transaction makes a call to an address that is not a contract, the proper error is inferred by our heuristics and that the frame in the stack trace has the right file name, line number, contract name and function name.

```json
{
  "transactions": [
    {
      "file": "c.sol",
      "contract": "C"
    },
    {
      "to": 0,
      "function": "test",
      "stackTrace": [
        {
          "type": "NONCONTRACT_ACCOUNT_CALLED_ERROR",
          "sourceReference": {
            "contract": "C",
            "file": "c.sol",
            "function": "test",
            "line": 11
          }
        }
      ]
    }
  ]
}
```

Each `test.json` file can have several fields, but the only mandatory one is a `transactions` array with the list of transactions that should be executed. The `TestDefinition` struct in [`corpus.rs`](../src/corpus.rs) describes the possible contents of a `test.json` file.

Transactions can be deployments or calls. In this case the first transaction deploys the contract `C` in `c.sol`, and the second transaction calls the function `test` in that contract. The contract that is called is indicated by `"to": 0`, which corresponds to the first contract deployed by this test.

Each transaction can have some "assertion" fields, which describe what should happen when that transaction is executed. The most common one is `stackTrace`, which indicates that the transaction should revert and that EDR should generate the given stack trace. In this case, the test is saying that the transaction should revert with a single frame, which has an inferred `NONCONTRACT_ACCOUNT_CALLED_ERROR` error, and points to line 11 of the `c.sol` file.

Another possible assertion field is `consoleLogs`, which specifies that the transaction should emit certain logs with `console.sol`. See for example [this `test.json`](0_8/console-logs/uint/uint/test.json).

## Directory structure

Each directory immediately under this one corresponds to a minor version of Solidity. In practice this means that tests under `0_8` will have a `pragma solidity ^0.8.0`, but it is possible to specify tests that are only run for a more precise range of versions.

There is one other directory, `version-independent`, which has tests that can be run with any version of solc.

## Running a single test

Set `EDR_STACK_TRACE_FILTER` to part of a test's path, e.g. `EDR_STACK_TRACE_FILTER=0_8/console-logs/uint/uint cargo test -p edr_stack_trace_tests`, to run only the tests whose path contains it.

## How tests are executed

[`tests/solc.rs`](../tests/solc.rs) lists the compiler configurations the tests run under, each with the directories it runs: a solc version, plus optimizer settings, and the minor-version directory matching it together with `version-independent`.

The solidity files in each test directory are compiled with that configuration and then the `test.json` description is executed. The tests a configuration is known to fail are listed in `known-failures/<configuration>.txt`; a failing test that isn't listed fails the run, and so does a listed test that passes.
