// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

contract TransientStorageStore {
    uint value;

    function tstore(uint key, uint val) public {
        assembly {
            tstore(key, val)
        }
    }

    function tload(uint key) public view returns (uint) {
        uint val;
        assembly {
            val := tload(key)
        }
        return val;
    }
}

contract Counter {
    uint256 public number;

    function incBy(uint by) public {
        require(by > 0, "incBy: increment should be positive");
        number += by;
    }
}

contract IsolateTest is Test {
    TransientStorageStore transientStorageStore;
    Counter counter;

    function setUp() public {
        transientStorageStore = new TransientStorageStore();
        counter = new Counter();
    }

    function testIsolateTest() public {
        transientStorageStore.tstore(1, 2);

        // a normal transaction would return 2 here because the value is stored in transient storage,
        // but if isolate mode is enabled, the value should be 0
        uint256 val = transientStorageStore.tload(1);

        assertEq(val, 0, "test wasn't called with isolate mode enabld");
    }

    function testExpectRevert() public {
        vm.expectRevert();
        counter.incBy(0);
    }
}
