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

    // ---------------------------------------------------------------------
    // Ported from upstream Foundry's BroadcastRawTransaction.t.sol.
    //
    // The signed fixtures are legacy transactions bound to chain id 1 with
    // gasPrice 100 wei and gas 200000, so the tests that use them first
    // switch to chain id 1 and set a non-zero base fee, as upstream does.
    // The blobs are copied verbatim from upstream; the `Signed transaction`
    // comments describe what each one decodes to.
    // ---------------------------------------------------------------------

    address constant UPSTREAM_SIGNER = 0x5316812db67073C4d4af8BB3000C5B86c2877e94;
    address constant UPSTREAM_RECIPIENT = 0x6Fd0A0CFF9A87aDF51695b40b4fA267855a8F4c6;
    uint256 constant UPSTREAM_GAS_PRICE = 100;
    uint256 constant UPSTREAM_AMOUNT = 17;

    // Signed transaction:
    // { from: UPSTREAM_SIGNER, to: UPSTREAM_RECIPIENT, gas: 200000, gasPrice: 100,
    //   value: 17, data: none, nonce: 0, chainId: 1 }
    bytes constant UPSTREAM_TRANSFER_TX =
        hex"f860806483030d40946fd0a0cff9a87adf51695b40b4fa267855a8f4c6118025a03ebeabbcfe43c2c982e99b376b5fb6e765059d7f215533c8751218cac99bbd80a00a56cf5c382442466770a756e81272d06005c9e90fb8dbc5b53af499d5aca856";

    // A well-formed RLP list that is an *unsigned* legacy transaction: six
    // fields, no v/r/s. Structurally valid RLP, but not a transaction.
    bytes constant UPSTREAM_UNSIGNED_TX =
        hex"dd806483030d40940993863c19b0defb183ca2b502db7d1b331ded757b80";

    // ERC20 fixture for the calldata tests. The two blobs below call
    // `approve` and `transfer` on a token etched at this address.
    address constant TOKEN = 0x5bF11839F61EF5ccEEaf1F4153e44df5D02825f7;
    address constant ALICE = 0x7ED31830602f9F7419307235c0610Fb262AA0375;
    address constant BOB = 0x70CF146aB98ffD5dE24e75dd7423F16181Da8E13;
    address constant CHARLIE = 0xae0900Cf97f8C233c64F7089cEC7d5457215BB8d;

    // Signed transaction:
    // { from: ALICE, to: TOKEN, value: 0, data: approve(BOB, 50), nonce: 0,
    //   gasPrice: 100, gasLimit: 200000, chainId: 1 }
    bytes constant ALICE_APPROVE_BOB_TX =
        hex"f8a5806483030d40945bf11839f61ef5cceeaf1f4153e44df5d02825f780b844095ea7b300000000000000000000000070cf146ab98ffd5de24e75dd7423f16181da8e13000000000000000000000000000000000000000000000000000000000000003225a0e25b9ef561d9a413b21755cc0e4bb6e80f2a88a8a52305690956130d612074dfa07bfd418bc2ad3c3f435fa531cdcdc64887f64ed3fb0d347d6b0086e320ad4eb1";
    // Signed transaction:
    // { from: CHARLIE, to: TOKEN, value: 0, data: transfer(BOB, 5), nonce: 0,
    //   gasPrice: 100, gasLimit: 200000, chainId: 1 }
    bytes constant CHARLIE_TRANSFER_BOB_TX =
        hex"f8a5806483030d40945bf11839f61ef5cceeaf1f4153e44df5d02825f780b844a9059cbb00000000000000000000000070cf146ab98ffd5de24e75dd7423f16181da8e13000000000000000000000000000000000000000000000000000000000000000525a0941562f519e33dfe5b44ebc2b799686cebeaeacd617dd89e393620b380797da2a0447dfd38d9444ccd571b000482c81674733761753430c81ee6669e9542c266a1";

    function _switchToUpstreamEnv() internal {
        vm.fee(1);
        vm.chainId(1);
    }

    /// A legacy transaction pays `gasPrice` for every unit of gas it uses,
    /// regardless of the base fee, and a plain transfer uses exactly 21000.
    /// Pins the exact fee, where the tests above only pin conservation, and
    /// runs under a non-zero base fee set by a cheatcode, which the nested
    /// EVM has to inherit.
    function testExactFeeForAPlainTransfer() public {
        _switchToUpstreamEnv();
        vm.deal(UPSTREAM_SIGNER, 1 ether);
        assertEq(UPSTREAM_RECIPIENT.balance, 0);

        vm.broadcastRawTransaction(UPSTREAM_TRANSFER_TX);

        assertEq(
            UPSTREAM_SIGNER.balance,
            1 ether - (UPSTREAM_GAS_PRICE * 21_000) - UPSTREAM_AMOUNT,
            "sender should have paid exactly 21000 gas at gasPrice plus the value"
        );
        assertEq(UPSTREAM_RECIPIENT.balance, UPSTREAM_AMOUNT, "recipient should have received the value");
    }

    /// The account the raw transaction credited can immediately spend that
    /// balance from a pranked call. This is the cheapest direct check that the
    /// running test's journal was refreshed with the broadcast's results.
    function testRecipientCanSpendTheBroadcastValue() public {
        _switchToUpstreamEnv();
        vm.deal(UPSTREAM_SIGNER, 1 ether);
        address random = address(uint160(uint256(keccak256(abi.encodePacked("random")))));

        vm.broadcastRawTransaction(UPSTREAM_TRANSFER_TX);
        assertEq(UPSTREAM_RECIPIENT.balance, UPSTREAM_AMOUNT);
        assertEq(random.balance, 0);

        uint256 value = 5;
        vm.prank(UPSTREAM_RECIPIENT);
        (bool success,) = random.call{value: value}("");
        assertTrue(success, "recipient should be able to spend what the broadcast sent it");

        assertEq(UPSTREAM_RECIPIENT.balance, UPSTREAM_AMOUNT - value, "recipient balance should reflect the spend");
        assertEq(random.balance, value, "value should have arrived");
    }

    /// The cheatcode-revert machinery keeps working after the nested EVM ran
    /// with the same inspector. Upstream added this after hitting a
    /// journaled-state bug in exactly this spot.
    function testCheatcodeRevertsStillWorkAfterABroadcast() public {
        _switchToUpstreamEnv();
        vm.deal(UPSTREAM_SIGNER, 1 ether);

        vm.broadcastRawTransaction(UPSTREAM_TRANSFER_TX);
        assertEq(UPSTREAM_RECIPIENT.balance, UPSTREAM_AMOUNT);

        vm._expectCheatcodeRevert();
        vm.assertFalse(true);
    }

    /// Two raw transactions with calldata, from two different signers, write
    /// to a contract's storage, interleaved with an ordinary pranked call. The
    /// only test in this file whose raw transactions carry calldata.
    function testMultipleSignedTransactionsWithCalldata() public {
        _switchToUpstreamEnv();

        // Equivalent to `new MyERC20()` at the address the fixtures were signed for.
        vm.etch(TOKEN, type(MyERC20).runtimeCode);
        MyERC20 token = MyERC20(TOKEN);

        token.mint(100, ALICE);
        assertEq(token.balanceOf(ALICE), 100);
        assertEq(token.balanceOf(BOB), 0);
        assertEq(token.balanceOf(CHARLIE), 0);

        // Equivalent to `vm.prank(ALICE); token.approve(BOB, 50);`
        vm.deal(ALICE, 10 ether);
        vm.broadcastRawTransaction(ALICE_APPROVE_BOB_TX);
        assertEq(token.allowance(ALICE, BOB), 50, "approve from the raw transaction should have landed");

        vm.deal(BOB, 1 ether);
        vm.prank(BOB);
        token.transferFrom(ALICE, CHARLIE, 20);
        assertEq(token.balanceOf(BOB), 0);
        assertEq(token.balanceOf(CHARLIE), 20);

        // Equivalent to `vm.prank(CHARLIE); token.transfer(BOB, 5);`
        vm.deal(CHARLIE, 1 ether);
        vm.broadcastRawTransaction(CHARLIE_TRANSFER_BOB_TX);

        assertEq(token.balanceOf(ALICE), 80, "alice should have lost the transferred amount");
        assertEq(token.balanceOf(BOB), 5, "bob should have received the raw transfer");
        assertEq(token.balanceOf(CHARLIE), 15, "charlie should have paid the raw transfer");
    }

    /// An unsigned transaction is well-formed RLP but not a transaction. Like
    /// the junk-bytes test above, only the crate-owned prefix is pinned.
    function testRevertIfTransactionIsUnsigned() public {
        vm._expectCheatcodeRevert(
            bytes("vm.broadcastRawTransaction: failed to decode RLP-encoded transaction")
        );
        vm.broadcastRawTransaction(UPSTREAM_UNSIGNED_TX);
    }
}

