@echo off
setlocal enableextensions
cd /d "%~dp0"

rem --- locate git -----------------------------------------------------------
set "GITEXE="
where git >nul 2>&1 && set "GITEXE=git"
if not defined GITEXE if exist "C:\Program Files\Git\cmd\git.exe" set "GITEXE=C:\Program Files\Git\cmd\git.exe"
if not defined GITEXE if exist "C:\Program Files\Git\bin\git.exe" set "GITEXE=C:\Program Files\Git\bin\git.exe"

rem --- auto-install if missing --------------------------------------------
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

for %%D in ("%GITEXE%") do set "GITDIR=%%~dpD"
set "PATH=%GITDIR%;%PATH%"

rem --- non-interactive safety ----------------------------------------------
set "GIT_TERMINAL_PROMPT=0"
set "GCM_INTERACTIVE=never"

rem --- ensure identity (only if missing) ------------------------------------
"%GITEXE%" config user.name >nul 2>&1
if errorlevel 1 "%GITEXE%" config user.name "flwjfxm0004" >nul 2>&1
"%GITEXE%" config user.email >nul 2>&1
if errorlevel 1 "%GITEXE%" config user.email "flwjfxm0004@users.noreply.github.com" >nul 2>&1
rem repo-local fallback if still empty
"%GITEXE%" config --get user.name >nul 2>&1
if errorlevel 1 "%GITEXE%" config --local user.name "flwjfxm0004" >nul 2>&1
"%GITEXE%" config --get user.email >nul 2>&1
if errorlevel 1 "%GITEXE%" config --local user.email "flwjfxm0004@users.noreply.github.com" >nul 2>&1

rem --- lfs (only if available) ---------------------------------------------
where git-lfs >nul 2>&1
if not errorlevel 1 (
    "%GITEXE%" lfs install --local >nul 2>&1
    "%GITEXE%" lfs pull >nul 2>&1
)

rem --- stage ---------------------------------------------------------------
"%GITEXE%" add -A
if errorlevel 1 (
    echo [ERROR] git add failed.
    exit /b 1
)

rem --- commit (only if staged changes) -------------------------------------
"%GITEXE%" diff --cached --quiet
if errorlevel 1 (
    "%GITEXE%" commit -q -m "Auto commit: %DATE% %TIME%"
    if errorlevel 1 (
        echo [ERROR] Commit failed.
        exit /b 1
    )
) else (
    echo [INFO] No changes to commit.
)

rem --- push (retry once with rebase on failure) -----------------------------
"%GITEXE%" push --quiet origin master
if errorlevel 1 (
    echo [WARN] Push failed. Pulling with rebase and retrying...
    "%GITEXE%" pull --rebase --autostash --quiet origin master
    if errorlevel 1 (
        echo [ERROR] Pull/rebase failed.
        exit /b 1
    )
    "%GITEXE%" push --quiet origin master
    if errorlevel 1 (
        echo [ERROR] Push failed.
        exit /b 1
    )
)

echo Done.
endlocal
exit /b 0