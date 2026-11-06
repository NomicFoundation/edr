// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract FuzzGasByInputTest is DSTest {
    uint256[] internal sink;

    // Execution gas grows with the input, so the reported gas statistics
    // reveal which inputs were generated.
    function testFuzz_gasByInput(uint256 x) public {
        for (uint256 i = 0; i < x % 64; ++i) {
            sink.push(i);
        }
    }
}
