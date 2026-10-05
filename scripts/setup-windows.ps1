#Requires -RunAsAdministrator
<#
.SYNOPSIS
    One-shot Windows dev environment setup for E-BIRForms (buwiz-forms).

.DESCRIPTION
    Idempotent -  safe to re-run; installed pieces are skipped. Installs:
      * Chocolatey (if missing)
      * OpenSSL (required by libsqlite3-sys bundled-sqlcipher)
      * just (task runner), Strawberry Perl + NASM (required by vendored
        openssl-src builds), PowerShell 7 (justfile windows-shell is pwsh)
      * Visual Studio 2022 Build Tools: MSVC C++ workload + Windows 11 SDK
      * Rust toolchain via rustup (stable; rust-toolchain.toml adds
        rustfmt + clippy components)
      * User env vars OPENSSL_DIR / OPENSSL_LIB_DIR so cargo finds OpenSSL

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\setup-windows.ps1
    # then, in a NEW PowerShell window:  just run
#>

$ErrorActionPreference = 'Stop'

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }

# 1. Chocolatey
if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    Write-Step "Installing Chocolatey"
    Set-ExecutionPolicy Bypass -Scope Process -Force
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
    Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
} else {
    Write-Step "Chocolatey already installed -  skipping"
}

# 2. System packages (choco install is idempotent)
Write-Step "Installing packages: openssl, just, strawberryperl, nasm, powershell-core"
choco install openssl just strawberryperl nasm powershell-core -y --no-progress
if ($LASTEXITCODE -ne 0) { Write-Error "choco install failed"; exit $LASTEXITCODE }

# 3. Visual Studio Build Tools (MSVC C++ workload + Windows 11 SDK)
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$hasVCTools = (Test-Path $vswhere) -and (& $vswhere -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath | Select-Object -First 1)
if (-not $hasVCTools) {
    Write-Step "Installing VS 2022 Build Tools (VCTools + Windows 11 SDK 22621)"
    $bootstrapper = "$env:TEMP\vs_buildtools.exe"
    curl.exe -sSfL -o $bootstrapper https://aka.ms/vs/17/release/vs_buildtools.exe
    $p = Start-Process -FilePath $bootstrapper -Wait -PassThru -ArgumentList `
        '--quiet','--wait','--norestart','--nocache',
        '--add','Microsoft.VisualStudio.Workload.VCTools',
        '--add','Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
        '--add','Microsoft.VisualStudio.Component.Windows11SDK.22621'
    if ($p.ExitCode -ne 0) { Write-Error "VS Build Tools install failed (exit $($p.ExitCode))"; exit $p.ExitCode }
} else {
    Write-Step "MSVC C++ build tools already installed -  skipping"
}

# 4. Rust via rustup (stable)
$cargoBin = "$env:USERPROFILE\.cargo\bin"
if (-not (Test-Path "$cargoBin\cargo.exe") -and -not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Step "Installing Rust (rustup, stable)"
    $rustupInit = "$env:TEMP\rustup-init.exe"
    curl.exe -sSfL -o $rustupInit https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
    & $rustupInit -y --default-toolchain stable --profile minimal
    if ($LASTEXITCODE -ne 0) { Write-Error "rustup install failed"; exit $LASTEXITCODE }
} else {
    Write-Step "Rust already installed -  skipping"
}

# 5. OPENSSL_DIR / OPENSSL_LIB_DIR (libsqlite3-sys bundled-sqlcipher fails without them)
$opensslDir = $null
foreach ($candidate in 'C:\Program Files\OpenSSL-Win64', 'C:\Program Files\OpenSSL') {
    if (Test-Path "$candidate\include") { $opensslDir = $candidate; break }
}
if (-not $opensslDir) { Write-Error "OpenSSL include directory not found after choco install"; exit 1 }

$libFile = Get-ChildItem -Path $opensslDir -Filter '*crypto*.lib' -Recurse |
    Where-Object { $_.FullName -match 'VC.x64.MD\b' } | Select-Object -First 1
if (-not $libFile) { $libFile = Get-ChildItem -Path $opensslDir -Filter '*crypto*.lib' -Recurse | Select-Object -First 1 }
if (-not $libFile) { Write-Error "No *crypto*.lib found inside $opensslDir"; exit 1 }

Write-Step "Setting user env: OPENSSL_DIR=$opensslDir, OPENSSL_LIB_DIR=$($libFile.DirectoryName)"
[Environment]::SetEnvironmentVariable('OPENSSL_DIR', $opensslDir, 'User')
[Environment]::SetEnvironmentVariable('OPENSSL_LIB_DIR', $libFile.DirectoryName, 'User')

# Strawberry Perl must precede Git Bash's MSYS perl on PATH for vendored
# openssl-src builds; choco puts it on the machine PATH already, so a fresh
# PowerShell window picks it up. For THIS session, fix PATH now too.
$env:Path = "C:\Strawberry\perl\bin;C:\Strawberry\c\bin;C:\Program Files\NASM;$cargoBin;" +
    [Environment]::GetEnvironmentVariable('Path','Machine') + ';' +
    [Environment]::GetEnvironmentVariable('Path','User')

Write-Step "Done. Open a NEW PowerShell window, then:  cd <repo> ; just run"
