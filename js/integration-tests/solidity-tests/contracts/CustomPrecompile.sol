// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

// Test vector from: https://github.com/C2SP/wycheproof/blob/4a6c2bf5dc4c0b67c770233ad33961ee653996a0/testvectors/ecdsa_secp256r1_sha256_test.json#L17
contract CustomPrecompile {
    bytes32 h =
        0xbb5a52f42f9c9261ed4361f59422a1e30036e7c32b270c8807a419feca605023;
    bytes32 r =
        0x2ba3a8be6b94d5ec80a6d9d1190a436effe50d85a1eee859b8cc6af9bd5c2e18;
    bytes32 s =
        0x4cd60b855d442f5b3c7b11eb6c4e0ae7525fe710fab9aa7c77a67f79e6fadd76;
    bytes32 x =
        0x2927b10512bae3eddcfe467828128bad2903269919f7086069c8c4df6c732838;
    bytes32 y =
        0xc7787964eaac00e5921fb1498a60f4606766b3d9685001558d1a974e7341513e;

    function rip2712Precompile() public view {
        bytes
            memory expected = hex"0000000000000000000000000000000000000000000000000000000000000001";
        (bool success, bytes memory returndata) = address(0x100).staticcall(
            abi.encode(h, r, s, x, y)
        );
        if (!success) {
            revert("precompile returned success = false");
        }
        if (keccak256(returndata) != keccak256(expected)) {
            revert("precompile returned wrong return data");
        }
    }
}
