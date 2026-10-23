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

    // Never reverts.
    function unlock(uint256 amount) external {}
}

// https://github.com/NomicFoundation/edr/issues/1871
//
// With the optimizer, both tests share the tail of
// `vm.expectRevert(bytes("exceeds")); vault.<fn>(11);`, so the CALL that
// performs `vault.unlock(11)` is mapped to `vault.lock(11)` in the other test.
contract Issue1871Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    Vault vault;

    function setUp() public {
        vault = new Vault();
    }

    function _locked(uint256 amount) internal {
        vault.deposit(100);
        vault.lock(amount);
    }

    function test_RevertWhen_LockExceeds() public {
        vault.deposit(10);
        vm.expectRevert(bytes("exceeds"));
        vault.lock(11);
    }

    // Fails on purpose: unlock never reverts.
    function test_RevertWhen_UnlockExceeds() public {
        _locked(10);
        vm.expectRevert(bytes("exceeds"));
        vault.unlock(11);
    }
}

// Modifiers are inlined, so a cheatcode call in one maps to the modifier
// rather than to the test function. That must not be mistaken for
// deduplicated code.
contract Issue1871ModifierTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    Vault vault;

    modifier checkDeposited(uint256 expected) {
        _;
        vm.assertEq(vault.deposited(), expected);
    }

    function setUp() public {
        vault = new Vault();
    }

    function test_ModifierFails() public checkDeposited(2) {
        vault.deposit(1);
    }
}
