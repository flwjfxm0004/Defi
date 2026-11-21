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

contract LiquidatorHelper_modeb_clone_optimism_143_4a97e6bbf9f5_9646fd43d337 {
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
// Audit block pin: 159801219

contract MidframeHelper_modeb_clone_optimism_143_4a97e6bbf9f5_9646fd43d337 {
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
        { (bool okB, bytes memory retB) = address(0x7F5c764cBc14f9669B88837ca1490cCa17c31607).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x7F5c764cBc14f9669B88837ca1490cCa17c31607).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x4200000000000000000000000000000000000006).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x4200000000000000000000000000000000000006).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x94b008aA00579c1307B0EF2c499aD98a8ce58e58).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x94b008aA00579c1307B0EF2c499aD98a8ce58e58).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x68f180fcCe6836688e9084f035309E29Bf0A2095).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x68f180fcCe6836688e9084f035309E29Bf0A2095).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x2E3D870790dC77A83DD1d18184Acc7439A53f475).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x2E3D870790dC77A83DD1d18184Acc7439A53f475).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x5d3a1Ff2b6BAb83b63cd9AD0787074081a52ef34).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x5d3a1Ff2b6BAb83b63cd9AD0787074081a52ef34).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x5e09cD6203575339E5b264dFf47E57aAD3891197).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x5e09cD6203575339E5b264dFf47E57aAD3891197).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0x61A34758576D651643A90838f3eCF86e8680874D).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0x61A34758576D651643A90838f3eCF86e8680874D).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xaCFd0281a29F8A478655c09fddE38B3f3b562dF3).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xaCFd0281a29F8A478655c09fddE38B3f3b562dF3).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xD1981327ecaf14afD20c6EA46BdA99cE1a3505B2).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xD1981327ecaf14afD20c6EA46BdA99cE1a3505B2).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xFe57042De76c8D6B1DF0E9E2047329fd3e2B7334).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xFe57042De76c8D6B1DF0E9E2047329fd3e2B7334).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xc40F949F8a4e094D1b49a23ea9241D289B7b2819).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xc40F949F8a4e094D1b49a23ea9241D289B7b2819).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
        { (bool okB, bytes memory retB) = address(0xe59EBa0D492cA53C6f46015EEa00517F2707dc77).staticcall(abi.encodeWithSelector(0x70a08231, address(this))); if (okB && retB.length >= 32) { uint256 tb = abi.decode(retB, (uint256)); if (tb > 0) { (bool okT, ) = address(0xe59EBa0D492cA53C6f46015EEa00517F2707dc77).call(abi.encodeWithSelector(0xa9059cbb, callerAddr, tb)); okT; } } }
    }
}

