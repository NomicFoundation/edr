// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract SimpleContract {
    uint256 public value;
    constructor(uint256 _value) {
        value = _value;
    }
}

// `intercept_initcode` in `crates/forge/tests/cli/test_cmd.rs`
contract InterceptInitcodeTest is DSTest {
    Vm vm = Vm(HEVM_ADDRESS);

    function testInterceptRegularCreate() public {
        // Set up interception
        vm.interceptInitcode();

        // Try to create a contract - this should revert with the initcode
        bytes memory initcode;
        try new SimpleContract(42) {
            assert(false);
        } catch (bytes memory interceptedInitcode) {
            initcode = interceptedInitcode;
        }

        // Verify the initcode contains the constructor argument
        assertTrue(initcode.length > 0, "initcode should not be empty");

        // The constructor argument is encoded as a 32-byte value at the end of the initcode
        // We need to convert the last 32 bytes to uint256
        uint256 value;
        assembly {
            value := mload(add(add(initcode, 0x20), sub(mload(initcode), 32)))
        }
        assertEq(value, 42, "initcode should contain constructor arg");
    }

    function testInterceptMultiple() public {
        // First interception
        vm.interceptInitcode();
        bytes memory initcode1;
        try new SimpleContract(1) {
            assert(false);
        } catch (bytes memory interceptedInitcode) {
            initcode1 = interceptedInitcode;
        }

        // Second interception
        vm.interceptInitcode();
        bytes memory initcode2;
        try new SimpleContract(2) {
            assert(false);
        } catch (bytes memory interceptedInitcode) {
            initcode2 = interceptedInitcode;
        }

        // Verify different initcodes
        assertTrue(initcode1.length > 0, "first initcode should not be empty");
        assertTrue(initcode2.length > 0, "second initcode should not be empty");

        // Extract constructor arguments from both initcodes
        uint256 value1;
        uint256 value2;
        assembly {
            value1 := mload(add(add(initcode1, 0x20), sub(mload(initcode1), 32)))
            value2 := mload(add(add(initcode2, 0x20), sub(mload(initcode2), 32)))
        }
        assertEq(value1, 1, "first initcode should contain first arg");
        assertEq(value2, 2, "second initcode should contain second arg");
    }
}
