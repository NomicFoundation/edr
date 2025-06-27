// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Test} from "forge-std/src/Test.sol";

contract L1ChainTest is Test {
    function testBeaconRootAddressHasCode() public view {
        address beaconRootAddress = 0x000F3df6D732807Ef1319fB7B8bB8522d0Beac02;

        uint256 codeSize;
        assembly {
            codeSize := extcodesize(beaconRootAddress)
        }

        vm.assertGt(codeSize, 0, "Address should have code");
    }
}
