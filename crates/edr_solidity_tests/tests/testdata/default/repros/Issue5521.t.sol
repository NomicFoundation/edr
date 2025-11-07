import "ds-test/test.sol";
import "cheats/Vm.sol";

contract Issue5521Test is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function test_stackPrank() public {
        string memory name = "player";
        uint256 privateKey = uint256(keccak256(abi.encodePacked(name)));
        address player = vm.addr(privateKey);
        vm.label(player, name);

        SenderLogger senderLogger = new SenderLogger();
        Contract c = new Contract();

        (address sender1, address origin1) = senderLogger.log();
        assertEq(sender1, address(this));
        assertEq(origin1, address(0x1804c8AB1F12E6bbf3894d4083f33e07309d1f38));  // Default sender

        vm.startPrank(player, player);
        (address sender2, address origin2) = senderLogger.log();
        assertEq(sender2, player);
        assertEq(origin2, player);

        c.f(); // vm.startPrank(player)
        (address sender3, address origin3) = senderLogger.log();
        assertEq(sender3, player);
        assertEq(origin3, player);
        vm.stopPrank();
    }
}

contract Contract {
    Vm public constant vm = Vm(address(bytes20(uint160(uint256(keccak256("hevm cheat code"))))));

    function f() public {
        vm.startPrank(msg.sender);
    }
}

contract SenderLogger {
    function log() public returns (address sender, address origin) {
        sender = msg.sender;
        origin = tx.origin;
    }
}
