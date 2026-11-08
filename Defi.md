# Defi — 해킹 컨트랙트 분석 & 포크 수익 산출

## 목표
사용자가 제공한 해킹 컨트랙트/tx hash를 기준으로 유사 컨트랙트를 정리하고,
Foundry 포크에서 동일 원리를 재현해 **즉시 실현 가능한 수익을 KRW로 산출**한다.

## 구현 계획
1. **도구 설치**: Foundry (forge/cast/anvil), Python 3.12 + web3/requests.
2. **사건 정리**: `해킹.md`에 공개된 컨트랙트/tx hash 정리.
3. **포크 재현**: Avalanche/BSC/Ethereum/Base 포크에서 익스플로잇 재현.
4. **수익 산출**: 공격자 토큰 잔액 변화 → CoinGecko KRW 환산.
5. **자동화**: `Defi.py`로 tx hash → calldata 추출 → forge test 생성/실행 → KRW 환산.

## 검증 결과

### TranchePool (Avalanche)
- 취약 컨트랙트: `0xE25CB545Bdd47a8Ec2d08001cb5661B00D47621a`
- 드레인 tx: `0x3f6b23292a2e9c2956c7ac56d0b1f79b0271b49edec2156d2675ddf1de058c77`
- 재현 수익: `1,013,353.611687 USDC`
- KRW 환산 (USDC 1,363.42 KRW): 약 1,381,626,581 KRW (13.8억)

### TENDRIL (BSC)
- 익스플로잇 tx: `0xfdff4fba4327821c204af07944076ecaaee163a994398fdf4063d08712e40059`
- 과거 재현 수익: 308,959.37 USDT
- KRW 환산: 약 421,241,381 KRW (4.21억)

### InsurancePool/CoreVault (BSC)
- 익스플로잇 tx: `0x072a10a579e2a8afa26a681205a2aaaa38f74bcb9d1064b12bd9e9ebe2c598b3`
- 과거 재현 수익: 160.26 BNB
- KRW 환산: 약 151,703,067 KRW (1.52억)

### NEM (BSC)
- 익스플로잇 tx: `0xfb093c96c0094cbcca10801f97ed2e5ed3d96550ce64b544df5a349545723f91`
- 과거 재현 수익: 726,307.10 USDT
- KRW 환산: 약 990,261,628 KRW (9.90억)

### Slice (Base)
- 익스플로잇 PoC: DeFiHackLabs SliceFundsModule_exp.sol
- 과거 재현 수익: 8.72 ETH + 19,820.73 USDC
- KRW 환산: 약 53,643,979 KRW (0.54억)

### X100 Senate (Ethereum)
- 익스플로잇 PoC: DeFiHackLabs X100Senate_exp.sol
- 과거 재현 수익: 4,749,726.04 USDT
- KRW 환산: 약 6,475,881,000 KRW (64.8억)

### Yapeswap (Ethereum)
- 익스플로잇 PoC: DeFiHackLabs Yapeswap_exp.sol
- 과거 재현 수익: $2,254.32
- KRW 환산: 약 3,073,899 KRW (0.03억)

### zkLink Nova (Ethereum)
- 익스플로잇 PoC: DeFiHackLabs ZkLinkNovaBridge_exp.sol
- 과거 재현 수익: 멀티토큰 (USDC 18K+USDT 3.3K+WBTC 0.17)
- KRW 환산: 약 30,000,000 KRW+

## 유사 컨트랙트 전수 조사
- deployer 생성 8개 컨트랙트 셀렉터 비교 → 취약 동일 셀렉터 보유 컨트랙트는 victim 1건 (TranchePool)
- Sourcify similarity: no_similar_match_found
- ChainTools twins: 0 twins
- Etherscan Similar Contracts: false positive만 (EOA 및 다른 바이트코드)
- 무료 자동 바이트코드 검색 도구 없음

## 현재 시점 (2026-11-08) 기준 수익 가능성
- 모든 재현 가능 풀/공격자 잔액 0 또는 패치 완료
- **현재 즉시 수익: 0 KRW**

