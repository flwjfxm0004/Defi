// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface Vm {
    struct Log {
        bytes32 topic0;
        bytes32[] topics;
        bytes data;
    }
    function prank(address msgSender) external;
    function prank(address msgSender, address txOrigin) external;
    function startPrank(address msgSender) external;
    function startPrank(address msgSender, address txOrigin) external;
    function stopPrank() external;
    function assume(bool condition) external pure;
    function recordLogs() external;
    function getRecordedLogs() external returns (Log[] memory);
}

contract LiquidatorHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337 {
    address internal _cbPool;
    address internal _cbPayer;
    address internal _cbToken;

    function _tBal(address tok, address who) internal view returns (uint256) {
        (bool ok, bytes memory r) = tok.staticcall(abi.encodeWithSelector(0x70a08231, who));
        if (ok && r.length >= 32) { return abi.decode(r, (uint256)); }
        return 0;
    }

    function _tApprove(address tok, address spender, uint256 amt) internal {
        (bool ok, bytes memory r) = tok.call(abi.encodeWithSelector(0x095ea7b3, spender, amt));
        require(ok && (r.length == 0 || (r.length >= 32 && abi.decode(r, (bool)))), "APPR");
    }

    function _tResetApprove(address tok, address spender) internal {
        (bool okA, bytes memory rA) = tok.staticcall(abi.encodeWithSelector(0xdd62ed3e, address(this), spender));
        if (okA && rA.length >= 32 && abi.decode(rA, (uint256)) == 0) { return; }
        (bool ok, bytes memory r) = tok.call(abi.encodeWithSelector(0x095ea7b3, spender, uint256(0)));
        if (okA && rA.length >= 32) {
            require(ok && (r.length == 0 || (r.length >= 32 && abi.decode(r, (bool)))), "APPR0");
        }
    }

    function _tTransferFrom(address tok, address from, address to, uint256 amt) internal {
        (bool ok, bytes memory r) = tok.call(abi.encodeWithSelector(0x23b872dd, from, to, amt));
        require(ok && (r.length == 0 || (r.length >= 32 && abi.decode(r, (bool)))), "TRFF");
    }

    function _tTransfer(address tok, address to, uint256 amt) internal {
        (bool ok, bytes memory r) = tok.call(abi.encodeWithSelector(0xa9059cbb, to, amt));
        require(ok && (r.length == 0 || (r.length >= 32 && abi.decode(r, (bool)))), "TRF");
    }

    function _isTok0(address pool, address tokIn) internal view returns (bool) {
        (bool ok0, bytes memory r0) = pool.staticcall(hex"0dfe1681");
        require(ok0 && r0.length >= 32, "T0");
        return abi.decode(r0, (address)) == tokIn;
    }

    function swapV2(address pool, address tokIn, address tokOut, uint256 amtIn, address to) external {
        uint256 outBalBefore = _tBal(tokOut, to);
        bool z2o = _isTok0(pool, tokIn);
        uint256 rIn;
        uint256 rOut;
        {
            (bool okR, bytes memory rR) = pool.staticcall(hex"0902f1ac");
            require(okR && rR.length >= 64, "RES");
            (uint112 rv0, uint112 rv1, ) = abi.decode(rR, (uint112, uint112, uint32));
            rIn = z2o ? uint256(rv0) : uint256(rv1);
            rOut = z2o ? uint256(rv1) : uint256(rv0);
            require(rIn > 0 && rOut > 0, "ZRES");
        }
        uint256 actualIn = _tBal(tokIn, pool);
        _tTransferFrom(tokIn, msg.sender, pool, _capV2Send(amtIn, rIn));
        {
            uint256 poolPost = _tBal(tokIn, pool);
            actualIn = poolPost > actualIn ? (poolPost - actualIn) : 0;
            require(actualIn > 0, "AIN");
        }
        uint256 outAmt = _v2Out(actualIn, rIn, rOut, 997);
        require(outAmt > 0, "OUT0");
        (bool okS, ) = pool.call(abi.encodeWithSelector(0x022c0d9f, z2o ? 0 : outAmt, z2o ? outAmt : 0, to, ""));
        if (!okS) {
            outAmt = _v2Out(actualIn, rIn, rOut, 995);
            require(outAmt > 0, "OUT995");
            (okS, ) = pool.call(abi.encodeWithSelector(0x022c0d9f, z2o ? 0 : outAmt, z2o ? outAmt : 0, to, ""));
            require(okS, "V2S");
        }
        require(_tBal(tokOut, to) > outBalBefore, "NOUT");
    }

    function _v2Out(uint256 aIn, uint256 resIn, uint256 resOut, uint256 feeNum) internal pure returns (uint256) {
        if (aIn == 0 || resIn == 0 || resOut == 0) { return 0; }
        uint256 maxR = resIn > type(uint256).max / 1000000 ? resIn : (resIn * 1000000);
        uint256 effIn = aIn > maxR ? maxR : aIn;
        while (effIn > type(uint256).max / feeNum || (effIn * feeNum) > type(uint256).max / resOut || resIn > type(uint256).max / 1000) {
            effIn >>= 1;
            resIn >>= 1;
        }
        if (effIn == 0 || resIn == 0) { return 0; }
        uint256 den = resIn * 1000 + effIn * feeNum;
        return den > 0 ? ((effIn * feeNum * resOut) / den) : 0;
    }

    function _capV2Send(uint256 amtIn, uint256 rIn) internal pure returns (uint256) {
        uint256 room = type(uint112).max > rIn ? (type(uint112).max - rIn) / 4 : 0;
        uint256 cap1 = rIn > type(uint256).max / 10000 ? rIn : (rIn * 10000);
        uint256 maxSend = (room > 0 && cap1 > room) ? room : cap1;
        return amtIn > maxSend ? maxSend : amtIn;
    }

    function swapSolidly(address pool, address tokIn, address tokOut, uint256 amtIn, address to) external {
        uint256 outBalBefore = _tBal(tokOut, to);
        bool z2o = _isTok0(pool, tokIn);
        uint256 sendIn = amtIn;
        {
            (bool okR, bytes memory rR) = pool.staticcall(hex"0902f1ac");
            if (okR && rR.length >= 64) {
                (uint256 rv0, uint256 rv1) = abi.decode(rR, (uint256, uint256));
                uint256 rIn = z2o ? rv0 : rv1;
                if (rIn > 0) { sendIn = _capV2Send(sendIn, rIn); }
            }
        }
        uint256 outAmt;
        for (uint256 step = 0; step < 24 && sendIn > 0; step++) {
            (bool okQ, bytes memory rQ) = pool.staticcall(abi.encodeWithSelector(0xf140a35a, sendIn, tokIn));
            if (okQ && rQ.length >= 32) {
                outAmt = abi.decode(rQ, (uint256));
                if (outAmt > 0) { break; }
            }
            sendIn >>= 1;
        }
        require(sendIn > 0 && outAmt > 0, "SOUT");
        uint256 poolInBefore = _tBal(tokIn, pool);
        _tTransferFrom(tokIn, msg.sender, pool, sendIn);
        uint256 poolInAfter = _tBal(tokIn, pool);
        uint256 actualIn = poolInAfter > poolInBefore ? (poolInAfter - poolInBefore) : 0;
        if (actualIn > 0 && actualIn < sendIn) {
            (bool okQ2, bytes memory rQ2) = pool.staticcall(abi.encodeWithSelector(0xf140a35a, actualIn, tokIn));
            require(okQ2 && rQ2.length >= 32, "SQ");
            outAmt = abi.decode(rQ2, (uint256));
            require(outAmt > 0, "SOUT");
        }
        (bool okS, ) = pool.call(abi.encodeWithSelector(0x022c0d9f, z2o ? 0 : outAmt, z2o ? outAmt : 0, to, ""));
        require(okS && _tBal(tokOut, to) > outBalBefore, "SOLS");
    }

    function swapV3(address pool, address tokIn, address tokOut, uint256 amtIn, address to) external {
        uint256 outBalBefore = _tBal(tokOut, to);
        bool z2o = _isTok0(pool, tokIn);
        _cbPool = pool;
        _cbPayer = msg.sender;
        _cbToken = tokIn;
        bool okS;
        {
            uint256 cappedV3In = amtIn > uint256(type(int256).max) ? uint256(type(int256).max) : amtIn;
            (okS, ) = pool.call(
                abi.encodeWithSelector(
                    0x128acb08,
                    to,
                    z2o,
                    int256(cappedV3In),
                    z2o ? uint160(4295128740) : uint160(1461446703485210103287273052203988822378723970341),
                    abi.encode(msg.sender, tokIn)
                )
            );
        }
        _cbPool = address(0);
        _cbPayer = address(0);
        _cbToken = address(0);
        require(okS && _tBal(tokOut, to) > outBalBefore, "V3S");
    }

    function _payV3(int256 a0, int256 a1) internal {
        require(msg.sender == _cbPool && _cbPool != address(0), "CBP");
        uint256 owe = a0 > 0 ? uint256(a0) : (a1 > 0 ? uint256(a1) : 0);
        if (owe > 0) { _tTransferFrom(_cbToken, _cbPayer, msg.sender, owe); }
    }

    function uniswapV3SwapCallback(int256 a0, int256 a1, bytes calldata) external { _payV3(a0, a1); }
    function pancakeV3SwapCallback(int256 a0, int256 a1, bytes calldata) external { _payV3(a0, a1); }
    function algebraSwapCallback(int256 a0, int256 a1, bytes calldata) external { _payV3(a0, a1); }

    function swapCurve(address pool, address tokIn, address tokOut, uint256 amtIn, int128 iIdx, int128 jIdx, bool isUnderlying, address to) external {
        uint256 inSelfBefore = _tBal(tokIn, address(this));
        {
            uint256 sendIn = amtIn;
            if (!isUnderlying) {
                uint256 poolInBal = _tBal(tokIn, pool);
                if (poolInBal > 0) { sendIn = _capV2Send(sendIn, poolInBal); }
            }
            _tTransferFrom(tokIn, msg.sender, address(this), sendIn);
        }
        uint256 tryIn = _tBal(tokIn, address(this));
        tryIn = tryIn > inSelfBefore ? (tryIn - inSelfBefore) : 0;
        require(tryIn > 0, "CAIN");
        uint256 selfBefore = _tBal(tokOut, address(this));
        _tResetApprove(tokIn, pool);
        _tApprove(tokIn, pool, tryIn);
        bool okC;
        for (uint256 step = 0; step < 16 && tryIn > 0; step++) {
            (okC, ) = pool.call(abi.encodeWithSelector(isUnderlying ? bytes4(0xa6417ed6) : bytes4(0x3df02124), iIdx, jIdx, tryIn, uint256(1)));
            if (!okC && iIdx >= 0 && jIdx >= 0) {
                (okC, ) = pool.call(abi.encodeWithSelector(isUnderlying ? bytes4(0x65b2489b) : bytes4(0x5b41b908), uint256(uint128(iIdx)), uint256(uint128(jIdx)), tryIn, uint256(1)));
            }
            if (okC) { break; }
            tryIn >>= 1;
        }
        _tResetApprove(tokIn, pool);
        {
            uint256 inSelfAfter = _tBal(tokIn, address(this));
            if (inSelfAfter > inSelfBefore) {
                _tTransfer(tokIn, msg.sender, inSelfAfter - inSelfBefore);
            }
        }
        require(okC, "CRVS");
        uint256 gain;
        {
            uint256 selfAfter = _tBal(tokOut, address(this));
            gain = selfAfter > selfBefore ? (selfAfter - selfBefore) : 0;
        }
        require(gain > 0, "NOUT");
        _tTransfer(tokOut, to, gain);
    }

    function unwindV2LP(address pool, uint256 amtIn, address to) external {
        _tTransferFrom(pool, msg.sender, pool, amtIn);
        (bool okB, ) = pool.call(abi.encodeWithSelector(0x89afcb44, to));
        require(okB, "LPB");
    }
}
// Audit block pin: 134747385

