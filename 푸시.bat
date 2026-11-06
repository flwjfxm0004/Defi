@echo off
setlocal
cd /d "%~dp0"

rem Locate git (PATH or common install locations)
set "GITEXE="
where git >nul 2>&1 && set "GITEXE=git"
if not defined GITEXE if exist "C:\Program Files\Git\cmd\git.exe" set "GITEXE=C:\Program Files\Git\cmd\git.exe"
if not defined GITEXE if exist "C:\Program Files\Git\bin\git.exe" set "GITEXE=C:\Program Files\Git\bin\git.exe"

rem Auto-install if missing
if not defined GITEXE (
    if exist "%~dp0bootstrap.ps1" (
        echo [INFO] git not found. Running bootstrap.ps1 to install git + python...
        powershell -ExecutionPolicy Bypass -File "%~dp0bootstrap.ps1"
        if exist "C:\Program Files\Git\cmd\git.exe" set "GITEXE=C:\Program Files\Git\cmd\git.exe"
    )
)

if not defined GITEXE (
    echo [ERROR] git not found. Run bootstrap.ps1 as Administrator first.
    pause
    exit /b 1
)

rem Put git on PATH for git-lfs etc.
for %%D in ("%GITEXE%") do set "GITDIR=%%~dpD"
set "PATH=%GITDIR%;%PATH%"

rem Ensure git-lfs hooks are installed
git lfs install >nul 2>&1

rem Fetch LFS objects (e.g. IPA) if missing (fresh clone)
git lfs pull >nul 2>&1

echo [1/3] Staging all changes...
git add -A

echo [2/3] Committing...
git diff --cached --quiet
if errorlevel 1 (
    git commit -m "Auto commit: %DATE% %TIME%"
    if errorlevel 1 (
        echo [ERROR] Commit failed.
        pause
        exit /b 1
    )
) else (
    echo No changes to commit.
)

echo [3/3] Pushing to origin/master...
git push origin master
if errorlevel 1 (
    echo [ERROR] Push failed.
    pause
    exit /b 1
)

echo Done.
endlocal
