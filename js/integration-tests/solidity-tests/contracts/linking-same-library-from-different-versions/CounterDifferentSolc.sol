// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.7.0;

import "./Lib.sol";

contract CounterDifferentSolc {
    uint public x;

    function increment() public {
        x = Lib.add(x, 1);
    }
}
