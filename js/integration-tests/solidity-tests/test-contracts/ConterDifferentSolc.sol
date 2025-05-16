// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.7.0;

import "../contracts/linking-same-library-from-different-versions/CounterDifferentSolc.sol";

contract CounterDifferentSolcTest {
    // Test calling a contract that links a library where the contract and the library were compiled with different solc versions.
    function testDifferentSolc() public {
        CounterDifferentSolc counter = new CounterDifferentSolc();
        counter.increment();
        require(counter.x() == 1, "Counter increment failed");
    }
}
