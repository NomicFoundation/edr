// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

/// Forks are built with the default hardfork; switching to one must keep the
/// configured `evm_version`. `SLOTNUM` is only valid from Amsterdam onward.
/// forge-config: default.evm_version = "Amsterdam"
contract ForkEvmVersionTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    address constant SLOT_READER = address(0x1234);

    function assertAmsterdam() internal {
        // SLOTNUM; MSTORE(0); RETURN(0, 32).
        vm.etch(SLOT_READER, hex"4b60005260206000f3");
        (bool success, ) = SLOT_READER.staticcall("");
        assertTrue(success);
    }

    function testEvmVersionPreservedAcrossForks() public {
        assertAmsterdam();

        uint256 forkA = vm.createSelectFork("mainnet", 15_977_624);
        assertAmsterdam();

        uint256 forkB = vm.createFork("mainnet", 15_977_625);
        vm.selectFork(forkB);
        assertAmsterdam();

        vm.selectFork(forkA);
        vm.rollFork(15_977_625);
        assertAmsterdam();
    }
}
