// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

interface PointEvaluation {
    function verify(bytes calldata input) external view returns (uint256);
}

// Fixture for `test_trace_marks_precompile_calls`.
contract PrecompileCallTest is DSTest {
    PointEvaluation internal constant POINT_EVALUATION_PRECOMPILE = PointEvaluation(address(0x0a));

    // The input is too short for the precompile, so the call fails. The
    // function returns a value, so Solidity calls the code-less precompile
    // address without first checking for code.
    function testPrecompileCallFails() public view {
        POINT_EVALUATION_PRECOMPILE.verify(hex"00");
    }
}
