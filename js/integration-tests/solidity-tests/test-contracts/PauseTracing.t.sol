// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/src/Test.sol";

contract TraceGenerator is Test {
    event DummyEvent(uint256 i);
    function call(uint256 i) public {
        emit DummyEvent(i);
    }
    function generate() public {
        for (uint256 i = 0; i < 10; i++) {
            if (i == 3) {
                vm.pauseTracing();
            }
            this.call(i);
            if (i == 7) {
                vm.resumeTracing();
            }
        }
    }
}
contract PauseTracingTest is Test {
    event DummyEvent(uint256 i);
    function setUp() public {
        emit DummyEvent(1);
        vm.pauseTracing();
        emit DummyEvent(2);
        emit DummyEvent(3);
    }
    function test() public {
        emit DummyEvent(3);
        TraceGenerator t = new TraceGenerator();
        vm.resumeTracing();
        t.generate();
    }
}