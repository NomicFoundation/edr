// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import "forge-std/src/Test.sol";
import {StdChains} from "forge-std/src/StdChains.sol";

// `should_preserve_fork_state_setup` in crates/forge/tests/cli/test_cmd.rs
contract ForkStateSetupTest is Test {
    struct Domain {
        StdChains.Chain chain;
        uint256 forkId;
    }

    struct Bridge {
        Domain source;
        Domain destination;
        uint256 someVal;
    }

    struct SomeStruct {
        Domain domain;
        Bridge[] bridges;
    }

    mapping(uint256 => SomeStruct) internal data;

    function setUp() public {
        // Temporary workaround for `https://eth.llamarpc.com/` being down
        setChain("mainnet", ChainData({
            name: "mainnet",
            rpcUrl: "https://reth-ethereum.ithaca.xyz/rpc",
            chainId: 1
        }));

        StdChains.Chain memory chain1 = getChain("mainnet");
        StdChains.Chain memory chain2 = getChain("base");
        Domain memory domain1 = Domain(chain1, vm.createFork(chain1.rpcUrl, 22253716));
        Domain memory domain2 = Domain(chain2, vm.createFork(chain2.rpcUrl, 28839981));
        data[1].domain = domain1;
        data[2].domain = domain2;

        vm.selectFork(domain1.forkId);

        data[2].bridges.push(Bridge(domain1, domain2, 123));
        vm.selectFork(data[2].domain.forkId);
        vm.selectFork(data[1].domain.forkId);
        data[2].bridges.push(Bridge(domain1, domain2, 456));

        assertEq(data[2].bridges.length, 2);
    }

    function test_assert_storage() public {
        vm.selectFork(data[2].domain.forkId);
        assertEq(data[2].bridges.length, 2);
    }

    function test_modify_and_storage() public {
        data[3].domain = Domain(getChain("base"), vm.createFork(getChain("base").rpcUrl, 28839981));
        data[3].bridges.push(Bridge(data[1].domain, data[2].domain, 123));
        data[3].bridges.push(Bridge(data[1].domain, data[2].domain, 456));

        vm.selectFork(data[2].domain.forkId);
        assertEq(data[3].bridges.length, 2);
    }
}
