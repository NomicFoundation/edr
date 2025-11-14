// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Target {
    event AnonymousEventNonIndexed(uint256 a) anonymous;

    function emitAnonymousEventNonIndexed(uint256 a) external {
        emit AnonymousEventNonIndexed(a);
    }
}

// `emit_diff_anonymous` in `crates/forge/tests/cli/failure_assertions.rs`
contract EmitDiffAnonymousTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    Target target;

    event DifferentAnonymousEventNonIndexed(string a) anonymous;

    function setUp() public {
        target = new Target();
    }

    function testShouldFailEmitDifferentEventNonIndexed() public {
        vm.expectEmitAnonymous(false, false, false, false, true);
        emit DifferentAnonymousEventNonIndexed("1");
        target.emitAnonymousEventNonIndexed(1);
    }
}
