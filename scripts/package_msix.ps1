[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ExePath,

    [string]$AssetsDir = "assets",

    [Parameter(Mandatory = $true)]
    [string]$Version,

    [Parameter(Mandatory = $true)]
    [int]$BuildNumber,

    [string]$Publisher,

    [Parameter(Mandatory = $true)]
    [string]$OutFile,

    [string]$Python = "python"
)

$ErrorActionPreference = 'Stop'

function Get-MsvcVersion {
    param([Parameter(Mandatory = $true)][string]$Path)

    $versionName = [regex]::Match(
        $Path,
        '\\MSVC\\(?<version>[^\\]+)\\'
    ).Groups['version'].Value
    try {
        return [version]$versionName
    } catch {
        return [version]'0.0'
    }
}

$exeFullPath = (Resolve-Path $ExePath).Path
$assetsFullPath = (Resolve-Path $AssetsDir).Path
$outFullPath = [IO.Path]::GetFullPath($OutFile)
$outDirectory = Split-Path -Parent $outFullPath
if (-not (Test-Path $outDirectory)) {
    New-Item -ItemType Directory -Force -Path $outDirectory | Out-Null
}

if ($Version -notmatch '^(\d+)\.(\d+)\.\d+(?:[-+].*)?$') {
    throw "Version must be an app semantic version: $Version"
}
$msixVersion = "$($matches[1]).$($matches[2]).$BuildNumber.0"
$stage = Join-Path $outDirectory ("msix-staging-" + [guid]::NewGuid().ToString('N'))

