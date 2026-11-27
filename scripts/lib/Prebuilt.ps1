# Shared prebuilt-tool installer for the Windows setup script. Dot-sourced, not run:
# cargo-binstall is downloaded at scripts\impeccable.pin's CARGO_BINSTALL_VERSION and
# crates use only their own crate-meta-data release artifacts. Every failure falls
# back to the caller's normal cargo install from source.

$script:PrebuiltScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
$script:PrebuiltPinFile = Join-Path (Split-Path $script:PrebuiltScriptDir -Parent) 'impeccable.pin'

function Read-PrebuiltPin([string]$Key) {
    foreach ($line in Get-Content -LiteralPath $script:PrebuiltPinFile -ErrorAction SilentlyContinue) {
        if ($line -match "^$([regex]::Escape($Key))=(\S+)") { return $matches[1] }
    }
}

function Get-PrebuiltCargoBin {
    $homeDir = if ($env:USERPROFILE) { $env:USERPROFILE } else { $HOME }
    $root = if ($env:CARGO_INSTALL_ROOT) { $env:CARGO_INSTALL_ROOT } elseif ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $homeDir '.cargo' }
    Join-Path $root 'bin'
}

function Use-PrebuiltCargoBin {
    $bin = Get-PrebuiltCargoBin
    $separator = [IO.Path]::PathSeparator
    $entries = @($env:Path -split [regex]::Escape($separator) | Where-Object { $_ })
    if ($entries -notcontains $bin) { $env:Path = "$bin$separator$env:Path" }
}

function Get-CargoBinstallVersion {
    try {
        $output = & cargo-binstall -V 2>$null
        if ($LASTEXITCODE -ne 0) { return $null }
        return "$($output | Select-Object -First 1)"
    } catch {
        return $null
    }
}

function Test-CargoBinstallPin([string]$Pin) {
    $version = Get-CargoBinstallVersion
    return ($version -eq $Pin) -or ($version -eq "cargo-binstall $Pin")
}

# Ensure-CargoBinstall: true when the pinned cargo-binstall is usable. Download or
# extraction failure is deliberately non-fatal so callers can build from source.
function Ensure-CargoBinstall {
    Use-PrebuiltCargoBin
    if ($env:BUWIZ_SETUP_NO_PREBUILT -eq '1') { return $false }

    $pin = Read-PrebuiltPin 'CARGO_BINSTALL_VERSION'
    if (-not $pin) { return $false }
    if (Test-CargoBinstallPin $pin) { return $true }

    $arch = switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64' { 'x86_64' }
        'ARM64' { 'aarch64' }
        default { $null }
    }
    if (-not $arch) { return $false }
    $asset = "cargo-binstall-$arch-pc-windows-msvc.zip"
    $url = "https://github.com/cargo-bins/cargo-binstall/releases/download/v$pin/$asset"
    $cargoBin = Get-PrebuiltCargoBin
    $temp = Join-Path ([IO.Path]::GetTempPath()) ("cargo-binstall-" + [guid]::NewGuid().ToString('N'))
    $extract = Join-Path $temp 'extract'
    try {
        New-Item -ItemType Directory -Force -Path $extract | Out-Null
        New-Item -ItemType Directory -Force -Path $cargoBin | Out-Null
        $archive = Join-Path $temp $asset
        & curl.exe -fsSL -o $archive $url
        if ($LASTEXITCODE -ne 0) { return $false }
        Expand-Archive -LiteralPath $archive -DestinationPath $extract -Force
        $binary = Get-ChildItem -LiteralPath $extract -Recurse -Filter 'cargo-binstall.exe' -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if (-not $binary) { return $false }
        Copy-Item -LiteralPath $binary.FullName -Destination (Join-Path $cargoBin 'cargo-binstall.exe') -Force
    } catch {
        return $false
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
    return (Test-CargoBinstallPin $pin)
}

function Write-PrebuiltStamp([string]$Crate, [string]$Version) {
    $stamp = Join-Path (Get-PrebuiltCargoBin) ".$Crate.buwiz-prebuilt-version"
    try {
        Set-Content -LiteralPath $stamp -Value $Version -ErrorAction Stop
    } catch {
    }
}

# Get-PrebuiltInstalledVersion <crate> <binary>: cargo-binstall versions at least
# some installs in Cargo's own install list, but the stamp beside the binary keeps
# detection working where it does not. Cargo's list wins so a later source build
# can never be hidden by an older stamp.
function Get-PrebuiltInstalledVersion([string]$Crate, [string]$Binary = $Crate) {
    try {
        $pattern = '^' + [regex]::Escape($Crate) + ' v([0-9][0-9.]*):'
        foreach ($line in @(cargo install --list 2>$null)) {
            if ($line -match $pattern) { return $matches[1] }
        }
    } catch {
    }
    $cargoBin = Get-PrebuiltCargoBin
    $binaryPath = Join-Path $cargoBin "$Binary.exe"
    $stamp = Join-Path $cargoBin ".$Crate.buwiz-prebuilt-version"
    if ((Test-Path -LiteralPath $binaryPath) -and (Test-Path -LiteralPath $stamp)) {
        return "$(Get-Content -LiteralPath $stamp -Raw)".Trim()
    }
}

# Install-PrebuiltOrBuild <crate> <version> [extra cargo-install args]
# Tries only the crate's own release metadata; cargo-binstall is never allowed to
# use a third-party prebuilt store or to compile implicitly here.
function Install-PrebuiltOrBuild([string]$Crate, [string]$Version, [string[]]$ExtraArgs = @()) {
    Use-PrebuiltCargoBin
    if ($env:BUWIZ_SETUP_NO_PREBUILT -eq '1') {
        Write-Host "prebuilt installs disabled for $Crate; using cargo install"
        & cargo install $Crate --version $Version @ExtraArgs
        return
    }
    if (-not (Ensure-CargoBinstall)) {
        Write-Host "cargo-binstall unavailable for $Crate; falling back to cargo install"
        & cargo install $Crate --version $Version @ExtraArgs
        return
    }
    & cargo binstall --no-confirm --strategies crate-meta-data --version $Version $Crate
    if ($LASTEXITCODE -eq 0) {
        Write-PrebuiltStamp $Crate $Version
        return
    }
    Write-Host "cargo binstall failed for $Crate; falling back to cargo install"
    & cargo install $Crate --version $Version @ExtraArgs
}
