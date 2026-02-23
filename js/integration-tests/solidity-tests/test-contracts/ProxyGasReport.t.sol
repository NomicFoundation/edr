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

contract Proxy {
    // EIP-1967 implementation slot: keccak256("eip1967.proxy.implementation") - 1
    // Using a pseudo-random slot avoids storage collision with the implementation contract.
    bytes32 private constant _IMPLEMENTATION_SLOT =
        0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;

    constructor(address _implementation) {
        assembly {
            sstore(_IMPLEMENTATION_SLOT, _implementation)
        }
    }

    fallback() external payable {
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
}

// A second proxy using a different storage slot, for chaining: OuterProxy -> Proxy -> Implementation.
contract OuterProxy {
    // keccak256("eip1967.proxy.outer.implementation") - 1
    bytes32 private constant _IMPLEMENTATION_SLOT =
        0x097722eeaeebd20615d3502088cf9d0cf82fb0d6076cab73f374c401316eb701;

    constructor(address _implementation) {
        assembly {
            sstore(_IMPLEMENTATION_SLOT, _implementation)
        }
    }

    fallback() external payable {
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
        // reads its _IMPLEMENTATION_SLOT from the caller's (OuterProxy's) storage.
        // We must store the Implementation address there so the chain works.
        bytes32 innerProxyImplSlot = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
        vm.store(
            address(outerProxy),
            innerProxyImplSlot,
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
