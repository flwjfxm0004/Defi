# DeFi Pipeline — 에이전트 워크플로우 매뉴얼

## 1. 목표

2026년 1월 1일부터 11월 10일까지 실제로 해킹당한 EVM 기반 취약 컨트랙트 200개 이상의 코드와 유사한 컨트랙트 중, 씨드의 해킹 원리에서 설명한 그 코드 결함이 실제로 존재하는지를 Foundry 기반 익스플로잇 테스트로 확정한다.

최종 산출물: `results/final_verdicts.parquet`

각 후보 주소에 대한 4종 판정:

| 판정 | 조건 | 의미 |
|---|---|---|
| **한화 즉시 OO 이익 가능** | forge PASS (플래시론 상환·가스·수수료·슬리피지 차감 후 순이익 > 0) | 실제 취약 확정. OO = 실현 가능한 원화 순이익 |
| **취약 하지 않음** | forge FAIL (순이익 ≤ 0) | 공격 불가 |
| **미실행** | forge 미실행 | PoC 미작성, RPC 실패, 또는 판정 미기재 |
| **잔액 없음** | 도달 가능 잔액 < $100 | 검증 대상 아님 |

## 2. 사전 준비 (최초 1회)

```bash
pip install duckdb pandas requests pyarrow
curl -L https://foundry.paradigm.xyz | bash && foundryup
```

`해킹.md`가 `DeFi.py`와 같은 디렉토리에 있어야 한다.

## 3. MEV 보호 (기본값)

PoC의 공격 트랜잭션은 대량 스왑과 플래시론을 포함하므로 **MEV 보호가 필수**다. 샌드위치·프런트런 공격을 당하면 수익이 사라진다.

### 3-1. 기본값: MEV Blocker

| 항목 | MEV Blocker | Flashbots Protect |
|---|---|---|
| **샌드위치/프런트런 방지** | ✅ | ✅ |
| **백런 MEV 환급** | **90%** | 0% |
| **포함 속도** | **가장 빠름** | 2위 (근소 차) |
| **실행 가격(수수료)** | 보통 | **가장 낮음** |
| **Revert Protection** | 제한적 | **강력** |
| **성숙도** | 양호 | **2021년부터 운영** |

**기본 RPC**: `https://rpc.mevblocker.io` (Ethereum)

### 3-2. 예외: Flashbots Protect

**Revert Protection이 결정적인 PoC**(실패 시 가스 손실이 큰 케이스)는 Flashbots Protect를 사용한다: `https://rpc.flashbots.net`

### 3-3. 체인별 RPC

| 체인 | 기본 RPC | 비고 |
|---|---|---|
| Ethereum | `https://rpc.mevblocker.io` | MEV Blocker |
| Arbitrum | `https://arb1.arbitrum.io/rpc` | MEV Blocker 지원 여부 확인 필요 |
| Polygon | `https://polygon-rpc.com` | Polygon Private Mempool 병행 검토 |
| Base | `https://mainnet.base.org` | |
| BSC | `https://bsc-dataseed.binance.org` | |

환경변수로 override 가능:

```bash
export RPC_1="https://rpc.mevblocker.io"
export RPC_42161="https://arb1.arbitrum.io/rpc"
```

## 4. 순이익 계산 — 반드시 고려할 비용

PoC의 **gross profit**이 아니라 **net profit**으로 판정한다. 아래 비용을 모두 차감한다.

| 비용 항목 | 설명 | 기본 처리 |
|---|---|---|
| **가스비** | 공격 트랜잭션 실행 비용. **빌더 팁 포함** | 🔴 필수 |
| **플래시론 수수료** | Balancer V2 0%, Aave V3 0.05%, Morpho 0% | 🔴 필수 |
| **스왑 수수료** | DEX 거래 수수료 (0.3%, 0.05% 등). 홉마다 누적 | 🔴 필수 |
| **슬리피지** | 대량 스왑 시 가격 영향 | 🔴 필수 |
| **브릿지 수수료** | 크로스체인 공격 시 | 🟡 해당 시 |
| **프로토콜 출금 수수료** | 일부 프로토콜의 인출/상환 수수료 | 🟢 해당 시 |
| **청산 페널티** | 청산 기반 공격 시 | 🟢 해당 시 |
| **MEV bribe** | Flashbots/MEV Blocker 사용 시 **0** | ✅ 0 고정 |

