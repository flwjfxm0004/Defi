# Permanent stop: kill all Pica/PcCafe/World Class processes WITHOUT shutting down the PC.
# CRITICAL: World Class shutdown protection triggers PC power-off when WCCustomer dies
# while its watchdogs (wchwmon, WCCST) are still alive. Therefore watchdogs MUST be
# killed FIRST and fully before any WCCustomer process is touched.

$processPatterns = 'PicaClient|Picaigreen|PcCafeMonitor|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|피시\.ps1'
$processNamePatterns = '^(pm|AutoStart|CraftBox|picaigreen|PicaClient|WCGuard|wchwmon|wcwatch|WCCustomer|WCCST|wcguard-service|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스)$'
$watchdogNames = @('wchwmon','WCCST','WCGuard','wcwatch','wcguard-service')
$customerNames = @('WCCustomer','WorldClassClient','WorldClassServer','WorldClassManager')

function Kill-ByName($name) {
    Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
        try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue; Write-Host "Killed: $($_.Name) ($($_.Id))" } catch {}
    }
}

function Kill-ByPathPattern() {
    Get-Process -ErrorAction SilentlyContinue | Where-Object { $_.Path -and ($_.Path -match $processPatterns) } | ForEach-Object {
        try { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue; Write-Host "Killed(path): $($_.Name) ($($_.Id))" } catch {}
    }
}

# ============================================================
# STEP 1: Kill ALL watchdogs FIRST (so WCCustomer death does NOT trigger PC shutdown)
# ============================================================
Write-Host "[1/7] Removing watchdogs before touching WCCustomer..."
foreach ($wd in $watchdogNames) { Kill-ByName $wd }
Kill-ByPathPattern
Start-Sleep -Seconds 3
# Second pass to catch any that respawned during the first pass
foreach ($wd in $watchdogNames) { Kill-ByName $wd }
Kill-ByPathPattern
Start-Sleep -Seconds 2

# ============================================================
# STEP 2: Kill WCCustomer only AFTER watchdogs are dead
# ============================================================
Write-Host "[2/7] Removing WCCustomer processes (watchdogs already removed)..."
foreach ($c in $customerNames) { Kill-ByName $c }
Kill-ByPathPattern
Start-Sleep -Seconds 2

# ============================================================
# STEP 3: Final sweep - kill any remaining related processes
# ============================================================
Write-Host "[3/7] Final process sweep..."
$all = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all) {
    $kill = $false
    if ($p.Path -and ($p.Path -match $processPatterns)) { $kill = $true }
    if ($p.Name -match $processNamePatterns) { $kill = $true }
    if ($kill) {
        try { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; Write-Host "Final kill: $($p.Name) ($($p.Id))" } catch {}
    }
}
Start-Sleep -Seconds 2

# ============================================================
# STEP 4: Stop and permanently disable/delete services
# ============================================================
Write-Host "[4/7] Removing services..."
$services = @('MWMiniflt','pmsys','igreensys','WCGuard','wchwmon','wcwatch','WorldClassClient','WorldClassServer','WorldClassManager','wcworldclass','WCWorldClass')
foreach ($s in $services) {
    $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
    if ($svc) {
        if ($svc.Status -eq 'Running') { Stop-Service -Name $s -Force -ErrorAction SilentlyContinue }
        try { Set-Service -Name $s -StartupType Disabled -ErrorAction SilentlyContinue } catch {}
        try { sc.exe config $s start= disabled 2>&1 | Out-Null } catch {}
        try { sc.exe delete $s 2>&1 | Out-Null } catch {}
        Write-Host "Service removed: $s"
    }
}

# ============================================================
# STEP 5: Remove registry Run/RunOnce entries (all hives/paths) - NO BACKUP
# ============================================================
Write-Host "[5/7] Removing registry auto-start entries..."
$targetNames = @('AutoRun','picaigreen','picaigreenRun','PcCafeMonitorClient','PicaClient','igreen','WCGuard','wchwmon','wcwatch','WCCustomer','wcguard-service','WCCST','WorldClass','world-class-user','WorldClassClient','WorldClassServer','WorldClassManager','wcworldclass','WCWorldClass','월드클래스')
function Is-TargetName($name) {
    foreach ($t in $targetNames) {
        if ($name -eq $t) { return $true }
        if ($name -like "*$t*") { return $true }
    }
    return $false
}
function Is-TargetValue($value) {
    if (-not $value) { return $false }
    return $value -match 'PicaClient|Picaigreen|PcCafeMonitor|igreen|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|피시\.ps1'
}

