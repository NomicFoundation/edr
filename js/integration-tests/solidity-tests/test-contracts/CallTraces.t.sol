// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.0;

import {Test} from "forge-std/src/Test.sol";

contract CallTraces is Test {
    function testNoChildren() public {
    }

    function testSingleChildCall() public {
        this.childCall(55);
    }

    function testSingleEvent() public {
        emit SomeEvent(123, "hello");
    }

    function testManyChildren() public {
        emit OneEvent(1);
        this.childCall(2);
        emit OneEvent(3);
        this.childCall(4);
        emit OneEvent(5);
    }

    function testNestedCalls() public {
        this.nestedCall();
    }

    function testCallWithValue() public {
        vm.deal(address(this), 2 ether);
        payable(address(this)).transfer(1 ether);
    }

    function testCheatcodeCall() public pure {
        vm.addr(1);
    }

    function testLabelAddress() public {
        address someone = address(0x1000000000000000000000000000000000000000);
        vm.label(someone, "a labelled someone");
        (bool b, ) = someone.call("");
        require(b);
    }

    function testRawBytesCall() public {
        (bool success,) = address(this).call(hex"deadbeef");
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
        (bool success1,) = address(this).staticcall(abi.encodeCall(this.simpleCall, ()));
        require(success1);

        (bool success2,) = address(this).delegatecall(abi.encodeCall(this.simpleCall, ()));
        require(success2);
    }

    function testRevertedCreate() public {
        try new Failing() {} catch {}
    }

    function testUnlabeledAddress() public {
        address target = address(0xaBcDef1234567890123456789012345678901234);
        (bool success,) = target.call("");
        require(success);
    }

    function testEmptyCallData() public {
        // Since this contract has a receive function, this call will be recognized as that
        (bool success1,) = address(this).call("");
        require(success1);

        // This other contract has a fallback function and no receive function
        address other = address(new WithFallback());
        (bool success2,) = address(other).call("");
        require(success2);

        address nonContract = address(0x1000000000000000000000000000000000000000);
        (bool success3,) = address(nonContract).call("");
        require(success3);
    }

    function testRevertedCall() public {
        try this.revertWithEmpty() {} catch {}
        try this.revertWithString() {} catch {}
        try this.revertWithCustomError() {} catch {}
        try this.revertWithBytes() {} catch {}
    }

    function testWithFuzzing(uint256 x) public {
        emit OneEvent(x);
    }

    // State and external interface used by the tests above

    uint256 state;

    error CustomRevertError(uint256 code, string reason);

    event OneEvent(uint256 x);
    event SomeEvent(uint256 x, string s);
    event AnonEvent(bytes32 indexed, bytes32 indexed, bytes) anonymous;

    function simpleCall() external {}

    function childCall(uint256 x) external returns (uint256) {
        state = x;
        return 365;
    }

    function nestedCall() external {
        this.childCall(0);
    }

    function returnWithoutDeclaration() external {
        state = 0; // silence mutability warning
        assembly {
            mstore(0, hex"12340042")
            return(0, 4)
        }
    }

    function revertWithEmpty() external {
        state = 0; // silence mutability warning
        revert();
    }

    function revertWithString() external {
        state = 0; // silence mutability warning
        revert("Something went wrong");
    }

    function revertWithCustomError() external {
        state = 0; // silence mutability warning
        revert CustomRevertError(42, "Custom error occurred");
    }

    function revertWithBytes() external {
        state = 0; // silence mutability warning
        assembly {
            mstore(0, hex"deadbeefcafe")
            revert(0, 6)
        }
    }

    fallback() external {}

    receive() external payable {}
}

// Markers make creation and deployed code different

contract CreateMe {
    bytes32 public create_me_marker;
    constructor() {
        create_me_marker = "CreateMe";
    }
}

contract Failing {
    bytes32 public failing_marker;
    constructor() {
        failing_marker = "Failing";
        revert("Constructor failed");
    }
}

contract WithFallback {
    bytes32 public with_fallback_marker;
    constructor() {
        with_fallback_marker = "WithFallback";
    }
    fallback() external {}
}
