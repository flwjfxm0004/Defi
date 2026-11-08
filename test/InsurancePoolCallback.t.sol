// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import "forge-std/Test.sol";

contract InsurancePoolCallbackTest is Test {
    address target = 0x87242a55F4B77991584cC285E874b26BB7881Dce;

    function setUp() public {
        vm.createSelectFork("mainnet");
    }

    function test_receiveFlashLoan_reverts_for_attacker() public {
        (bool ok, bytes memory data) = target.call(abi.encodeWithSignature("receiveFlashLoan(address[],uint256[],uint256[],bytes)", new address[](0), new uint256[](0), new uint256[](0), ""));
        require(!ok, "expected revert");
        require(data.length > 0, "expected error data");
    }

    function test_uniswapV3SwapCallback_reverts_for_attacker() public {
        (bool ok,) = target.call(abi.encodeWithSignature("uniswapV3SwapCallback(int256,int256,bytes)", int256(0), int256(0), ""));
        require(!ok, "expected revert");
    }
}
