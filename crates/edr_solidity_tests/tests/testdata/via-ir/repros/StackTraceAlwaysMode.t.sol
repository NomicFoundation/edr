// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

// Fixture for `always_mode_produces_stack_trace_for_failing_test`: a failing
// test that has a `setUp`, and reverts inside a called contract.
contract AlwaysStackTraceTest is DSTest {
    Reverter reverter;
    uint256[] public fixtureAmount = [1];

    function setUp() public {
        reverter = new Reverter();
    }

    // `test`-prefixed so the revert counts as a failure (triggers a stack trace).
    function testRevertHasStackTrace() public {
        reverter.boom();
    }

    // `table`-prefixed with a param => runs as a table test (fed by
    // `fixtureAmount`), covering the table-test stack-trace path.
    function tableRevertHasStackTrace(uint256 amount) public {
        require(amount == 1, "amount");
        reverter.boom();
    }

    // `test`-prefixed with a param => runs as a fuzz test. Reverts for every
    // input, so the first run yields a counterexample, covering the fuzz
    // stack-trace path. The param is unnamed so it doesn't pick up
    // `fixtureAmount`.
    function testFuzzRevertHasStackTrace(uint256) public {
        reverter.boom();
    }
}

contract Reverter {
    function boom() public pure {
        require(false, "boom");
    }
}

// Covers the invariant stack-trace path: `increment` is the only fuzzable
// function on `Counter` (the getter is view, so it's excluded), so the first
// fuzzed call breaks the invariant deterministically.
contract AlwaysStackTraceInvariantTest is DSTest {
    Counter counter;

    function setUp() public {
        counter = new Counter();
    }

    function invariantCountIsZero() public view {
        require(counter.count() == 0, "count is not zero");
    }
}

contract Counter {
    uint256 public count;

    function increment() public {
        count += 1;
    }
}

// Covers the invariant setup-failure stack-trace path: the invariant is
// already broken in the initial state, so the campaign fails during the
// initial invariant check, before any fuzzed calls. `Counter` is deployed so
// there is a fuzzable target and target selection doesn't fail first.
contract AlwaysStackTraceInvariantInitialTest is DSTest {
    Counter counter;
    Reverter reverter;

    function setUp() public {
        counter = new Counter();
        reverter = new Reverter();
    }

    function invariantAlwaysBroken() public view {
        reverter.boom();
    }
}
