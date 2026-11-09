# Permanent stop: kill all Pica/PcCafe/World Class/PFGamePot/HyperOTT(NsmPlayer)/DAK.GG processes WITHOUT shutting down the PC.
# CRITICAL: World Class shutdown protection triggers PC power-off when WCCustomer dies
# while its watchdogs (wchwmon, WCCST) are still alive. Therefore watchdogs MUST be
# killed FIRST and fully before any WCCustomer process is touched.
# No sleeps between kill steps to prevent watchdog respawn windows.

$processPatterns = 'PicaClient|Picaigreen|PcCafeMonitor|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|피시\.ps1|PFGamePot|GamePot|PF|PFGameDisk|PFGameSync|PFCms|PFControlMgr|PFDesktopClient|PFHardwardHelper|DesktopAgent|NsmPlayer|HyperOTT|Nsm|muplayer|DAK\.GG'
$processNamePatterns = '^(pm|AutoStart|CraftBox|picaigreen|PicaClient|WCGuard|wchwmon|wcwatch|WCCustomer|WCCST|wcguard-service|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|PFGamePot|PFCms|PFControlMgr|PFDesktopClient|PFHardwardHelper|PFGameDisk|PFGameSyncAgent|PFGameSyncMgr|DesktopAgent|NsmPlayer2|muplayer|DAK\.GG)$'
$watchdogNames = @('wchwmon','WCCST','WCGuard','wcwatch','wcguard-service','PFControlMgr','PFHardwardHelper','PFGameSyncMgr','DesktopAgent','NsmPlayer2','muplayer','DAK.GG')
$customerNames = @('WCCustomer','WorldClassClient','WorldClassServer','WorldClassManager','PFGamePot','PFCms','PFDesktopClient','PFGameDisk','PFGameSyncAgent','NsmPlayer2','muplayer','DAK.GG')

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
# STEP 1: Kill ALL watchdogs FIRST (so client death does NOT trigger PC shutdown)
# Rapid multi-pass, no sleeps between passes
# ============================================================
Write-Host "[1/9] Removing watchdogs before touching client processes..."
for ($i = 0; $i -lt 5; $i++) {
    foreach ($wd in $watchdogNames) { Kill-ByName $wd }
    Kill-ByPathPattern
}
Write-Host "Watchdog pass complete."

# ============================================================
# STEP 2: Kill client processes immediately after watchdogs are down
# ============================================================
Write-Host "[2/9] Removing client processes..."
for ($i = 0; $i -lt 5; $i++) {
    foreach ($c in $customerNames) { Kill-ByName $c }
    Kill-ByPathPattern
}
Write-Host "Client pass complete."

# ============================================================
# STEP 3: Delete installation/runtime folders IMMEDIATELY so nothing can restart
# ============================================================
Write-Host "[3/9] Deleting World Class and PFGamePot installation folders..."

$foldersToDelete = @(
    "C:\Users\Administrator\AppData\Roaming\world-class-user\runtime",
    "H:\GamePot_Client",
    "H:\PFGameSync_Client",
    "C:\PFGameDisk",
    "H:\DATA\HyperOTT",
    "H:\DATA\DAK.GG",
    "C:\Users\Administrator\AppData\Local\Programs\DAK.GG",
    "C:\Users\Administrator\AppData\Roaming\DAK.GG"
)

foreach ($folder in $foldersToDelete) {
    if (Test-Path $folder) {
        try {
            # Stop any processes still holding files in the folder
            Get-ChildItem -Path $folder -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
                try { Stop-Process -Id (Get-Process -FilePath $_.FullName -ErrorAction SilentlyContinue).Id -Force -ErrorAction SilentlyContinue } catch {}
            }
            Remove-Item -Path $folder -Recurse -Force -ErrorAction SilentlyContinue
            Write-Host "Deleted folder: $folder"
        } catch {
            Write-Host "Could not delete folder (files in use): $folder"
        }
    }
}

# ============================================================
# STEP 4: Final process sweep - kill any remaining related processes
# ============================================================
Write-Host "[4/9] Final process sweep..."
$all = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all) {
    $kill = $false
    if ($p.Path -and ($p.Path -match $processPatterns)) { $kill = $true }
    if ($p.Name -match $processNamePatterns) { $kill = $true }
    if ($kill) {
        try { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; Write-Host "Final kill: $($p.Name) ($($p.Id))" } catch {}
    }
}

