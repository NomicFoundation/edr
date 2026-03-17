// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

contract Deploy {
    bool public setupCalled;

    constructor(bool _setupCalled) {
        setupCalled = _setupCalled;
    }
}

contract SetUpSuccessTest is Test {
    bool public setUpCalled = false;

    function setUp() public {
        setUpCalled = true;
    }

    function test_a() public {
        Deploy deploy = new Deploy(setUpCalled);

        assertTrue(deploy.setupCalled());
    }

    function test_b() public {
        Deploy deploy = new Deploy(setUpCalled);

        assertTrue(deploy.setupCalled());
    }
}

contract SetUpRevertTest is Test {
    function setUp() public pure {
        revert("setUp intentionally reverted");
    }

    function test_a() public {
        Deploy deploy = new Deploy(false);

        // This test should not run because setUp reverts.
        assertTrue(deploy.setupCalled(), "This test should not run");
    }

    function test_b() public {
        Deploy deploy = new Deploy(true);

        // This test should not run because setUp reverts.
        assertTrue(deploy.setupCalled(), "This test should not run");
    }
}
