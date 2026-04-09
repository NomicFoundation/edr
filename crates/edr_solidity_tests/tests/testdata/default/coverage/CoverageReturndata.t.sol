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
        // Result is ABI-encoded Error(string). Decode the revert reason.
        assertEq(decodeRevertReason(result), "expected revert reason");
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
        // Result is ABI-encoded Error(string). Decode the revert reason.
        assertEq(decodeRevertReason(result), "constructor failed");
    }

    /// @dev Decodes the reason from ABI-encoded Error(string) revert data.
    function decodeRevertReason(bytes memory data) internal pure returns (string memory) {
        require(data.length >= 4, "data too short for Error(string)");
        // Validate the Error(string) selector (first 4 bytes after the length prefix).
        bytes4 selector;
        assembly {
            selector := mload(add(data, 0x20))
        }
        require(
            selector == bytes4(keccak256("Error(string)")),
            "selector is not Error(string)"
        );
        // Skip the 4-byte selector to get the ABI-encoded string payload.
        bytes memory payload;
        assembly {
            payload := add(data, 4)
        }
        return abi.decode(payload, (string));
    }
}
