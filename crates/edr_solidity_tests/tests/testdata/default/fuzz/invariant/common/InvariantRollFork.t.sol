// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "ds-test/test.sol";
import "cheats/Vm.sol";
import {console} from "../../../logs/console.sol";

interface IERC20 {
    function totalSupply() external view returns (uint256 supply);
}

contract RollForkHandler is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    uint256 public totalSupply;

    function work() external {
        console.log("roll fork");
        vm.rollFork(block.number + 1);
        totalSupply = IERC20(0x6B175474E89094C44Da98b954EedeAC495271d0F).totalSupply();
        console.log("work totalSupply", totalSupply);
    }
}

contract InvariantRollForkBlockTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    RollForkHandler forkHandler;

    function setUp() public {
        vm.createSelectFork("rpcAliasMainnet", 19812632);
        forkHandler = new RollForkHandler();
    }

    function invariant_fork_handler_block() public {
        require(block.number < 19812634, "too many blocks mined");
    }
}

contract InvariantRollForkStateTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);
    RollForkHandler forkHandler;

    function setUp() public {
        vm.createSelectFork("rpcAliasMainnet", 19812632);
        forkHandler = new RollForkHandler();
    }

    function invariant_fork_handler_state() public {
        console.log("this address", address(this));
        console.log("fork handler address", address(forkHandler));
        console.log("pre totalSupply", forkHandler.totalSupply());
        require(forkHandler.totalSupply() < 3254378807384273078310283461, "wrong supply");
        console.log("post totalSupply", forkHandler.totalSupply());
    }
}
