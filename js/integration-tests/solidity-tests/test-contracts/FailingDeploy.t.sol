// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract FailingDeployTest {
    uint immutable value;

    constructor() {
        revert("Deployment failed");
        value = 1; // Unreachable but needed for immutable
    }

    // This is only here to treat this as a test suite
    function testOk() public pure {
        assert(true);
    }
}
