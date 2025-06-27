// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Test} from "forge-std/src/Test.sol";

interface IL1Block {
    function basefee() external view returns (uint256);
}

contract OpChainTest is Test {
    function testL1BlockPredeploy() public view {
        uint basefee = IL1Block(0x4200000000000000000000000000000000000015).basefee();
        require(basefee > 0, "basefee should be a positive number");
    }
}
