# Shared pinned-tool installer for the Windows setup script. Dot-sourced, not run:
# gpui-agent-cli and impeccable may be downloaded from their own hexuria GitHub
# release archives after a sha256 check against the digest pinned in this
# repository; everything else and every failure is a pinned cargo install from
# source. Install-PinnedTool returns one Boolean and sends tool output to the
# host, never to the success output stream.

$script:PrebuiltScriptDir = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
$script:PrebuiltPinFile = Join-Path (Split-Path $script:PrebuiltScriptDir -Parent) 'impeccable.pin'

# The fixed release table. Only these two repositories are ours; no other crate
# may take the download route.
$script:PrebuiltReleaseTable = @{
    'gpui-agent-cli' = @{
        Repo = 'hexuria/gpui-agent'
        Binary = 'gpui-agent'
        Targets = @(
            'aarch64-apple-darwin',
            'x86_64-apple-darwin',
            'aarch64-unknown-linux-gnu',
            'x86_64-unknown-linux-gnu',
            'x86_64-pc-windows-msvc'
        )
    }
    'impeccable' = @{
        Repo = 'hexuria/impeccable-rust'
        Binary = 'impeccable'
        Targets = @(
            'aarch64-apple-darwin',
            'x86_64-apple-darwin',
            'aarch64-unknown-linux-musl',
            'x86_64-unknown-linux-musl',
            'x86_64-pc-windows-msvc',
            'aarch64-pc-windows-msvc'
        )
    }
}
$script:PrebuiltFailedStep = ''

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

function Get-PrebuiltBinaryName([string]$Crate) {
    $binary = $Crate
    if ($script:PrebuiltReleaseTable.ContainsKey($Crate)) {
        $binary = $script:PrebuiltReleaseTable[$Crate].Binary
    }
    return "$binary.exe"
}

function Get-PrebuiltTarget([string]$Crate) {
    $arch = switch ($env:PROCESSOR_ARCHITECTURE) {
        'AMD64' { 'x86_64' }
        'ARM64' { 'aarch64' }
        default { $null }
    }
    if (-not $arch) {
        $arch = switch ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture) {
            'X64' { 'x86_64' }
            'Arm64' { 'aarch64' }
            default { $null }
        }
    }
    if (-not $arch) { return $null }
    $target = "$arch-pc-windows-msvc"
    if ($script:PrebuiltReleaseTable[$Crate].Targets -contains $target) { return $target }
    return $null
}

function Get-PrebuiltArchiveName([string]$Crate, [string]$Target, [string]$Version) {
    if ($Crate -eq 'gpui-agent-cli') {
        return "gpui-agent-cli-$Target-v$Version.tgz"
    }
    if ($Target -like '*-pc-windows-msvc') {
        return "impeccable-$Target-v$Version.zip"
    }
    return "impeccable-$Target-v$Version.tar.gz"
}

function Get-PrebuiltSha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256 -ErrorAction Stop).Hash.ToLowerInvariant()
}

function Get-PrebuiltCurlCommand {
    if ($env:OS -eq 'Windows_NT') { return 'curl.exe' }
    return 'curl'
}

function Invoke-PrebuiltDownload([string]$Url, [string]$Output) {
    $curl = Get-PrebuiltCurlCommand
    & $curl -fsSL --max-time 60 --retry 1 -o $Output $Url | Out-Host
    return ($LASTEXITCODE -eq 0)
}

# Locate a binary in an archive listing. Accept it at the top level or exactly
# one folder down, and only when exactly one such entry exists.
function Get-PrebuiltArchiveBinaryPath([string]$Archive, [string]$Binary) {
    $entries = @()
    if ($Archive -like '*.zip') {
        if (Get-Command unzip -ErrorAction SilentlyContinue) {
            $entries = @(& unzip -Z1 $Archive 2>$null)
            if ($LASTEXITCODE -ne 0) { return $null }
        } else {
            try {
                Add-Type -AssemblyName System.IO.Compression.FileSystem -ErrorAction Stop
                $zip = [IO.Compression.ZipFile]::OpenRead($Archive)
                try {
                    $entries = @($zip.Entries | ForEach-Object { $_.FullName })
                } finally {
                    $zip.Dispose()
                }
            } catch {
                return $null
            }
        }
    } else {
        $entries = @(& tar -tzf $Archive 2>$null)
        if ($LASTEXITCODE -ne 0) { return $null }
    }

    $candidate = $null
    $count = 0
    foreach ($entry in $entries) {
        $stripped = "$entry" -replace '^[.]/', ''
        if (-not $stripped -or $stripped.EndsWith('/')) { continue }
        $isMatch = $false
        if ($stripped -eq $Binary) {
            $isMatch = $true
        } elseif ($stripped.EndsWith("/$Binary")) {
            $directory = $stripped.Substring(0, $stripped.Length - $Binary.Length - 1)
            if ($directory -and -not $directory.Contains('/')) { $isMatch = $true }
        }
        if ($isMatch) {
            $candidate = $stripped
            $count += 1
        }
    }
    if ($count -ne 1) { return $null }
    return $candidate
}

