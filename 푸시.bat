@echo off
setlocal
cd /d "%~dp0"

rem Ensure git is available
where git >nul 2>&1
if errorlevel 1 (
    if exist "C:\Program Files\Git\cmd\git.exe" (
        set "PATH=C:\Program Files\Git\cmd;%PATH%"
    ) else (
        echo [ERROR] git not found. Install git first.
        pause
        exit /b 1
    )
)

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
