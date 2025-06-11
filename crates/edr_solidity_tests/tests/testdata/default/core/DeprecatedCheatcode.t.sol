// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract DeprecatedCheatcodeTest is DSTest {
  Vm constant vm = Vm(HEVM_ADDRESS);

  function test_deprecated_cheatcode() public view {
    vm.keyExists('{"a": 123}', ".a");

    vm.keyExists('{"a": 123}', ".a");
  }
}

contract DeprecatedCheatcodeFuzzTest is DSTest {
  Vm constant vm = Vm(HEVM_ADDRESS);

  function test_deprecated_cheatcode(uint256 a) public view {
    vm.keyExists('{"a": 123}', ".a");
  }
}

contract Counter {
  uint256 a;

  function count() public {
    a++;
  }
}

contract DeprecatedCheatcodeInvariantTest is DSTest {
  Vm constant vm = Vm(HEVM_ADDRESS);

  function setUp() public {
    Counter counter = new Counter();
  }

  /// forge-config: default.invariant.runs = 1

  function invariant_deprecated_cheatcode() public {
    vm.keyExists('{"a": 123}', ".a");
  }
}
