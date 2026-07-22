// SPDX-License-Identifier: MIT
pragma solidity ^0.8.34;

/// Stack-trace inference scenarios: each contract targets a StackTraceEntry
/// variant that Scenarios.t.sol doesn't reach — dispatch-level errors
/// (payability, missing fallback/receive, calldata decoding), returndata
/// shape, calls to non-contract accounts, and external library linking.

contract NotPayable {
    uint256 public stored;

    function store(uint256 v) public {
        stored = v;
    }
}

contract NoFallback {
    function ping() public pure returns (uint256) {
        return 1;
    }
}

contract NonPayableFallback {
    uint256 public hits;

    fallback() external {
        hits += 1;
    }
}

contract RequiresArgs {
    function needsBoth(uint256 a, uint256 b) public pure returns (uint256) {
        return a + b;
    }
}

interface IReturnsWord {
    function get() external returns (uint256);
}

contract ReturnsNothing {
    function get() external {}
}

contract ExpectsWord {
    function callGet(address target) public returns (uint256) {
        return IReturnsWord(target).get();
    }
}

library ExternalLib {
    function fail() public pure {
        require(false, "external lib boom");
    }
}

contract UsesExternalLib {
    function go() public pure {
        ExternalLib.fail();
    }
}

contract GuardedBareRevert {
    bool public armed = true;

    modifier guarded() {
        if (armed) {
            revert();
        }
        _;
    }

    function fire() external guarded {}
}

contract ValidatedCounter {
    uint256 public count;

    modifier validates(uint256 v) {
        require(v != 13, "unlucky");
        require(v < 1000, "too large");
        _;
        require(count != 666, "post-mortem");
    }

    function bumpIfValid(uint256 v) public validates(v) {
        count = v;
    }
}

contract ValidatedCounterCaller {
    ValidatedCounter public target;

    constructor() {
        target = new ValidatedCounter();
    }

    function callBump(uint256 v) public {
        target.bumpIfValid(v);
    }
}
