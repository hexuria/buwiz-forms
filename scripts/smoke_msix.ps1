[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$Package,

    [string]$Certificate,

    [int]$TimeoutSeconds = 60,

    [string]$LogPath,

    [string]$StderrLogPath,

    [switch]$RequireActivation
)

$ErrorActionPreference = 'Stop'
$logLines = [Collections.Generic.List[string]]::new()
$failureMessage = $null
$activeStage = 'registration'
$registrationResult = 'pending'
$layoutResult = 'pending'
$activationResult = 'pending'
$installedPackage = $null
$importedCertificateThumbprint = $null
$installationAttempted = $false
$refusedExistingRegistration = $false
$layoutProcess = $null
$layoutStderrTask = $null
$layoutStderr = ''
$layoutExitCode = $null
$identityName = 'GOLDCODERSCORP.ebirforms'
$packagePath = $null
$installLocation = $null
$tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("ebirforms-msix-smoke-" + [guid]::NewGuid().ToString('N'))
$layoutPath = Join-Path $tempRoot 'layout'
$scratchLocalAppData = Join-Path $tempRoot 'LOCALAPPDATA'
$scratchDatabase = Join-Path $tempRoot 'bir-smoke-test.db'
$runtimePattern = '(?i)^(vcruntime140(?:_\d+)?|msvcp140(?:_\w+)?|concrt140)\.dll$'

if ([string]::IsNullOrWhiteSpace($StderrLogPath) -and
    -not [string]::IsNullOrWhiteSpace($LogPath)) {
    $mainLogPath = [IO.Path]::GetFullPath($LogPath)
    $StderrLogPath = Join-Path (Split-Path -Parent $mainLogPath) 'smoke-msix-layout-stderr.log'
}

function Write-Log {
    param([Parameter(Mandatory = $true)][string]$Message)

    $logLines.Add($Message)
    Write-Host $Message
}

function Add-DiagnosticOutput {
    param(
        [Parameter(Mandatory = $true)][string]$Title,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )

    Write-Log "=== $Title ==="
    try {
        $output = & $Command 2>&1 | Out-String -Width 240
        if ([string]::IsNullOrWhiteSpace($output)) {
            Write-Log "(no output)"
        } else {
            Write-Log $output.TrimEnd()
        }
    } catch {
        Write-Log "Diagnostic command failed: $($_.Exception.Message)"
    }
}

