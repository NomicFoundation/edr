// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Vault {
    uint256 public deposited;
    uint256 public locked;

    function deposit(uint256 amount) external {
        deposited += amount;
    }

    function lock(uint256 amount) external {
        require(amount <= deposited - locked, "exceeds");
        locked += amount;
    }
}

// https://github.com/NomicFoundation/edr/issues/1871
//
// With `viaIR`, both tests share `vm.expectRevert(bytes("exceeds"));
// vault.lock(11);`, so the CALL maps to `test_A_LockRevert`.
contract Issue1871Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    Vault vault;

    function setUp() public {
        vault = new Vault();
    }

    // Fails on purpose: lock does not revert.
    function test_B_LockNoRevert() public {
        vault.deposit(100);
        vm.expectRevert(bytes("exceeds"));
        vault.lock(11);
    }

    function test_A_LockRevert() public {
        vault.deposit(10);
        vm.expectRevert(bytes("exceeds"));
        vault.lock(11);
    }

    function _checkDeposited(uint256 expected) internal view {
        vm.assertEq(vault.deposited(), expected);
    }

    // Fails on purpose inside an internal helper, which `viaIR` inlines.
    function test_HelperFails() public {
        vault.deposit(1);
        _checkDeposited(2);
    }
}
