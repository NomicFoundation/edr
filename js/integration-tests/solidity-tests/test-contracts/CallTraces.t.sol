// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

contract UnderTest {
    function foo() external {
    }
}

contract CallTraces {
    function testNoChildren() public {
    }

    function childCall() public {}
    function nestedCall() public {
        this.childCall();
    }

    event OneEvent(uint256 x);
    event AnonEvent(bytes32 indexed, bytes32 indexed, bytes);

    function testSingleChildCall() public {
        this.childCall();
    }

    function testSingleEvent() public {
        emit OneEvent(123);
    }

    function testManyChildren() public {
        emit OneEvent(1);
        this.childCall();
        emit OneEvent(2);
        this.childCall();
        emit OneEvent(3);
    }

    function testNestedCalls() public {
        this.nestedCall();
    }
}
