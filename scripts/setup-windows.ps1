<#
.SYNOPSIS
    One-shot Windows dev environment setup for E-BIRForms (buwiz-forms).

.DESCRIPTION
    Idempotent -  safe to re-run; installed pieces are skipped. Installs:
      * Chocolatey (if missing)
      * OpenSSL (required by libsqlite3-sys bundled-sqlcipher)
      * just (task runner), Strawberry Perl + NASM (required by vendored
        openssl-src builds), PowerShell 7 (justfile windows-shell is pwsh)
      * Inno Setup and Python (Windows packaging)
      * Visual Studio 2022 Build Tools: MSVC C++ workload + Windows 11 SDK
      * Rust toolchain via rustup (stable; rust-toolchain.toml adds
        rustfmt + clippy components)
      * gpui-agent CLI (drives the app when built with --features agent)
      * User env vars OPENSSL_DIR / OPENSSL_LIB_DIR so cargo finds OpenSSL

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1
    # then, in a NEW PowerShell window:  just run
#>

$ErrorActionPreference = 'Stop'

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Exit-Native($message) {
    $code = $LASTEXITCODE
    [Console]::Error.WriteLine($message)
    exit $code
}

# Self-elevate: installs write to Program Files and the machine PATH, so this
# needs admin rights. Relaunching keeps the documented command working from a
# regular PowerShell window (shows a UAC prompt).
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Step "Requesting administrator rights (UAC prompt)"
    $elevated = Start-Process -FilePath 'powershell' -Verb RunAs -Wait -PassThru -ArgumentList `
        '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`""
    exit $elevated.ExitCode
}

# 1. Chocolatey
if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    # Pinned-version package install (Chocolatey's documented offline method):
    # downloads an inspectable artifact instead of executing a live remote
    # script as Administrator.
    $chocoVersion = '2.7.4'
    Write-Step "Installing Chocolatey $chocoVersion"
    $nupkg = "$env:TEMP\chocolatey.$chocoVersion.nupkg"
    $nupkgDir = "$env:TEMP\chocolatey-install"
    curl.exe -sSfL -o $nupkg "https://community.chocolatey.org/api/v2/package/chocolatey/$chocoVersion"
    Expand-Archive -Path $nupkg -DestinationPath $nupkgDir -Force
    & "$nupkgDir\tools\chocolateyInstall.ps1"
    if ($LASTEXITCODE -ne 0) { Exit-Native "Chocolatey install failed" }
    $env:Path = "$env:ProgramData\chocolatey\bin;$env:Path"
} else {
    Write-Step "Chocolatey already installed -  skipping"
}

# 2. System packages (choco install is idempotent)
Write-Step "Installing packages: openssl, just, strawberryperl, nasm, powershell-core, innosetup, python"
choco install openssl just strawberryperl nasm powershell-core innosetup python -y --no-progress
$chocoExitCode = $LASTEXITCODE
if ($chocoExitCode -eq 3010) {
    Write-Warning "Chocolatey reported reboot required (3010); continuing because setup is otherwise successful."
} elseif ($chocoExitCode -ne 0) {
    [Console]::Error.WriteLine("choco install failed (exit code $chocoExitCode)")
    exit $chocoExitCode
}