try {
    New-Item -ItemType Directory -Path $stage | Out-Null
    Copy-Item -LiteralPath $exeFullPath -Destination (Join-Path $stage 'bir.exe')
    Copy-Item -LiteralPath $assetsFullPath -Destination (Join-Path $stage 'assets') -Recurse

    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) {
        throw "vswhere.exe was not found: $vswhere"
    }
    $installationPath = & $vswhere -latest -products '*' `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    $vswhereExitCode = $LASTEXITCODE
    if ($vswhereExitCode -ne 0) {
        throw "vswhere failed with exit code $vswhereExitCode"
    }
    if (-not $installationPath) {
        throw "Visual Studio C++ x64 tools were not found by vswhere"
    }
    $installationPath = ($installationPath | Select-Object -First 1).Trim()

    $dumpbin = Get-ChildItem -Path (Join-Path $installationPath 'VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe') `
        -File -ErrorAction SilentlyContinue |
        Sort-Object { Get-MsvcVersion $_.FullName } -Descending |
        Select-Object -First 1
    if (-not $dumpbin) {
        throw "dumpbin.exe was not found under $installationPath"
    }

    $dependents = & $dumpbin.FullName /dependents $exeFullPath 2>&1
    $dumpbinExitCode = $LASTEXITCODE
    if ($dumpbinExitCode -ne 0) {
        throw "dumpbin /dependents failed with exit code $dumpbinExitCode"
    }
    $runtimeImports = @(
        $dependents |
            ForEach-Object { "$_".Trim() } |
            Where-Object { $_ -match '(?i)^(vcruntime140(?:_\d+)?|msvcp140(?:_\w+)?|concrt140)\.dll$' } |
            Sort-Object -Unique
    )
    if ($runtimeImports.Count -eq 0) {
        throw "dumpbin did not report any supported MSVC runtime DLL imports for $exeFullPath"
    }

    $redistRoots = @(
        Get-ChildItem -Path (Join-Path $installationPath 'VC\Redist\MSVC') `
            -Directory -ErrorAction SilentlyContinue |
            Sort-Object { Get-MsvcVersion $_.FullName } -Descending
    )
    $redistCrtDirectories = @(
        foreach ($redistRoot in $redistRoots) {
            Get-ChildItem -Path (Join-Path $redistRoot.FullName 'x64\Microsoft.VC14*.CRT') `
                -Directory -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending
        }
    )
    foreach ($runtimeDll in $runtimeImports) {
        $runtimeFile = $null
        foreach ($crtDirectory in $redistCrtDirectories) {
            $candidate = Join-Path $crtDirectory.FullName $runtimeDll
            if (Test-Path $candidate -PathType Leaf) {
                $runtimeFile = $candidate
                break
            }
        }
        if (-not $runtimeFile) {
            $systemCandidate = Join-Path $env:WINDIR "System32\$runtimeDll"
            if (Test-Path $systemCandidate -PathType Leaf) {
                $runtimeFile = $systemCandidate
            }
        }
        if (-not $runtimeFile) {
            throw "Required MSVC runtime DLL was not found: $runtimeDll"
        }
        Copy-Item -LiteralPath $runtimeFile -Destination (Join-Path $stage $runtimeDll)
    }
    Write-Host "MSVC runtime DLLs: $($runtimeImports -join ', ')"

    & $Python (Join-Path $PSScriptRoot 'audit_no_legacy.py') --package-root $stage
    $auditExitCode = $LASTEXITCODE
    if ($auditExitCode -ne 0) {
        throw "audit_no_legacy.py failed with exit code $auditExitCode"
    }

    $manifestPath = Join-Path $PSScriptRoot '..\assets\windows\AppxManifest.xml'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw
    $manifest = $manifest.Replace('MSIX_VERSION_PLACEHOLDER', $msixVersion)
    if (-not [string]::IsNullOrWhiteSpace($Publisher)) {
        $escapedPublisher = [System.Security.SecurityElement]::Escape($Publisher)
        $identityMatch = [regex]::Match($manifest, '<Identity\b[^>]*>')
        if (-not $identityMatch.Success) {
            throw "AppxManifest.xml does not contain an Identity element"
        }
        $identity = $identityMatch.Value
        $publisherAttribute = [regex]::Match($identity, '\bPublisher="[^"]*"')
        if (-not $publisherAttribute.Success) {
            throw "AppxManifest.xml Identity does not contain Publisher"
        }
        $replacement = 'Publisher="' + $escapedPublisher + '"'
        $identity = $identity.Remove(
            $publisherAttribute.Index,
            $publisherAttribute.Length
        ).Insert($publisherAttribute.Index, $replacement)
        $manifest = $manifest.Remove($identityMatch.Index, $identityMatch.Length).
            Insert($identityMatch.Index, $identity)
    }
    $manifestPath = Join-Path $stage 'AppxManifest.xml'
    [IO.File]::WriteAllText(
        $manifestPath,
        $manifest,
        [Text.UTF8Encoding]::new($false)
    )

    $sdkBins = @(
        Get-ChildItem -Path 'C:\Program Files (x86)\Windows Kits\10\bin\10.*\x64' `
            -Directory -ErrorAction SilentlyContinue |
            Sort-Object { [version](Split-Path $_.Parent.FullName -Leaf) } -Descending
    )
    if ($sdkBins.Count -eq 0) {
        throw "Windows SDK bin\10.*\x64 directory was not found"
    }
    $sdkBin = $sdkBins[0].FullName
    $makePri = Join-Path $sdkBin 'MakePri.exe'
    $makeAppx = Join-Path $sdkBin 'MakeAppx.exe'
    if (-not (Test-Path $makeAppx)) {
        throw "MakeAppx.exe was not found in the newest Windows SDK: $makeAppx"
    }
    if (Test-Path $makePri) {
        Write-Host "Generating resources.pri for advanced tile icons..."
        & $makePri createconfig /cf (Join-Path $stage 'priconfig.xml') /dq en-US /pv 10.0.0 /o
        $makePriExitCode = $LASTEXITCODE
        if ($makePriExitCode -ne 0) {
            throw "MakePri createconfig failed with exit code $makePriExitCode"
        }
        & $makePri new /pr $stage /cf (Join-Path $stage 'priconfig.xml') `
            /in 'GOLDCODERSCORP.ebirforms' /of (Join-Path $stage 'resources.pri') /o
        $makePriExitCode = $LASTEXITCODE
        if ($makePriExitCode -ne 0) {
            throw "MakePri new failed with exit code $makePriExitCode"
        }
        Remove-Item (Join-Path $stage 'priconfig.xml') -Force
    }

    if (Test-Path $outFullPath) {
        Remove-Item -LiteralPath $outFullPath -Force
    }
    & $makeAppx pack /d $stage /p $outFullPath /o
    $makeAppxExitCode = $LASTEXITCODE
    if ($makeAppxExitCode -ne 0) {
        throw "MakeAppx pack failed with exit code $makeAppxExitCode"
    }
    Write-Host "MSIX package created: $outFullPath"
} finally {
    if (Test-Path $stage) {
        Remove-Item -LiteralPath $stage -Recurse -Force
    }
}
