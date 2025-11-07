// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";

contract FuzzerDict {
    // Immutables should get added to the dictionary.
    address public immutable immutableOwner;
    // Regular storage variables should also get added to the dictionary.
    address public storageOwner;

    constructor(address _immutableOwner, address _storageOwner) {
        immutableOwner = _immutableOwner;
        storageOwner = _storageOwner;
    }
}

contract FuzzerDictTest is DSTest {
    FuzzerDict fuzzerDict;

    function setUp() public {
        fuzzerDict = new FuzzerDict(address(100), address(200));
    }

    function testImmutableOwner(address who) public {
        assertTrue(who != fuzzerDict.immutableOwner());
    }

    function testStorageOwner(address who) public {
        assertTrue(who != fuzzerDict.storageOwner());
    }
}