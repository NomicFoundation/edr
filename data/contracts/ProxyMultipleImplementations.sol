// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

// We need to define two different proxies so that their implementation
// storage slots are different, so we can chain them.
abstract contract BaseProxy {
    fallback() external payable {
        address impl = getImplementation();
        assembly {
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

    function getImplementation() internal view virtual returns (address);

    receive() external payable {}
}

contract Proxy is BaseProxy {
    address implementation;

    constructor(address _impl) {
        implementation = _impl;
    }

    function getImplementation() internal view override returns (address) {
        return implementation;
    }
}

contract Proxy2 is BaseProxy {
    uint _storage_gap = 0;
    address implementation;

    constructor(address _impl) {
        implementation = _impl;
    }

    function getImplementation() internal view override returns (address) {
        return implementation;
    }
}

contract Impl1 {
    function one() external returns (uint256) {
        return 1;
    }
}

contract Impl2 {
    function two() external returns (uint256) {
        return 2;
    }
}
