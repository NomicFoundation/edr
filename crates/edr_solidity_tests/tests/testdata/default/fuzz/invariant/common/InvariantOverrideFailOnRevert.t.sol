// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "ds-test/test.sol";

struct FuzzSelector {
    address addr;
    bytes4[] selectors;
}

contract Handler is DSTest {
    function doSomething() public {
        require(false, "failed on revert");
    }
}

// Same as `InvariantHandlerFailure`, but the invariant opts into
// `fail-on-revert` via inline configuration.
contract InvariantOverrideFailOnRevert is DSTest {
    bytes4[] internal selectors;

    Handler handler;

    function targetSelectors() public returns (FuzzSelector[] memory) {
        FuzzSelector[] memory targets = new FuzzSelector[](1);
        bytes4[] memory selectors = new bytes4[](1);
        selectors[0] = handler.doSomething.selector;
        targets[0] = FuzzSelector(address(handler), selectors);
        return targets;
    }

    function setUp() public {
        handler = new Handler();
    }

    /// forge-config: default.invariant.fail-on-revert = true
    function statefulFuzz_BrokenInvariant() public {}
}
