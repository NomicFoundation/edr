// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

contract StochasticWrongContract {
    uint256 public a;
    uint256 public b;
    uint256 public both;

    function addToA(uint256 amount) external {
        // This is an intentional bug in the contract to trigger invariant failure.
        // If the conditional is removed, the invariant will pass.
        if (amount % 13 != 0) {
            a += amount;
        }
        both += amount;
    }

    function addToB(uint256 amount) external {
        b += amount;
        both += amount;
    }
}

// Test that the invariant testing works correctly by catching a bug in the contract.
contract FailingInvariantTest is Test {
    StochasticWrongContract wrongContract;

    function setUp() external {
        wrongContract = new StochasticWrongContract();
    }

    function invariant() external {
        assertEq(wrongContract.a() + wrongContract.b(), wrongContract.both());
    }
}

// Test where the invariant condition reverts on an empty sequence.
contract BuggyInvariantTest is Test {
    StochasticWrongContract wrongContract;

    function setUp() external {
        wrongContract = new StochasticWrongContract();
    }

    function invariant() external {
        require(1 == 2, "one is not two");
    }
}

// Test where the invariant test is failing, but uses an impure cheatcode so we can't generate stack traces.
contract ImpureInvariantTest is Test {
    StochasticWrongContract wrongContract;

    function setUp() external {
        wrongContract = new StochasticWrongContract();
    }

    function invariant() external {
        assert(vm.unixTime() > 0);
        assertEq(wrongContract.a() + wrongContract.b(), wrongContract.both());
    }
}
