// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract TimeoutHandler is DSTest {
    uint256 public count;

    function increment() public {
        count++;
    }
}

contract TimeoutTest is DSTest {
    TimeoutHandler handler;

    function setUp() public {
        handler = new TimeoutHandler();
    }

    function invariant_counter_timeout() public view {
        // Invariant will fail if more than 10000 increments.
        // Make sure test timeouts after one second and remaining runs are canceled.
        require(handler.count() < 10000);
    }
}