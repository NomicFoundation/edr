// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./test.sol";
import "./Vm.sol";

contract StochasticWrongContract {
    uint256 public val1;
    uint256 public val2;
    uint256 public val3;

    function addToA(uint256 amount) external {
        // This is an intentional bug in the contract to trigger invariant failure.
        // If the conditional is removed, the invariant will pass.
        if (amount % 13 != 0) {
            val1 += amount;
        }
        val3 += amount;
    }

    function addToB(uint256 amount) external {
        val2 += amount;
        val3 += amount;
    }
}

// Test that the invariant testing works correctly by catching a bug in the contract.
contract FailingInvariantTest is DSTest {
    StochasticWrongContract val;

    function setUp() external {
        val = new StochasticWrongContract();
    }

    function invariant() external {
        assertEq(val.val1() + val.val2(), val.val3());
    }
}

