// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

interface Vm {
    function eip712HashType(
        string calldata typeNameOrDefinition
    ) external pure returns (bytes32);
}

/// A type name that is neither an inline definition nor a struct defined in
/// this contract's sources must fail to resolve, reverting the test.
contract Eip712UnknownTest {
    Vm constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    function testUnknownTypeReverts() external pure {
        vm.eip712HashType("ThisTypeIsNotDefinedAnywhere");
    }
}
