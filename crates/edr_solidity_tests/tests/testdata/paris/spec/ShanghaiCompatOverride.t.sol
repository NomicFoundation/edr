// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// Same as `ShanghaiCompat`, but the test opts into the Shanghai EVM version via
// inline configuration, so `PUSH0` is available and the test passes on a Paris
// default spec.
contract ShanghaiCompatOverride is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    /// forge-config: default.evmVersion = "Shanghai"
    function testPush0() public {
        address target = address(uint160(uint256(0xc4f3)));

        bytes memory bytecode = hex"365f5f37365ff3";
        // 36 CALLDATASIZE
        // 5F PUSH0
        // 5F PUSH0
        // 37 CALLDATACOPY -> copies calldata at mem[0..calldatasize]

        // 36 CALLDATASIZE
        // 5F PUSH0
        // F3 RETURN -> returns mem[0..calldatasize]

        vm.etch(target, bytecode);

        (bool success, bytes memory result) = target.call(bytes("hello PUSH0"));
        assertTrue(success);
        assertEq(string(result), "hello PUSH0");
    }
}
