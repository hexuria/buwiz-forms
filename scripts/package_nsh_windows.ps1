[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $PackageRoot,
    [Parameter(Mandatory)] [string] $OutDir,
    [switch] $SmokeMsix,
    # The Microsoft Store's package alone: the .msix and SHA256SUMS-windows-store-x64.txt, never signed (the Store
    # signs it). release.yml builds it in a job of its own, given the Google OAuth client only when STORE_GMAIL is 1.
    [switch] $MsixOnly,
    # The direct downloads alone (Setup.exe, the .msi, the updater ZIP): a release's .msix comes from the Store job.
    [switch] $NoMsix
)

$ErrorActionPreference = 'Stop'
if ($MsixOnly -and $NoMsix) { throw 'Pass -MsixOnly or -NoMsix, not both' }
if ($SmokeMsix -and $NoMsix) { throw '-SmokeMsix needs the .msix, which -NoMsix leaves out' }
if ($MsixOnly -and -not [string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE_PFX)) {
    throw 'The Store package is never signed here: the Microsoft Store signs it. Unset WINDOWS_CERTIFICATE_PFX for -MsixOnly'
}
$PSNativeCommandUseErrorActionPreference = $true
$repo = Split-Path -Parent $PSScriptRoot
$PackageRoot = (Resolve-Path -LiteralPath $PackageRoot).Path
$config = Get-Content -Raw -LiteralPath (Join-Path $repo 'nsh/nsh.json') | ConvertFrom-Json
$version = $config.version
if ($version -isnot [string] -or $version -notmatch '\A(?:0|[1-9][0-9]{0,2})\.(?:0|[1-9][0-9]{0,2})\.(?:0|[1-9][0-9]{0,4})\z') {
    throw "Invalid Windows release version: $version"
}
$major, $minor, $patch = $version -split '\.'
if ([int]$major -gt 255 -or [int]$minor -gt 255 -or [int]$patch -gt 65535) {
    throw "Windows release version exceeds MSI limits (255.255.65535): $version"
}
if ($env:GITHUB_REF_TYPE -eq 'tag' -and $env:GITHUB_REF_NAME -cne "v$version") {
    throw "Release tag $env:GITHUB_REF_NAME does not match Windows app version $version"
}
$packagedApp = Get-Content -Raw -LiteralPath (Join-Path $PackageRoot 'eBIRForms/nsh/app.json') | ConvertFrom-Json
if ($packagedApp.version -cne $version -or $packagedApp.id -cne $config.id) {
    throw "NSH payload identity or version does not match nsh/nsh.json (expected $($config.id) $version)"
}
$OutDir = (New-Item -ItemType Directory -Force -Path $OutDir).FullName
$stem = "eBIRForms-Windows-x64-$version"
$setup = Join-Path $OutDir "$stem-Setup.exe"
$msi = Join-Path $OutDir "$stem.msi"
$msix = Join-Path $OutDir "$stem.msix"
$zip = Join-Path $OutDir "eBIRForms-$version-windows-x86_64.zip"

$iscc = Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'
$wix = Get-ChildItem "${env:ProgramFiles(x86)}\WiX Toolset v3.*\bin" -Directory -ErrorAction SilentlyContinue |
    Where-Object { Test-Path (Join-Path $_.FullName 'heat.exe') } |
    Sort-Object FullName -Descending | Select-Object -First 1
$sdk = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\10.*\x64" -Directory |
    Where-Object { Test-Path (Join-Path $_.FullName 'makeappx.exe') } |
    Sort-Object { [version]$_.Parent.Name } -Descending | Select-Object -First 1
if (-not $sdk -or (-not $MsixOnly -and (-not (Test-Path -LiteralPath $iscc) -or -not $wix))) {
    throw 'Inno Setup 6, WiX 3 and the Windows SDK with MakeAppx are required (-MsixOnly needs only the SDK)'
}

$hasPfx = -not [string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE_PFX)
$hasPassword = -not [string]::IsNullOrWhiteSpace($env:WINDOWS_CERTIFICATE_PASSWORD)
if ($hasPfx -xor $hasPassword) {
    throw 'Configure both WINDOWS_CERTIFICATE_PFX and WINDOWS_CERTIFICATE_PASSWORD, or neither'
}
if (-not $hasPfx) {
    Write-Output '::warning title=Unsigned Windows artifacts::No Windows signing certificate; the MSIX cannot be sideloaded until signed and trusted.'
}
$scratch = Join-Path ([IO.Path]::GetTempPath()) ("nsh-release-" + [guid]::NewGuid().ToString('N'))
$stage = Join-Path $scratch 'eBIRForms'
$exe = Join-Path $stage 'eBIRForms.exe'
$marker = 'ebirforms-no-self-update'
$pfx = Join-Path $scratch 'signing.pfx'

