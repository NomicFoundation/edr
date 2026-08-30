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
    // EIP-1559 transactions signed by the well-known development key
    // 0xac09...ff80, sending 1 ether with empty calldata to `RECORDER`, with
    // gas 200000, maxFeePerGas 1 gwei and maxPriorityFeePerGas 0. The test
    // block's base fee is 0, so the effective gas price is 0 and balances
    // move by exactly the transferred value.
    // ---------------------------------------------------------------------
    address constant SIGNER = 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266;
    address payable constant RECORDER = payable(0x00000000000000000000000000000000000B0b01);

    bytes constant CALL_TX =
        hex"02f871827a698080843b9aca0083030d409400000000000000000000000000000000000b0b01880de0b6b3a764000080c001a09aaeeeb42c4aa82e62fd7e8c21eb99917667ac7e83bed1aa80f663bfbcb5c727a0268aac2d27903031448c40ea5aa3d173bae8713deedc3463a10191a38ba6a259";
    // Identical, but signed for chain id 1 while the tests run on 31337.
    bytes constant CALL_TX_WRONG_CHAIN_ID =
        hex"02f86f018080843b9aca0083030d409400000000000000000000000000000000000b0b01880de0b6b3a764000080c080a0b09ca38a8a2b093d8d25cdc9a4306d48261fe5b8e109203dbcdf8536c6eb5564a01133c1668673f3ebd2df01b45307620019bd9bb07a06c0ec1771ee98730a4954";
    // Identical, but declares nonce 42 while the signer's nonce is 0.
    bytes constant CALL_TX_WRONG_NONCE =
        hex"02f871827a692a80843b9aca0083030d409400000000000000000000000000000000000b0b01880de0b6b3a764000080c001a09efca1332dc5757d8939261633f3bf606984419acd5d9bd10839b3e705024943a0419d678609a9fede554dd735eb3234ba85007f97f21ace5a6d7da980acba684e";

    function setUp() public {
        Recorder recorder = new Recorder();
        vm.etch(RECORDER, address(recorder).code);
    }

    /// The motivating case: replaying a pre-signed deterministic-deployment
    /// bootstrap. Asserts the transaction's effect on state, and that it was
    /// executed as the keyless deployer rather than as the test contract.
    function testNicksMethodBootstrap() public {
        vm.deal(NICKS_DEPLOYER, NICKS_MAX_FEE);

        assertEq(NICKS_FACTORY.code.length, 0, "factory should not exist yet");
        assertEq(vm.getNonce(NICKS_DEPLOYER), 0, "deployer should be at nonce 0");

        vm.broadcastRawTransaction(NICKS_TX);

        // The contract landed, and it landed at the address derived from the
        // *signature's* signer, not from the test contract.
        assertEq(NICKS_FACTORY, vm.computeCreateAddress(NICKS_DEPLOYER, 0), "unexpected factory address");
        assertTrue(NICKS_FACTORY != vm.computeCreateAddress(address(this), vm.getNonce(address(this))));
        assertGt(NICKS_FACTORY.code.length, 0, "factory should have code");

        // And it works.
        (bool ok, bytes memory ret) = NICKS_FACTORY.staticcall("");
        assertTrue(ok, "call to deployed contract failed");
        assertEq(abi.decode(ret, (uint256)), 42, "deployed contract returned the wrong value");

        // The deployer, not the test contract, paid for it and consumed a nonce.
        assertEq(vm.getNonce(NICKS_DEPLOYER), 1, "deployer nonce should have been consumed");
        assertLt(NICKS_DEPLOYER.balance, NICKS_MAX_FEE, "deployer should have paid the gas");
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
        assertTrue(recorder.lastSender() != address(this), "sender must not be the test contract");
        assertTrue(recorder.lastSender() != msg.sender, "sender must not be the test caller");

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

        assertEq(Recorder(RECORDER).lastSender(), SIGNER, "prank must not reassign the sender");
    }

    // ---------------------------------------------------------------------
    // Rejections
    // ---------------------------------------------------------------------

    /// A transaction signed for another chain is rejected rather than replayed.
    /// The chain id is checked by the EVM, so the exact wording belongs to revm;
    /// what is pinned here is that it fails and leaves no trace.
    function testRevertIfWrongChainId() public {
        vm.deal(SIGNER, 10 ether);

        vm._expectCheatcodeRevert();
        vm.broadcastRawTransaction(CALL_TX_WRONG_CHAIN_ID);

        assertEq(Recorder(RECORDER).calls(), 0, "rejected transaction must not execute");
        assertEq(SIGNER.balance, 10 ether, "rejected transaction must not move value");
        assertEq(vm.getNonce(SIGNER), 0, "rejected transaction must not consume a nonce");
    }

    /// The signer has to be able to pay for the transaction it signed.
    function testRevertIfSenderCannotPay() public {
        // One wei short of the transaction's maximum fee.
        vm.deal(NICKS_DEPLOYER, NICKS_MAX_FEE - 1);

        vm._expectCheatcodeRevert();
        vm.broadcastRawTransaction(NICKS_TX);

        assertEq(NICKS_FACTORY.code.length, 0, "rejected transaction must not deploy");
        assertEq(NICKS_DEPLOYER.balance, NICKS_MAX_FEE - 1, "rejected transaction must not charge");
    }

    function testRevertIfNotAValidTransaction() public {
        vm._expectCheatcodeRevert(
            bytes("vm.broadcastRawTransaction: failed to decode RLP-encoded transaction: input too short")
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
        assertEq(fromDeclaredNonce.code.length, 0, "should not deploy at the declared-nonce address");
    }
}
