@echo off
setlocal enableextensions
cd /d "%~dp0."

rem --- locate git -----------------------------------------------------------
set "GITEXE="
for /f "delims=" %%G in ('where git 2^>nul') do if not defined GITEXE set "GITEXE=%%G"
if not defined GITEXE if exist "C:\Program Files\Git\cmd\git.exe" set "GITEXE=C:\Program Files\Git\cmd\git.exe"
if not defined GITEXE if exist "C:\Program Files\Git\bin\git.exe" set "GITEXE=C:\Program Files\Git\bin\git.exe"

if not defined GITEXE (
    echo [ERROR] git not found.
    exit /b 1
)

rem --- non-interactive safety ----------------------------------------------
set "GIT_TERMINAL_PROMPT=0"
set "GCM_INTERACTIVE=never"
set "GIT_ASKPASS="
set "SSH_ASKPASS="


rem --- network hang guard ---------------------------------------------------
"%GITEXE%" config --local http.lowSpeedLimit 1000 >nul 2>&1
"%GITEXE%" config --local http.lowSpeedTime 10 >nul 2>&1

rem --- ensure identity (only if missing) ------------------------------------
"%GITEXE%" config --get user.name >nul 2>&1
if errorlevel 1 "%GITEXE%" config --local user.name "flwjfxm0004" >nul 2>&1
"%GITEXE%" config --get user.email >nul 2>&1
if errorlevel 1 "%GITEXE%" config --local user.email "flwjfxm0004@users.noreply.github.com" >nul 2>&1

rem --- current branch (fallback: master) ------------------------------------
set "BRANCH=master"
for /f "delims=" %%B in ('"%GITEXE%" rev-parse --abbrev-ref HEAD 2^>nul') do set "BRANCH=%%B"
if "%BRANCH%"=="" set "BRANCH=master"
if "%BRANCH%"=="HEAD" set "BRANCH=master"

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

rem --- unstage likely secrets (single call, defense in depth) ---------------
"%GITEXE%" reset -q HEAD -- .env .env.* *.pem *.key *.p12 *.pfx id_rsa id_ed25519 *secret* *credential* *.keystore *.token *.asc *.ovpn *.jks secrets.yaml secrets.yml .npmrc .pypirc >nul 2>&1

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

rem --- push (retry up to 2 times with rebase on failure, early exit) --------
set "PUSH_TRIES=0"

:push_try
"%GITEXE%" push --quiet origin %BRANCH%
if not errorlevel 1 goto push_ok
"%GITEXE%" pull --rebase --autostash --quiet origin %BRANCH%
if errorlevel 1 (
    "%GITEXE%" rebase --abort >nul 2>&1
    goto push_fail
)
set /a PUSH_TRIES+=1
if %PUSH_TRIES% lss 2 goto push_try
goto push_fail

:push_fail
echo [ERROR] Push failed.
exit /b 1

:push_ok
echo Done.
endlocal
exit /b 0