// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "../contracts/linking-same-library-from-different-versions/CounterSameSolc.sol";

contract CounterSameSolcTest {
    // Test calling a contract that links a library where both the contract and the library were compiled with the same solc version.
    function testSameSolc() public {
        CounterSameSolc counter = new CounterSameSolc();
        counter.increment();
        require(counter.x() == 1, "Counter increment failed");
    }
}
