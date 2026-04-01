// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.26;

import "ds-test/test.sol";
import "./InstrumentedCoverageTest.sol";

contract CoverageReturndataTest is DSTest {
    CoverageCall coverageCall;

    function setUp() public {
        coverageCall = new CoverageCall();
    }

    function testForwardSuccessfulCall() public {
        (bool success, bytes memory result) = address(coverageCall).call(
            abi.encodeWithSignature("forwardSuccessfulCall()")
        );
        assertTrue(success, "forwardSuccessfulCall() should succeed");
        uint256 value = abi.decode(result, (uint256));
        assertEq(value, 42, "forwardSuccessfulCall() should return 42");
    }

    function testForwardRevertedCall() public {
        (bool success, bytes memory result) = address(coverageCall).call(
            abi.encodeWithSignature("forwardRevertedCall()")
        );
        assertTrue(success, "forwardRevertedCall() should succeed");
        assertTrue(result.length > 0, "should return revert data");
        assertTrue(
            containsString(result, "expected revert reason"),
            "should contain revert reason"
        );
    }

    function testDeployChild() public {
        (bool success, bytes memory result) = address(coverageCall).call(
            abi.encodeWithSignature("deployChild()")
        );
        assertTrue(success, "deployChild() should succeed");
        assertEq(result.length, 0, "should return empty data after successful CREATE");
    }

    function testDeployRevertingChild() public {
        (bool success, bytes memory result) = address(coverageCall).call(
            abi.encodeWithSignature("deployRevertingChild()")
        );
        assertTrue(success, "deployRevertingChild() should succeed");
        assertTrue(result.length > 0, "should return revert data");
        assertTrue(
            containsString(result, "constructor failed"),
            "should contain constructor revert reason"
        );
    }

    function containsString(bytes memory data, string memory search) internal pure returns (bool) {
        bytes memory searchBytes = bytes(search);
        if (searchBytes.length > data.length) return false;
        for (uint256 i = 0; i <= data.length - searchBytes.length; i++) {
            bool found = true;
            for (uint256 j = 0; j < searchBytes.length; j++) {
                if (data[i + j] != searchBytes[j]) {
                    found = false;
                    break;
                }
            }
            if (found) return true;
        }
        return false;
    }
}
