[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Installer,
    [string] $Python = 'python'
)

$ErrorActionPreference = 'Stop'

function Find-EbirFormsUninstallEntry {
    $uninstallPath = 'Software\Microsoft\Windows\CurrentVersion\Uninstall'
    foreach ($hive in @(
            [Microsoft.Win32.RegistryHive]::CurrentUser,
            [Microsoft.Win32.RegistryHive]::LocalMachine
        )) {
        foreach ($view in @(
                [Microsoft.Win32.RegistryView]::Registry64,
                [Microsoft.Win32.RegistryView]::Registry32
            )) {
            $base = $null
            $uninstall = $null
            try {
                $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($hive, $view)
                $uninstall = $base.OpenSubKey($uninstallPath)
                if ($null -eq $uninstall) { continue }
                foreach ($name in $uninstall.GetSubKeyNames()) {
                    $entry = $uninstall.OpenSubKey($name)
                    try {
                        if ($null -eq $entry) { continue }
                        $displayName = [string]$entry.GetValue('DisplayName')
                        if ($displayName -and $displayName -ilike 'eBIRForms*') {
                            return "$hive/$view/$name"
                        }
                    } finally {
                        if ($null -ne $entry) { $entry.Dispose() }
                    }
                }
            } finally {
                if ($null -ne $uninstall) { $uninstall.Dispose() }
                if ($null -ne $base) { $base.Dispose() }
            }
        }
    }
    return $null
}

if (-not (Test-Path -LiteralPath $Installer -PathType Leaf)) {
    throw "Installer was not found: $Installer"
}
$Installer = (Resolve-Path -LiteralPath $Installer).Path
$existing = Find-EbirFormsUninstallEntry
if ($existing) {
    throw "Refusing final Setup audit because an eBIRForms uninstall entry exists at $existing"
}

$installRoot = Join-Path ([IO.Path]::GetTempPath()) (
    'ebirforms-final-setup-' + [Guid]::NewGuid().ToString('N')
)
$uninstaller = $null
$uninstallSucceeded = $false
$installSucceeded = $false
try {
    New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
    $arguments = @(
        '/VERYSILENT',
        '/SUPPRESSMSGBOXES',
        '/NORESTART',
        '/CURRENTUSER',
        '/NOICONS',
        '/TASKS=""',
        "/DIR=`"$installRoot`""
    )
    $setup = Start-Process -FilePath $Installer -ArgumentList $arguments -Wait -PassThru
    if ($setup.ExitCode -ne 0) {
        throw "Setup.exe failed with exit code $($setup.ExitCode)"
    }
    $installSucceeded = $true
    $binary = Join-Path $installRoot 'bir.exe'
    $assets = Join-Path $installRoot 'assets'
    if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
        throw "Installed bir.exe was not found at $binary"
    }
    if (-not (Test-Path -LiteralPath $assets -PathType Container)) {
        throw "Installed assets directory was not found at $assets"
    }
    $auditScript = Join-Path $PSScriptRoot 'audit_no_legacy.py'
    & $Python $auditScript --package-root $installRoot
    $auditExitCode = $LASTEXITCODE
    if ($auditExitCode -ne 0) {
        throw "Installed-tree legacy audit failed with exit code $auditExitCode"
    }
}
finally {
    if (Test-Path -LiteralPath $installRoot -PathType Container) {
        $uninstaller = Get-ChildItem -LiteralPath $installRoot -Filter 'unins*.exe' -File |
            Select-Object -First 1
    }
    if ($null -ne $uninstaller) {
        $uninstall = Start-Process -FilePath $uninstaller.FullName -ArgumentList @(
            '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART'
        ) -Wait -PassThru
        if ($uninstall.ExitCode -eq 0) {
            $uninstallSucceeded = $true
        } else {
            [Console]::Error.WriteLine(
                "Uninstaller failed with exit code $($uninstall.ExitCode); retaining $installRoot"
            )
            throw "Final Setup uninstall failed; diagnostics retained at $installRoot"
        }
    } elseif ($installSucceeded) {
        [Console]::Error.WriteLine("No generated uninstaller found; retaining $installRoot")
        throw "Final Setup uninstaller is missing; diagnostics retained at $installRoot"
    }
    if ($uninstallSucceeded) {
        Remove-Item -LiteralPath $installRoot -Recurse -Force
    } elseif (Test-Path -LiteralPath $installRoot) {
        Write-Host "Final Setup audit diagnostics retained at $installRoot"
    }
}
