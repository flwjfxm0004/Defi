// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;
import "forge-std/Test.sol";

contract DeployTest is Test {
    function test_deploy() public {
        address d = vm.deployCode("test/data/insurance.hex");
        console2.log("deployed:", d);
    }
}
