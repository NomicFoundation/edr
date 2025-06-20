// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import "./Lib.sol";

contract CounterSameSolc {
    uint public x;

    function increment() public {
        x = Lib.add(x, 1);
    }
}
