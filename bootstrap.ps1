# Bootstrap dev tooling on this Windows PC (no winget/choco available)
# Run as Administrator. Downloads git + python installers and installs silently.

$ErrorActionPreference = 'Stop'
$tmp = "C:\Users\Administrator\Downloads\bootstrap"
New-Item -ItemType Directory -Path $tmp -Force | Out-Null

# Git for Windows
$gitUrl = "https://github.com/git-for-windows/git/releases/download/v2.47.0.windows.2/Git-2.47.0.2-64-bit.exe"
$gitExe = Join-Path $tmp "git-installer.exe"
Write-Host "Downloading Git..."
Invoke-WebRequest -Uri $gitUrl -OutFile $gitExe -UseBasicParsing
Write-Host "Installing Git..."
Start-Process -FilePath $gitExe -ArgumentList "/VERYSILENT","/NORESTART","/NOCANCEL","/SP-","/CLOSEAPPLICATIONS","/RESTARTAPPLICATIONS" -Wait
Write-Host "Git done."

# Python 3.12
$pyUrl = "https://www.python.org/ftp/python/3.12.7/python-3.12.7-amd64.exe"
$pyExe = Join-Path $tmp "python-installer.exe"
Write-Host "Downloading Python..."
Invoke-WebRequest -Uri $pyUrl -OutFile $pyExe -UseBasicParsing
Write-Host "Installing Python..."
Start-Process -FilePath $pyExe -ArgumentList "/quiet","InstallAllUsers=1","PrependPath=1","Include_pip=1" -Wait
Write-Host "Python done."

# Refresh PATH for this session
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
Write-Host "Verifying..."
git --version
python --version
pip --version
# Initialize git-lfs (bundled with Git for Windows)
try { git lfs install 2>&1 | Out-Null; Write-Host "git-lfs initialized" } catch { Write-Host "git-lfs init skipped: $_" }
Write-Host "Bootstrap complete. Restart terminal for PATH to persist."
