// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.26;

import "cheats/Vm.sol";

/// @dev Verifies that coverage instrumentation does not interfere with the
/// single-call `vm.prank` cheatcode. The instrumenter injects a `STATICCALL`
/// to the coverage address before each statement which should not consume
/// the single-call `prank`.
contract CoveragePrankHelper {
    Vm constant vm =
        Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    /// Sets a single-call prank for `pranked`, then immediately calls
    /// `target.recordSender()`. The the recorded sender should be `pranked`.
    function prankAndRecordSender(
        SenderRecorder target,
        address pranked
    ) external returns (address) {
        vm.prank(pranked);
        return target.recordSender();
    }

    /// Same as above but with `vm.prank(addr, origin)` so we also catch
    /// premature restoration of `tx.origin` by the cheatcodes inspector.
    function prankAndRecordSenderAndOrigin(
        SenderRecorder target,
        address pranked,
        address newOrigin
    ) external returns (address sender, address origin) {
        vm.prank(pranked, newOrigin);
        return target.recordSenderAndOrigin();
    }

    /// `vm.startPrank` is not single-call. Included to guard against regressions.
    function startPrankAndRecordTwice(
        SenderRecorder target,
        address pranked
    ) external returns (address first, address second) {
        vm.startPrank(pranked);
        first = target.recordSender();
        second = target.recordSender();
        vm.stopPrank();
    }
}

contract SenderRecorder {
    function recordSender() external view returns (address) {
        return msg.sender;
    }

    function recordSenderAndOrigin() external view returns (address, address) {
        return (msg.sender, tx.origin);
    }
}
