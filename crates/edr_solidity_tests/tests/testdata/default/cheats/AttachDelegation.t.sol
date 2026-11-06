// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// Adapted from Foundry's `testdata/default/cheats/AttachDelegation.t.sol`.
// Solidity tests have no broadcast, so the `vm.broadcast` calls are removed and
// an account nonce change is made with `vm.setNonce` instead of a transaction.
contract AttachDelegationTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    event ExecutedBy(uint256 id);

    uint256 alice_pk = 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d;
    address payable alice = payable(0x70997970C51812dc3A010C7d01b50e0d17dc79C8);
    uint256 bob_pk = 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a;
    address bob = 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC;

    SimpleDelegateContract implementation;
    SimpleDelegateContract implementation2;
    ERC20 token;

    function setUp() public {
        implementation = new SimpleDelegateContract(1);
        implementation2 = new SimpleDelegateContract(2);
        token = new ERC20(alice);
    }

    function mintCalls(uint256 amount, address to) internal view returns (SimpleDelegateContract.Call[] memory calls) {
        calls = new SimpleDelegateContract.Call[](1);
        calls[0] = SimpleDelegateContract.Call({to: address(token), data: abi.encodeCall(ERC20.mint, (amount, to)), value: 0});
    }

    function testSignDelegationReturnsTheDelegation() public {
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk);

        assertEq(signedDelegation.implementation, address(implementation));
        assertEq(uint256(signedDelegation.nonce), 0);
        assertTrue(signedDelegation.v <= 1);
        // Signing doesn't attach the delegation.
        assertEq(alice.code.length, 0);
    }

    function testSignDelegationUsesTheAccountNonce() public {
        vm.setNonce(alice, 5);
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk);
        assertEq(uint256(signedDelegation.nonce), 5);
    }

    function testCallSingleAttachDelegation() public {
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk);
        vm.attachDelegation(signedDelegation);

        assertEq0(alice.code, abi.encodePacked(hex"ef0100", address(implementation)));
        // The token's minter is Alice, so this only succeeds if the call runs as Alice.
        SimpleDelegateContract(alice).execute(mintCalls(100, bob));

        assertEq(token.balanceOf(bob), 100);
    }

    function testCallSingleAttachCrossChainDelegation() public {
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk, true);
        vm.attachDelegation(signedDelegation, true);

        require(alice.code.length > 0, "no code written to alice");
        SimpleDelegateContract(alice).execute(mintCalls(100, bob));

        assertEq(token.balanceOf(bob), 100);
    }

    function testCrossChainDelegationIsSignedForChainIdZero() public {
        // Attaching a cross-chain signature as a single-chain one recovers a
        // different authority, so nothing is written to Alice.
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk, true);
        vm.attachDelegation(signedDelegation);

        assertEq(alice.code.length, 0);
    }

    /// forge-config: default.allow_internal_expect_revert = true
    function testCallSingleAttachDelegationWithNonce() public {
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk, 11);
        vm._expectCheatcodeRevert("vm.attachDelegation: invalid nonce for 0x70997970c51812dc3a010c7d01b50e0d17dc79c8: expected 0, got 11");
        vm.attachDelegation(signedDelegation);

        signedDelegation = vm.signDelegation(address(implementation), alice_pk, 0);
        vm.attachDelegation(signedDelegation);
    }

    function testMultiCallAttachDelegation() public {
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk);
        vm.attachDelegation(signedDelegation);

        SimpleDelegateContract.Call[] memory calls = new SimpleDelegateContract.Call[](2);
        calls[0] =
            SimpleDelegateContract.Call({to: address(token), data: abi.encodeCall(ERC20.mint, (50, bob)), value: 0});
        calls[1] = SimpleDelegateContract.Call({
            to: address(token), data: abi.encodeCall(ERC20.mint, (50, address(this))), value: 0
        });

        SimpleDelegateContract(alice).execute(calls);

        assertEq(token.balanceOf(bob), 50);
        assertEq(token.balanceOf(address(this)), 50);
    }

    function testSwitchAttachDelegation() public {
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk);
        vm.attachDelegation(signedDelegation);

        vm.expectEmit(true, true, true, true);
        emit ExecutedBy(1);
        SimpleDelegateContract(alice).execute(mintCalls(100, bob));

        // The next delegation of Alice takes the next nonce.
        Vm.SignedDelegation memory signedDelegation2 = vm.signDelegation(address(implementation2), alice_pk);
        assertEq(uint256(signedDelegation2.nonce), 1);
        vm.attachDelegation(signedDelegation2);

        vm.expectEmit(true, true, true, true);
        emit ExecutedBy(2);
        SimpleDelegateContract(alice).execute(mintCalls(100, bob));

        assertEq(token.balanceOf(bob), 200);
    }

    /// forge-config: default.allow_internal_expect_revert = true
    function testAttachDelegationRevertInvalidSignature() public {
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk);
        // change v from 1 to 0
        signedDelegation.v = (signedDelegation.v + 1) % 2;
        vm.attachDelegation(signedDelegation);

        // empty revert because no bytecode was set to Alice's account
        vm.expectRevert();
        SimpleDelegateContract(alice).execute(mintCalls(100, bob));
    }

    /// forge-config: default.allow_internal_expect_revert = true
    function testAttachDelegationRevertsAfterNonceChange() public {
        Vm.SignedDelegation memory signedDelegation = vm.signDelegation(address(implementation), alice_pk);

        vm.setNonce(alice, 1);

        vm._expectCheatcodeRevert("vm.attachDelegation: invalid nonce for 0x70997970c51812dc3a010c7d01b50e0d17dc79c8: expected 1, got 0");
        vm.attachDelegation(signedDelegation);
    }

    function testCallSingleSignAndAttachDelegation() public {
        Vm.SignedDelegation memory signedDelegation = vm.signAndAttachDelegation(address(implementation), alice_pk);
        assertEq(signedDelegation.implementation, address(implementation));
        assertEq(uint256(signedDelegation.nonce), 0);

        assertEq0(alice.code, abi.encodePacked(hex"ef0100", address(implementation)));
        SimpleDelegateContract(alice).execute(mintCalls(100, bob));

        assertEq(token.balanceOf(bob), 100);
    }

    function testCallSingleSignAndAttachCrossChainDelegation() public {
        vm.signAndAttachDelegation(address(implementation), alice_pk, true);

        require(alice.code.length > 0, "no code written to alice");
        SimpleDelegateContract(alice).execute(mintCalls(100, bob));

        assertEq(token.balanceOf(bob), 100);
    }

    /// forge-config: default.allow_internal_expect_revert = true
    function testCallSingleSignAndAttachDelegationWithNonce() public {
        vm._expectCheatcodeRevert("vm.signAndAttachDelegation: invalid nonce for 0x70997970c51812dc3a010c7d01b50e0d17dc79c8: expected 0, got 11");
        vm.signAndAttachDelegation(address(implementation), alice_pk, 11);

        vm.signAndAttachDelegation(address(implementation), alice_pk, 0);
    }

    /// forge-config: default.allow_internal_expect_revert = true
    function testMultipleDelegations() public {
        vm.signAndAttachDelegation(address(implementation), alice_pk);
        vm.signAndAttachDelegation(address(implementation2), bob_pk);

        vm.expectEmit(true, true, true, true);
        emit ExecutedBy(1);
        SimpleDelegateContract(alice).execute(mintCalls(50, address(this)));
        vm.expectEmit(true, true, true, true);
        emit ExecutedBy(2);
        SimpleDelegateContract(payable(bob)).execute(new SimpleDelegateContract.Call[](0));

        assertEq(token.balanceOf(address(this)), 50);

        // Each authority's next delegation takes the nonce after its last one.
        vm._expectCheatcodeRevert("vm.signAndAttachDelegation: invalid nonce for 0x70997970c51812dc3a010c7d01b50e0d17dc79c8: expected 1, got 0");
        vm.signAndAttachDelegation(address(implementation), alice_pk, 0);
        vm.signAndAttachDelegation(address(implementation), alice_pk, 1);
        vm.signAndAttachDelegation(address(implementation2), bob_pk, 1);
    }

    function testDelegationToZeroAddressClearsCode() public {
        vm.signAndAttachDelegation(address(implementation), alice_pk);
        assertTrue(alice.code.length > 0);

        vm.signAndAttachDelegation(address(0), alice_pk);
        assertEq(alice.code.length, 0);
        // Alice's account is empty again, so it has no code hash.
        assertEq(alice.codehash, bytes32(0));
    }

    function testRevertIfPkZero() public {
        vm._expectCheatcodeRevert(bytes("vm.signDelegation: private key cannot be 0"));
        vm.signDelegation(address(implementation), 0);
    }
}

contract SimpleDelegateContract {
    event Executed(address indexed to, uint256 value, bytes data);
    event ExecutedBy(uint256 id);

    struct Call {
        bytes data;
        address to;
        uint256 value;
    }

    uint256 public immutable id;

    constructor(uint256 _id) {
        id = _id;
    }

    function execute(Call[] memory calls) external payable {
        for (uint256 i = 0; i < calls.length; i++) {
            Call memory call = calls[i];
            (bool success, bytes memory result) = call.to.call{value: call.value}(call.data);
            require(success, string(result));
            emit Executed(call.to, call.value, call.data);
        }
        emit ExecutedBy(id);
    }

    receive() external payable {}
}

contract ERC20 {
    address public minter;
    mapping(address => uint256) private _balances;

    constructor(address _minter) {
        minter = _minter;
    }

    function mint(uint256 amount, address to) public {
        _mint(to, amount);
    }

    function balanceOf(address account) public view returns (uint256) {
        return _balances[account];
    }

    function _mint(address account, uint256 amount) internal {
        require(msg.sender == minter, "ERC20: msg.sender is not minter");
        require(account != address(0), "ERC20: mint to the zero address");
        unchecked {
            _balances[account] += amount;
        }
    }
}
