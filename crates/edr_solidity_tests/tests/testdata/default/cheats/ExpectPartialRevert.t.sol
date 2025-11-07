// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Counter {
    error WrongNumber(uint256 number);
    function count() public pure {
        revert WrongNumber(0);
    }
}
contract ExpectPartialRevertTest is DSTest {
    Vm vm = Vm(HEVM_ADDRESS);
    function testExpectPartialRevertWithSelector() public {
        Counter counter = new Counter();
        vm.expectPartialRevert(Counter.WrongNumber.selector);
        counter.count();
    }
    function testExpectPartialRevertWith4Bytes() public {
        Counter counter = new Counter();
        vm.expectPartialRevert(bytes4(0x238ace70));
        counter.count();
    }
    function testExpectRevert() public {
        Counter counter = new Counter();
        vm.expectRevert(Counter.WrongNumber.selector);
        counter.count();
    }
}