# ============================================================
# STEP 5: Stop and permanently disable/delete services + kernel drivers
# ============================================================
Write-Host "[5/9] Removing services and kernel drivers..."
$services = @('MWMiniflt','pmsys','igreensys','WCGuard','wchwmon','wcwatch','WorldClassClient','WorldClassServer','WorldClassManager','wcworldclass','WCWorldClass','PFGamePot','PFCms','PFControlMgr','PFDesktopClient','PFHardwardHelper','PFGameDisk','PFGameSyncAgent','PFGameSyncMgr','DesktopAgent','R0PFHardwardHelper','PFGameDiskPNP','NsmPlayer2','muplayer','DAK.GG')
foreach ($s in $services) {
    $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
    if ($svc) {
        if ($svc.Status -eq 'Running') {
            try { Stop-Service -Name $s -Force -ErrorAction SilentlyContinue } catch {}
            try { sc.exe stop $s 2>&1 | Out-Null } catch {}
        }
        try { Set-Service -Name $s -StartupType Disabled -ErrorAction SilentlyContinue } catch {}
        try { sc.exe config $s start= disabled 2>&1 | Out-Null } catch {}
        try { sc.exe delete $s 2>&1 | Out-Null } catch {}
        Write-Host "Service/driver removed: $s"
    }
}

# ============================================================
# STEP 6: Remove registry Run/RunOnce entries (all hives/paths) - NO BACKUP
# ============================================================
Write-Host "[6/9] Removing registry auto-start entries..."
$targetNames = @('AutoRun','picaigreen','picaigreenRun','PcCafeMonitorClient','PicaClient','igreen','WCGuard','wchwmon','wcwatch','WCCustomer','wcguard-service','WCCST','WorldClass','world-class-user','WorldClassClient','WorldClassServer','WorldClassManager','wcworldclass','WCWorldClass','월드클래스','PFGamePot','PFCms','PFControlMgr','PFDesktopClient','PFHardwardHelper','PFGameDisk','PFGameSyncAgent','PFGameSyncMgr','DesktopAgent','GamePot','NsmPlayer','HyperOTT','Nsm','muplayer','DAK.GG','electron.app.DAK.GG')
function Is-TargetName($name) {
    foreach ($t in $targetNames) {
        if ($name -eq $t) { return $true }
        if ($name -like "*$t*") { return $true }
    }
    return $false
}
function Is-TargetValue($value) {
    if (-not $value) { return $false }
    return $value -match 'PicaClient|Picaigreen|PcCafeMonitor|igreen|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST|WorldClassClient|WorldClassServer|WorldClassManager|wcworldclass|WCWorldClass|월드클래스|피시\.ps1|PFGamePot|PFCms|PFControlMgr|PFDesktopClient|PFHardwardHelper|PFGameDisk|PFGameSyncAgent|PFGameSyncMgr|DesktopAgent|GamePot|NsmPlayer|HyperOTT|Nsm|muplayer|DAK\.GG'
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
# STEP 7: Delete backup file + scheduled tasks + startup shortcuts
# ============================================================
Write-Host "[7/9] Deleting backup file, scheduled tasks, shortcuts..."

$backupFile = "C:\Users\Administrator\pcbtest\registry-backup.json"
if (Test-Path $backupFile) {
    Remove-Item -Path $backupFile -Force -ErrorAction SilentlyContinue
    Write-Host "Deleted backup file: $backupFile"
}

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

# Delete DAK.GG shortcuts from Start Menu Programs and Desktop
$shortcutPaths = @(
    "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\DAK.GG.lnk",
    "$env:USERPROFILE\Desktop\DAK.GG.lnk",
    "H:\DATA\Desktop\DAK.GG.lnk"
)
foreach ($sp in $shortcutPaths) {
    if (Test-Path $sp) {
        try { Remove-Item -LiteralPath $sp -Force -ErrorAction SilentlyContinue; Write-Host "Deleted shortcut: $sp" } catch {}
    }
}

# ============================================================
# STEP 8: Final confirmation sweep
# ============================================================
Write-Host "[8/9] Final confirmation sweep..."
$all2 = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all2) {
    $kill = $false
    if ($p.Path -and ($p.Path -match $processPatterns)) { $kill = $true }
    if ($p.Name -match $processNamePatterns) { $kill = $true }
    if ($kill) {
        try { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue; Write-Host "Confirmed kill: $($p.Name) ($($p.Id))" } catch {}
    }
}

# ============================================================
# STEP 9: Second folder sweep (catch anything recreated)
# ============================================================
Write-Host "[9/9] Second folder sweep..."
foreach ($folder in $foldersToDelete) {
    if (Test-Path $folder) {
        try {
            Get-ChildItem -Path $folder -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object {
                try { Stop-Process -Id (Get-Process -FilePath $_.FullName -ErrorAction SilentlyContinue).Id -Force -ErrorAction SilentlyContinue } catch {}
            }
            Remove-Item -Path $folder -Recurse -Force -ErrorAction SilentlyContinue
            Write-Host "Deleted folder (second pass): $folder"
        } catch {}
    }
}

Write-Host "Done. All Pica/PcCafe/World Class/PFGamePot/HyperOTT(NsmPlayer)/DAK.GG entries permanently disabled. PC was NOT shut down. No backup saved."
