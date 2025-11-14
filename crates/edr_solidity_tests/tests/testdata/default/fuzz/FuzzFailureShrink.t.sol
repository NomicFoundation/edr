contract Counter {
    uint256 public number = 0;

    function addOne(uint256 x) external pure returns (uint256) {
        return x + 100_000_000;
    }
}

// `should_not_shrink_fuzz_failure` from `crates/forge/tests/cli/test_cmd.rs`
contract FuzzFailureShrinkTest {
    Counter public counter;

    function setUp() public {
        counter = new Counter();
    }

    function testAddOne(uint256 x) public view {
        require(counter.addOne(x) == x + 100_000_000, "not equal");
    }
}