contract Verify_modeb_clone_optimism_143_4a97e6bbf9f5_9646fd43d337 {
    Vm constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);
    address constant TARGET = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae;
    address constant TOK_0 = 0x7F5c764cBc14f9669B88837ca1490cCa17c31607;
    address constant TOK_1 = 0x4200000000000000000000000000000000000006;
    address constant TOK_2 = 0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb;
    address constant TOK_3 = 0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85;
    address constant TOK_4 = 0x94b008aA00579c1307B0EF2c499aD98a8ce58e58;
    address constant TOK_5 = 0x68f180fcCe6836688e9084f035309E29Bf0A2095;
    address constant TOK_6 = 0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1;
    address constant TOK_7 = 0x2E3D870790dC77A83DD1d18184Acc7439A53f475;
    address constant TOK_8 = 0x5d3a1Ff2b6BAb83b63cd9AD0787074081a52ef34;
    address constant TOK_9 = 0x5e09cD6203575339E5b264dFf47E57aAD3891197;
    address constant TOK_10 = 0x61A34758576D651643A90838f3eCF86e8680874D;
    address constant TOK_11 = 0xaCFd0281a29F8A478655c09fddE38B3f3b562dF3;
    address constant TOK_12 = 0xD1981327ecaf14afD20c6EA46BdA99cE1a3505B2;
    address constant TOK_13 = 0xFe57042De76c8D6B1DF0E9E2047329fd3e2B7334;
    address constant TOK_14 = 0xc40F949F8a4e094D1b49a23ea9241D289B7b2819;
    address constant TOK_15 = 0xe59EBa0D492cA53C6f46015EEa00517F2707dc77;
    address constant VAULT_0 = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae;
    address constant VAULT_1 = 0x255483434aba5a75dc60c1391bB162BCd9DE2882;
    address constant VAULT_2 = 0x5e09cD6203575339E5b264dFf47E57aAD3891197;
    address constant VAULT_3 = 0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb;
    address constant VAULT_4 = 0x6131B5fae19EA4f9D964eAc0408E4408b66337b5;
    address constant VAULT_5 = 0x61A34758576D651643A90838f3eCF86e8680874D;
    address constant VAULT_6 = 0x3Ef3D8bA38EBe18DB133cEc108f4D14CE00Dd9Ae;
    address constant VAULT_7 = 0x09b355767a3Cc98AE9d0be7870678265f1F489e7;
    address constant VAULT_8 = 0x10fb9a037a68D60730FB7FC2FE77BF8495223094;
    address constant VAULT_9 = 0x18e6A996279e239EAf744DDe7bf25E9550720C41;
    address constant VAULT_10 = 0x239240E8cB50114B097c71819C56a3a4eA91d516;
    address constant VAULT_11 = 0x397925C8f511A4a1F8877e198003B0B90174796b;
    address constant VAULT_12 = 0x4423055126bC979Cf20b6CF63CCE083BCf1A446E;
    address constant VAULT_13 = 0x483DDBB9113aEE97d9dAe19B048a3edC9FC8ac72;
    address constant VAULT_14 = 0x6859b2aE7CE9fb0c4FAA71798aC5498B41B42D7A;
    address constant VAULT_15 = 0x9646fDAD06d3e24444381f44362a3B0eB343D337;
    address constant VAULT_16 = 0x9d0a410D18705ee58C30211EFc6025627beBe824;
    address constant VAULT_17 = 0xA1Fc967fDa140dA8eE241cCa5191c746d818f75c;
    address constant VAULT_18 = 0xaCD105FEEa362D5c27CAAbA0B45F53D91B92dE27;
    address constant VAULT_19 = 0xaCFd0281a29F8A478655c09fddE38B3f3b562dF3;
    address constant VAULT_20 = 0xb48091BfCA7f55F35d42b56804E2659BdcEB9337;
    address constant VAULT_21 = 0xbAe4fc1956B635A739aC3e11F9e25f0D0C1321CA;
    address constant VAULT_22 = 0xc24205B12fB056822b2b2c42B5ee18272A0A8E74;
    address constant VAULT_23 = 0xc3393567DC2F1d071254E880c9f501641491471f;
    address constant VAULT_24 = 0xD1981327ecaf14afD20c6EA46BdA99cE1a3505B2;
    address constant VAULT_25 = 0xeB6613FAC35fED17c276e3FE45D67Da67685f1eF;
    address constant VAULT_26 = 0xFe57042De76c8D6B1DF0E9E2047329fd3e2B7334;

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
            vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
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
        address[27] memory vaults = _vaultTable();
        for (uint256 vIdx = 0; vIdx < 27; vIdx++) {
            if (_erc20Allowance(token, who, vaults[vIdx]) > 0) {
                return 0;
            }
        }
        if (_hasResidualSpenderAllowance(token, who)) {
            return 0;
        }
        uint256 whoBefore = _erc20Bal(token, who);
        uint256 sinkBefore = _erc20Bal(token, sink);
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
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

    function _vaultTable() internal pure returns (address[27] memory v) {
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
        address[27] memory vaults = _vaultTable();
        for (uint256 vIdx = 0; vIdx < 27; vIdx++) {
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
        uint256 settledGain = _verifyPostCommitSettlement(asset, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, rawCallerGain);
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
        if (address(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f).code.length > 0) {
            return (liqCredit, liqDebited);
        }
        LiquidatorHelper_modeb_clone_optimism_143_4a97e6bbf9f5_9646fd43d337 _liq;
        {
            uint256 liqIn;
            {
                uint256 preGain = snapMid.callerBal[3] > snapBefore.callerBal[3] ? (snapMid.callerBal[3] - snapBefore.callerBal[3]) : 0;
                uint256 midLoss = _satAdd(snapBefore.vaultBal[3] > snapMid.vaultBal[3] ? (snapBefore.vaultBal[3] - snapMid.vaultBal[3]) : 0, snapMid.supply[3] > snapBefore.supply[3] ? (snapMid.supply[3] - snapBefore.supply[3]) : 0);
                uint256 availLoss = midLoss > liqDebited[3] ? (midLoss - liqDebited[3]) : 0;
                uint256 curBal = _erc20Bal(0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                uint256 curGain = curBal > snapBefore.callerBal[3] ? (curBal - snapBefore.callerBal[3]) : 0;
                liqIn = preGain < availLoss ? preGain : availLoss;
                if (curGain < liqIn) { liqIn = curGain; }
            }
            if (liqIn > 0) {
                uint256 kBefore = _erc20Bal(0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                uint256 qBefore = _erc20Bal(0x4200000000000000000000000000000000000006, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                if (address(_liq) == address(0)) { _liq = new LiquidatorHelper_modeb_clone_optimism_143_4a97e6bbf9f5_9646fd43d337(); }
                _dexSwapStep(address(_liq), 0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb, liqIn, abi.encodeWithSelector(_liq.swapV3.selector, 0x04F6C85A1B00F6D9B75f91FD23835974Cc07E65c, 0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb, 0x4200000000000000000000000000000000000006, liqIn, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f));
                vm.stopPrank();
                uint256 spent;
                uint256 gained;
                {
                    uint256 kAfter = _erc20Bal(0x1F32b1c2345538c0c6f582fCB022739c4A194Ebb, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                    uint256 qAfter = _erc20Bal(0x4200000000000000000000000000000000000006, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                    spent = kBefore > kAfter ? (kBefore - kAfter) : 0;
                    gained = qAfter > qBefore ? (qAfter - qBefore) : 0;
                }
                if (spent > 0 && spent <= liqIn && gained > 0) {
                    liqDebited[3] = _satAdd(liqDebited[3], spent);
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
                uint256 curBal = _erc20Bal(0x68f180fcCe6836688e9084f035309E29Bf0A2095, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                uint256 curGain = curBal > snapBefore.callerBal[6] ? (curBal - snapBefore.callerBal[6]) : 0;
                liqIn = preGain < availLoss ? preGain : availLoss;
                if (curGain < liqIn) { liqIn = curGain; }
            }
            if (liqIn > 0) {
                uint256 kBefore = _erc20Bal(0x68f180fcCe6836688e9084f035309E29Bf0A2095, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                uint256 qBefore = _erc20Bal(0x4200000000000000000000000000000000000006, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                if (address(_liq) == address(0)) { _liq = new LiquidatorHelper_modeb_clone_optimism_143_4a97e6bbf9f5_9646fd43d337(); }
                _dexSwapStep(address(_liq), 0x68f180fcCe6836688e9084f035309E29Bf0A2095, liqIn, abi.encodeWithSelector(_liq.swapV3.selector, 0x85C31FFA3706d1cce9d525a00f1C7D4A2911754c, 0x68f180fcCe6836688e9084f035309E29Bf0A2095, 0x4200000000000000000000000000000000000006, liqIn, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f));
                vm.stopPrank();
                uint256 spent;
                uint256 gained;
                {
                    uint256 kAfter = _erc20Bal(0x68f180fcCe6836688e9084f035309E29Bf0A2095, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                    uint256 qAfter = _erc20Bal(0x4200000000000000000000000000000000000006, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
                    spent = kBefore > kAfter ? (kBefore - kAfter) : 0;
                    gained = qAfter > qBefore ? (qAfter - qBefore) : 0;
                }
                if (spent > 0 && spent <= liqIn && gained > 0) {
                    liqDebited[6] = _satAdd(liqDebited[6], spent);
                    liqCredit[2] = _satAdd(liqCredit[2], gained);
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
            uint256 settledW = _verifyPostCommitSettlement(assets[wIdx], 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, chkWAmt);
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
            uint256 settledNat = _verifyPostCommitSettlement(address(0), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, chkNatAmt);
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
        if (address(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f).code.length > 0) {
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
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"0fddfc44"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"0fddfc44"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function _probe_slice_1(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"00000001");
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000001"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"00000001");
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000001"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000001"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function _probe_slice_2(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"00000002");
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000002"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000002"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000002"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"00000002");
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000002"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000002"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000002"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function _probe_slice_3(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"00000003");
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000003"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000003"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000003"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"00000003");
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000003"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000003"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"00000003"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function _probe_slice_4(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"0fddfc44"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"0fddfc44"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"0fddfc44"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"0fddfc44"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function _probe_slice_5(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"1431b23a"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"1431b23a"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"1431b23a"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"1431b23a"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"1431b23a"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"1431b23a"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"1431b23a"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"1431b23a"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"1431b23a"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"1431b23a"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function _probe_slice_6(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"16dc5270"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"16dc5270"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"16dc5270"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"16dc5270"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"16dc5270"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"16dc5270"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"16dc5270"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"16dc5270"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"16dc5270"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"16dc5270"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function _probe_slice_7(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"3659cfe6"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"3659cfe6"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function _probe_slice_8(uint256 _w) internal {
        _w;
        {
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"3659cfe6"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"3659cfe6"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
            {
                (bool _ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w)));
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(hex"3659cfe6"); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, uint256(_w))); }
                if (!_ok) { (_ok, ) = 0x0001805c0B57DBd48B5c5c26E237a135dDC678ae.call(abi.encodeWithSelector(bytes4(hex"3659cfe6"), uint256(_w), 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f)); }
            }
        }
    }

    function test_slice_0_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_0(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_0_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_0(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_0_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_0(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_0_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_0(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_0_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_0(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(0, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_1(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_1(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_1(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_1(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_1_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_1(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(1, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_2(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_2(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_2(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_2(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_2_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_2(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(2, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_3(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_3(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_3(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_3(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_3_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_3(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(3, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_4(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_4(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_4(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_4(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_4_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_4(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(4, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_5(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_5(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_5(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_5(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_5_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_5(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(5, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_6(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_6(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_6(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_6(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_6_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_6(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(6, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_7(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_7(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_7(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_7(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_7_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_7(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(7, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w0() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_8(uint256(1000000000000000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w1() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_8(uint256(65536));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w2() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_8(uint256(65537));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w3() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_8(uint256(645513696463189844));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }

    function test_slice_8_w4() public {
        ConservationSnap memory snapBefore = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 gasStart = gasleft();
        vm.startPrank(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f, 0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _probe_slice_8(uint256(1000000));
        vm.stopPrank();
        uint256 gasUsed = gasStart > gasleft() ? (gasStart - gasleft()) : 21000;
        ConservationSnap memory snapMid = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        uint256 liqGasStart = gasleft();
        (uint256[17] memory liqCredit, uint256[17] memory liqDebited) = _tryLiquidateGains(snapBefore, snapMid);
        {
            uint256 liqGasUsed = liqGasStart > gasleft() ? (liqGasStart - gasleft()) : 0;
            for (uint256 li = 0; li < 17; li++) { if (liqDebited[li] > 0) { gasUsed += liqGasUsed; break; } }
        }
        ConservationSnap memory snapAfter = _takeSnap(0x9B017DC2A3f749d5a0A1225ddd08a43100dC2b3f);
        _assertConservationLiq(8, snapBefore, snapAfter, liqCredit, liqDebited, gasUsed);
    }
}
