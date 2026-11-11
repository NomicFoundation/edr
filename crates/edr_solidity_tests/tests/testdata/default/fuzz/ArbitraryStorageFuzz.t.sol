// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract ArbitraryStorageFuzzCounter {
    uint256 public a;
}

// `vm.setArbitraryStorage` is pure, so a failure is re-run for its stack
// trace, and the value read from arbitrary storage comes from the cheatcode
// RNG, which is reseeded for every fuzz run.
contract ArbitraryStorageFuzzTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testFuzz_arbitraryStorage_shouldFail(uint256) public {
        ArbitraryStorageFuzzCounter counter = new ArbitraryStorageFuzzCounter();
        vm.setArbitraryStorage(address(counter));
        require(counter.a() % 4 != 0, "hit value 0");
    }
}
