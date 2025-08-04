// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";
import {console} from "../logs/console.sol";

// https://github.com/foundry-rs/foundry/issues/4523
contract Issue4523Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    mapping(uint256 => bytes32) map;

    function test_GasMeter() public {
        vm.pauseGasMetering();
        consumeGas();
        vm.resumeGasMetering();

        consumeGas();
    }

    function test_GasLeft() public {
        consumeGas();

        uint256 start = gasleft();
        consumeGas();
        console.log("Gas cost:", start - gasleft());
    }

    function consumeGas() private {
        for (uint256 i = 0; i < 100; i++) {
            map[i] = keccak256(abi.encode(i));
        }
    }
}
