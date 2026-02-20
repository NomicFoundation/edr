// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

contract Implementation {
    uint256 public value;

    function setValue(uint256 newValue) public {
        value = newValue;
    }

    function increment() public {
        value++;
    }
}

contract Proxy {
    // EIP-1967 implementation slot
    bytes32 private constant _IMPLEMENTATION_SLOT =
        0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;

    constructor(address _implementation) {
        assembly {
            sstore(_IMPLEMENTATION_SLOT, _implementation)
        }
    }

    function _delegate() internal {
        assembly {
            let impl := sload(_IMPLEMENTATION_SLOT)
            calldatacopy(0, 0, calldatasize())
            let result := delegatecall(gas(), impl, 0, calldatasize(), 0, 0)
            returndatacopy(0, 0, returndatasize())
            switch result
            case 0 {
                revert(0, returndatasize())
            }
            default {
                return(0, returndatasize())
            }
        }
    }

    function setValue(uint256) external {
        _delegate();
    }

    function increment() external {
        _delegate();
    }
}