**제외 항목**:
- 프로토콜 일시정지/블랙리스트 리스크 (사후 운영 조치, PoC 범위 밖)
- 브릿지 지연/실패 리스크 (단일 트랜잭션 PoC 전제와 무관. 브릿지 수수료는 포함)

### 4-1. 순이익 공식

```
Net Profit = Gross Profit
           - Gas (빌더 팁 포함)
           - Flash Loan Fee
           - Swap Fees (모든 DEX 홉)
           - Slippage
           - Bridge Fee (해당 시)
           - Protocol Withdrawal Fee (해당 시)
           - Liquidation Penalty (해당 시)
```

**PASS 조건**: `Net Profit > 0` **그리고** 최소 임계값 이상 (가스비만 겨우 넘기는 수준은 실질적으로 무의미)

## 5. 디렉토리 구조

| 경로 | 역할 | 생성 주체 |
|---|---|---|
| `DeFi.md` | 이 매뉴얼 | 사전 제공 |
| `DeFi.py` | 파이프라인 실행 도구 | 사전 제공 |
| `해킹.md` | 입력 데이터 | 사전 제공 |
| `data/` | 중간 산출물 | DeFi.py |
| `results/` | 판정 산출물 | DeFi.py |
| `data/foundry/test/` | **에이전트가 작성한 PoC** | **에이전트** |

## 6. 에이전트의 역할

1. 이 파일을 처음부터 끝까지 읽는다
2. `python DeFi.py`를 실행한다
3. 출력이 `PENDING_POCS: N`이면 → N개의 PoC를 작성한다
4. `python DeFi.py`를 다시 실행한다
5. 출력이 `COMPLETE`이면 종료. `results/final_verdicts.parquet` 확인

**에이전트가 직접 하는 것**: PoC 작성, forge test 실행, 오류 수정, 판정 작성(비용 포함). 나머지는 `DeFi.py`가 처리.

## 7. DeFi.py가 하는 일 (Phase 1~6 + 집계)

| Phase | 동작 | 산출물 |
|---|---|---|
| 1 | `해킹.md` 파싱 | `data/seeds.json` (`date`, `project`, `chain_label`, `label`, `display` 포함) |
| 2 | Sourcify Parquet 전체 다운로드 (~136GB) | `data/sourcify/` |
| 3 | DuckDB 중복 제거 | `data/sourcify.duckdb::unique_bytecodes` |
| 4 | 유사도 검색 (exact hash + containment) | `data/candidates.parquet` |
| 5 | 후보별 온체인 잔액 조회 | `results/funds_{cid}.json` |
| 6 | PoC 요청 파일 생성 | `results/poc_request_{cid}.json` |
| 7 | Foundry 초기화 + 플래시론 주소 온체인 검증 | `data/foundry/` |
| 8 | `verdict_{cid}.json` 집계 | `results/final_verdicts.parquet` |

모든 Phase는 idempotent. 이미 완료된 단계는 자동 SKIP.

## 8. 워크플로우

### 8-1. 최초 실행

```bash
python DeFi.py
```

Phase 1~6이 자동 진행. Phase 2(다운로드)는 네트워크 속도에 따라 수 시간 소요.

출력 예시:

```
[Phase 6/8] PoC 요청 생성
  IN_SCOPE: 1247 candidates
  [OK] 1247 poc requests generated

PENDING_POCS: 1247 candidates need PoC
PENDING_VERDICTS: 0 candidates need verdict
```

### 8-2. PoC 작성 (에이전트)

각 `results/poc_request_{cid}.json` 예시:

