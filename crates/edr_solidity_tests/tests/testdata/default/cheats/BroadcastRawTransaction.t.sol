// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

/// Records who called it, so a test can tell the difference between the test
/// contract and the address that actually signed a raw transaction.
contract Recorder {
    address public lastSender;
    uint256 public lastValue;
    uint256 public calls;

    receive() external payable {
        lastSender = msg.sender;
        lastValue = msg.value;
        calls += 1;
    }
}

/// Target for a raw transaction that is valid but whose execution reverts.
contract Reverter {
    receive() external payable {
        revert("Reverter: nope");
    }
}

/// Calls the cheatcode one frame below the test function, so that the nested
/// EVM's journal depth is not the same as it is when the test calls the
/// cheatcode directly.
contract Broadcaster {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);

    function broadcast(bytes calldata raw) external {
        vm.broadcastRawTransaction(raw);
    }

    function broadcastThenRevert(bytes calldata raw) external {
        vm.broadcastRawTransaction(raw);
        revert("Broadcaster: rolled back");
    }
}

contract BroadcastRawTransactionTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    // ---------------------------------------------------------------------
    // Nick's method: a pre-signed, keyless deployment.
    //
    // Legacy (pre-EIP-155, so no chain id) creation transaction, nonce 0,
    // gasPrice 100 gwei, gas 100000, value 0, with a made-up but valid
    // signature (r = s = 0x2222..22, v = 27). Nobody holds the private key;
    // the sender only exists as the output of ecrecover, which is exactly why
    // this transaction cannot be re-encoded as a normal call or pranked: the
    // deployed address is derived from *this* sender at *this* nonce.
    //
    // Init code 600a600c600039600a6000f3602a60005260206000f3 returns the
    // 10-byte runtime 602a60005260206000f3, which returns 42 for any call.
    //
    // Both legacy blobs below are the same 103-byte RLP envelope, hand
    // assembled, differing only in the single nonce byte (`80` vs `07`).
    // Decode with: cast decode-transaction <hex>
    // ---------------------------------------------------------------------
    bytes constant NICKS_TX =
        hex"f8678085174876e800830186a0808096600a600c600039600a6000f3602a60005260206000f31ba02222222222222222222222222222222222222222222222222222222222222222a02222222222222222222222222222222222222222222222222222222222222222";
    address constant NICKS_DEPLOYER = 0x015B263b6C0d90A87B8F1809749b7ffE9e442C49;
    address constant NICKS_FACTORY = 0xDE61810BeA1f40ed5D943d89844A7111f461D0De;
    // gas limit (100000) * gasPrice (100 gwei)
    uint256 constant NICKS_MAX_FEE = 0.01 ether;

    // The same init code and signature, but the transaction declares nonce 7
    // instead of 0, so it is signed by a different keyless address.
    bytes constant NICKS_TX_NONCE_7 =
        hex"f8670785174876e800830186a0808096600a600c600039600a6000f3602a60005260206000f31ba02222222222222222222222222222222222222222222222222222222222222222a02222222222222222222222222222222222222222222222222222222222222222";
    address constant NICKS_DEPLOYER_NONCE_7 = 0x4d60d2dfaaB1500D4Bf88fFd3616E38D2A185bB8;

    // ---------------------------------------------------------------------
    // EIP-1559 transactions signed by the well-known development key, sending
    // 1 ether with empty calldata, nonce 0, gas 200000, maxFeePerGas 1 gwei,
    // maxPriorityFeePerGas 0. The test block's base fee is 0, so the effective
    // gas price is 0 and balances move by exactly the transferred value.
    //
    // Regenerate any of these with:
    //   cast mktx --raw \
    //     --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
    //     --chain <id> --nonce <n> --gas-limit 200000 \
    //     --gas-price 1gwei --priority-gas-price 0 --value 1ether <to>
    // ---------------------------------------------------------------------
    address constant SIGNER = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266;
    address payable constant RECORDER = payable(0x00000000000000000000000000000000000B0b01);
    address payable constant REVERTER = payable(0x00000000000000000000000000000000000b0B03);

    // to RECORDER, chain id 31337, nonce 0.
    bytes constant CALL_TX =
        hex"02f871827a698080843b9aca0083030d409400000000000000000000000000000000000b0b01880de0b6b3a764000080c001a09aaeeeb42c4aa82e62fd7e8c21eb99917667ac7e83bed1aa80f663bfbcb5c727a0268aac2d27903031448c40ea5aa3d173bae8713deedc3463a10191a38ba6a259";
    // Identical, but signed for chain id 1 while the tests run on 31337.
    bytes constant CALL_TX_WRONG_CHAIN_ID =
        hex"02f86f018080843b9aca0083030d409400000000000000000000000000000000000b0b01880de0b6b3a764000080c080a0b09ca38a8a2b093d8d25cdc9a4306d48261fe5b8e109203dbcdf8536c6eb5564a01133c1668673f3ebd2df01b45307620019bd9bb07a06c0ec1771ee98730a4954";
    // Identical, but declares nonce 42 while the signer's nonce is 0.
    bytes constant CALL_TX_WRONG_NONCE =
        hex"02f871827a692a80843b9aca0083030d409400000000000000000000000000000000000b0b01880de0b6b3a764000080c001a09efca1332dc5757d8939261633f3bf606984419acd5d9bd10839b3e705024943a0419d678609a9fede554dd735eb3234ba85007f97f21ace5a6d7da980acba684e";
    // To REVERTER instead of RECORDER: valid transaction, reverting execution.
    bytes constant REVERTER_TX =
        hex"02f871827a698080843b9aca0083030d409400000000000000000000000000000000000b0b03880de0b6b3a764000080c001a0acddf892167b546ca12a4e25ced8a24607dbe5463879d951208b687a9c9eadbea014c7b07ec9f56bd214ca9c52d651ad8c38360de51617a2f2cb588bf21ac3b4c6";

    Broadcaster broadcaster;

    function setUp() public {
        // The fixture blobs are signed for a specific environment. Assert it
        // here so drift in the test harness fails legibly, rather than as an
        // "invalid chain ID" that reads like an implementation bug.
        assertEq(block.chainid, 31337, "fixtures are signed for chain id 31337");
        assertEq(block.basefee, 0, "balance assertions assume a zero base fee");

        // Fixture self-check: the constants really are the sender and the
        // CREATE address the pre-signed blob commits to. Independent of the
        // cheatcode; it must hold before any test runs.
        assertEq(NICKS_FACTORY, vm.computeCreateAddress(NICKS_DEPLOYER, 0), "bad NICKS_FACTORY constant");

        vm.etch(RECORDER, address(new Recorder()).code);
        vm.etch(REVERTER, address(new Reverter()).code);
        broadcaster = new Broadcaster();
    }

    /// The motivating case: replaying a pre-signed deterministic-deployment
    /// bootstrap. Asserts the transaction's effect on state, and that it was
    /// executed as the keyless deployer rather than as the test contract.
    function testNicksMethodBootstrap() public {
        vm.deal(NICKS_DEPLOYER, NICKS_MAX_FEE);
        uint256 coinbaseBefore = block.coinbase.balance;

        assertEq(NICKS_FACTORY.code.length, 0, "factory should not exist yet");
        assertEq(vm.getNonce(NICKS_DEPLOYER), 0, "deployer should be at nonce 0");

        vm.broadcastRawTransaction(NICKS_TX);

        assertGt(NICKS_FACTORY.code.length, 0, "factory should have code");

        // And it works.
        (bool ok, bytes memory ret) = NICKS_FACTORY.staticcall("");
        assertTrue(ok, "call to deployed contract failed");
        assertEq(abi.decode(ret, (uint256)), 42, "deployed contract returned the wrong value");

        // The deployer, not the test contract, paid for it and consumed a nonce.
        // Conservation of the fee against the coinbase pins that the charge is
        // both real and charged to the signer: no other account funds it.
        assertEq(vm.getNonce(NICKS_DEPLOYER), 1, "deployer nonce should have been consumed");
        uint256 spent = NICKS_MAX_FEE - NICKS_DEPLOYER.balance;
        assertGt(spent, 0, "deployer should have paid the gas");
        assertEq(block.coinbase.balance - coinbaseBefore, spent, "fee must come from the deployer");
    }

    /// The sender is recovered from the signature, so the transaction is not
    /// re-attributed to whoever invoked the cheatcode.
    function testSenderIsTheSignatureSigner() public {
        vm.deal(SIGNER, 10 ether);
        uint256 testContractBalance = address(this).balance;

        vm.broadcastRawTransaction(CALL_TX);

        Recorder recorder = Recorder(RECORDER);
        assertEq(recorder.calls(), 1, "recorder should have been called once");
        assertEq(recorder.lastSender(), SIGNER, "sender should be the signature's signer");

        // Value came out of the signer's balance, not the test contract's.
        assertEq(recorder.lastValue(), 1 ether, "wrong value received");
        assertEq(RECORDER.balance, 1 ether, "wrong recipient balance");
        assertEq(SIGNER.balance, 9 ether, "signer should have paid the value");
        assertEq(address(this).balance, testContractBalance, "test contract must not have paid");
        assertEq(vm.getNonce(SIGNER), 1, "signer nonce should have been consumed");
    }

    /// A prank must not change who the transaction is from: the signature does.
    function testPrankDoesNotOverrideTheSigner() public {
        vm.deal(SIGNER, 10 ether);

        vm.prank(address(0xdeadbeef));
        vm.broadcastRawTransaction(CALL_TX);

        assertEq(Recorder(RECORDER).calls(), 1, "transaction should have executed");
        assertEq(Recorder(RECORDER).lastSender(), SIGNER, "prank must not reassign the sender");
    }

    /// The one-shot prank is not spent by the cheatcode; it still applies to
    /// the next real call. Pinned because the cheatcode runs a whole nested
    /// transaction, which is exactly the kind of thing that could plausibly
    /// consume it. The follow-up call below has to be the first external call
    /// after the cheatcode, since any assertion that reads the recorder would
    /// itself spend the prank.
    function testPrankIsNotSpentByBroadcast() public {
        vm.deal(SIGNER, 10 ether);

        vm.prank(address(0xdeadbeef));
        vm.broadcastRawTransaction(CALL_TX);

        (bool ok,) = RECORDER.call("");
        assertTrue(ok, "follow-up call failed");

        Recorder recorder = Recorder(RECORDER);
        assertEq(recorder.calls(), 2, "broadcast and follow-up should both have landed");
        assertEq(recorder.lastSender(), address(0xdeadbeef), "prank should survive the cheatcode");
    }

    /// The cheatcode is not restricted to being called from the test function
    /// itself. This exercises a different journal depth for the nested EVM.
    function testBroadcastFromANestedFrame() public {
        vm.deal(SIGNER, 10 ether);

        broadcaster.broadcast(CALL_TX);

        assertEq(Recorder(RECORDER).calls(), 1, "transaction should have executed");
        assertEq(Recorder(RECORDER).lastSender(), SIGNER, "sender should be the signature's signer");
        assertTrue(Recorder(RECORDER).lastSender() != address(broadcaster), "sender must not be the caller");
        assertEq(SIGNER.balance, 9 ether, "signer should have paid the value");
    }

    // ---------------------------------------------------------------------
    // Rejections
    //
    // `_expectCheatcodeRevert(bytes)` matches the expected reason as a
    // *substring* of the cheatcode error, and the full error chain is
    // preserved, so these pin the specific failure and not merely the fact
    // that something failed.
    // ---------------------------------------------------------------------

    /// A transaction signed for another chain is rejected rather than replayed.
    function testRevertIfWrongChainId() public {
        vm.deal(SIGNER, 10 ether);

        vm._expectCheatcodeRevert(bytes("invalid chain ID"));
        vm.broadcastRawTransaction(CALL_TX_WRONG_CHAIN_ID);

        assertEq(Recorder(RECORDER).calls(), 0, "rejected transaction must not execute");
        assertEq(SIGNER.balance, 10 ether, "rejected transaction must not move value");
        assertEq(vm.getNonce(SIGNER), 0, "rejected transaction must not consume a nonce");
    }

    /// Control for the test above: the rejected blob is a perfectly good
    /// transaction that is simply bound to chain id 1, and it is signed by
    /// `SIGNER` like every other 1559 fixture here. Without this, the rejection
    /// test above would pass just as well on a blob with a junk signature.
    function testWrongChainIdTransactionIsValidOnItsOwnChain() public {
        vm.deal(SIGNER, 10 ether);
        vm.chainId(1);

        vm.broadcastRawTransaction(CALL_TX_WRONG_CHAIN_ID);

        assertEq(Recorder(RECORDER).calls(), 1, "transaction should have executed on chain 1");
        assertEq(Recorder(RECORDER).lastSender(), SIGNER, "sender should be the signature's signer");
    }

    /// The signer has to be able to pay for the transaction it signed. Note
    /// this depends on the Solidity test env leaving revm's balance check
    /// enabled, unlike its nonce check (see below).
    function testRevertIfSenderCannotPay() public {
        // One wei short of the transaction's maximum fee.
        vm.deal(NICKS_DEPLOYER, NICKS_MAX_FEE - 1);

        vm._expectCheatcodeRevert(bytes("lack of funds"));
        vm.broadcastRawTransaction(NICKS_TX);

        assertEq(NICKS_FACTORY.code.length, 0, "rejected transaction must not deploy");
        assertEq(NICKS_DEPLOYER.balance, NICKS_MAX_FEE - 1, "rejected transaction must not charge");
        assertEq(vm.getNonce(NICKS_DEPLOYER), 0, "rejected transaction must not consume a nonce");
    }

    /// Only the part of the message this crate owns is pinned; the tail comes
    /// from `alloy-rlp` and may be reworded by a dependency bump.
    function testRevertIfNotAValidTransaction() public {
        vm._expectCheatcodeRevert(
            bytes("vm.broadcastRawTransaction: failed to decode RLP-encoded transaction")
        );
        vm.broadcastRawTransaction(hex"deadbeef");
    }

    // ---------------------------------------------------------------------
    // Nonce handling
    //
    // Solidity tests run with revm's nonce check disabled (`disable_nonce_check`),
    // exactly as `forge test` does, so a transaction whose declared nonce does
    // not match the sender's account nonce is NOT rejected. It executes, and the
    // *account* nonce is what is consumed and what any CREATE address is derived
    // from. These tests pin that behaviour so the divergence from real-network
    // semantics is deliberate rather than incidental.
    // ---------------------------------------------------------------------

    function testDeclaredNonceIsNotEnforced() public {
        vm.deal(SIGNER, 10 ether);

        // Declares nonce 42; the signer's account nonce is 0.
        vm.broadcastRawTransaction(CALL_TX_WRONG_NONCE);

        assertEq(Recorder(RECORDER).calls(), 1, "transaction should have executed");
        // The account nonce advanced by one from its actual value, not to 43.
        assertEq(vm.getNonce(SIGNER), 1, "account nonce should have been used");
    }

    function testCreateAddressUsesTheAccountNonce() public {
        vm.deal(NICKS_DEPLOYER_NONCE_7, NICKS_MAX_FEE);

        // The transaction declares nonce 7, but the account is at nonce 0.
        vm.broadcastRawTransaction(NICKS_TX_NONCE_7);

        address fromAccountNonce = vm.computeCreateAddress(NICKS_DEPLOYER_NONCE_7, 0);
        address fromDeclaredNonce = vm.computeCreateAddress(NICKS_DEPLOYER_NONCE_7, 7);

        assertGt(fromAccountNonce.code.length, 0, "should deploy at the account-nonce address");
        assertEq(fromAccountNonce.code, hex"602a60005260206000f3", "unexpected deployed runtime");
        assertEq(fromDeclaredNonce.code.length, 0, "should not deploy at the declared-nonce address");
        assertEq(vm.getNonce(NICKS_DEPLOYER_NONCE_7), 1, "account nonce should have been consumed");
    }

    // ---------------------------------------------------------------------
    // Transaction-level semantics that are easy to get wrong silently
    // ---------------------------------------------------------------------

    /// A raw transaction whose *execution* reverts is still a valid, included
    /// transaction: the cheatcode does not fail, the nonce is consumed and the
    /// value is not transferred. Only transaction *validation* failures (chain
    /// id, funds, decoding) surface as cheatcode errors.
    function testRevertingTransactionIsIncludedNotRejected() public {
        vm.deal(SIGNER, 10 ether);

        vm.broadcastRawTransaction(REVERTER_TX);

        assertEq(vm.getNonce(SIGNER), 1, "nonce should have been consumed");
        assertEq(SIGNER.balance, 10 ether, "value should not have been transferred");
        assertEq(REVERTER.balance, 0, "reverted transfer must not land");
    }

    /// State produced by a broadcast transaction is committed to the database
    /// rather than journalled, so it is not undone when the frame that
    /// broadcast it reverts. `vm.snapshotState`/`vm.revertToState` is the way
    /// to roll it back. This matches upstream Foundry; pinned here because it
    /// is surprising and otherwise invisible.
    function testBroadcastSurvivesAnOuterRevert() public {
        vm.deal(NICKS_DEPLOYER, NICKS_MAX_FEE);

        try broadcaster.broadcastThenRevert(NICKS_TX) {
            revert("expected the outer frame to revert");
        } catch {}

        assertGt(NICKS_FACTORY.code.length, 0, "broadcast state should survive the outer revert");
    }

    /// ...and a state snapshot does roll it back.
    function testStateSnapshotRollsBackABroadcast() public {
        vm.deal(NICKS_DEPLOYER, NICKS_MAX_FEE);
        uint256 snapshot = vm.snapshotState();

        vm.broadcastRawTransaction(NICKS_TX);
        assertGt(NICKS_FACTORY.code.length, 0, "factory should be deployed");

        assertTrue(vm.revertToState(snapshot), "revertToState failed");
        assertEq(NICKS_FACTORY.code.length, 0, "snapshot should undo the broadcast");
        assertEq(vm.getNonce(NICKS_DEPLOYER), 0, "snapshot should undo the nonce bump");
    }
}
