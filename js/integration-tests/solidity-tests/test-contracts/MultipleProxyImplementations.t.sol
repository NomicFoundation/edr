// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {Test} from "forge-std/Test.sol";

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

contract Proxy1 is BaseProxy {
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

contract SameProxyDifferentImplementationsTest is Test {
    function test_proxiedCallsToImpl1AndImpl2AreTrackedSeparately() public {
        Impl1 impl1 = new Impl1();
        Impl2 impl2 = new Impl2();

        Proxy1 proxy1 = new Proxy1(address(impl1));
        Proxy1 proxy2 = new Proxy1(address(impl2));

        Impl1 i1 = Impl1(address(proxy1));
        Impl2 i2 = Impl2(address(proxy2));

        // Calling the proxied impls
        i1.one();
        i2.two();

        // Calling the impl directly works
        impl1.one();
    }
}

contract SameImplementationWithDifferentProxyChainsTest is Test {
    function test_proxiedCallsToImpl1AreTrackedSeparatelyWithDifferentProxyChains()
        public
    {
        // We use the same impl but different proxy chains
        Impl1 impl1 = new Impl1();

        Proxy1 proxy1 = new Proxy1(address(impl1));
        Proxy1 proxy2 = new Proxy1(address(impl1));

        // We use a proxy in front of Proxy1
        Proxy2 proxy11 = new Proxy2(address(proxy1));

        Impl1 i1 = Impl1(address(proxy11));
        Impl1 i2 = Impl1(address(proxy2));

        i1.one();
        i2.one();
    }
}