```json
{
  "candidate_id": 3,
  "candidate_chain": 1,
  "candidate_address": "0x...",
  "seed_incident_no": 4,
  "seed_date": "01/10",
  "seed_project": "FutureSwap",
  "seed_chain_label": "Arbitrum",
  "seed_label": "Proxy",
  "seed_display": "01/10 — FutureSwap (Arbitrum) [Proxy]",
  "seed_address": "0xf7ca7384cc6619866749955065f17bedd3ed80bc",
  "seed_principle": "changePosition()이 수수료를 토큰 단위로 계산했으나 FeeManager가 bps로 해석...",
  "similarity": 0.87,
  "match_type": "containment",
  "source": "// FutureSwap Proxy...",
  "funds": {
    "decision": "IN_SCOPE",
    "reachable_usd_approx": 4521.3,
    "erc20_holdings": [],
    "native_balance_wei": "1200000000000000000"
  },
  "rpc_url": "https://rpc.mevblocker.io",
  "flash_loan_info": {
    "provider": "Balancer V2",
    "fee": "0%",
    "address": "0xBA12222222228d8Ba445958a75a0704d566BF2C8"
  },
  "test_file_target": "data/foundry/test/test_3.sol",
  "verdict_file_target": "results/verdict_3.json"
}
```

**에이전트가 봐야 할 필드**:
- `seed_display` → 해킹.md의 정확한 블록
- `seed_principle` → 재현해야 할 취약점 (자연어 원문)
- `source` → 타깃 컨트랙트 소스
- `funds` → 자금 위치
- `rpc_url` → MEV Blocker 기본. Revert Protection이 필요하면 Flashbots로 교체
- `flash_loan_info` → 프로바이더·수수료·주소 (온체인 검증 필수)

**에이전트는 Foundry Solidity 테스트를 작성해 `data/foundry/test/test_{cid}.sol`에 저장한다.**

**PoC 기본 원칙**:
1. **공격자 초기 자금은 플래시론** (`flash_loan_info` 참조)
2. **가스 최적화 + 순이익 극대화** 기본값 (`foundry.toml`에 `optimizer = true`)
3. **`vm`은 필수만**: `vm.createSelectFork`, `vm.startPrank`, `vm.expectRevert`, `vm.snapshot`/`vm.revertTo`
4. **순이익**: 플래시론 상환 후, **가스·스왑 수수료·슬리피지·브릿지 수수료까지 차감**한 값
5. **PASS 조건**: `assertGt(netProfit, 0, "No profit")`

템플릿은 에이전트가 자율 작성.

### 8-3. 병렬 forge test (에이전트)

```bash
cd data/foundry

ls ../../results/poc_request_*.json | \
  xargs -P 8 -I {} bash -c '
    cid=$(basename {} | grep -oP "[0-9]+")
    testfile="test/test_${cid}.sol"
    [ -f "$testfile" ] || exit 0

    rpc=$(python3 -c "import json; print(json.load(open(\"../../results/poc_request_${cid}.json\"))[\"rpc_url\"])")

    forge test --match-path "$testfile" --fork-url "$rpc" -vvv \
      > "../../results/forge_log_${cid}.txt" 2>&1
    echo "$cid:$?" >> "../../results/forge_status.txt"
  '

cd ../..
```

### 8-4. 오류 확인 및 PoC 수정 반복

```bash
cat results/forge_status.txt | grep -v ":0$"
```

실패한 `cid`의 `results/forge_log_{cid}.txt`를 읽고, 원인 파악 → PoC 수정 → 단일 재실행 → PASS까지 반복.

### 8-5. 판정 작성 (에이전트)

**PASS인 경우 — 순이익과 비용을 구조화**:

```json
{
  "candidate_id": 3,
  "verdict": "한화 즉시 OO 이익 가능",
  "gross_profit_native": "1.20",
  "costs": {
    "gas_native": "0.05",
    "flash_loan_fee_native": "0.0006",
    "swap_fees_native": "0.02",
    "slippage_native": "0.08",
    "bridge_fee_native": "0.00",
    "protocol_withdrawal_fee_native": "0.00",
    "liquidation_penalty_native": "0.00",
    "mev_bribe_native": "0.00"
  },
  "net_profit_native": "1.0494",
  "profit_native_unit": "ETH",
  "net_profit_krw": 4197600,
  "notes": "MEV Blocker 사용. Balancer V2 플래시론. 슬리피지 0.08 ETH 반영."
}
```