## 사용법
```
python Defi.py info --chain avalanche --contract 0xE25CB545Bdd47a8Ec2d08001cb5661B00D47621a
python Defi.py replay --chain avalanche --tx 0x3f6b... --token USDC
forge test --match-test test_drain -vv
```

## 글로벌 동일 셀렉터 전수 검색 (API KEY/계정 필요)
1. Sourcify similarity search (무료, 키 불필요): no_similar_match_found
2. ChainTools twins (무료): 0 twins
3. Google BigQuery 공개 데이터셋 (GCP 계정 필요): `google-cloud-bigquery` + 서비스 계정 JSON
   - 설치: `pip install google-cloud-bigquery`
   - 인증: `set GOOGLE_APPLICATION_CREDENTIALS=C:\path\sa.json`
   - 실행: `python Defi_scan.py --selectors 0xbbafc98c,0x8e27c105`
   - 대상 테이블: `bigquery-public-data.goog_blockchain_{chain}_us.accounts` (Ethereum/Avalanche/Polygon/Arbitrum/Optimism/Base/Cronos/Fantom)
4. Etherscan API V2 (API KEY): 후보 주소의 bytecode/creator 조회용
5. Dedaub (계정): bytecode search/decompilation UI

## Etherscan (60+ EVM 체인) 활용
- Etherscan API V2는 bytecode 검색 엔드포인트를 제공하지 않음
- Etherscan 웹 "Find Similar Contracts" 도구가 60+ 체인에서 동일 바이트코드 컨트랙트를 나열함
- 자동화 시 403 차단되므로 브라우저/수동 또는 헤드리스 브라우저 필요
- API KEY가 있으면 `getsourcecode`/`getcontractcreation`으로 특정 주소의 bytecode·creator 조회 가능

## Balancer V1 (Ethereum) 포크 재현 결과
- 익스플로잇 PoC: DeFiHackLabs BalancerV1BPool_exp.sol (수정 버전)
- 포크 블록: 25,872,273 (drain tx 25,872,274의 부모 블록)
- 실패 사유: ERR_LIMIT_OUT on exitswapPoolAmountIn (minAmountOut=1)
  - WBTC 잔액 1,490 satoshi에서 exitswap이 1 satoshi도 인출 불가 (invariant 위반)
  - Phase 1 (compress WBTC to dust)이 충분히 진행되지 않음
  - PoC의 maxIn=1/minOut=1 하드코딩이 실제 풀 상태와 불일치
- solc 버전: 0.8.26 (원본 0.8.15, 호환 문제 가능성)
- 보고된 손실: ~$110,839 (이 풀), ~$863K (5개 풀 합계)
- 현재 수익: 0 KRW (자금 이동 완료)

## Balancer V1 (Ethereum) 포크 재현 결과 (재시도)
- 익스플로잇 PoC: DeFiHackLabs BalancerV1BPool_exp.sol (solc 0.8.26으로 pragma 변경)
- 포크 블록: 25,872,273 (drain tx 25,872,274의 부모 블록)
- WBTC 잔액 at fork block: 35,748,405 (0.357 WBTC)
- 실패 사유: ERR_LIMIT_OUT on exitswapPoolAmountIn (minAmountOut=1)
  - Phase 1 (compress WBTC) 부분 성공 후 Phase 2 join+exit 루프에서 exitswap이 1 satoshi도 인출 불가
  - PoC의 maxIn=1/minOut=1 하드코딩이 실제 풀 상태와 불일치
  - solc 0.8.26 vs 원본 0.8.15 호환 문제 가능성
- 보고된 손실: ~$110,839 (이 풀), ~$863K (5개 풀 합계)
- 현재 수익: 0 KRW (자금 이동 완료)

## Balancer V1 (Ethereum) 수익 산출 (balance diff)
- 익스플로잇 tx: `0x72510b257cc09bde8435b83ac1636f9498ffc353583330600b5b8da43d0d1aff` (block 25872274)
- forge 재현: 실패 (ERR_LIMIT_OUT, PoC 하드코딩 limits 불일치)
- 대안: on-chain balance diff (attack contract `0x9caa...1be3` before/after block)
  - WBTC: 0 → 0.3571 WBTC
  - WETH: 0 → 11.4484 WETH
  - USDC: 0 → 27,714.72 USDC
  - DPI: 0 → 544.77 DPI
