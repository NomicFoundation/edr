// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

// Fixture for `test_fuzz_gas_report_samples_bound_collected_traces`: every run
// calls `bump` exactly once, so the number of calls the gas report sees is the
// number of traces kept for it.
contract FuzzGasReportSamplesTest is DSTest {
    Bumper bumper;

    function setUp() public {
        bumper = new Bumper();
    }

    function testFuzz_bump(uint256) public {
        bumper.bump();
    }
}

contract Bumper {
    uint256 public count;

    function bump() public {
        count += 1;
    }
}
