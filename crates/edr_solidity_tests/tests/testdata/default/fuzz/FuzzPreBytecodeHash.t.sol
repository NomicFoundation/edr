// SPDX-License-Identifier: MIT OR Apache-2.0

// pre bytecode hash version, was introduced in 0.6.0
pragma solidity 0.5.17;

import "ds-test/test.sol";

// `can_test_pre_bytecode_hash` from `crates/forge/tests/cli/test_cmd.rs`
contract FuzzPreBytecodeHash is DSTest {
    function testArrayPreBytecodeHash(uint64[2] calldata) external {
        assertTrue(true);
    }
}