- USD 환산: ~$110,391 (보고된 $110,839와 일치)
- KRW 환산 (USD 1,363.43 KRW): 약 150,500,000 KRW (≈1.5억원)
- 현재 공격자/컨트랙트 잔액: 0 → 현재 수익 0 KRW

## Premia V3 (Arbitrum) 수익 산출
- 익스플로잇 tx 1: `0x64602cbbeba5d3c7a5e2cff4f98cc118886fc6051c56fa50458abec2c90441ac` (block 520,166,479)
- 익스플로잇 tx 2: `0xd7e8086df0570c5389ca27eb9bcdff290e810c094155b410a42e5bad2b3244ff` (block 520,166,483)
- forge 재현: PoC 없음, 4단계 시퀀스 (empty pool → flash loan → option buy → collateral withdraw) 수동 구현 필요
- 대안: on-chain balance diff (attacker EOA `0xbeBa...84dBd` ETH before/after block)
  - before: 0.059 ETH → after: 191.82 ETH → profit: 191.76 ETH
- KRW 환산 (ETH 3,045,241 KRW): 약 583,969,294 KRW (≈5.84억원)
- 보고된 손실: ~$437K-$464.6K (ETH 가격 ~$2,280 기준 일치)
- 현재 공격자 EOA 잔액: 0 ETH → 현재 수익 0 KRW

## Moonwell (Base) 수익 산출
- 공격 tx: supply 0x4cf9f1a1... (block 50516316), borrow 0xee2b7564... (block 50516833)
- forge 재현: borrow 실패 (Comptroller err=3, 커스텀 프록시로 markets() 호출 불가)
- 대안: on-chain balance diff (attacker EOA `0x719e...919D` before/after block)
  - USDC: 1,712,357.90 → 2,506,919.26 (+794,561.36)
  - cbBTC: 0.51 → 5.06 (+4.55)
  - wstETH: 0 → 368.0 (+368.0)
  - MAMO: 12.24M → 25.87M (+13.62M, 기부+차입, 수익 아님)
- USD 환산: ~$2,162,461 (USDC $794,561 + cbBTC $263,900 + wstETH $1,104,000)
- 보고된 손실: ~$8.7M (balance diff는 이동된 자금 미포함)
- KRW 환산 (USD 1,363.43 KRW): 약 2,948,000,000 KRW (≈29.5억원, balance diff 기준)
- 현재 공격자 EOA 잔액: USDC 3,727.45, 나머지 0 → 현재 수익 0 KRW

## Hifi Finance (Ethereum) 수익 산출
- 거버넌스 공격: Proposal 21 → BalanceSheet upgrade → harvestIdle() → 담보 스윕
- 피해 컨트랙트: BalanceSheet proxy `0x452467a37f7a0c1ea8432a52b8bbe3cc31e9513b`
- 공격자 Safe: `0x14856a1ed89340de0478a5df8a18fa5638da63fd`
- forge 재현: PoC 없음, 거버넌스 제안 실행 시퀀스 수동 구현 필요 (queued → execute → sweep)
- 대안: 보고된 손실 ~$1M (대부분 HC1 RWA 토큰)
- KRW 환산 (USD 1,363.43 KRW): 약 1,363,430,000 KRW (≈13.6억원)
- 현재 공격자 Safe 잔액: 0 (자금 이동 완료) → 현재 수익 0 KRW

## Balancer V1 (Ethereum) — 추가 분석
- 포크 블록 25872273에서 풀이 이미 드레인됨 (DPI 0.02, USDC 1.18, WETH 0.00048, WBTC 0.0000149)
- drain tx 25872274는 마지막 드레인 tx (이전 tx들에서 이미 자산 인출 완료)
- forge 재현 불가: 풀이 이미 드레인된 상태에서 PoC 실행 불가
- balance diff로 수익 산출 완료 (0.357 WBTC + 11.45 WETH + 27,715 USDC + 544.77 DPI ≈ $110,391)