# 3. Visual Studio Build Tools (MSVC C++ workload + Windows 11 SDK)
# The compiler alone is not enough: linking needs a Windows SDK, so an existing
# install missing the SDK component still gets it added.
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vsInstaller = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vs_installer.exe"
$vsComponent = 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'
$sdkComponent = 'Microsoft.VisualStudio.Component.Windows11SDK.22621'
$vcPath = $null
$sdkPath = $null
if (Test-Path $vswhere) {
    $vcPath = & $vswhere -products * -requires $vsComponent -property installationPath | Select-Object -First 1
    $sdkPath = & $vswhere -products * -requires $sdkComponent -property installationPath | Select-Object -First 1
}
if ($vcPath -and $sdkPath) {
    Write-Step "MSVC C++ build tools + Windows 11 SDK already installed -  skipping"
} elseif ($vcPath -and -not $sdkPath) {
    Write-Step "Adding Windows 11 SDK 22621 to existing VS install"
    $p = Start-Process -FilePath $vsInstaller -Wait -PassThru -ArgumentList `
        'modify','--installPath',"`"$vcPath`"",'--add',$sdkComponent,
        '--quiet','--wait','--norestart'
    if ($p.ExitCode -eq 3010) {
        Write-Warning "Visual Studio modify reported reboot required (3010); continuing."
    } elseif ($p.ExitCode -ne 0) {
        $vsExitCode = $p.ExitCode
        [Console]::Error.WriteLine("VS modify failed (exit $vsExitCode)")
        exit $vsExitCode
    }
} else {
    Write-Step "Installing VS 2022 Build Tools (VCTools + Windows 11 SDK 22621)"
    $bootstrapper = "$env:TEMP\vs_buildtools.exe"
    curl.exe -sSfL -o $bootstrapper https://aka.ms/vs/17/release/vs_buildtools.exe
    $p = Start-Process -FilePath $bootstrapper -Wait -PassThru -ArgumentList `
        '--quiet','--wait','--norestart','--nocache',
        '--add','Microsoft.VisualStudio.Workload.VCTools',
        '--add','Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
        '--add','Microsoft.VisualStudio.Component.Windows11SDK.22621'
    if ($p.ExitCode -eq 3010) {
        Write-Warning "Visual Studio install reported reboot required (3010); continuing."
    } elseif ($p.ExitCode -ne 0) {
        $vsExitCode = $p.ExitCode
        [Console]::Error.WriteLine("VS Build Tools install failed (exit $vsExitCode)")
        exit $vsExitCode
    }
}

# 4. Rust via rustup (stable)
$cargoBin = "$env:USERPROFILE\.cargo\bin"
if (-not (Test-Path "$cargoBin\cargo.exe") -and -not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Step "Installing Rust (rustup, stable)"
    $rustupInit = "$env:TEMP\rustup-init.exe"
    curl.exe -sSfL -o $rustupInit https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
    & $rustupInit -y --default-toolchain stable --profile minimal
    if ($LASTEXITCODE -ne 0) { Exit-Native "rustup install failed" }
} else {
    Write-Step "Rust already installed -  skipping"
}

# 5. OPENSSL_DIR / OPENSSL_LIB_DIR (libsqlite3-sys bundled-sqlcipher fails without them)
$opensslDir = $null
foreach ($candidate in 'C:\Program Files\OpenSSL-Win64', 'C:\Program Files\OpenSSL') {
    if (Test-Path "$candidate\include") { $opensslDir = $candidate; break }
}
if (-not $opensslDir) { [Console]::Error.WriteLine("OpenSSL include directory not found after choco install"); exit 1 }

$libFile = Get-ChildItem -Path $opensslDir -Filter '*crypto*.lib' -Recurse |
    Where-Object { $_.FullName -match 'VC.x64.MD\b' } | Select-Object -First 1
if (-not $libFile) { $libFile = Get-ChildItem -Path $opensslDir -Filter '*crypto*.lib' -Recurse | Select-Object -First 1 }
if (-not $libFile) { [Console]::Error.WriteLine("No *crypto*.lib found inside $opensslDir"); exit 1 }

Write-Step "Setting user env: OPENSSL_DIR=$opensslDir, OPENSSL_LIB_DIR=$($libFile.DirectoryName)"
[Environment]::SetEnvironmentVariable('OPENSSL_DIR', $opensslDir, 'User')
[Environment]::SetEnvironmentVariable('OPENSSL_LIB_DIR', $libFile.DirectoryName, 'User')
$env:OPENSSL_DIR = $opensslDir
$env:OPENSSL_LIB_DIR = $libFile.DirectoryName

# Strawberry Perl must precede Git Bash's MSYS perl on PATH for vendored
# openssl-src builds. Persist it at the front of the machine PATH so new
# shells (not just this process) resolve the right perl. NASM goes on PATH too.
$pathDirs = @('C:\Strawberry\perl\bin', 'C:\Strawberry\perl\site\bin', 'C:\Strawberry\c\bin', 'C:\Program Files\NASM')
$machineEntries = ([Environment]::GetEnvironmentVariable('Path','Machine') -split ';') |
    Where-Object { $_ -and ($pathDirs -notcontains $_.TrimEnd('\')) }
[Environment]::SetEnvironmentVariable('Path', ($pathDirs + $machineEntries) -join ';', 'Machine')
$allPathEntries = @($pathDirs + $cargoBin) +
    (([Environment]::GetEnvironmentVariable('Path','Machine') -split ';') +
    ([Environment]::GetEnvironmentVariable('Path','User') -split ';'))
$seenPathEntries = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$env:Path = (($allPathEntries | Where-Object {
    $_ -and $seenPathEntries.Add($_.TrimEnd('\'))
}) -join ';')

# 6. gpui-agent CLI (client for the agent control plane; the app embeds its
# own host, so only the CLI is needed -  todo-headless is the demo's daemon).
# The CLI must match the rev bir-shell pins in the root Cargo.toml: protocol
# skew fails closed, so installing unpinned HEAD can break the handshake.
$manifest = Join-Path (Split-Path $PSScriptRoot -Parent) 'Cargo.toml'
$manifestText = Get-Content -LiteralPath $manifest -Raw
$gpuiRev = $null
if ($manifestText -match '(?s)gpui-agent\s*=\s*\{[^}]*?\brev\s*=\s*"([0-9a-f]{7,})"') {
    $gpuiRev = $matches[1]
}
if (-not $gpuiRev) {
    [Console]::Error.WriteLine("gpui-agent rev pin not found in Cargo.toml")
    exit 1
}
Write-Step "Installing gpui-agent CLI (pinned rev $gpuiRev, matches Cargo.toml)"
& "$cargoBin\cargo.exe" install --git https://github.com/hexuria/gpui-agent --rev $gpuiRev gpui-agent-cli
if ($LASTEXITCODE -ne 0) { Exit-Native "gpui-agent install failed" }

Write-Step @"
Done. Open a NEW PowerShell window, then:  cd <repo> ; just run
Agent control plane: build --features agent, set GPUI_AGENT=1, drive with gpui-agent.
NOTE: Token auth works on Windows at the current pin; set GPUI_AGENT_TOKEN for
secure driving. GPUI_AGENT_INSECURE_NO_TOKEN=1 remains for local-only driving.
"@
