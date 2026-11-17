@echo off
setlocal enableextensions
chcp 65001 >nul

rem Thin launcher: all Git behavior lives in temp\피시.py (push).
rem This file must NOT re-implement add/commit/push, git config, or git lookup.
rem Works from temp\ (normal) or from a copy in the repo root.
set "SCRIPT_DIR=%~dp0"
if exist "%SCRIPT_DIR%피시.py" (
    set "PYSIDE=%SCRIPT_DIR%피시.py"
    set "DEFI_ROOT=%SCRIPT_DIR%.."
) else if exist "%SCRIPT_DIR%temp\피시.py" (
    set "PYSIDE=%SCRIPT_DIR%temp\피시.py"
    set "DEFI_ROOT=%SCRIPT_DIR%."
) else (
    echo [ERROR] not found: temp\피시.py next to this script.
    exit /b 1
)

set "PYEXE="
if exist "%DEFI_ROOT%\python.exe" set "PYEXE=%DEFI_ROOT%\python.exe"
if not defined PYEXE for /f "delims=" %%P in ('where python 2^>nul') do if not defined PYEXE set "PYEXE=%%P"
if not defined PYEXE for /f "delims=" %%P in ('where py 2^>nul') do if not defined PYEXE set "PYEXE=%%P"
if not defined PYEXE (
    echo [ERROR] python not found. Restore bundled python.exe or install Python 3.12.
    exit /b 1
)

"%PYEXE%" -B -X utf8 "%PYSIDE%" push %*
exit /b %ERRORLEVEL%