contract MidframeHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337 {
    address internal _cbTarget;
    bytes4 internal _cbSel;
    uint256 internal _cbDepth;
    bytes internal _cbData;

    function prepareCallback(address targetAddr, bytes4 sel) external {
        _cbTarget = targetAddr;
        _cbSel = sel;
        _cbDepth = 0;
        delete _cbData;
    }

    function prepareCallbackData(address targetAddr, bytes4 sel, bytes calldata data) external {
        _cbTarget = targetAddr;
        _cbSel = sel;
        _cbDepth = 0;
        _cbData = data;
    }

    function _invokeCallback() internal {
        if (_cbDepth < 1 && _cbTarget != address(0)) {
            _cbDepth++;
            if (_cbData.length > 0) {
                (bool okD, ) = _cbTarget.call(_cbData);
                okD;
            } else if (_cbSel != bytes4(0)) {
                (bool ok, ) = _cbTarget.call(abi.encodeWithSelector(_cbSel, uint256(1)));
                if (!ok) {
                    (ok, ) = _cbTarget.call(abi.encodeWithSelector(_cbSel));
                }
                ok;
            }
        }
    }

    function onERC721Received(address, address, uint256, bytes calldata) external returns (bytes4) {
        _invokeCallback();
        return 0x150b7a02;
    }

    function onERC1155Received(address, address, uint256, uint256, bytes calldata) external returns (bytes4) {
        _invokeCallback();
        return 0xf23a6e61;
    }

    function onERC1155BatchReceived(address, address, uint256[] calldata, uint256[] calldata, bytes calldata) external returns (bytes4) {
        _invokeCallback();
        return 0xbc197c81;
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

    function sweepTo(address callerAddr) external {
        if (address(this).balance > 0) {
            (bool ok, ) = payable(callerAddr).call{value: address(this).balance}("");
            ok;
        }
        { (bool okB, bytes memory retB) = address(0x55d398326f99059fF775485246999027B3197955).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x55d398326f99059fF775485246999027B3197955).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x1Ba42e5193dfA8B03D15dd1B86a3113bbBEF8Eeb).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x1Ba42e5193dfA8B03D15dd1B86a3113bbBEF8Eeb).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x1CE0c2827e2eF14D5C4f29a091d735A204794041).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x1CE0c2827e2eF14D5C4f29a091d735A204794041).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x235B6fe22B4642aDa16D311855c49Ce7DE260841).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x235B6fe22B4642aDa16D311855c49Ce7DE260841).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x2c3a8Ee94dDD97244a93Bc48298f97d2C412F7Db).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x2c3a8Ee94dDD97244a93Bc48298f97d2C412F7Db).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x23396cF899Ca06c4472205fC903bDB4de249D6fC).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x23396cF899Ca06c4472205fC903bDB4de249D6fC).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x21cAef8A43163Eea865baeE23b9C2E327696A3bf).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x21cAef8A43163Eea865baeE23b9C2E327696A3bf).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x1D2F0da169ceB9fC7B3144628dB156f3F6c60dBE).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x1D2F0da169ceB9fC7B3144628dB156f3F6c60dBE).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x3EE2200Efb3400fAbB9AacF31297cBdD1d435D47).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x3EE2200Efb3400fAbB9AacF31297cBdD1d435D47).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x0782b6d8c4551B9760e74c0545a9bCD90bdc41E5).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x0782b6d8c4551B9760e74c0545a9bCD90bdc41E5).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x0c6302401aa5325C2E36F170f52C00B1d220B405).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x0c6302401aa5325C2E36F170f52C00B1d220B405).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x000Ae314E2A2172a039B26378814C252734f556A).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x000Ae314E2A2172a039B26378814C252734f556A).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
    }
}

