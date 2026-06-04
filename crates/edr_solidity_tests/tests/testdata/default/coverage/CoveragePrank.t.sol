// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.26;

import "ds-test/test.sol";
import "./InstrumentedCoveragePrankTest.sol";

/// Regression test for https://github.com/NomicFoundation/edr/issues/1391 —
/// coverage instrumentation must not interfere with `vm.prank`. Each scenario
/// runs through `CoveragePrankHelper`, which is instrumented with coverage
/// probes between the cheatcode and the call it should affect.
contract CoveragePrankTest is DSTest {
    CoveragePrankHelper helper;
    SenderRecorder target;

    function setUp() public {
        helper = new CoveragePrankHelper();
        target = new SenderRecorder();
    }

    /// `vm.prank(addr)` must apply to the next call even when a coverage
    /// probe is injected between the cheatcode and that call.
    function testPrankSurvivesCoverageProbe() public {
        address pranked = address(0xCAFE);
        address recorded = helper.prankAndRecordSender(target, pranked);
        assertEq(
            recorded,
            pranked,
            "vm.prank should make pranked the msg.sender of the next call"
        );
    }

    /// `vm.prank(addr, origin)` must apply both `msg.sender` and
    /// `tx.origin` to the next call, despite intervening coverage probes.
    function testPrankWithOriginSurvivesCoverageProbe() public {
        address pranked = address(0xCAFE);
        address newOrigin = address(0xBEEF);
        (address sender, address origin) = helper.prankAndRecordSenderAndOrigin(
            target,
            pranked,
            newOrigin
        );
        assertEq(
            sender,
            pranked,
            "vm.prank(addr, origin) should set msg.sender"
        );
        assertEq(
            origin,
            newOrigin,
            "vm.prank(addr, origin) should set tx.origin"
        );
    }

    /// `vm.startPrank` was already unaffected by the bug. This
    /// guards against changes that break the recurrent prank.
    function testStartPrankStillWorks() public {
        address pranked = address(0xCAFE);
        (address first, address second) = helper.startPrankAndRecordTwice(
            target,
            pranked
        );
        assertEq(
            first,
            pranked,
            "vm.startPrank should set msg.sender on the first call"
        );
        assertEq(
            second,
            pranked,
            "vm.startPrank should keep msg.sender on subsequent calls"
        );
    }
}
