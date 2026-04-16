// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";

contract Implementation {
    uint256 public value;

    function setValue(uint256 newValue) public {
        value = newValue;
    }

    function increment() public {
        value++;
    }
}

// EIP-1967 implementation slot: keccak256("eip1967.proxy.implementation") - 1
// Using a pseudo-random slot avoids storage collision with the implementation contract.
bytes32 constant _IMPLEMENTATION_SLOT_INNER = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;

// keccak256("eip1967.proxy.outer.implementation") - 1
bytes32 constant _IMPLEMENTATION_SLOT_OUTER = 0x097722eeaeebd20615d3502088cf9d0cf82fb0d6076cab73f374c401316eb701;

contract Proxy {
    constructor(address _implementation) {
        assembly {
            sstore(_IMPLEMENTATION_SLOT_INNER, _implementation)
        }
    }

    fallback() external payable {
        assembly {
            let impl := sload(_IMPLEMENTATION_SLOT_INNER)
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
}

// A second proxy using a different storage slot, for chaining: OuterProxy -> Proxy -> Implementation.
contract OuterProxy {
    constructor(address _implementation) {
        assembly {
            sstore(_IMPLEMENTATION_SLOT_OUTER, _implementation)
        }
    }

    fallback() external payable {
        assembly {
            let impl := sload(_IMPLEMENTATION_SLOT_OUTER)
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
}

contract ProxyGasReportTest is Test {
    Proxy public proxy;
    Implementation public impl;

    function setUp() public {
        impl = new Implementation();
        proxy = new Proxy(address(impl));
    }

    function test_proxySetValue() public {
        Implementation(address(proxy)).setValue(42);
        assertEq(Implementation(address(proxy)).value(), 42);
    }

    function test_proxyIncrement() public {
        Implementation(address(proxy)).increment();
        assertEq(Implementation(address(proxy)).value(), 1);
    }
}

contract ChainedProxyGasReportTest is Test {
    OuterProxy public outerProxy;
    Proxy public innerProxy;
    Implementation public impl;

    function setUp() public {
        impl = new Implementation();
        innerProxy = new Proxy(address(impl));
        outerProxy = new OuterProxy(address(innerProxy));

        // When OuterProxy DELEGATECALLs to inner Proxy's code, the inner Proxy
        // reads its _IMPLEMENTATION_SLOT_INNER from the caller's (OuterProxy's) storage.
        // We must store the Implementation address there so the chain works.
        vm.store(
            address(outerProxy),
            _IMPLEMENTATION_SLOT_INNER,
            bytes32(uint256(uint160(address(impl))))
        );
    }

    function test_chainedProxySetValue() public {
        Implementation(address(outerProxy)).setValue(99);
        assertEq(Implementation(address(outerProxy)).value(), 99);
    }

    function test_chainedProxyIncrement() public {
        Implementation(address(outerProxy)).increment();
        assertEq(Implementation(address(outerProxy)).value(), 1);
    }
}

contract Impl1 {
    function one() external pure returns (uint256) {
        return 1;
    }
}

contract Impl2 {
    function two() external pure returns (uint256) {
        return 2;
    }
}

contract SameProxyWithDifferentImplementationsTest is Test {
    function test_proxiedCallsToImpl1AndImpl2AreTrackedSeparately() public {
        Impl1 impl1 = new Impl1();
        Impl2 impl2 = new Impl2();

        Proxy proxy1 = new Proxy(address(impl1));
        Proxy proxy2 = new Proxy(address(impl2));

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
    Impl1 public impl;
    Proxy public innerProxy1;
    Proxy public innerProxy2;
    OuterProxy public outerProxy;

    function setUp() public {
        // We use the same impl but different proxy chains
        impl = new Impl1();

        innerProxy1 = new Proxy(address(impl));
        innerProxy2 = new Proxy(address(impl));

        outerProxy = new OuterProxy(address(innerProxy1));

        // When OuterProxy DELEGATECALLs to inner Proxy's code, the inner Proxy
        // reads its _IMPLEMENTATION_SLOT_INNER from the caller's (OuterProxy's) storage.
        // We must store the Implementation address there so the chain works.
        vm.store(
            address(outerProxy),
            _IMPLEMENTATION_SLOT_INNER,
            bytes32(uint256(uint160(address(impl))))
        );
    }

    function test_proxiedCallsToImpl1AreTrackedSeparatelyWithDifferentProxyChains()
        public
        view
    {
        Impl1 i1 = Impl1(address(outerProxy));
        Impl1 i2 = Impl1(address(innerProxy2));

        i1.one();
        i2.one();
    }
}
