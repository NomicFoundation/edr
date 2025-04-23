// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.7.0;

import "./CounterOld.sol";

contract CounterOldTest {
    function test() public {
        CounterOld counter = new CounterOld();
        counter.increment();
        require(counter.x() == 1, "Counter increment failed");
    }
}