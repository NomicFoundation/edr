// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import "./Permit.sol";
import {Permit as VendoredPermit} from "./vendor/Permit.sol";

interface Vm {
    function eip712HashType(
        string calldata typeNameOrDefinition
    ) external pure returns (bytes32);
}

/// Both `Permit` definitions are reachable from here, so this suite passes
/// only once `eip712ExcludeGlobs` drops the vendored one.
contract Eip712ExcludeTest {
    Vm constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    function testHashTypeResolvesTheUnexcludedPermit() external pure {
        require(
            vm.eip712HashType("Permit") ==
                keccak256(
                    "Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)"
                ),
            "Permit type hash mismatch"
        );
    }
}
