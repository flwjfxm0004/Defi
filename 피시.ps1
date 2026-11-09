# Comprehensive stop: kill all Pica/PcCafe processes, disable services/drivers, remove registry Run entries (backed up).
$backupFile = "C:\Users\Administrator\pcbtest\registry-backup.json"
$backup = @{}

# Helper: convert PSCustomObject (from ConvertFrom-Json) into hashtable recursively
function ConvertTo-Hashtable($obj) {
    if ($null -eq $obj) { return $null }
    if ($obj -is [System.Collections.IDictionary]) {
        $ht = @{}
        foreach ($k in $obj.Keys) { $ht[$k] = ConvertTo-Hashtable $obj[$k] }
        return $ht
    }
    if ($obj -is [System.Management.Automation.PSCustomObject]) {
        $ht = @{}
        foreach ($p in $obj.PSObject.Properties) { $ht[$p.Name] = ConvertTo-Hashtable $p.Value }
        return $ht
    }
    if ($obj -is [System.Collections.IEnumerable] -and -not ($obj -is [string])) {
        $arr = @()
        foreach ($i in $obj) { $arr += (ConvertTo-Hashtable $i) }
        return $arr
    }
    return $obj
}

if (Test-Path $backupFile) {
    try {
        $loaded = Get-Content $backupFile -Raw | ConvertFrom-Json
        $backup = ConvertTo-Hashtable $loaded
        if ($null -eq $backup) { $backup = @{} }
    } catch {
        $backup = @{}
    }
} else {
    $backup = @{}
}

# 1) Stop protection minifilter driver first (so processes can be killed)
foreach ($drv in @('MWMiniflt')) {
    $svc = Get-Service -Name $drv -ErrorAction SilentlyContinue
    if ($svc) {
        if ($svc.Status -eq 'Running') { Stop-Service -Name $drv -Force -ErrorAction SilentlyContinue }
        try { Set-Service -Name $drv -StartupType Disabled -ErrorAction SilentlyContinue } catch {}
        try { sc.exe config $drv start= disabled 2>&1 | Out-Null } catch {}
        Write-Host "Driver stopped+disabled: $drv"
    }
}

# 2) Disable services
foreach ($s in @('pmsys','igreensys','WCGuard','wchwmon','wcwatch')) {
    $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
    if ($svc) {
        if ($svc.Status -eq 'Running') { Stop-Service -Name $s -Force -ErrorAction SilentlyContinue }
        try { Set-Service -Name $s -StartupType Disabled -ErrorAction SilentlyContinue } catch {}
        Write-Host "Service stopped+disabled: $s"
    }
}

# 3) Remove registry Run entries (all hives/paths)
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
# Also every loaded user hive
Get-ChildItem 'Registry::HKEY_USERS' -ErrorAction SilentlyContinue | ForEach-Object {
    $p = Join-Path $_.PSPath 'SOFTWARE\Microsoft\Windows\CurrentVersion\Run'
    if (Test-Path $p) {
        $runKeys += @{ Hive="HKU_$($_.PSChildName)"; Path=$p }
    }
    $p2 = Join-Path $_.PSPath 'SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce'
    if (Test-Path $p2) {
        $runKeys += @{ Hive="HKU_$($_.PSChildName)_RunOnce"; Path=$p2 }
    }
}

$targetNames = @('AutoRun','picaigreen','picaigreenRun','PcCafeMonitorClient','PicaClient','igreen','WCGuard','wchwmon','wcwatch','WCCustomer','wcguard-service','WCCST','WorldClass','world-class-user')
function Is-Target($name, $value) {
    foreach ($t in $targetNames) {
        if ($name -eq $t) { return $true }
        if ($name -like "*$t*") { return $true }
    }
    if ($value -and ($value -match 'PicaClient|Picaigreen|PcCafeMonitor|igreen|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST')) { return $true }
    return $false
}

foreach ($rk in $runKeys) {
    if (-not (Test-Path $rk.Path)) { continue }
    $props = Get-ItemProperty -Path $rk.Path -ErrorAction SilentlyContinue
    if (-not $props) { continue }
    foreach ($prop in $props.PSObject.Properties) {
        if ($prop.Name -match '^PS') { continue }
        if (Is-Target $prop.Name $prop.Value) {
            if (-not $backup.ContainsKey($rk.Hive)) { $backup[$rk.Hive] = @{} }
            $backup[$rk.Hive][$prop.Name] = $prop.Value
            Remove-ItemProperty -Path $rk.Path -Name $prop.Name -ErrorAction SilentlyContinue
            Write-Host "Removed registry: $($rk.Hive)\$($prop.Name) = $($prop.Value)"
        }
    }
}

# 4) Remove scheduled tasks whose action references pica/pc cafe/world class
Get-ScheduledTask -ErrorAction SilentlyContinue | ForEach-Object {
    $task = $_
    $match = $false
    foreach ($a in $task.Actions) {
        if ($a.Execute -match 'pica|PcCafe|igreen|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST' -or $a.Arguments -match 'pica|PcCafe|igreen|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST') { $match = $true }
    }
    if ($match) {
        try { Unregister-ScheduledTask -TaskName $task.TaskName -TaskPath $task.TaskPath -Confirm:$false -ErrorAction SilentlyContinue; Write-Host "Removed scheduled task: $($task.TaskPath)$($task.TaskName)" } catch {}
    }
}

# 5) Remove startup folder shortcuts pointing to pica/world class
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
            if ($sc.TargetPath -match 'pica|PcCafe|igreen|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST') {
                Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue
                Write-Host "Removed startup shortcut: $($_.FullName)"
            }
        } catch {}
    }
}

# Save backup
try {
    $backup | ConvertTo-Json -Depth 5 | Set-Content -Path $backupFile -Encoding UTF8
    Write-Host "Backup saved: $backupFile"
} catch {
    Write-Host "Backup save failed: $_"
}

# 6) Kill all Pica/PcCafe/World Class related processes (by path or name)
$all = Get-Process -ErrorAction SilentlyContinue
foreach ($p in $all) {
    $kill = $false
    if ($p.Path -and ($p.Path -match 'PicaClient|Picaigreen|PcCafeMonitor|WCGuard|wchwmon|wcwatch|WCCustomer|WorldClass|world-class-user|WCCST')) { $kill = $true }
    if ($p.Name -match '^(pm|AutoStart|CraftBox|picaigreen|PicaClient|WCGuard|wchwmon|wcwatch|WCCustomer|WCCST|wcguard-service)') { $kill = $true }
    if ($kill) {
        Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
        Write-Host "Killed: $($p.Name) ($($p.Id))"
    }
}
# 7) Register a recurring guard (every 1 minute) so the pica guard cannot revive
try {
    $taskName = "PicaStopAllGuard"
    $action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-ExecutionPolicy Bypass -WindowStyle Hidden -File `"C:\Defi\피시.ps1`""
    $principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -LogonType ServiceAccount -RunLevel Highest
    $trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 1) -RepetitionDuration (New-TimeSpan -Days 3650)
    $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable -Hidden
    $task = New-ScheduledTask -Action $action -Principal $principal -Trigger $trigger -Settings $settings
    Register-ScheduledTask -TaskName $taskName -InputObject $task -Force -ErrorAction Stop | Out-Null
    Start-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
    Write-Host "Guard task registered: $taskName (every 1 minute)"
} catch {
    Write-Host "Guard task registration failed: $_"
}
Write-Host "Done."