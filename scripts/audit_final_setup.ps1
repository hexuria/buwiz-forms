[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Installer,
    [string] $Python = 'python',
    [switch] $Nsh
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
                        if ($name -ieq 'eBIRForms_is1' -or
                            ($displayName -and $displayName -ilike 'eBIRForms*')) {
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
$uninstallError = $null
$installSucceeded = $false
try {
    New-Item -ItemType Directory -Path $installRoot -Force | Out-Null
    $arguments = @(
        '/VERYSILENT',
        '/SUPPRESSMSGBOXES',
        '/NORESTART',
        '/NOICONS',
        '/TASKS=""',
        "/DIR=`"$installRoot`""
    )
    if (-not $Nsh) { $arguments += '/CURRENTUSER' }
    $setup = Start-Process -FilePath $Installer -ArgumentList $arguments -Wait -PassThru
    if ($setup.ExitCode -ne 0) {
        throw "Setup.exe failed with exit code $($setup.ExitCode)"
    }
    $installSucceeded = $true
    if ($Nsh) {
        $appRoot = Join-Path $installRoot 'eBIRForms-app'
        foreach ($file in @('eBIRForms.exe', 'nsh/bundle.nshc', 'nsh/app.json')) {
            if (-not (Test-Path -LiteralPath (Join-Path $appRoot $file) -PathType Leaf)) {
                throw "Missing installed NSH file: $file"
            }
        }
        if (-not (Test-Path -LiteralPath (Join-Path $appRoot 'print-overrides') -PathType Container)) {
            throw 'NSH Setup is missing print overrides'
        }
        if (Test-Path -LiteralPath (Join-Path $appRoot 'ebirforms-no-self-update')) {
            throw 'Setup must allow self-update'
        }
        $registration = Find-EbirFormsUninstallEntry
        if ($registration -notlike 'CurrentUser/*') {
            throw "NSH Setup must register its uninstall entry per user, found: $registration"
        }
        # An update can add files that are not in Inno's original uninstall log.
        New-Item -ItemType File -Path (Join-Path $appRoot 'updater-audit-probe') | Out-Null
    } else {
        $appRoot = $installRoot
        $binary = Join-Path $installRoot 'bir.exe'
        $assets = Join-Path $installRoot 'assets'
        if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
            throw "Installed bir.exe was not found at $binary"
        }
        if (-not (Test-Path -LiteralPath $assets -PathType Container)) {
            throw "Installed assets directory was not found at $assets"
        }
    }
    $auditScript = Join-Path $PSScriptRoot 'audit_no_legacy.py'
    & $Python $auditScript --package-root $appRoot
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
        try {
            $uninstall = Start-Process -FilePath $uninstaller.FullName -ArgumentList @(
                '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART'
            ) -Wait -PassThru
            if ($uninstall.ExitCode -ne 0) {
                $uninstallError = "Uninstaller failed with exit code $($uninstall.ExitCode)"
            } else {
                $remaining = Find-EbirFormsUninstallEntry
                if ($remaining) {
                    $uninstallError = "Uninstall registration remains at $remaining"
                } elseif ($Nsh -and (Test-Path -LiteralPath (Join-Path $installRoot 'eBIRForms-app'))) {
                    $uninstallError = 'Uninstall left the updater-managed app directory behind'
                } else {
                    $uninstallSucceeded = $true
                }
            }
        } catch {
            $uninstallError = "Could not run generated uninstaller: $($_.Exception.Message)"
        }
    } elseif ($installSucceeded) {
        $uninstallError = 'No generated uninstaller found'
    }
    if ($uninstallSucceeded -and (Test-Path -LiteralPath $installRoot)) {
        Remove-Item -LiteralPath $installRoot -Recurse -Force
    } elseif (Test-Path -LiteralPath $installRoot) {
        Write-Host "Final Setup audit diagnostics retained at $installRoot"
    }
    if ($uninstallError) {
        [Console]::Error.WriteLine("$uninstallError; retaining $installRoot")
        throw "$uninstallError; diagnostics retained at $installRoot"
    }
}
