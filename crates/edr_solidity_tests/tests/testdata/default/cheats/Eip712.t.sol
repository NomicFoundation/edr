// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";

// EIP-712 worked example from https://eips.ethereum.org/EIPS/eip-712
contract Eip712Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    string constant MAIL_TYPE =
        "Mail(Person from,Person to,string contents)Person(string name,address wallet)";
    string constant PERSON_TYPE = "Person(string name,address wallet)";

    struct Person {
        string name;
        address wallet;
    }

    struct Mail {
        Person from;
        Person to;
        string contents;
    }

    address constant COW = 0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826;
    address constant BOB = 0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB;

    function _mail() internal pure returns (Mail memory) {
        return
            Mail({
                from: Person({name: "Cow", wallet: COW}),
                to: Person({name: "Bob", wallet: BOB}),
                contents: "Hello, Bob!"
            });
    }

    function _hashPerson(Person memory p) internal pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(PERSON_TYPE)),
                    keccak256(bytes(p.name)),
                    p.wallet
                )
            );
    }

    function _hashMail(Mail memory m) internal pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    keccak256(bytes(MAIL_TYPE)),
                    _hashPerson(m.from),
                    _hashPerson(m.to),
                    keccak256(bytes(m.contents))
                )
            );
    }

    function testEip712HashType() public {
        assertEq(vm.eip712HashType(MAIL_TYPE), keccak256(bytes(MAIL_TYPE)));
    }

    function testEip712HashTypeNormalizesWhitespace() public {
        // Non-canonical whitespace after commas should be normalized before hashing.
        assertEq(
            vm.eip712HashType(
                "Mail(address from, address to, string contents)"
            ),
            keccak256(bytes("Mail(address from,address to,string contents)"))
        );
    }

    function testEip712HashStruct() public {
        Mail memory m = _mail();
        assertEq(vm.eip712HashStruct(MAIL_TYPE, abi.encode(m)), _hashMail(m));
    }

    function testEip712HashTypedData() public {
        string memory json = '{"types":{'
        '"EIP712Domain":['
        '{"name":"name","type":"string"},'
        '{"name":"version","type":"string"},'
        '{"name":"chainId","type":"uint256"},'
        '{"name":"verifyingContract","type":"address"}'
        "],"
        '"Person":['
        '{"name":"name","type":"string"},'
        '{"name":"wallet","type":"address"}'
        "],"
        '"Mail":['
        '{"name":"from","type":"Person"},'
        '{"name":"to","type":"Person"},'
        '{"name":"contents","type":"string"}'
        "]"
        "},"
        '"primaryType":"Mail",'
        '"domain":{'
        '"name":"Ether Mail",'
        '"version":"1",'
        '"chainId":1,'
        '"verifyingContract":"0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"'
        "},"
        '"message":{'
        '"from":{"name":"Cow","wallet":"0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826"},'
        '"to":{"name":"Bob","wallet":"0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB"},'
        '"contents":"Hello, Bob!"'
        "}"
        "}";

        bytes32 domainTypeHash = keccak256(
            "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
        );
        bytes32 domainSeparator = keccak256(
            abi.encode(
                domainTypeHash,
                keccak256(bytes("Ether Mail")),
                keccak256(bytes("1")),
                uint256(1),
                address(0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC)
            )
        );
        bytes32 expected = keccak256(
            abi.encodePacked(
                bytes2(0x1901),
                domainSeparator,
                _hashMail(_mail())
            )
        );

        assertEq(vm.eip712HashTypedData(json), expected);
    }

    // Bindings-path overloads are unsupported: they would require filesystem
    // access, which EDR intentionally does not provide for EIP-712 cheatcodes.
    // See https://github.com/NomicFoundation/edr/issues/1365 for context.

    function testEip712HashTypeBindingsPathReverts() public {
        vm._expectCheatcodeRevert();
        vm.eip712HashType("bindings.json", "Mail");
    }

    function testEip712HashStructBindingsPathReverts() public {
        vm._expectCheatcodeRevert();
        vm.eip712HashStruct("bindings.json", "Mail", hex"");
    }
}
