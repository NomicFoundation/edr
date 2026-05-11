// Auto-generated from data/contracts/test/CoveragePrank.sol — do not edit manually.
// Regenerate with:
//   cargo run -p edr_tool_solidity -- --instrument-only data/contracts/test/CoveragePrank.sol
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
__NomicFoundationCoverage.sendHit(0x5b1bae0df22ecaf575aecbdf8cbb50dee5231ee532ff064624e1929c956b344b);         vm.prank(pranked);
__NomicFoundationCoverage.sendHit(0xc7d6f1bb9c09169a7eb25121686028c9dfabe35ee6446f8185a70e6095eb379d);         return target.recordSender();
    }

    /// Same as above but with `vm.prank(addr, origin)` so we also catch
    /// premature restoration of `tx.origin` by the cheatcodes inspector.
    function prankAndRecordSenderAndOrigin(
        SenderRecorder target,
        address pranked,
        address newOrigin
    ) external returns (address sender, address origin) {
__NomicFoundationCoverage.sendHit(0x3326dd1b26b6c6c307e314f36057f8931367d71faf7852f4a06c48a34a189b5f);         vm.prank(pranked, newOrigin);
__NomicFoundationCoverage.sendHit(0x1898f99f1097997a59caa86e6252288467890e24cccf8546fd26dcb3d7f770d6);         return target.recordSenderAndOrigin();
    }

    /// `vm.startPrank` is not single-call. Included to guard against regressions.
    function startPrankAndRecordTwice(
        SenderRecorder target,
        address pranked
    ) external returns (address first, address second) {
__NomicFoundationCoverage.sendHit(0xb067465d36072d7321324caee093760308d3c41ce8798fa84c9768753cbaf4aa);         vm.startPrank(pranked);
__NomicFoundationCoverage.sendHit(0x2285ec72bd18c12ef5f684a28f29b1eda10ef36be3d5dc0c9dce7927eaa1b17f);         first = target.recordSender();
__NomicFoundationCoverage.sendHit(0x00bd8bf3087aa84dde1a4c57333c755c000a75fea4810f29b2483557be82fe8f);         second = target.recordSender();
__NomicFoundationCoverage.sendHit(0x7f58c135e11378c2014a7e57a49a267757f785f1db070980427b39bdea5513ce);         vm.stopPrank();
    }
}

contract SenderRecorder {
    function recordSender() external view returns (address) {
__NomicFoundationCoverage.sendHit(0xa6effab53389158e07b84454221a66a7a9a5ea65890d20813c29d562d1cea9ca);         return msg.sender;
    }

    function recordSenderAndOrigin() external view returns (address, address) {
__NomicFoundationCoverage.sendHit(0x50020f1a925abb966e10d34cb078ddc6659c8289b29f53efc10771c12dbd4ae3);         return (msg.sender, tx.origin);
    }
}

import "__NomicFoundationCoverage-1fe87c59-dedc-4831-8918-604bc223bbfa.sol";