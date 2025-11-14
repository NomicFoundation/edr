import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Issue10012Test is DSTest {
    function test_something() public {
        CounterTestA counter = new CounterTestA();
        counter.doSomething();
    }
}

contract CounterTestA is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function doSomething() public {
        vm.startStateDiffRecording();
        require(1 > 2);
    }
}
