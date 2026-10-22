// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

// State-access operations repriced by EIP-8038, each behind its own function.
// `noop` is the baseline their cost is measured against.
contract StateAccess {
    uint256 private slot;

    function noop() external {
        burnGas();
    }

    function readBalance() external view returns (uint256) {
        burnGas();
        // An account outside the genesis state, so the access is cold.
        return address(0xaa).balance;
    }

    function write(uint256 value) external {
        burnGas();
        slot = value;
    }

    // Expands memory to 4,096 words, burning execution gas that EIP-8038
    // leaves unchanged. Every call pays it, so neither the calldata floor
    // (EIP-7976) nor EIP-3529's cap on refunds of a fifth of the gas used
    // distorts a measurement.
    function burnGas() private pure {
        assembly ("memory-safe") {
            mstore(0x1ffe0, 0)
        }
    }
}
