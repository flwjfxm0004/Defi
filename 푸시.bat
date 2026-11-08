@echo off
setlocal enableextensions enabledelayedexpansion
cd /d "%~dp0"

rem --- locate git -----------------------------------------------------------
set "GITEXE="
for /f "delims=" %%G in ('where git 2^>nul') do if not defined GITEXE set "GITEXE=%%G"
if not defined GITEXE if exist "C:\Program Files\Git\cmd\git.exe" set "GITEXE=C:\Program Files\Git\cmd\git.exe"
if not defined GITEXE if exist "C:\Program Files\Git\bin\git.exe" set "GITEXE=C:\Program Files\Git\bin\git.exe"

if not defined GITEXE (
    if exist "%~dp0bootstrap.ps1" (
        echo [INFO] git not found. Running bootstrap.ps1...
        powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0bootstrap.ps1"
        if exist "C:\Program Files\Git\cmd\git.exe" set "GITEXE=C:\Program Files\Git\cmd\git.exe"
    )
)
if not defined GITEXE (
    echo [ERROR] git not found.
    exit /b 1
)

rem --- non-interactive safety ----------------------------------------------
set "GIT_TERMINAL_PROMPT=0"
set "GCM_INTERACTIVE=never"

rem --- repo-local hygiene (avoid LF<->CRLF churn) ---------------------------
"%GITEXE%" config --local core.autocrlf false >nul 2>&1

rem --- ensure identity (only if missing) ------------------------------------
"%GITEXE%" config --get user.name >nul 2>&1
if errorlevel 1 "%GITEXE%" config --local user.name "flwjfxm0004" >nul 2>&1
"%GITEXE%" config --get user.email >nul 2>&1
if errorlevel 1 "%GITEXE%" config --local user.email "flwjfxm0004@users.noreply.github.com" >nul 2>&1

rem --- lfs (only if git-lfs present AND lfs files tracked) ------------------
where git-lfs >nul 2>&1
if not errorlevel 1 (
    "%GITEXE%" lfs install --local >nul 2>&1
    "%GITEXE%" lfs ls-files 2>nul | findstr /r /c:"." >nul
    if not errorlevel 1 "%GITEXE%" lfs pull >nul 2>&1
)

rem --- stage ---------------------------------------------------------------
"%GITEXE%" add -A
if errorlevel 1 (
    echo [ERROR] git add failed.
    exit /b 1
)

rem --- commit (only if staged changes) -------------------------------------
"%GITEXE%" diff --cached --quiet
if errorlevel 2 (
    echo [ERROR] git diff failed.
    exit /b 1
)
if errorlevel 1 (
    "%GITEXE%" commit -q -m "Auto commit: %DATE% %TIME%"
    if errorlevel 1 (
        echo [ERROR] Commit failed.
        exit /b 1
    )
) else (
    echo [INFO] No changes to commit.
)

rem --- push (retry up to 2 times with rebase on failure) --------------------
set "PUSH_OK=0"
for /l %%i in (1,1,2) do (
    if "!PUSH_OK!"=="0" (
        "%GITEXE%" push --quiet origin master
        if not errorlevel 1 (
            set "PUSH_OK=1"
        ) else (
            "%GITEXE%" pull --rebase --autostash --quiet origin master
        )
    )
)
if "!PUSH_OK!"=="0" (
    echo [ERROR] Push failed.
    exit /b 1
)

echo Done.
endlocal
exit /b 0