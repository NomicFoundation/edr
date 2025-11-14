import {Test} from "forge-std/src/Test.sol";
import {Contract} from"../contracts/ContractLibraryMultiFork.sol";

// `can_use_libs_in_multi_fork` from `crates/forge/tests/cli/test_cmd.rs`
contract LibraryMultiForkTest is Test {
    function setUp() public {
        vm.createSelectFork("alchemyMainnet");
    }

    function test() public {
        new Contract();
    }
}