function Sign-WindowsArtifact([string] $Path) {
    if ($hasPfx) {
        $signtool = Join-Path $sdk.FullName 'signtool.exe'
        & $signtool sign /fd SHA256 /td SHA256 /tr http://timestamp.digicert.com /f $pfx /p $env:WINDOWS_CERTIFICATE_PASSWORD $Path
        & $signtool verify /pa /all /v $Path
    }
}

function Test-Payload([string] $Path, [bool] $Managed) {
    foreach ($file in @('eBIRForms.exe', 'nsh/bundle.nshc', 'nsh/app.json')) {
        if (-not (Test-Path -LiteralPath (Join-Path $Path $file) -PathType Leaf)) {
            throw "Missing NSH payload file '$file' in $Path"
        }
    }
    if (-not (Test-Path -LiteralPath (Join-Path $Path 'print-overrides') -PathType Container)) {
        throw "Missing print overrides in $Path"
    }
    if ((Test-Path -LiteralPath (Join-Path $Path $marker)) -ne $Managed) {
        throw "Unexpected updater policy for $Path; expected a package-managed payload: $Managed"
    }
    & python (Join-Path $PSScriptRoot 'audit_no_legacy.py') --package-root $Path
}

try {
    New-Item -ItemType Directory -Path $scratch | Out-Null
    Copy-Item -LiteralPath (Join-Path $PackageRoot 'eBIRForms') -Destination $stage -Recurse
    Copy-Item -LiteralPath (Join-Path $repo 'print-overrides') -Destination $stage -Recurse
    Test-Payload $stage $false

    $publisher = $null
    if ($hasPfx) {
        [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE_PFX))
        $cert = [Security.Cryptography.X509Certificates.X509Certificate2]::new($pfx, $env:WINDOWS_CERTIFICATE_PASSWORD)
        try { $publisher = $cert.Subject } finally { $cert.Dispose() }
        Sign-WindowsArtifact $exe
    }

    $built = [Collections.Generic.List[string]]::new()
    if (-not $MsixOnly) {
        # Recompile NSH's per-user Setup and rebuild its ZIP from the signed payload,
        # including the print overrides. Neither should carry the package-manager marker.
        $iss = Get-Content -LiteralPath (Join-Path $PackageRoot 'eBIRForms.iss') -Raw
        $source = 'Source: "eBIRForms\*"'
        if (-not $iss.Contains("$source; DestDir: `"{app}`"") -or $iss -notmatch '(?m)^PrivilegesRequired=lowest\r?$') {
            throw 'The generated NSH Setup no longer uses the expected per-user layout'
        }
        $auditIss = Join-Path $scratch 'eBIRForms.iss'
        # NSH swaps everything beside the executable. Keep Inno's uninstaller one level up.
        $iss = $iss.Replace($source, 'Source: "{#StageDir}\*"').Replace('{app}', '{app}\eBIRForms-app')
        $iss += "`n[UninstallDelete]`nType: filesandordirs; Name: `"{app}\eBIRForms-app`"`n"
        Set-Content -LiteralPath $auditIss -Value $iss -Encoding utf8
        & python (Join-Path $PSScriptRoot 'audit_installer_sources.py') --installer $auditIss --stage-dir $stage
        & $iscc /Q "/DStageDir=$stage" "/O$OutDir" "/F$stem-Setup" $auditIss
        if (Test-Path -LiteralPath $zip) { Remove-Item -LiteralPath $zip }
        [IO.Compression.ZipFile]::CreateFromDirectory($stage, $zip, [IO.Compression.CompressionLevel]::Optimal, $true)
        $built.Add($setup)
    }

    New-Item -ItemType File -Path (Join-Path $stage $marker) | Out-Null
    if (-not $MsixOnly) {
        $wixDir = Join-Path $repo 'packaging/windows'
        $harvest = Join-Path $scratch 'harvest.wxs'
        $heatArgs = @(
            'dir', $stage, '-dr', 'INSTALLDIR', '-cg', 'NshPayload', '-ag', '-sfrag', '-srd', '-scom', '-sreg',
            '-var', 'var.SourceDir', '-t', (Join-Path $wixDir 'per-user-harvest.xsl'), '-out', $harvest
        )
        & (Join-Path $wix.FullName 'heat.exe') @heatArgs
        # WiX does not auto-generate GUIDs for components that have files and
        # HKCU key paths. Heat's component IDs are stable for the destination paths.
        [xml]$components = Get-Content -LiteralPath $harvest -Raw
        $namespace = [Xml.XmlNamespaceManager]::new($components.NameTable)
        $namespace.AddNamespace('w', 'http://schemas.microsoft.com/wix/2006/wi')
        foreach ($component in $components.SelectNodes('//w:Component', $namespace)) {
            $identity = [Text.Encoding]::UTF8.GetBytes("dev.goldcoders.bir.msi/$($component.Id)")
            $digest = [Security.Cryptography.SHA256]::HashData($identity)
            $component.SetAttribute('Guid', [guid]::new([byte[]]$digest[0..15]).ToString())
        }
        $components.Save($harvest)
        & (Join-Path $wix.FullName 'candle.exe') (Join-Path $wixDir 'nsh.wxs') $harvest -arch x64 "-dSourceDir=$stage" "-dVersion=$version" -out "$scratch\"
        # ICE91 warns about files in a user profile if a per-machine install
        # were requested. This package is per-user only.
        & (Join-Path $wix.FullName 'light.exe') (Join-Path $scratch 'nsh.wixobj') (Join-Path $scratch 'harvest.wixobj') -out $msi -sice:ICE91
        $built.Add($msi)
    }

    $msixArgs = @{
        ExePath = $exe
        NshPayload = $stage
        AssetsDir = (Join-Path $repo 'assets')
        Version = $version
        BuildNumber = [int]$patch
        OutFile = $msix
    }
    if ($publisher) { $msixArgs.Publisher = $publisher }
    if (-not $NoMsix) {
        & (Join-Path $PSScriptRoot 'package_msix.ps1') @msixArgs
        $built.Add($msix)
    }

    foreach ($installer in $built) { Sign-WindowsArtifact $installer }

    if (-not $MsixOnly) {
        & (Join-Path $PSScriptRoot 'audit_final_setup.ps1') -Installer $setup -Nsh
        $extractedMsi = New-Item -ItemType Directory -Path (Join-Path $scratch 'msi')
        $extraction = Start-Process msiexec.exe -Wait -PassThru -ArgumentList @(
            '/a', ('"{0}"' -f $msi), '/qn', ('TARGETDIR="{0}"' -f $extractedMsi.FullName)
        )
        if ($extraction.ExitCode -ne 0) { throw "MSI extraction failed: $($extraction.ExitCode)" }
        $msiPayload = @(Get-ChildItem $extractedMsi.FullName -Filter eBIRForms.exe -File -Recurse)
        if ($msiPayload.Count -ne 1) { throw 'Expected one NSH executable in the extracted MSI' }
        Test-Payload $msiPayload[0].DirectoryName $true
    }

    if (-not $NoMsix) {
        $extractedMsix = Join-Path $scratch 'msix'
        & (Join-Path $sdk.FullName 'makeappx.exe') unpack /p $msix /d $extractedMsix
        Test-Payload $extractedMsix $true

        if ($SmokeMsix) {
            $smokeScript = Join-Path $PSScriptRoot 'smoke_msix.ps1'
            $smokeArgs = @('-NoProfile', '-File', $smokeScript, '-Package', $msix)
            $testCertificate = $null
            try {
                if (-not $hasPfx) {
                    # Sign a disposable copy, not the artifact we publish.
                    $testCertificate = New-SelfSignedCertificate -Type CodeSigningCert `
                        -Subject 'CN=eBIRForms NSH CI Test Signing' -CertStoreLocation 'Cert:\CurrentUser\My'
                    $msixArgs.Publisher = $testCertificate.Subject
                    $msixArgs.OutFile = Join-Path $scratch 'smoke.msix'
                    & (Join-Path $PSScriptRoot 'package_msix.ps1') @msixArgs
                    & (Join-Path $sdk.FullName 'signtool.exe') sign /fd SHA256 /sha1 $testCertificate.Thumbprint $msixArgs.OutFile
                    $testCertificatePath = Join-Path $scratch 'smoke.cer'
                    Export-Certificate -Cert $testCertificate -FilePath $testCertificatePath | Out-Null
                    $smokeArgs = @('-NoProfile', '-File', $smokeScript, '-Package', $msixArgs.OutFile, '-Certificate', $testCertificatePath)
                }
                & powershell.exe @smokeArgs
            } finally {
                if ($testCertificate) {
                    Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($testCertificate.Thumbprint)" -DeleteKey
                }
            }
        }
    }

    if (-not $MsixOnly) {
        $extractedZip = Join-Path $scratch 'zip'
        [IO.Compression.ZipFile]::ExtractToDirectory($zip, $extractedZip)
        Test-Payload (Join-Path $extractedZip 'eBIRForms') $false
        $built.Add($zip)
    }

    # The Store job's sums have a name of their own, so the release's two Windows sums files never collide.
    $sums = if ($MsixOnly) { 'SHA256SUMS-windows-store-x64.txt' } else { 'SHA256SUMS-windows-x64.txt' }
    $built | ForEach-Object {
        $hash = Get-FileHash -LiteralPath $_ -Algorithm SHA256
        "$($hash.Hash.ToLowerInvariant())  $(Split-Path -Leaf $_)"
    } | Set-Content -LiteralPath (Join-Path $OutDir $sums) -Encoding ascii
} finally {
    if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
}
