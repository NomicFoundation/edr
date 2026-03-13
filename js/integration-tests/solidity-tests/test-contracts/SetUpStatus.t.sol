// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

contract SetUpSuccessTest is Test {
    bool public setUpCalled = false;

    function setUp() public {
        setUpCalled = true;
    }

    function test_a() public view {
        assertTrue(setUpCalled);
    }

    function test_b() public view {
        assertTrue(setUpCalled);
    }
}

contract SetUpRevertTest is Test {
    function setUp() public pure {
        revert("setUp intentionally reverted");
    }

    function test_a() public pure {
        // This test should not run because setUp reverts.
        assertTrue(false, "This test should not run");
    }

    function test_b() public pure {
        // This test should not run because setUp reverts.
        assertTrue(true, "This test should not run");
    }
}