function Write-FailureDiagnostics {
    Add-DiagnosticOutput 'Get-AppxLastError' { Get-AppxLastError }
    Add-DiagnosticOutput 'Get-AppxLog' { Get-AppxLog -All }

    foreach ($logName in @(
        'Microsoft-Windows-AppModel-Runtime/Admin',
        'Microsoft-Windows-AppXDeploymentServer/Operational'
    )) {
        Add-DiagnosticOutput "Last 50 events: $logName" {
            Get-WinEvent -LogName $logName -MaxEvents 50 |
                Format-List TimeCreated, Id, LevelDisplayName, ProviderName, Message
        }
    }
    Add-DiagnosticOutput 'Last 50 Application errors' {
        Get-WinEvent -FilterHashtable @{ LogName = 'Application'; Level = 2 } -MaxEvents 50 |
            Format-List TimeCreated, Id, LevelDisplayName, ProviderName, Message
    }
    Add-DiagnosticOutput 'WER entries for bir.exe' {
        $werRoots = @(
            (Join-Path $env:ProgramData 'Microsoft\Windows\WER\ReportArchive'),
            (Join-Path $env:ProgramData 'Microsoft\Windows\WER\ReportQueue')
        )
        Get-ChildItem -Path $werRoots -Filter Report.wer -Recurse -File `
            -ErrorAction SilentlyContinue |
            Select-String -Pattern 'bir\.exe' |
            Select-Object -Last 100
    }
}

function Write-LayoutStderr {
    if ([string]::IsNullOrWhiteSpace($StderrLogPath)) {
        return
    }
    $path = [IO.Path]::GetFullPath($StderrLogPath)
    $directory = Split-Path -Parent $path
    if (-not (Test-Path $directory)) {
        New-Item -ItemType Directory -Force -Path $directory | Out-Null
    }
    [IO.File]::WriteAllText($path, $layoutStderr, [Text.UTF8Encoding]::new($false))
    Write-Log "Layout stderr log: $path"
}

function Stop-LayoutProcess {
    param([Parameter(Mandatory = $true)][System.Diagnostics.Process]$Process)

    $Process.Refresh()
    if (-not $Process.HasExited) {
        $Process.Kill()
        $Process.WaitForExit()
    }
    $Process.Refresh()
    return [int64]$Process.ExitCode
}

function Get-LayoutExitHex {
    if ($null -eq $layoutExitCode) {
        return '0x00000000 (process not started)'
    }
    $unsignedExitCode = [int64]$layoutExitCode
    if ($unsignedExitCode -lt 0) {
        $unsignedExitCode += 4294967296
    }
    return ('0x{0:X8}' -f $unsignedExitCode)
}

function Write-LayoutFailureDetails {
    if ($layoutProcess) {
        try {
            $layoutProcess.Refresh()
            if (-not $layoutProcess.HasExited) {
                $script:layoutExitCode = 259
                $layoutProcess.Kill()
                $layoutProcess.WaitForExit()
            } else {
                $script:layoutExitCode = [int64]$layoutProcess.ExitCode
            }
            if ($layoutStderrTask) {
                $script:layoutStderr = $layoutStderrTask.GetAwaiter().GetResult()
            }
        } catch {
            Write-Log "Could not collect layout process status: $($_.Exception.Message)"
        }
    }

    Write-Log "Layout process exit code: $(Get-LayoutExitHex)"
    Write-Log '=== Layout stderr tail (last 100 lines) ==='
    if ([string]::IsNullOrWhiteSpace($layoutStderr)) {
        Write-Log '(empty)'
    } else {
        @($layoutStderr -split "\r?\n" | Select-Object -Last 100) |
            ForEach-Object { Write-Log $_ }
    }
    Write-LayoutStderr
}

try {
    $packagePath = (Resolve-Path $Package).Path
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [System.IO.Compression.ZipFile]::OpenRead($packagePath)
    try {
        $manifestEntry = $archive.GetEntry('AppxManifest.xml')
        if ($manifestEntry) {
            $reader = [IO.StreamReader]::new($manifestEntry.Open())
            try {
                [xml]$manifest = $reader.ReadToEnd()
            } finally {
                $reader.Dispose()
            }
            $identityNode = $manifest.SelectSingleNode(
                '/*[local-name()="Package"]/*[local-name()="Identity"]'
            )
            if ($identityNode -and $identityNode.GetAttribute('Name')) {
                $identityName = $identityNode.GetAttribute('Name')
            }
        }
    } finally {
        $archive.Dispose()
    }
    Write-Log "Package: $packagePath"
    Write-Log "Identity: $identityName"

    $existingPackage = @(Get-AppxPackage -Name $identityName -ErrorAction SilentlyContinue)
    if ($existingPackage.Count -gt 0) {
        $refusedExistingRegistration = $true
        throw "Refusing to smoke-test because $identityName is already registered."
    }

    if (-not [string]::IsNullOrWhiteSpace($Certificate)) {
        $certificatePath = (Resolve-Path $Certificate).Path
        $certificateObject = [Security.Cryptography.X509Certificates.X509Certificate2]::new(
            $certificatePath
        )
        $certificateStore = 'Cert:\LocalMachine\TrustedPeople'
        $existingTrustedCertificate = Get-ChildItem $certificateStore |
            Where-Object { $_.Thumbprint -eq $certificateObject.Thumbprint } |
            Select-Object -First 1
        if (-not $existingTrustedCertificate) {
            Import-Certificate -FilePath $certificatePath `
                -CertStoreLocation $certificateStore | Out-Null
            $importedCertificateThumbprint = $certificateObject.Thumbprint
        }
        Write-Log "Trusted test certificate: $($certificateObject.Thumbprint)"
        $certificateObject.Dispose()
    }

    $installationAttempted = $true
    Add-AppxPackage -Path $packagePath
    $installedPackage = Get-AppxPackage -Name $identityName |
        Select-Object -First 1
    if (-not $installedPackage) {
        throw "Add-AppxPackage completed but $identityName is not registered."
    }
    if ([string]$installedPackage.Status -ne 'Ok') {
        throw "Package registration status is $($installedPackage.Status), expected Ok."
    }
    $installLocation = [IO.Path]::GetFullPath($installedPackage.InstallLocation).TrimEnd('\') + '\'
    if (-not (Test-Path (Join-Path $installLocation 'bir.exe') -PathType Leaf)) {
        throw "Registered package does not contain bir.exe in $installLocation"
    }
    foreach ($shellResource in @('app\main.js', 'print-overrides')) {
        if (-not (Test-Path (Join-Path $installLocation $shellResource))) {
            throw "Registered package does not contain $shellResource in $installLocation"
        }
    }
    $familyName = $installedPackage.PackageFamilyName

    New-Item -ItemType Directory -Force -Path $tempRoot, $scratchLocalAppData | Out-Null
    $sdkBins = @(
        Get-ChildItem -Path 'C:\Program Files (x86)\Windows Kits\10\bin\10.*\x64' `
            -Directory -ErrorAction SilentlyContinue |
            Sort-Object { [version](Split-Path $_.Parent.FullName -Leaf) } -Descending
    )
    if ($sdkBins.Count -eq 0) {
        throw 'Windows SDK bin\10.*\x64 directory was not found.'
    }
    $makeAppx = Join-Path $sdkBins[0].FullName 'MakeAppx.exe'
    if (-not (Test-Path $makeAppx -PathType Leaf)) {
        throw "MakeAppx.exe was not found in the newest Windows SDK: $makeAppx"
    }
    & $makeAppx unpack /p $packagePath /d $layoutPath /o
    $unpackExitCode = $LASTEXITCODE
    if ($unpackExitCode -ne 0) {
        throw "MakeAppx unpack failed with exit code $unpackExitCode."
    }

    $layoutExe = Join-Path $layoutPath 'bir.exe'
    if (-not (Test-Path $layoutExe -PathType Leaf)) {
        throw 'Unpacked MSIX layout does not contain bir.exe.'
    }
    if (-not (Test-Path (Join-Path $layoutPath 'app\main.js') -PathType Leaf)) {
        throw 'Unpacked MSIX layout does not contain app\main.js.'
    }
    $runtimeFiles = @(
        Get-ChildItem -Path $layoutPath -File |
            Where-Object { $_.Name -match $runtimePattern } |
            Sort-Object Name
    )
    if ($runtimeFiles.Count -eq 0) {
        throw 'Unpacked MSIX layout contains no supported VC runtime DLLs.'
    }
    foreach ($runtimeFile in $runtimeFiles) {
        if (-not (Test-Path (Join-Path $installLocation $runtimeFile.Name) -PathType Leaf)) {
            throw "Registered package is missing packaged VC runtime $($runtimeFile.Name)."
        }
    }
    Write-Log "Registration: Status=Ok; bir.exe and $($runtimeFiles.Count) packaged VC runtime DLL(s) are present."
    $registrationResult = 'passed'

    $activeStage = 'layout-run'
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $layoutExe
    $startInfo.WorkingDirectory = Join-Path $env:WINDIR 'System32'
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardError = $true
    foreach ($variableName in @($startInfo.EnvironmentVariables.Keys)) {
        if ($variableName -like 'BIR_SFTP_*') {
            $startInfo.EnvironmentVariables.Remove($variableName)
        }
    }
    $startInfo.EnvironmentVariables['LOCALAPPDATA'] = $scratchLocalAppData
    $startInfo.EnvironmentVariables['BIR_DATABASE_PATH'] = $scratchDatabase
    $startInfo.EnvironmentVariables['BIR_SFTP_DRY_RUN'] = '1'

    $layoutProcess = [System.Diagnostics.Process]::new()
    $layoutProcess.StartInfo = $startInfo
    if (-not $layoutProcess.Start()) {
        throw 'Could not start bir.exe from the unpacked MSIX layout.'
    }
    $layoutStderrTask = $layoutProcess.StandardError.ReadToEndAsync()
    $layoutDeadline = (Get-Date).AddSeconds($TimeoutSeconds)
    $layoutWindow = [IntPtr]::Zero
    while ((Get-Date) -lt $layoutDeadline) {
        $layoutProcess.Refresh()
        if ($layoutProcess.HasExited) {
            $layoutExitCode = [int64]$layoutProcess.ExitCode
            $layoutStderr = $layoutStderrTask.GetAwaiter().GetResult()
            throw "Unpacked bir.exe exited before showing a window (exit code $(Get-LayoutExitHex))."
        }
        $layoutWindow = $layoutProcess.MainWindowHandle
        if ($layoutWindow -ne [IntPtr]::Zero) {
            break
        }
        Start-Sleep -Seconds 1
    }
    if ($layoutWindow -eq [IntPtr]::Zero) {
        throw "Unpacked bir.exe did not show a window within $TimeoutSeconds seconds."
    }
    $layoutProcessId = $layoutProcess.Id
    Write-Log "Layout window started: PID $layoutProcessId, HWND $layoutWindow"
    Start-Sleep -Seconds 10
    $layoutProcess.Refresh()
    if ($layoutProcess.HasExited) {
        $layoutExitCode = [int64]$layoutProcess.ExitCode
        throw "Unpacked bir.exe PID $layoutProcessId exited within 10 seconds (exit code $(Get-LayoutExitHex))."
    }
    $layoutResult = 'passed'
    $layoutExitCode = Stop-LayoutProcess -Process $layoutProcess
    $layoutStderr = $layoutStderrTask.GetAwaiter().GetResult()
    $layoutProcess.Dispose()
    $layoutProcess = $null
    Write-Log "Layout run: window visible and PID $layoutProcessId persisted for 10 seconds."
    Write-LayoutStderr

    $activeStage = 'packaged-activation'
    $uacPolicy = Get-ItemProperty `
        -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System' `
        -Name EnableLUA -ErrorAction Stop
    $enableLua = [int]$uacPolicy.EnableLUA
    if ($enableLua -eq 0) {
        $activationResult = 'skipped (UAC is disabled (EnableLUA=0))'
        $warning = '::warning::Packaged MSIX activation not verifiable on this host: UAC is disabled (EnableLUA=0); registration and layout launch were verified'
        Write-Log $warning
        if ($RequireActivation) {
            throw 'Packaged MSIX activation is required, but unavailable because UAC is disabled (EnableLUA=0).'
        }
    } elseif ($enableLua -eq 1) {
        Start-Process "shell:AppsFolder\$familyName!eBIRForms"
        $activationDeadline = (Get-Date).AddSeconds($TimeoutSeconds)
        $activationProcessId = $null
        $activationWindow = [IntPtr]::Zero
        while ((Get-Date) -lt $activationDeadline) {
            foreach ($candidate in @(Get-Process -Name bir -ErrorAction SilentlyContinue)) {
                try {
                    $candidatePath = $candidate.Path
                } catch {
                    $candidatePath = $null
                }
                if ($candidatePath -and $candidatePath.StartsWith(
                    $installLocation,
                    [StringComparison]::OrdinalIgnoreCase
                )) {
                    $activationProcessId = $candidate.Id
                    $activationWindow = $candidate.MainWindowHandle
                    if ($activationWindow -ne [IntPtr]::Zero) {
                        break
                    }
                }
            }
            if ($activationWindow -ne [IntPtr]::Zero) {
                break
            }
            Start-Sleep -Seconds 1
        }
        if ($activationWindow -eq [IntPtr]::Zero) {
            throw "Packaged activation did not show a window within $TimeoutSeconds seconds."
        }
        Write-Log "Packaged activation window started: PID $activationProcessId, HWND $activationWindow"
        Start-Sleep -Seconds 10
        $persistentActivation = Get-Process -Id $activationProcessId -ErrorAction SilentlyContinue
        if (-not $persistentActivation) {
            throw "Packaged activation PID $activationProcessId exited within 10 seconds."
        }
        $activationResult = 'passed'
        $processId = $activationProcessId
        Write-Log 'Packaged activation: visible window and process persisted for 10 seconds.'
    } else {
        throw "Unexpected UAC EnableLUA value: $enableLua"
    }
} catch {
    if (-not $failureMessage) {
        $failureMessage = $_.Exception.Message
    }
    if ($activeStage -eq 'layout-run') {
        Write-LayoutFailureDetails
    }
    Write-Log "FAIL: $failureMessage"
    Write-FailureDiagnostics
} finally {
    if ($layoutProcess) {
        try {
            if (-not $layoutProcess.HasExited) {
                $layoutProcess.Kill()
                $layoutProcess.WaitForExit()
            }
            if ($layoutStderrTask) {
                $layoutStderr = $layoutStderrTask.GetAwaiter().GetResult()
            }
            $layoutProcess.Dispose()
        } catch {
            if (-not $failureMessage) {
                $failureMessage = $_.Exception.Message
            }
            Write-Log "Cleanup failed to stop layout process: $($_.Exception.Message)"
        }
        $layoutProcess = $null
    }
    if ($processId) {
        Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
    }
    if ($installedPackage) {
        try {
            Remove-AppxPackage -Package $installedPackage.PackageFullName
            Write-Log "Removed smoke-installed package: $($installedPackage.PackageFullName)"
        } catch {
            if (-not $failureMessage) {
                $failureMessage = $_.Exception.Message
            }
            Write-Log "Cleanup failed to remove smoke-installed package: $($_.Exception.Message)"
        }
    }
    if (
        -not $installedPackage -and
        $installationAttempted -and
        -not $refusedExistingRegistration
    ) {
        try {
            $partialRegistration = Get-AppxPackage -Name $identityName -ErrorAction SilentlyContinue |
                Select-Object -First 1
            if ($partialRegistration) {
                Remove-AppxPackage -Package $partialRegistration.PackageFullName
                Write-Log "Removed partially registered smoke package: $($partialRegistration.PackageFullName)"
            }
        } catch {
            if (-not $failureMessage) {
                $failureMessage = $_.Exception.Message
            }
            Write-Log "Cleanup failed to remove partial smoke package: $($_.Exception.Message)"
        }
    }
    if ($importedCertificateThumbprint) {
        try {
            Get-ChildItem 'Cert:\LocalMachine\TrustedPeople' |
                Where-Object { $_.Thumbprint -eq $importedCertificateThumbprint } |
                Remove-Item -Force
            Write-Log "Removed imported test certificate: $importedCertificateThumbprint"
        } catch {
            if (-not $failureMessage) {
                $failureMessage = $_.Exception.Message
            }
            Write-Log "Cleanup failed to remove imported test certificate: $($_.Exception.Message)"
        }
    }
    if (Test-Path $tempRoot) {
        try {
            Remove-Item -LiteralPath $tempRoot -Recurse -Force
            Write-Log 'Removed smoke temporary directory.'
        } catch {
            if (-not $failureMessage) {
                $failureMessage = $_.Exception.Message
            }
            Write-Log "Cleanup failed to remove smoke temporary directory: $($_.Exception.Message)"
        }
    }
}

if ($failureMessage) {
    foreach ($stageName in @('registration', 'layout-run', 'packaged-activation')) {
        if ($stageName -eq $activeStage -and $stageName -eq 'packaged-activation' -and
            $activationResult -like 'skipped (*RequireActivation*)') {
            continue
        }
        if ($stageName -eq $activeStage -and $stageName -eq 'registration' -and
            $registrationResult -eq 'pending') {
            $registrationResult = "failed ($failureMessage)"
        } elseif ($stageName -eq $activeStage -and $stageName -eq 'layout-run' -and
            $layoutResult -eq 'pending') {
            $layoutResult = "failed ($failureMessage)"
        } elseif ($stageName -eq $activeStage -and $stageName -eq 'packaged-activation' -and
            $activationResult -eq 'pending') {
            $activationResult = "failed ($failureMessage)"
        } elseif ($stageName -ne $activeStage) {
            $stageVariable = switch ($stageName) {
                'registration' { $registrationResult }
                'layout-run' { $layoutResult }
                'packaged-activation' { $activationResult }
            }
            if ($stageVariable -eq 'pending') {
                $reason = if ($stageName -eq 'packaged-activation' -and
                    $activeStage -ne 'packaged-activation') {
                    'not reached after an earlier failure'
                } else {
                    'not reached after an earlier failure'
                }
                switch ($stageName) {
                    'registration' { $registrationResult = "skipped ($reason)" }
                    'layout-run' { $layoutResult = "skipped ($reason)" }
                    'packaged-activation' { $activationResult = "skipped ($reason)" }
                }
            }
        }
    }
}

Write-Log "MSIX smoke summary: registration=$registrationResult; layout-run=$layoutResult; packaged-activation=$activationResult"
if (-not [string]::IsNullOrWhiteSpace($StderrLogPath) -and
    -not (Test-Path ([IO.Path]::GetFullPath($StderrLogPath)))) {
    Write-LayoutStderr
}
if (-not [string]::IsNullOrWhiteSpace($LogPath)) {
    $logPath = [IO.Path]::GetFullPath($LogPath)
    $logDirectory = Split-Path -Parent $logPath
    if (-not (Test-Path $logDirectory)) {
        New-Item -ItemType Directory -Force -Path $logDirectory | Out-Null
    }
    [IO.File]::WriteAllLines($logPath, $logLines, [Text.UTF8Encoding]::new($false))
}
if ($failureMessage) {
    exit 1
}
