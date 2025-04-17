// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

// Contract to be tested with overflow vulnerability
contract MyContract {
    function addWithOverflow(uint256 a, uint256 b) public pure returns (uint256) {
        return a + b;
    }
}

// A fuzz function that calls an impure function during setup that errors.
contract ImpureFuzzSetup is Test {
    MyContract public myContract;
    string fileContents;

    function setUp() public {
        myContract = new MyContract();
        fileContents = vm.readFile("./invalid-path");
    }

    function testFuzzAddWithOverflow(uint256 a, uint256 b) public {
        uint256 result = myContract.addWithOverflow(a, b);
        assertEq(result, a + b);
    }
}

// A fuzz function that calls an impure function during contract execution that errors.
contract ImpureFuzzTest is Test {
    MyContract public myContract;

    function setUp() public {
        myContract = new MyContract();
    }

    function testFuzzAddWithOverflow(uint256 a, uint256 b) public {
        // Impure cheatcode
        assert(vm.unixTime() > 0);
        uint256 result = myContract.addWithOverflow(a, b);
        assertEq(result, a + b);
    }
}
