// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "ds-test/test.sol";

    struct FuzzSelector {
        address addr;
        bytes4[] selectors;
    }

contract AfterInvariantHandler {
    uint256 public count;

    function inc() external {
        count += 1;
    }
}

contract InvariantAfterInvariantTest is DSTest {
    AfterInvariantHandler handler;

    function setUp() public {
        handler = new AfterInvariantHandler();
    }

    function targetSelectors() public returns (FuzzSelector[] memory) {
        FuzzSelector[] memory targets = new FuzzSelector[](1);
        bytes4[] memory selectors = new bytes4[](1);
        selectors[0] = handler.inc.selector;
        targets[0] = FuzzSelector(address(handler), selectors);
        return targets;
    }

    function afterInvariant() public {
        require(handler.count() < 10, "afterInvariant failure");
    }

    function invariant_after_invariant_failure() public view {
        require(handler.count() < 20, "invariant after invariant failure");
    }

    function invariant_failure() public view {
        require(handler.count() < 9, "invariant failure");
    }

    function invariant_success() public view {
        require(handler.count() < 11, "invariant should not fail");
    }
}
