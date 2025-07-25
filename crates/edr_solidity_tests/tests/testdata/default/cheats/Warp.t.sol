// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract WarpTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testWarp() public {
        vm.warp(10);
        assertEq(block.timestamp, 10, "warp failed");
    }

    function testWarpFuzzed(uint32 jump) public {
        uint256 pre = block.timestamp;
        vm.warp(block.timestamp + jump);
        assertEq(block.timestamp, pre + jump, "warp failed");
    }

    function testWarp2() public {
        assertEq(block.timestamp, 1);
        vm.warp(100);
        assertEq(block.timestamp, 100);
    }
}
