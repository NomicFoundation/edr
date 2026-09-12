// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

// Fixture for `test_invariant_gas_report_samples_bound_collected_run_traces`:
// the handler never reverts, so every run performs exactly `depth` calls and
// the number of calls the gas report sees is a function of the sample budget.
contract GasReportSamplesTest is DSTest {
    Bumper bumper;

    function setUp() public {
        bumper = new Bumper();
    }

    function invariant_alwaysHolds() public {}
}

contract Bumper {
    uint256 public count;

    function bump() public {
        count += 1;
    }
}
