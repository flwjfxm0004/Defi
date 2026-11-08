// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Test.sol";

contract InsurancePoolProbe is Test {
    function test_ethereum_code() public {
        vm.createSelectFork("mainnet");
        address a = 0x5504acac99cc0053034761B253101F7d94008Aa6;
        assertTrue(a.code.length > 0);
    }
}
