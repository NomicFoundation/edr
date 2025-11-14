import {Test} from "forge-std/Test.sol";

contract AssumeHandler is Test {
    function fuzzMe(uint256 a) public {
        require(false, "Invariant failure");
    }
}

contract InvariantSourceChangeTest is Test {
    function setUp() public {
        AssumeHandler handler = new AssumeHandler();
    }
    function invariant_assume() public {}
}
