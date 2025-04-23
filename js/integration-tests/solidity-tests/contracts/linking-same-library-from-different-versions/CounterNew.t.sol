// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "./CounterNew.sol";

contract CounterNewTest {
    function test() public {
        CounterNew counter = new CounterNew();
        counter.increment();
        require(counter.x() == 1, "Counter increment failed");
    }
}