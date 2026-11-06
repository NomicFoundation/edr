// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

/// Sepolia blocks carry the EIP-7843 slot number from Amsterdam onward. The
/// two pinned blocks are 122 blocks but 124 slots apart (two missed slots).
/// forge-config: default.evm_version = "Amsterdam"
contract ForkSlotNumberTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    address constant SLOT_READER = address(0x1234);

    uint256 constant BLOCK_A = 12_069_000;
    uint64 constant SLOT_A = 11_511_859;
    uint256 constant BLOCK_B = 12_069_122;
    uint64 constant SLOT_B = 11_511_983;
    // First transaction of `BLOCK_B`, so rolling to it replays nothing.
    bytes32 constant TX_IN_B = 0x68a1142ba09c111ed3bc7831cda6e941ad88b9f0ea3cc5d569ea69b4090e5437;

    function readSlotNumber() internal returns (uint256) {
        // SLOTNUM; MSTORE(0); RETURN(0, 32).
        vm.etch(SLOT_READER, hex"4b60005260206000f3");
        (bool success, bytes memory result) = SLOT_READER.staticcall("");
        require(success, "SLOTNUM failed");
        return abi.decode(result, (uint256));
    }

    function assertSlotNumber(uint64 expected) internal {
        assertEq(vm.getSlotNumber(), expected);
        assertEq(readSlotNumber(), expected);
    }

    function testForkReadsHeaderSlot() public {
        vm.createSelectFork("sepolia", BLOCK_A);
        assertSlotNumber(SLOT_A);
    }

    function testRollForkUpdatesSlot() public {
        vm.createSelectFork("sepolia", BLOCK_A);
        vm.rollFork(BLOCK_B);
        assertSlotNumber(SLOT_B);
    }

    function testRollForkToTransactionUpdatesSlot() public {
        vm.createSelectFork("sepolia", BLOCK_A);
        vm.rollFork(TX_IN_B);
        assertSlotNumber(SLOT_B);
    }

    function testRollSlotPreservedAcrossForks() public {
        uint256 forkA = vm.createFork("sepolia", BLOCK_A);
        uint256 forkB = vm.createFork("sepolia", BLOCK_B);

        vm.selectFork(forkA);
        assertSlotNumber(SLOT_A);
        vm.rollSlot(1000);
        assertSlotNumber(1000);

        vm.selectFork(forkB);
        assertSlotNumber(SLOT_B);
        vm.rollSlot(2000);
        assertSlotNumber(2000);

        vm.selectFork(forkA);
        assertSlotNumber(1000);

        vm.selectFork(forkB);
        assertSlotNumber(2000);
    }
}
