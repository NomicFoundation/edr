// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import {Test} from "forge-std/src/Test.sol";

contract TargetContractTest1 {
    bool public flag1 = true;

    function set1(int256 val) public returns (bool) {
        // Condition that can never be true
        if (val % 10 == 0 && val % 10 == 1) {
            flag1 = false;
        }
        return flag1;
    }
}

contract InvariantTestFunctionOverride is Test {
    TargetContractTest1 inv;

    function setUp() public {
        inv = new TargetContractTest1();
    }

    function invariant_neverFalse() public view {
        require(inv.flag1(), "false");
    }
}