contract Verify_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337 {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);
    address constant TARGET = 0x0000000000000000000000000000000000001000;
    address constant TOK_0 = 0x55d398326f99059fF775485246999027B3197955;
    address constant TOK_1 = 0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c;
    address constant TOK_2 = 0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d;
    address constant TOK_3 = 0x1Ba42e5193dfA8B03D15dd1B86a3113bbBEF8Eeb;
    address constant TOK_4 = 0x1CE0c2827e2eF14D5C4f29a091d735A204794041;
    address constant TOK_5 = 0x235B6fe22B4642aDa16D311855c49Ce7DE260841;
    address constant TOK_6 = 0x2c3a8Ee94dDD97244a93Bc48298f97d2C412F7Db;
    address constant TOK_7 = 0x23396cF899Ca06c4472205fC903bDB4de249D6fC;
    address constant TOK_8 = 0x21cAef8A43163Eea865baeE23b9C2E327696A3bf;
    address constant TOK_9 = 0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3;
    address constant TOK_10 = 0x1D2F0da169ceB9fC7B3144628dB156f3F6c60dBE;
    address constant TOK_11 = 0x3EE2200Efb3400fAbB9AacF31297cBdD1d435D47;
    address constant TOK_12 = 0x0782b6d8c4551B9760e74c0545a9bCD90bdc41E5;
    address constant TOK_13 = 0x0c6302401aa5325C2E36F170f52C00B1d220B405;
    address constant TOK_14 = 0x000Ae314E2A2172a039B26378814C252734f556A;
    address constant TOK_15 = 0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56;
    address constant VAULT_0 = 0x0000000000000000000000000000000000001000;
    address constant VAULT_1 = 0x16b9a82891338f9bA80E2D6970FddA79D1eb0daE;
    address constant VAULT_2 = 0x2b9A66237682AcEed60edabF21395fa213ba637B;
    address constant VAULT_3 = 0x1231DEB6f5749EF6cE6943a275A1D3E7486F4EaE;
    address constant VAULT_4 = 0x0Bad40D9e9C369f2223c835E108f43a45fd223B5;
    address constant VAULT_5 = 0xBA6a49A97Cc725B3C39d6C5ea6dEfFddb64fe6b8;
    address constant VAULT_6 = 0x5340a7278848EE51D35c30693D6FBFf06d1a0d73;
    address constant VAULT_7 = 0xcD678A28609a3B141c40bd879E7624dF139ffc83;
    address constant VAULT_8 = 0x0782b6d8c4551B9760e74c0545a9bCD90bdc41E5;
    address constant VAULT_9 = 0xAE406F357541F45f01bEC21F9f28C43757f202E4;
    address constant VAULT_10 = 0xB7b7631b97D93344b2A29e926E42578006794b3B;
    address constant VAULT_11 = 0x1d6F03B0B20b2EC05b37bF60F56af442Ced66666;
    address constant VAULT_12 = 0xf04B12b838Ae18Bbc32CE950a4Ada6a558aaC2F1;
    address constant VAULT_13 = 0xFEC7D27525cC4efDe5b785EEb5E37Df90E9cd1d5;
    address constant VAULT_14 = 0x164F19FE4D5F9e98df381D72fB9738AAf2C986e5;
    address constant VAULT_15 = 0x6E30c17D2554DCA5A1Ac178939764c6Bf61AB95a;
    address constant VAULT_16 = 0xADD14A5474B63743cD9F85B6f2f7e543D23a9c1d;
    address constant VAULT_17 = 0x1A35C16cE21903Bc17Fd020c4ED73fEdC70c1b2A;
    address constant VAULT_18 = 0x1356062E82a940A98ff7172bA1093e206b775015;
    address constant VAULT_19 = 0x66426a835c71C20AC222C8441117f78Aa8f2FF12;
    address constant VAULT_20 = 0xE71547170c5ad5120992B85Cf1288FAb23d29A61;
    address constant VAULT_21 = 0x5ae569d8a0539a6A603E96A26ac8CaEA7CEba377;
    address constant VAULT_22 = 0x02Db8a1c2170AEE6cd17715EcD1c567C9B296E2c;
    address constant VAULT_23 = 0x045e2Df638eBEc29130DD3bE61161cbA5F00a9c8;
    address constant VAULT_24 = 0x04E051D8e3627A79785f8E210B1d61A877AA737A;
    address constant VAULT_25 = 0x07D274a68393E8b8a2CCf19A2ce4Ba3518735253;
    address constant VAULT_26 = 0x0FC3045A569f55233BA26d92dB2e7DbB47C30D76;
    address constant VAULT_27 = 0x1221db5424DC22DF9f426ec38A4F31950f6a5600;
    address constant VAULT_28 = 0x124Aa9169E6c0851a4A54D0ebb9158f2313d9Dd8;
    address constant VAULT_29 = 0x14f37168AB9eAFCD94d5b142a00E6e9B261Bad48;
    address constant VAULT_30 = 0x1630F08370917E79df0B7572395a5e907508bBBc;
    address constant VAULT_31 = 0x1841044080E6872c90Ac5048F0D1954CC546e9B7;

    struct ConservationSnap {
        uint256[17] callerBal;
        uint256[17] vaultBal;
        uint256[17] supply;
        uint256 wnativeBacking;
    }

    event ConservationGain(uint256 sliceIdx, address asset, uint256 callerGain, uint256 vaultLoss, uint256 gasUsed);
    event ConservationSupplyMint(uint256 sliceIdx, address asset, uint256 supplyInc);
    event ConservationBridge(address tokAsset, uint256 tokSpent, address quoteAsset, uint256 quoteBought);

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
            vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
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
        address[32] memory vaults = _vaultTable();
        for (uint256 vIdx = 0; vIdx < 32; vIdx++) {
            if (_erc20Allowance(token, who, vaults[vIdx]) > 0) {
                return 0;
            }
        }
        if (_hasResidualSpenderAllowance(token, who)) {
            return 0;
        }
        uint256 whoBefore = _erc20Bal(token, who);
        uint256 sinkBefore = _erc20Bal(token, sink);
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
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

    function _vaultTable() internal pure returns (address[32] memory v) {
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
        v[21] = VAULT_21;
        v[22] = VAULT_22;
        v[23] = VAULT_23;
        v[24] = VAULT_24;
        v[25] = VAULT_25;
        v[26] = VAULT_26;
        v[27] = VAULT_27;
        v[28] = VAULT_28;
        v[29] = VAULT_29;
        v[30] = VAULT_30;
        v[31] = VAULT_31;
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
        address[32] memory vaults = _vaultTable();
        for (uint256 vIdx = 0; vIdx < 32; vIdx++) {
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
        s.wnativeBacking = address(TOK_1).balance;
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
        uint256 settledGain = _verifyPostCommitSettlement(asset, 0x804826Cd2F031D32d9f98FCC74E41338f610630b, rawCallerGain);
        if (settledGain > 0) {
            if (sInc > 0) {
                emit ConservationSupplyMint(sliceIdx, asset, sInc);
            }
            emit ConservationGain(sliceIdx, asset, settledGain, conservedLoss, gasUsed);
        }
    }


    function _dexSwapStep(address liq, address tokIn, uint256 amtIn, bytes memory payload) internal {
        (bool okZ, ) = tokIn.call(abi.encodeWithSelector(0x095ea7b3, liq, uint256(0)));
        okZ;
        (bool okAp, bytes memory rAp) = tokIn.call(abi.encodeWithSelector(0x095ea7b3, liq, amtIn));
        if (okAp && (rAp.length == 0 || (rAp.length >= 32 && abi.decode(rAp, (bool))))) {
            (bool okSw, ) = liq.call(payload);
            okSw;
        }
        (bool okRst, ) = tokIn.call(abi.encodeWithSelector(0x095ea7b3, liq, uint256(0)));
        okRst;
    }

    function _tryLiquidateGains(ConservationSnap memory snapBefore, ConservationSnap memory snapMid)
        internal
        returns (uint256[17] memory liqCredit, uint256[17] memory liqDebited)
    {
        if (address(0x804826Cd2F031D32d9f98FCC74E41338f610630b).code.length > 0) {
            return (liqCredit, liqDebited);
        }
        LiquidatorHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337 _liq;
        {
            uint256 liqIn;
            {
                uint256 preGain = snapMid.callerBal[4] > snapBefore.callerBal[4] ? (snapMid.callerBal[4] - snapBefore.callerBal[4]) : 0;
                uint256 midLoss = _satAdd(snapBefore.vaultBal[4] > snapMid.vaultBal[4] ? (snapBefore.vaultBal[4] - snapMid.vaultBal[4]) : 0, snapMid.supply[4] > snapBefore.supply[4] ? (snapMid.supply[4] - snapBefore.supply[4]) : 0);
                uint256 availLoss = midLoss > liqDebited[4] ? (midLoss - liqDebited[4]) : 0;
                uint256 curBal = _erc20Bal(0x1Ba42e5193dfA8B03D15dd1B86a3113bbBEF8Eeb, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 curGain = curBal > snapBefore.callerBal[4] ? (curBal - snapBefore.callerBal[4]) : 0;
                liqIn = preGain < availLoss ? preGain : availLoss;
                if (curGain < liqIn) { liqIn = curGain; }
            }
            if (liqIn > 0) {
                uint256 kBefore = _erc20Bal(0x1Ba42e5193dfA8B03D15dd1B86a3113bbBEF8Eeb, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 qBefore = _erc20Bal(0x55d398326f99059fF775485246999027B3197955, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                if (address(_liq) == address(0)) { _liq = new LiquidatorHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337(); }
                _dexSwapStep(address(_liq), 0x1Ba42e5193dfA8B03D15dd1B86a3113bbBEF8Eeb, liqIn, abi.encodeWithSelector(_liq.swapV3.selector, 0x4d1b90273d5B0Ea98101154D73A6C7D7A19884DB, 0x1Ba42e5193dfA8B03D15dd1B86a3113bbBEF8Eeb, 0x55d398326f99059fF775485246999027B3197955, liqIn, 0x804826Cd2F031D32d9f98FCC74E41338f610630b));
                vm.stopPrank();
                uint256 spent;
                uint256 gained;
                {
                    uint256 kAfter = _erc20Bal(0x1Ba42e5193dfA8B03D15dd1B86a3113bbBEF8Eeb, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    uint256 qAfter = _erc20Bal(0x55d398326f99059fF775485246999027B3197955, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    spent = kBefore > kAfter ? (kBefore - kAfter) : 0;
                    gained = qAfter > qBefore ? (qAfter - qBefore) : 0;
                }
                if (spent > 0 && spent <= liqIn && gained > 0) {
                    liqDebited[4] = _satAdd(liqDebited[4], spent);
                    liqCredit[1] = _satAdd(liqCredit[1], gained);
                }
            }
        }
        {
            uint256 liqIn;
            {
                uint256 preGain = snapMid.callerBal[5] > snapBefore.callerBal[5] ? (snapMid.callerBal[5] - snapBefore.callerBal[5]) : 0;
                uint256 midLoss = _satAdd(snapBefore.vaultBal[5] > snapMid.vaultBal[5] ? (snapBefore.vaultBal[5] - snapMid.vaultBal[5]) : 0, snapMid.supply[5] > snapBefore.supply[5] ? (snapMid.supply[5] - snapBefore.supply[5]) : 0);
                uint256 availLoss = midLoss > liqDebited[5] ? (midLoss - liqDebited[5]) : 0;
                uint256 curBal = _erc20Bal(0x1CE0c2827e2eF14D5C4f29a091d735A204794041, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 curGain = curBal > snapBefore.callerBal[5] ? (curBal - snapBefore.callerBal[5]) : 0;
                liqIn = preGain < availLoss ? preGain : availLoss;
                if (curGain < liqIn) { liqIn = curGain; }
            }
            if (liqIn > 0) {
                uint256 kBefore = _erc20Bal(0x1CE0c2827e2eF14D5C4f29a091d735A204794041, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 qBefore = _erc20Bal(0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                if (address(_liq) == address(0)) { _liq = new LiquidatorHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337(); }
                _dexSwapStep(address(_liq), 0x1CE0c2827e2eF14D5C4f29a091d735A204794041, liqIn, abi.encodeWithSelector(_liq.swapV3.selector, 0x1D8B1eD9b3da5d510FD3723e0Dd02476E0d7e781, 0x1CE0c2827e2eF14D5C4f29a091d735A204794041, 0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c, liqIn, 0x804826Cd2F031D32d9f98FCC74E41338f610630b));
                vm.stopPrank();
                uint256 spent;
                uint256 gained;
                {
                    uint256 kAfter = _erc20Bal(0x1CE0c2827e2eF14D5C4f29a091d735A204794041, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    uint256 qAfter = _erc20Bal(0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    spent = kBefore > kAfter ? (kBefore - kAfter) : 0;
                    gained = qAfter > qBefore ? (qAfter - qBefore) : 0;
                }
                if (spent > 0 && spent <= liqIn && gained > 0) {
                    liqDebited[5] = _satAdd(liqDebited[5], spent);
                    liqCredit[2] = _satAdd(liqCredit[2], gained);
                }
            }
        }
        {
            uint256 liqIn;
            {
                uint256 preGain = snapMid.callerBal[6] > snapBefore.callerBal[6] ? (snapMid.callerBal[6] - snapBefore.callerBal[6]) : 0;
                uint256 midLoss = _satAdd(snapBefore.vaultBal[6] > snapMid.vaultBal[6] ? (snapBefore.vaultBal[6] - snapMid.vaultBal[6]) : 0, snapMid.supply[6] > snapBefore.supply[6] ? (snapMid.supply[6] - snapBefore.supply[6]) : 0);
                uint256 availLoss = midLoss > liqDebited[6] ? (midLoss - liqDebited[6]) : 0;
                uint256 curBal = _erc20Bal(0x235B6fe22B4642aDa16D311855c49Ce7DE260841, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 curGain = curBal > snapBefore.callerBal[6] ? (curBal - snapBefore.callerBal[6]) : 0;
                liqIn = preGain < availLoss ? preGain : availLoss;
                if (curGain < liqIn) { liqIn = curGain; }
            }
            if (liqIn > 0) {
                uint256 kBefore = _erc20Bal(0x235B6fe22B4642aDa16D311855c49Ce7DE260841, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 qBefore = _erc20Bal(0x55d398326f99059fF775485246999027B3197955, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                if (address(_liq) == address(0)) { _liq = new LiquidatorHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337(); }
                _dexSwapStep(address(_liq), 0x235B6fe22B4642aDa16D311855c49Ce7DE260841, liqIn, abi.encodeWithSelector(_liq.swapV3.selector, 0x64fa53217bde4179f54C9706dE431b73814e92db, 0x235B6fe22B4642aDa16D311855c49Ce7DE260841, 0x55d398326f99059fF775485246999027B3197955, liqIn, 0x804826Cd2F031D32d9f98FCC74E41338f610630b));
                vm.stopPrank();
                uint256 spent;
                uint256 gained;
                {
                    uint256 kAfter = _erc20Bal(0x235B6fe22B4642aDa16D311855c49Ce7DE260841, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    uint256 qAfter = _erc20Bal(0x55d398326f99059fF775485246999027B3197955, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    spent = kBefore > kAfter ? (kBefore - kAfter) : 0;
                    gained = qAfter > qBefore ? (qAfter - qBefore) : 0;
                }
                if (spent > 0 && spent <= liqIn && gained > 0) {
                    liqDebited[6] = _satAdd(liqDebited[6], spent);
                    liqCredit[1] = _satAdd(liqCredit[1], gained);
                }
            }
        }
        {
            uint256 liqIn;
            {
                uint256 preGain = snapMid.callerBal[7] > snapBefore.callerBal[7] ? (snapMid.callerBal[7] - snapBefore.callerBal[7]) : 0;
                uint256 midLoss = _satAdd(snapBefore.vaultBal[7] > snapMid.vaultBal[7] ? (snapBefore.vaultBal[7] - snapMid.vaultBal[7]) : 0, snapMid.supply[7] > snapBefore.supply[7] ? (snapMid.supply[7] - snapBefore.supply[7]) : 0);
                uint256 availLoss = midLoss > liqDebited[7] ? (midLoss - liqDebited[7]) : 0;
                uint256 curBal = _erc20Bal(0x2c3a8Ee94dDD97244a93Bc48298f97d2C412F7Db, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 curGain = curBal > snapBefore.callerBal[7] ? (curBal - snapBefore.callerBal[7]) : 0;
                liqIn = preGain < availLoss ? preGain : availLoss;
                if (curGain < liqIn) { liqIn = curGain; }
            }
            if (liqIn > 0) {
                uint256 kBefore = _erc20Bal(0x2c3a8Ee94dDD97244a93Bc48298f97d2C412F7Db, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 qBefore = _erc20Bal(0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                if (address(_liq) == address(0)) { _liq = new LiquidatorHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337(); }
                _dexSwapStep(address(_liq), 0x2c3a8Ee94dDD97244a93Bc48298f97d2C412F7Db, liqIn, abi.encodeWithSelector(_liq.swapV3.selector, 0x4D3bF29Ba30f8Bfe4624e7678709afA195689c5D, 0x2c3a8Ee94dDD97244a93Bc48298f97d2C412F7Db, 0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c, liqIn, 0x804826Cd2F031D32d9f98FCC74E41338f610630b));
                vm.stopPrank();
                uint256 spent;
                uint256 gained;
                {
                    uint256 kAfter = _erc20Bal(0x2c3a8Ee94dDD97244a93Bc48298f97d2C412F7Db, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    uint256 qAfter = _erc20Bal(0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    spent = kBefore > kAfter ? (kBefore - kAfter) : 0;
                    gained = qAfter > qBefore ? (qAfter - qBefore) : 0;
                }
                if (spent > 0 && spent <= liqIn && gained > 0) {
                    liqDebited[7] = _satAdd(liqDebited[7], spent);
                    liqCredit[2] = _satAdd(liqCredit[2], gained);
                }
            }
        }
        {
            uint256 liqIn;
            {
                uint256 preGain = snapMid.callerBal[8] > snapBefore.callerBal[8] ? (snapMid.callerBal[8] - snapBefore.callerBal[8]) : 0;
                uint256 midLoss = _satAdd(snapBefore.vaultBal[8] > snapMid.vaultBal[8] ? (snapBefore.vaultBal[8] - snapMid.vaultBal[8]) : 0, snapMid.supply[8] > snapBefore.supply[8] ? (snapMid.supply[8] - snapBefore.supply[8]) : 0);
                uint256 availLoss = midLoss > liqDebited[8] ? (midLoss - liqDebited[8]) : 0;
                uint256 curBal = _erc20Bal(0x23396cF899Ca06c4472205fC903bDB4de249D6fC, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 curGain = curBal > snapBefore.callerBal[8] ? (curBal - snapBefore.callerBal[8]) : 0;
                liqIn = preGain < availLoss ? preGain : availLoss;
                if (curGain < liqIn) { liqIn = curGain; }
            }
            if (liqIn > 0) {
                uint256 kBefore = _erc20Bal(0x23396cF899Ca06c4472205fC903bDB4de249D6fC, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 qBefore = _erc20Bal(0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                if (address(_liq) == address(0)) { _liq = new LiquidatorHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337(); }
                _dexSwapStep(address(_liq), 0x23396cF899Ca06c4472205fC903bDB4de249D6fC, liqIn, abi.encodeWithSelector(_liq.swapV2.selector, 0x05faf555522Fa3F93959F86B41A3808666093210, 0x23396cF899Ca06c4472205fC903bDB4de249D6fC, 0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56, liqIn, 0x804826Cd2F031D32d9f98FCC74E41338f610630b));
                vm.stopPrank();
                uint256 spent;
                uint256 gained;
                {
                    uint256 kAfter = _erc20Bal(0x23396cF899Ca06c4472205fC903bDB4de249D6fC, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    uint256 qAfter = _erc20Bal(0xe9e7CEA3DedcA5984780Bafc599bD69ADd087D56, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    spent = kBefore > kAfter ? (kBefore - kAfter) : 0;
                    gained = qAfter > qBefore ? (qAfter - qBefore) : 0;
                }
                if (spent > 0 && spent <= liqIn && gained > 0) {
                    liqDebited[8] = _satAdd(liqDebited[8], spent);
                    liqCredit[16] = _satAdd(liqCredit[16], gained);
                }
            }
        }
        {
            uint256 liqIn;
            {
                uint256 preGain = snapMid.callerBal[9] > snapBefore.callerBal[9] ? (snapMid.callerBal[9] - snapBefore.callerBal[9]) : 0;
                uint256 midLoss = _satAdd(snapBefore.vaultBal[9] > snapMid.vaultBal[9] ? (snapBefore.vaultBal[9] - snapMid.vaultBal[9]) : 0, snapMid.supply[9] > snapBefore.supply[9] ? (snapMid.supply[9] - snapBefore.supply[9]) : 0);
                uint256 availLoss = midLoss > liqDebited[9] ? (midLoss - liqDebited[9]) : 0;
                uint256 curBal = _erc20Bal(0x21cAef8A43163Eea865baeE23b9C2E327696A3bf, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 curGain = curBal > snapBefore.callerBal[9] ? (curBal - snapBefore.callerBal[9]) : 0;
                liqIn = preGain < availLoss ? preGain : availLoss;
                if (curGain < liqIn) { liqIn = curGain; }
            }
            if (liqIn > 0) {
                uint256 kBefore = _erc20Bal(0x21cAef8A43163Eea865baeE23b9C2E327696A3bf, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                uint256 qBefore = _erc20Bal(0x55d398326f99059fF775485246999027B3197955, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                if (address(_liq) == address(0)) { _liq = new LiquidatorHelper_modeb_clone_bsc_143_4a97e6bbf9f5_9646fd43d337(); }
                _dexSwapStep(address(_liq), 0x21cAef8A43163Eea865baeE23b9C2E327696A3bf, liqIn, abi.encodeWithSelector(_liq.swapV3.selector, 0x83A0A8A723262651Ae9C54BBbA929F167443bC59, 0x21cAef8A43163Eea865baeE23b9C2E327696A3bf, 0x55d398326f99059fF775485246999027B3197955, liqIn, 0x804826Cd2F031D32d9f98FCC74E41338f610630b));
                vm.stopPrank();
                uint256 spent;
                uint256 gained;
                {
                    uint256 kAfter = _erc20Bal(0x21cAef8A43163Eea865baeE23b9C2E327696A3bf, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    uint256 qAfter = _erc20Bal(0x55d398326f99059fF775485246999027B3197955, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
                    spent = kBefore > kAfter ? (kBefore - kAfter) : 0;
                    gained = qAfter > qBefore ? (qAfter - qBefore) : 0;
                }
                if (spent > 0 && spent <= liqIn && gained > 0) {
                    liqDebited[9] = _satAdd(liqDebited[9], spent);
                    liqCredit[1] = _satAdd(liqCredit[1], gained);
                }
            }
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
        uint256 unbackedDrainWei = backingDropWei > wBurnWei ? (backingDropWei - wBurnWei) : 0;
        uint256 bReqWei = _scaleToWei(b.supply[wIdx], wDec);
        uint256 aReqWei = _scaleToWei(a.supply[wIdx], wDec);
        uint256 defBeforeWei = bReqWei > b.wnativeBacking ? (bReqWei - b.wnativeBacking) : 0;
        uint256 defAfterWei = aReqWei > a.wnativeBacking ? (aReqWei - a.wnativeBacking) : 0;
        uint256 insolvDeltaWei = defAfterWei > defBeforeWei ? (defAfterWei - defBeforeWei) : 0;
        unbackedReserveWei = unbackedDrainWei > insolvDeltaWei ? unbackedDrainWei : insolvDeltaWei;
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
            uint256 settledW = _verifyPostCommitSettlement(assets[wIdx], 0x804826Cd2F031D32d9f98FCC74E41338f610630b, chkWAmt);
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
            uint256 settledNat = _verifyPostCommitSettlement(address(0), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, chkNatAmt);
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
        if (address(0x804826Cd2F031D32d9f98FCC74E41338f610630b).code.length > 0) {
            return;
        }
        address[17] memory assets = _assetTable();
        uint256 wIdx = 2;
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
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000001"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000001"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function _probe_slice_1(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000001"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000001"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000001"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function _probe_slice_2(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000002");
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000002"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000002"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000002"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000002");
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000002"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000002"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000002"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function _probe_slice_3(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000003");
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000003"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000003"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000003"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000003");
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000003"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000003"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000003"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function _probe_slice_4(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000004");
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000004"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000004"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000004"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000004");
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000004"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000004"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000004"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function _probe_slice_5(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"0000000a"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"0000000a"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function _probe_slice_6(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"0000000a"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"0000000a"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"0000000a"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"0000000a"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function _probe_slice_7(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000015"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000015"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function _probe_slice_8(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000015"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000015"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
            {
                (bool _ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), bytes("")));
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(hex"00000015"); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0000000000000000000000000000000000001000.call(abi.encodeWithSelector(bytes4(hex"00000015"), uint256(_w), 0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b)); }
            }
        }
    }

    function test_slice_0_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_0(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_0_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_0(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_0_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_0(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_0_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_0(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_0_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_0(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_1(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_1(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_1(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_1(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_1(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_2(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_2(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_2(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_2(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_2(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_3(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_3(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_3(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_3(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_3(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_4(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_4(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_4(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_4(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_4(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_5(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_5(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_5(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_5(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_5(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_6(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_6(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_6(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_6(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_6(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_7(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_7(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_7(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_7(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_7(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_8(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_8(uint256(4731));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_8(uint256(4732));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_8(uint256(605291165795991017837230642));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 gasStart = gasleft();
        vm.startPrank(0x804826Cd2F031D32d9f98FCC74E41338f610630b, 0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _probe_slice_8(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x804826Cd2F031D32d9f98FCC74E41338f610630b);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }
}
