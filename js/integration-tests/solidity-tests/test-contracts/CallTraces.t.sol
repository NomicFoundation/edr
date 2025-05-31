// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Test} from "forge-std/src/Test.sol";

contract CreateMe {}

contract CallTraces is Test {
    function testNoChildren() public {
    }

    function childCall() public {}
    function nestedCall() public {
        this.childCall();
    }

    function returnWithoutDeclaration() external {
        assembly {
            mstore(0x00, 0x42424242)
            return(0x00, 0x20)
        }
    }

    fallback() external {
        // Handle raw bytes calls
    }

    event OneEvent(uint256 x);
    event AnonEvent(bytes32 indexed, bytes32 indexed, bytes) anonymous;

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

    function testCallWithValue() public {
        vm.deal(address(this), 2 ether);
        payable(address(this)).transfer(1 ether);
    }

    receive() external payable {}

    function testCheatcodeCall() public {
        vm.addr(1);
    }

    function testLabelAddress() public {
        address someone = address(0x1000000000000000000000000000000000000000);
        vm.label(someone, "a labelled someone");
        someone.call("");
    }

    function testRawBytesCall() public {
        (bool success, bytes memory data) = address(this).call(hex"deadbeef");
        require(success);
    }

    function testUndecodedOutputs() public {
        this.returnWithoutDeclaration();
    }

    function testAnonymousEvent() public {
        emit AnonEvent(bytes32(uint256(1)), bytes32(uint256(2)), "test data");
    }

    function testCreateContract() public {
        new CreateMe();
    }

    function testStaticAndDelegateCall() public {
        // Static call
        (bool success1,) = address(this).staticcall(abi.encodeWithSignature("childCall()"));
        require(success1);

        // Delegate call
        (bool success2,) = address(this).delegatecall(abi.encodeWithSignature("childCall()"));
        require(success2);
    }
}
