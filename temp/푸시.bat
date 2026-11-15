@echo off
setlocal enableextensions
chcp 65001 >nul

rem --- locate Defi repo root (works from temp\ or root) ----------------------
set "SCRIPT_DIR=%~dp0"
if exist "%SCRIPT_DIR%..\.git" (
    cd /d "%SCRIPT_DIR%.."
) else (
    cd /d "%SCRIPT_DIR%."
)
set "DEFI_ROOT=%CD%"
set "PATH=%DEFI_ROOT%;%DEFI_ROOT%\tools\python;%DEFI_ROOT%\tools\git\cmd;%DEFI_ROOT%\tools\git\ucrt64\bin;%DEFI_ROOT%\tools\git\usr\bin;%PATH%"

rem --- prefer bundled python + 피시.py push when available -------------------
if exist "%DEFI_ROOT%\python.exe" if exist "%DEFI_ROOT%\temp\피시.py" (
    "%DEFI_ROOT%\python.exe" -B -X utf8 "%DEFI_ROOT%\temp\피시.py" push %*
    exit /b %ERRORLEVEL%
)

rem --- fallback: bundled portable git -> system git --------------------------
set "GITEXE="
if exist "%DEFI_ROOT%\tools\git\cmd\git.exe" set "GITEXE=%DEFI_ROOT%\tools\git\cmd\git.exe"
if not defined GITEXE for /f "delims=" %%G in ('where git 2^>nul') do if not defined GITEXE set "GITEXE=%%G"
if not defined GITEXE if exist "C:\Program Files\Git\cmd\git.exe" set "GITEXE=C:\Program Files\Git\cmd\git.exe"
if not defined GITEXE (
    echo [ERROR] git not found.
    exit /b 1
)

set "GIT_TERMINAL_PROMPT=0"
"%GITEXE%" config --local credential.helper manager >nul 2>&1
"%GITEXE%" config --local core.quotepath false >nul 2>&1
"%GITEXE%" config --local core.autocrlf false >nul 2>&1
"%GITEXE%" add -A
"%GITEXE%" diff --cached --quiet
if errorlevel 1 "%GITEXE%" commit -m "Auto commit: %DATE% %TIME%"
"%GITEXE%" push origin HEAD
endlocal
exit /b %ERRORLEVEL%