function Expand-PrebuiltArchive([string]$Archive, [string]$Destination) {
    if ($Archive -like '*.zip') {
        if (Get-Command unzip -ErrorAction SilentlyContinue) {
            & unzip -q -o $Archive -d $Destination | Out-Host
            return ($LASTEXITCODE -eq 0)
        }
        try {
            Expand-Archive -LiteralPath $Archive -DestinationPath $Destination -Force -ErrorAction Stop
            return $true
        } catch {
            return $false
        }
    }
    & tar -xzf $Archive -C $Destination | Out-Host
    return ($LASTEXITCODE -eq 0)
}

# Write-PrebuiltStamp <crate> <version> <route> <binary>: three lines beside the
# binary — version, route, sha256 of the binary bytes installed just now.
function Write-PrebuiltStamp([string]$Crate, [string]$Version, [string]$Route, [string]$Binary) {
    $cargoBin = Get-PrebuiltCargoBin
    $binaryPath = Join-Path $cargoBin $Binary
    $stamp = Join-Path $cargoBin ".$Crate.buwiz-installed"
    $temporary = "$stamp.tmp"
    try {
        if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) { return $false }
        $digest = Get-PrebuiltSha256 $binaryPath
        Set-Content -LiteralPath $temporary -Value @($Version, $Route, $digest) -ErrorAction Stop
        Move-Item -LiteralPath $temporary -Destination $stamp -Force -ErrorAction Stop
        return $true
    } catch {
        Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
        return $false
    }
}

# Get-PrebuiltInstalledVersion <crate> <binary>: the stamp answers only while
# the binary bytes are still the bytes it describes. Otherwise a plain crates.io
# line in cargo's install list answers; a bracketed local/git source and missing
# evidence answer nothing.
function Get-PrebuiltInstalledVersion([string]$Crate, [string]$Binary = $Crate) {
    $cargoBin = Get-PrebuiltCargoBin
    if (-not $Binary.EndsWith('.exe')) { $Binary = "$Binary.exe" }
    $binaryPath = Join-Path $cargoBin $Binary
    if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) { return $null }
    $stamp = Join-Path $cargoBin ".$Crate.buwiz-installed"
    if (Test-Path -LiteralPath $stamp -PathType Leaf) {
        $lines = @(Get-Content -LiteralPath $stamp -ErrorAction SilentlyContinue)
        if ($lines.Count -ge 3 -and $lines[0] -and $lines[2]) {
            try {
                if ((Get-PrebuiltSha256 $binaryPath) -eq $lines[2].ToLowerInvariant()) {
                    return $lines[0]
                }
            } catch {
            }
        }
    }
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        $pattern = '^' + [regex]::Escape($Crate) + ' v([0-9][0-9.]*):$'
        foreach ($line in @(& cargo install --list 2>$null)) {
            if ($line -match $pattern) { return $matches[1] }
        }
    }
    return $null
}

# Write-PrebuiltPinnedReport <label> <crate> <binary> <pin>: check-mode row,
# true only when Get-PrebuiltInstalledVersion proves the pinned version.
function Write-PrebuiltPinnedReport([string]$Label, [string]$Crate, [string]$Binary, [string]$Pin) {
    $installed = Get-PrebuiltInstalledVersion $Crate $Binary
    if (-not $installed) { $installed = 'unknown' }
    Write-Host "$Label  $installed  (pin $Pin)"
    return ($installed -eq $Pin)
}

function Invoke-PrebuiltSourceInstall([string]$Crate, [string]$Version, [string]$Binary, [string[]]$ExtraArgs) {
    & cargo install $Crate --version "=$Version" @ExtraArgs | Out-Host
    if ($LASTEXITCODE -ne 0) { return $false }
    return (Write-PrebuiltStamp $Crate $Version 'source' $Binary)
}

