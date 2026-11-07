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
