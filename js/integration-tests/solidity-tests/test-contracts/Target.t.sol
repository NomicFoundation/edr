// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";

contract InvariantTestNoTarget is Test {
    uint256 count;

    function setCount(uint256  _count) public {
        count = _count;
    }

    function setUp() public {
    }

    function invariant_check_count() public {
    }
}

contract InvariantTestTarget is Test {
    uint256 count;

    function setCount(uint256  _count) public {
        count = _count;
    }

    function setUp() public {
        targetContract(address(this));
    }

    function invariant_check_count() public {
    }
}