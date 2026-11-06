# Ghidra + Temurin JDK 21 setup on Windows (run as Administrator)
$ErrorActionPreference = 'Stop'
$tmp = "C:\Defi\tools\ghidra_setup"
New-Item -ItemType Directory -Path $tmp -Force | Out-Null

# 1. JDK 21 (Temurin) - latest GA
if (-not (Test-Path "C:\Program Files\Eclipse Adoptium\jdk-21*")) {
    Write-Host "Fetching JDK 21 download link..."
    $r = Invoke-RestMethod -Uri "https://api.adoptium.net/v3/assets/feature_releases/21/ga?architecture=x64&image_type=jdk&os=windows&vendor=eclipse"
    $link = $r[0].binaries[0].package.link
    $zip = Join-Path $tmp "jdk21.zip"
    Write-Host "Downloading JDK 21 (~205MB)..."
    Invoke-WebRequest -Uri $link -OutFile $zip -UseBasicParsing
    Write-Host "Extracting JDK..."
    Expand-Archive -LiteralPath $zip -DestinationPath "C:\Program Files\Eclipse Adoptium" -Force
    Remove-Item $zip
    Write-Host "JDK done."
} else {
    Write-Host "JDK already installed."
}

# find java
$java = Get-ChildItem "C:\Program Files\Eclipse Adoptium" -Recurse -Filter "java.exe" -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty FullName
if (-not $java) { throw "java.exe not found" }
$env:JAVA_HOME = Split-Path (Split-Path $java)
Write-Host "JAVA_HOME=$env:JAVA_HOME"

# 2. Ghidra (latest release)
$ghidraDir = "C:\Defi\tools\ghidra"
if (-not (Test-Path "$ghidraDir\ghidraRun.bat")) {
    Write-Host "Fetching Ghidra latest release..."
    $rel = Invoke-RestMethod -Uri "https://api.github.com/repos/NationalSecurityAgency/ghidra/releases/latest"
    $asset = $rel.assets | Where-Object { $_.name -like "ghidra_*PUBLIC*.zip" } | Select-Object -First 1
    $zip = Join-Path $tmp "ghidra.zip"
    Write-Host "Downloading $($asset.name) (~400MB)..."
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $zip -UseBasicParsing
    Write-Host "Extracting Ghidra..."
    New-Item -ItemType Directory -Path $ghidraDir -Force | Out-Null
    Expand-Archive -LiteralPath $zip -DestinationPath $ghidraDir -Force
    Remove-Item $zip
    # move contents up if nested
    $nested = Get-ChildItem $ghidraDir -Directory | Where-Object { $_.Name -like "ghidra_*" } | Select-Object -First 1
    if ($nested) {
        Get-ChildItem $nested.FullName | Move-Item -Destination $ghidraDir -Force
        Remove-Item $nested.FullName -Recurse -Force
    }
    Write-Host "Ghidra done."
} else {
    Write-Host "Ghidra already installed."
}

Write-Host "Setup complete. Run: $ghidraDir\ghidraRun.bat"
Write-Host "JAVA_HOME should be set to: $env:JAVA_HOME"
