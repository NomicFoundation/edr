// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.26;

/// @dev Validates that coverage instrumentation preserves the returndata buffer
/// across call + returndatacopy and deploy + returndatacopy patterns. Without
/// the fix, coverage probes injected between the .call() and the assembly block
/// would clobber the returndata buffer, causing these functions to return wrong
/// or empty data.
contract CoverageCall {
    Target private target;

    constructor() {
        target = new Target();
    }

    /// Forwards a successful call and returns the result via returndatacopy.
    function forwardSuccessfulCall() public returns (uint256) {
        (bool success, ) = address(target).call(
            abi.encodeWithSignature("getValue()")
        );
        if (!success) {
            revert("call failed");
        }
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            returndatacopy(ptr, 0, returndatasize())
            return(ptr, returndatasize())
        }
    }

    /// Forwards a reverted call and returns the original revert data.
    function forwardRevertedCall() public {
        (bool success, ) = address(target).call(
            abi.encodeWithSignature("willRevert()")
        );
        if (success) {
            revert("call should have failed");
        }
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            returndatacopy(ptr, 0, returndatasize())
            return(ptr, returndatasize())
        }
    }

    /// Deploys a child contract and returns the returndata after CREATE
    /// (which should be empty on success).
    function deployChild() public {
        CoverageDeploySuccess deployed = new CoverageDeploySuccess();
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            returndatacopy(ptr, 0, returndatasize())
            return(ptr, returndatasize())
        }
    }

    /// Deploys a child contract that reverts and returns the original
    /// revert data via returndatacopy.
    function deployRevertingChild() public {
        try new CoverageDeployRevert() {
            revert("should never happen");
        } catch {
            assembly ("memory-safe") {
                let ptr := mload(0x40)
                returndatacopy(ptr, 0, returndatasize())
                return(ptr, returndatasize())
            }
        }
    }
}

contract Target {
    function getValue() external pure returns (uint256) {
        return 42;
    }

    function willRevert() external pure {
        revert("expected revert reason");
    }
}

contract CoverageDeploySuccess {
    uint256 public value;

    constructor() {
        value = 123;
    }
}

contract CoverageDeployRevert {
    constructor() {
        uint256 x = 1;
        revert("constructor failed");
    }
}
