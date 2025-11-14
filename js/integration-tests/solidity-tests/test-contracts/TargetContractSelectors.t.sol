// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

contract InvariantTargetTestSelectors is Test {
    bool fooCalled;
    bool testSanityCalled;
    bool testTableCalled;
    uint256 invariantCalledNum;
    uint256 setUpCalledNum;

    function setUp() public {
       targetContract(address(this));
    }

    function beforeTestSetup() public {
    }

    // Only this selector should be targeted.
    function foo() public {
        fooCalled = true;
    }

    function fixtureCalled() public returns (bool[] memory) {
    }

    function table_sanity(bool called) public {
        testTableCalled = called;
    }

    function test_sanity() public {
        testSanityCalled = true;
    }

    function afterInvariant() public {
    }

    function invariant_foo_called() public view {
    }

    function invariant_testSanity_considered_target() public {
    }

    function invariant_setUp_considered_target() public {
        setUpCalledNum++;
    }

    function invariant_considered_target() public {
        invariantCalledNum++;
    }
}