# Defi 파이프라인: 멀티체인 블록 스캐너 & 수익 추정

## 목표
- `해킹.md` 공개 컨트랙트 기준 유사 컨트랙트 멀티체인 탐지
- Foundry 재현으로 즉시 실현 가능 수익 KRW 추정

## 구현
1. 환경: Foundry (forge/cast/anvil), Python 3.11+ (aiohttp)
2. 입력: `해킹.md` 컨트랙트/tx hash
3. 스캔: `Defi.py scan` — receipts contractAddress 수집 + 바이트코드 셀렉터 매칭 (10 패턴)
4. 재현: Foundry fork PoC
5. 수익: balance diff / on-chain tx 기준 KRW

## 검증 결과 (2026-11-08)
| 프로토콜 | 체인 | tx/block | 실현 수익 | KRW |
|---|---|---|---|---|
| TranchePool | Avalanche | 0x3f6b…8c77 | 1,013,353.61 USDC | 1,381,626,581 |
| TENDRIL | BSC | 0xfdff…0059 | 308,959.37 USDT | 421,241,381 |
| InsurancePool/CoreVault | BSC | 0x072a…c598b3 | 160.26 BNB | 151,703,067 |
| NEM | BSC | 0xfb09…23f91 | 726,307.10 USDT | 990,261,628 |
| Slice | Base | PoC | 8.72 ETH + 19,820.73 USDC | 53,643,979 |
| X100 Senate | Ethereum | PoC | 4,749,726.04 USDT | 6,475,881,000 |
| Yapeswap | Ethereum | PoC | $2,254.32 | 3,073,899 |
| zkLink Nova | Ethereum | PoC | USDC 18K+USDT 3.3K+WBTC 0.17 | 30,000,000+ |
| Balancer V1 | Ethereum | balance diff | $110,391 | 150,500,000 |
| Premia V3 | Arbitrum | balance diff | 191.76 ETH | 583,969,294 |
| Moonwell | Base | balance diff | ~$2,162,461 | 2,948,000,000 |
| Hifi Finance | Ethereum | governance | ~$1M | 1,363,430,000 |

## 스캔 결과 (scan_results.json)
- InsurancePool 패턴 7건: ethereum 2, gnosis 3, mantle 1, polygon 1
- 나머지 9 패턴: 미발견
- Foundry: InsurancePool 코드 존재 확인만 (`test/InsurancePool.t.sol`)

## 사용법
```bash
python Defi.py scan --chains <chain|all> --blocks <N>
```
- `--chains`: 쉼표 구분 또는 `all`
- `--blocks`: 스캔 블록 수

## 참고
- `info`/`replay` 미구현, `scan`만 지원
- KRW 고정 환율 (USDC/USDT/DAI=1363, ETH=3,053,802, BNB=946,619)
- 0 KRW 항목은 공격자 자산 이동 완료
- forge 실패 케이스는 balance diff 대체