// SPDX-License-Identifier: MIT
pragma solidity 0.8.24;

import "./Eip712Imported.sol";
import "@fixtures/Eip712External.sol";

interface Vm {
    function eip712HashType(
        string calldata typeNameOrDefinition
    ) external pure returns (bytes32);

    function eip712HashStruct(
        string calldata typeNameOrDefinition,
        bytes calldata abiEncodedData
    ) external pure returns (bytes32);
}

// Defined in this file; referenced by `Mail`.
struct Person {
    address wallet;
    string name;
}

// Defined in this file; references `Person`, so its canonical type inlines the
// `Person` dependency.
struct Mail {
    Person from;
    Person to;
    string contents;
}

// A struct with only static members, used to check `eip712HashStruct`.
struct Point {
    uint256 x;
    uint256 y;
}

/// Exercises the lazy EIP-712 type resolution used by `vm.eip712HashType` and
/// `vm.eip712HashStruct`: each type below is resolved by parsing this test
/// contract's own sources (and its relative/mapped imports) on demand, with no
/// canonical types configured up front.
contract Eip712LazyTest {
    Vm constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    function testHashTypeFromLocalStructs() external pure {
        require(
            vm.eip712HashType("Mail") ==
                keccak256(
                    "Mail(Person from,Person to,string contents)Person(address wallet,string name)"
                ),
            "Mail type hash mismatch"
        );
        require(
            vm.eip712HashType("Person") ==
                keccak256("Person(address wallet,string name)"),
            "Person type hash mismatch"
        );
    }

    function testHashTypeFromRelativeImport() external pure {
        require(
            vm.eip712HashType("Asset") ==
                keccak256("Asset(address token,uint256 amount)"),
            "Asset type hash mismatch"
        );
    }

    function testHashTypeFromMappedImport() external pure {
        require(
            vm.eip712HashType("Coupon") ==
                keccak256("Coupon(uint256 id,address issuer)"),
            "Coupon type hash mismatch"
        );
    }

    function testHashTypeFromInlineDefinition() external pure {
        // A type given as a full definition (contains `(`) is parsed directly,
        // not looked up from sources.
        require(
            vm.eip712HashType("Foo(uint256 a,bytes32 b)") ==
                keccak256("Foo(uint256 a,bytes32 b)"),
            "inline type hash mismatch"
        );
    }

    function testHashStruct() external pure {
        Point memory point = Point({x: 7, y: 11});
        bytes32 expected = keccak256(
            abi.encode(keccak256("Point(uint256 x,uint256 y)"), point.x, point.y)
        );
        require(
            vm.eip712HashStruct("Point", abi.encode(point)) == expected,
            "Point struct hash mismatch"
        );
    }
}
