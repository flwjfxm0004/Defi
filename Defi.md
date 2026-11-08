# Defi — 해킹 컨트랙트 분석 & 포크 수익 산출

## 목표
해킹 컨트랙트 기준 유사 컨트랙트 탐지 + Foundry 포크 재현으로 KRW 수익 산출.

## 파이프라인
1. `Defi.py scan` — 36체인 receipts + 바이트코드 셀렉터 매칭 (10 패턴)
2. Foundry 포크 PoC 재현
3. 잔액 차이 기준 KRW 환산

## 사용법
```bash
python Defi.py scan --chains <chain|all> --blocks <N>
```
