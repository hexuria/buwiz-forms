# Runs the accounts-and-lock Windows probes and writes every result to $Out (JSON lines plus a transcript).
# Run from an elevated PowerShell on a test machine. It sets the clock forward by 3 hours within this boot and puts it
# back, with the Windows Time service stopped meanwhile and resynced at the end.
param(
    [string]$Probe = ".\target\release\accounts-lock-windows-probes.exe",
    [string]$Data = "D:\probe-data",
    [string]$Out = ".\probe-results"
)
$ErrorActionPreference = "Continue"
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$Log = Join-Path $Out "results.jsonl"
Remove-Item $Log -ErrorAction SilentlyContinue

function Note($what) { "{`"note`":`"$what`"}" | Tee-Object -FilePath $Log -Append }
function Run([string[]]$ProbeArgs) { & $Probe @ProbeArgs 2>&1 | Tee-Object -FilePath $Log -Append }
function BootIdNow {
    $value = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Memory Management\PrefetchParameters' -Name BootId -ErrorAction SilentlyContinue).BootId
    "{`"probe`":`"boot`",`"registry_BootId`":`"$value`",`"wall`":`"$((Get-Date).ToString('o'))`"}" | Tee-Object -FilePath $Log -Append
}

Note "machine: $((Get-CimInstance Win32_OperatingSystem).Caption) build $((Get-CimInstance Win32_OperatingSystem).BuildNumber)"
Note "filesystem of ${Data}: $((Get-Volume -FilePath (Split-Path $Data -Qualifier)).FileSystem)"

# 1. Folder fsync and the step-4 move, on NTFS ($Data) and in this user's LocalAppData (where the app's data lives).
foreach ($dir in @($Data, (Join-Path $env:LOCALAPPDATA "accounts-lock-probe"))) {
    Note "fsync and move in $dir"
    Run @("fsync", (Join-Path $dir "fsync"))
    Run @("move", (Join-Path $dir "move"))
}

# 2. The owner-lock holder: two paths, the same path again, another process refused, then released.
Note "holder"
Run @("holder", (Join-Path $Data "a\bir_data.db.owner.lock"), (Join-Path $Data "b\bir_data.db.owner.lock"))

# 3. Same boot, clock set forward 3 hours and back.
Note "clock: before"
BootIdNow; Run @("boot")
Stop-Service w32time -ErrorAction SilentlyContinue
Set-Date -Date (Get-Date).AddHours(3) | Out-Null
Note "clock: set forward 3 h"
BootIdNow; Run @("boot")
Start-Sleep -Seconds 10
Note "clock: 10 s later"
BootIdNow; Run @("boot")
Set-Date -Date (Get-Date).AddHours(-3) | Out-Null
Start-Service w32time -ErrorAction SilentlyContinue
w32tm /resync /force | Out-Null
Note "clock: set back and resynced"
BootIdNow; Run @("boot")

Note "done"