/// Minimal ERC20 for the calldata tests, ported from upstream Foundry.
contract MyERC20 {
    mapping(address => uint256) private _balances;
    mapping(address => mapping(address => uint256)) private _allowances;

    function mint(uint256 amount, address to) public {
        _mint(to, amount);
    }

    function balanceOf(address account) public view returns (uint256) {
        return _balances[account];
    }

    function transfer(address to, uint256 amount) public returns (bool) {
        _transfer(msg.sender, to, amount);
        return true;
    }

    function allowance(address owner, address spender) public view returns (uint256) {
        return _allowances[owner][spender];
    }

    function approve(address spender, uint256 amount) public returns (bool) {
        _approve(msg.sender, spender, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) public returns (bool) {
        _spendAllowance(from, msg.sender, amount);
        _transfer(from, to, amount);
        return true;
    }

    function _transfer(address from, address to, uint256 amount) internal {
        require(from != address(0), "ERC20: transfer from the zero address");
        require(to != address(0), "ERC20: transfer to the zero address");

        uint256 fromBalance = _balances[from];
        require(fromBalance >= amount, "ERC20: transfer amount exceeds balance");
        unchecked {
            _balances[from] = fromBalance - amount;
            _balances[to] += amount;
        }
    }

    function _mint(address account, uint256 amount) internal {
        require(account != address(0), "ERC20: mint to the zero address");
        unchecked {
            _balances[account] += amount;
        }
    }

    function _approve(address owner, address spender, uint256 amount) internal {
        require(owner != address(0), "ERC20: approve from the zero address");
        require(spender != address(0), "ERC20: approve to the zero address");
        _allowances[owner][spender] = amount;
    }

    function _spendAllowance(address owner, address spender, uint256 amount) internal {
        uint256 currentAllowance = allowance(owner, spender);
        if (currentAllowance != type(uint256).max) {
            require(currentAllowance >= amount, "ERC20: insufficient allowance");
            unchecked {
                _approve(owner, spender, currentAllowance - amount);
            }
        }
    }
}
