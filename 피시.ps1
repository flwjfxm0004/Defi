# Permanent stop: kill all Pica/PcCafe/World Class processes, disable/delete services/drivers, remove all auto-start entries.
# 1) Kill all related processes FIRST (so watchdogs cannot respawn during cleanup)
$processPatterns = 'PicaClient|Picaigreen|PcCafeMonitor|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|피시\.ps1'
$processNamePatterns = '^(pm|AutoStart|CraftBox|picaigreen|PicaClient|WCGuard|wchwmon|wcwatch|WCCustomer|WCCST|wcguard-service|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스)$'
$all = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all) {
    $kill = $false
    if ($p.Path -and ($p.Path -match $processPatterns)) { $kill = $true }
    if ($p.Name -match $processNamePatterns) { $kill = $true }
    if ($kill) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        Write-Host "Killed: $($p.Name) ($($p.Id))"
    }
}

# 2) Stop and permanently disable/delete services
$services = @('MWMiniflt','pmsys','igreensys','WCGuard','wchwmon','wcwatch','WorldClassClient','WorldClassServer','WorldClassManager','wcworldclass','WCWorldClass')
foreach ($s in $services) {
    $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
    if ($svc) {
        if ($svc.Status -eq 'Running') { Stop-Service -Name $s -Force -ErrorAction SilentlyContinue }
        try { Set-Service -Name $s -StartupType Disabled -ErrorAction SilentlyContinue } catch {}
        try { sc.exe config $s start= disabled 2>&1 | Out-Null } catch {}
        try { sc.exe delete $s 2>&1 | Out-Null } catch {}
        Write-Host "Service stopped+disabled+deleted: $s"
    }
}

# 3) Remove registry Run/RunOnce entries (all hives/paths) - NO BACKUP
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
            Write-Host "Removed registry: $($rk.Hive)\$($prop.Name) = $($prop.Value)"
        }
    }
}

# 4) Delete old backup file so nothing can be restored
$backupFile = "C:\Users\Administrator\pcbtest\registry-backup.json"
if (Test-Path $backupFile) {
    Remove-Item -Path $backupFile -Force -ErrorAction SilentlyContinue
    Write-Host "Deleted backup file: $backupFile"
}

# 5) Remove scheduled tasks whose action references pica/pc cafe/world class (including self-guard)
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

# 6) Remove startup folder shortcuts pointing to pica/world class
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

# 7) Final sweep: kill any processes that may have respawned
$all2 = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all2) {
    $kill = $false
    if ($p.Path -and ($p.Path -match $processPatterns)) { $kill = $true }
    if ($p.Name -match $processNamePatterns) { $kill = $true }
    if ($kill) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        Write-Host "Final kill: $($p.Name) ($($p.Id))"
    }
}

Write-Host "Done. All Pica/PcCafe/World Class entries permanently disabled. No backup saved."