$runKeys = @(
    @{ Hive='HKCU'; Path='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' },
    @{ Hive='HKCU_RunOnce'; Path='HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce' },
    @{ Hive='HKLM'; Path='HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' },
    @{ Hive='HKLM_RunOnce'; Path='HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce' },
    @{ Hive='HKLM_WOW64'; Path='HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Run' },
    @{ Hive='HKLM_WOW64_RunOnce'; Path='HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\RunOnce' },
    @{ Hive='HKU18'; Path='Registry::HKEY_USERS\S-1-5-18\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' },
    @{ Hive='HKUDef'; Path='Registry::HKEY_USERS\.DEFAULT\SOFTWARE\Microsoft\Windows\CurrentVersion\Run' }
)
Get-ChildItem 'Registry::HKEY_USERS' -ErrorAction SilentlyContinue | ForEach-Object {
    $p = Join-Path $_.PSPath 'SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
    if (Test-Path $p) { $runKeys += @{ Hive="HKU_$($_.PSChildName)"; Path=$p } }
    $p2 = Join-Path $_.PSPath 'SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce'
    if (Test-Path $p2) { $runKeys += @{ Hive="HKU_$($_.PSChildName)_RunOnce"; Path=$p2 } }
}

foreach ($rk in $runKeys) {
    if (-not (Test-Path $rk.Path)) { continue }
    $props = Get-ItemProperty -Path $rk.Path -ErrorAction SilentlyContinue
    if (-not $props) { continue }
    foreach ($prop in $props.PSObject.Properties) {
        if ($prop.Name -match '^PS') { continue }
        if (Is-TargetName $prop.Name -or Is-TargetValue $prop.Value) {
            Remove-ItemProperty -Path $rk.Path -Name $prop.Name -ErrorAction SilentlyContinue
            Write-Host "Removed registry: $($rk.Hive)\$($prop.Name)"
        }
    }
}

# ============================================================
# STEP 6: Delete backup file + runtime folder + scheduled tasks + startup shortcuts
# ============================================================
Write-Host "[6/7] Deleting backup file, runtime folder, scheduled tasks, shortcuts..."

$backupFile = "C:\Users\Administrator\pcbtest\registry-backup.json"
if (Test-Path $backupFile) {
    Remove-Item -Path $backupFile -Force -ErrorAction SilentlyContinue
    Write-Host "Deleted backup file: $backupFile"
}

# Delete the World Class runtime folder so it cannot restart
$runtimeDir = "C:\Users\Administrator\AppData\Roaming\world-class-user\runtime"
if (Test-Path $runtimeDir) {
    Remove-Item -Path $runtimeDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "Deleted runtime folder: $runtimeDir"
}

# Scheduled tasks
Get-ScheduledTask -ErrorAction SilentlyContinue | ForEach-Object {
    $task = $_
    $match = $false
    foreach ($a in $task.Actions) {
        if ($a.Execute -match $processPatterns -or $a.Arguments -match $processPatterns) { $match = $true }
    }
    if ($match) {
        try { Unregister-ScheduledTask -TaskName $task.TaskName -TaskPath $task.TaskPath -Confirm:$false -ErrorAction SilentlyContinue; Write-Host "Removed scheduled task: $($task.TaskPath)$($task.TaskName)" } catch {}
    }
}

# Startup folder shortcuts
$startupFolders = @(
    "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup",
    "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup"
)
foreach ($folder in $startupFolders) {
    if (-not (Test-Path $folder)) { continue }
    Get-ChildItem -Path $folder -Filter *.lnk -ErrorAction SilentlyContinue | ForEach-Object {
        try {
            $sh = New-Object -ComObject WScript.Shell
            $sc = $sh.CreateShortcut($_.FullName)
            if ($sc.TargetPath -match $processPatterns) {
                Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
                Write-Host "Removed startup shortcut: $($_.FullName)"
            }
        } catch {}
    }
}

# ============================================================
# STEP 7: Final confirmation sweep
# ============================================================
Write-Host "[7/7] Final confirmation sweep..."
$all2 = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all2) {
    $kill = $false
    if ($p.Path -and ($p.Path -match $processPatterns)) { $kill = $true }
    if ($p.Name -match $processNamePatterns) { $kill = $true }
    if ($kill) {
        try { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; Write-Host "Confirmed kill: $($p.Name) ($($p.Id))" } catch {}
    }
}

Write-Host "Done. All Pica/PcCafe/World Class entries permanently disabled. PC was NOT shut down. No backup saved."