# Try the direct release download. On false, $script:PrebuiltFailedStep names
# the failed step; every temporary file or directory has already been removed.
function Try-InstallPrebuilt([string]$Crate, [string]$Version, [string]$Binary) {
    $script:PrebuiltFailedStep = ''
    if (-not $script:PrebuiltReleaseTable.ContainsKey($Crate)) {
        $script:PrebuiltFailedStep = 'release table lookup'
        return $false
    }
    $target = Get-PrebuiltTarget $Crate
    if (-not $target) {
        $script:PrebuiltFailedStep = 'target lookup'
        return $false
    }
    $archive = Get-PrebuiltArchiveName $Crate $target $Version
    $toolKey = $Crate.ToUpperInvariant().Replace('-', '_')
    $versionPin = Read-PrebuiltPin "${toolKey}_VERSION"
    $digestGeneration = Read-PrebuiltPin "${toolKey}_SHA256_FOR"
    if ($digestGeneration -ne $versionPin -or $Version -ne $versionPin) {
        $script:PrebuiltFailedStep = 'pin version'
        return $false
    }
    $targetKey = $target.Replace('-', '_')
    $expected = Read-PrebuiltPin "${toolKey}_SHA256_$targetKey"
    if (-not $expected -or $expected -notmatch '^[0-9a-f]{64}$') {
        $script:PrebuiltFailedStep = 'digest pin'
        return $false
    }
    $repo = $script:PrebuiltReleaseTable[$Crate].Repo
    $url = "https://github.com/$repo/releases/download/v$Version/$archive"
    $temp = Join-Path ([IO.Path]::GetTempPath()) ("buwiz-prebuilt-" + [guid]::NewGuid().ToString('N'))
    try {
        New-Item -ItemType Directory -Force -Path $temp -ErrorAction Stop | Out-Null
        $downloaded = Join-Path $temp $archive
        if (-not (Invoke-PrebuiltDownload $url $downloaded)) {
            $script:PrebuiltFailedStep = 'archive download'
            return $false
        }
        $actual = Get-PrebuiltSha256 $downloaded
        if ($actual -ne $expected) {
            $script:PrebuiltFailedStep = 'digest check'
            return $false
        }
        $binaryPath = Get-PrebuiltArchiveBinaryPath $downloaded $Binary
        if (-not $binaryPath) {
            $script:PrebuiltFailedStep = 'archive layout'
            return $false
        }
        if ($Crate -eq 'gpui-agent-cli' -and $binaryPath -ne "gpui-agent-cli/$Binary") {
            $script:PrebuiltFailedStep = 'archive layout'
            return $false
        }
        $extract = Join-Path $temp 'extract'
        New-Item -ItemType Directory -Force -Path $extract -ErrorAction Stop | Out-Null
        if (-not (Expand-PrebuiltArchive $downloaded $extract)) {
            $script:PrebuiltFailedStep = 'archive unpack'
            return $false
        }
        $source = Join-Path $extract ($binaryPath -replace '/', [IO.Path]::DirectorySeparatorChar)
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            $script:PrebuiltFailedStep = 'archive layout'
            return $false
        }
        $cargoBin = Get-PrebuiltCargoBin
        New-Item -ItemType Directory -Force -Path $cargoBin -ErrorAction Stop | Out-Null
        Move-Item -LiteralPath $source -Destination (Join-Path $cargoBin $Binary) -Force -ErrorAction Stop
        if (-not (Write-PrebuiltStamp $Crate $Version 'prebuilt' $Binary)) {
            $script:PrebuiltFailedStep = 'stamp'
            return $false
        }
        return $true
    } catch {
        if (-not $script:PrebuiltFailedStep) { $script:PrebuiltFailedStep = 'unexpected error' }
        return $false
    } finally {
        Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Install-PinnedTool <crate> <version> [extra cargo-install args]
# Returns one Boolean and nothing else on the success output stream. The
# download route exists only for the fixed table above; any failure is announced
# once and retried as a pinned cargo install from source with the caller's extra
# arguments.
function Install-PinnedTool([string]$Crate, [string]$Version, [string[]]$ExtraArgs = @()) {
    Use-PrebuiltCargoBin
    $binary = Get-PrebuiltBinaryName $Crate
    $installed = Get-PrebuiltInstalledVersion $Crate $binary
    if ($installed -eq $Version) {
        Write-Host "$Crate already at $Version"
        return $true
    }
    if ($env:BUWIZ_SETUP_NO_PREBUILT -eq '1') {
        Write-Host "prebuilt installs disabled for $Crate; building from source"
        return (Invoke-PrebuiltSourceInstall $Crate $Version $binary $ExtraArgs)
    }
    if (Try-InstallPrebuilt $Crate $Version $binary) { return $true }
    Write-Host "$Crate prebuilt $($script:PrebuiltFailedStep) failed; building from source"
    return (Invoke-PrebuiltSourceInstall $Crate $Version $binary $ExtraArgs)
}
