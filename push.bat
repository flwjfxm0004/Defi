@echo off
REM Push Defi ledger: add named files only, then commit and push master.
REM New PC: git clone, then run engine; _relay_state.json auto-restores queue/cursor/sessions.
REM Parquet restore is optional (clone discovery only); primary audit continues without it.
chcp 65001 >nul
cd /d C:\Defi
where git >nul 2>nul || set PATH=C:\Defi\tools\git\cmd;%PATH%

echo [1/6] Before status
git status --short
pause

echo [2/6] Optimize object store
git gc --auto

echo [3/6] Stage named files
git add -- .gitignore auto_loop.py push.bat
git add -- "Defi_구현.py" "Defi_구현_test.py" "원하는 것.md" "지시서.md" "audit_artifacts/_relay_state.json"
git add -- "audit_artifacts/*/result.json" "audit_artifacts/*/pins.json" "audit_artifacts/*/verdicts/verdict_*.json" "audit_artifacts/*/forge/test/*.t.sol"
if exist "audit_artifacts\_relay_seed_index.json" git add -- "audit_artifacts/_relay_seed_index.json"

echo [4/6] Staged review
git status --short
git diff --cached --stat
pause

echo [5/6] Commit
git commit -m "audit: ledger receipts + engine/docs + push automation"

echo [6/6] Push master
git push origin master

echo [Done] After status
git status --short
pause