**FAIL인 경우**:

```json
{
  "candidate_id": 5,
  "verdict": "취약 하지 않음",
  "notes": "3가지 공격 벡터 시도, 모든 순이익 ≤ 0. allowance 검사 존재."
}
```

**판정 규칙**:
- forge PASS (순이익 > 0) → `한화 즉시 OO 이익 가능`
- forge FAIL → `취약 하지 않음`
- 판정 불가 (RPC 오류 등) → `미실행`
- 자금 < $100 → `잔액 없음` (DeFi.py가 자동 작성)

**`net_profit_krw`가 없으면 `profit_krw`를 사용**한다. 둘 다 없으면 순이익 미계산으로 간주.

### 8-6. 재실행 및 집계

```bash
python DeFi.py
```

`phase_report`가 `verdict_{cid}.json`을 모아 `results/final_verdicts.parquet` 생성.

## 9. 판정 규칙

| 판정 | 조건 | 결정 주체 |
|---|---|---|
| **한화 즉시 OO 이익 가능** | forge PASS (순이익 > 0) | 에이전트 (`verdict_{cid}.json`) |
| **취약 하지 않음** | forge FAIL | 에이전트 (`verdict_{cid}.json`) |
| **미실행** | forge 미실행 또는 판정 미기재 | DeFi.py (자동) |
| **잔액 없음** | 잔액 < $100 | DeFi.py (자동) |

판정은 오직 forge 결과로만 결정. 코드 유사 매칭만으로 취약 판정하지 않는다.

## 10. 실패 대응

| 상황 | 조치 |
|---|---|
| Phase 2 다운로드 실패 | `python DeFi.py` 재실행 (자동 재개) |
| Phase 3 dedup 실패 | 로그 확인, `data/sourcify_schema.json`의 실제 스키마와 대조 |
| forge 컴파일 실패 | Solidity 문법, import 경로, forge-std 버전 확인 |
| forge 런타임 revert | `forge_log_{cid}.txt` trace 확인 → PoC 수정 |
| forge 순이익 ≤ 0 | 다른 공격 벡터, 다른 플래시론 프로바이더, 다른 MEV RPC 시도 |
| RPC 실패 | `export RPC_{chain_id}="..."` |

## 11. 절대 규칙

1. **`DeFi.py`를 수정하지 않는다.** 도구는 블랙박스다.
2. **Phase를 건너뛰지 않는다.**
3. **forge PASS 없이 `한화 즉시 OO 이익 가능`을 만들지 않는다.**
4. **잔액 없는 후보를 건너뛰지 않는다.** `잔액 없음`도 유효한 판정이다.
5. **PoC 저장 경로를 지킨다.** `data/foundry/test/test_{cid}.sol`
6. **판정 저장 경로를 지킨다.** `results/verdict_{cid}.json`
7. **forge test는 병렬로.** 순차 실행은 시간 낭비.
8. **플래시론 주소는 온체인 확인 후 사용.** 하드코딩 금지.
9. **순이익은 가스·플래시론 수수료·스왑 수수료·슬리피지까지 차감한 값이다.** Gross profit으로 판정하지 않는다.
10. **MEV 보호는 기본값 MEV Blocker.** Revert Protection이 결정적인 PoC만 Flashbots Protect.

## 12. 추적성

모든 후보는 원본 사건으로 역추적 가능:

```
final_verdicts.parquet
  ├── seed_display    → "01/10 — FutureSwap (Arbitrum) [Proxy]"
  ├── seed_principle  → 해킹.md의 해킹 원리 원문
  ├── candidate_address → 검증 대상
  ├── verdict          → 판정 결과
  ├── net_profit_krw   → 실현 순이익 (있는 경우)
  └── notes            → 비용 내역 요약
```

해킹.md에서 `seed_display`(예: `01/10 — FutureSwap (Arbitrum)`)를 검색하면 원본 사건 블록을 찾을 수 있다.