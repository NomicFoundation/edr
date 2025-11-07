import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Counter {
    uint256 public number;

    function setNumber(uint256 newNumber) public {
        number = newNumber;
    }

    function increment() public {
        number++;
    }
}

contract Issue9526Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function test_start_stop_recording() public {
        vm.startDebugTraceRecording();
        Counter counter = new Counter();
        counter.increment();
        vm.stopAndReturnDebugTraceRecording();
    }
}
