<#
.SYNOPSIS
    Windows setup for building the NSH app for Android and running it in the
    Android emulator.

.DESCRIPTION
    Idempotent: safe to re-run; installed pieces are skipped. Run after
    scripts\setup-windows.ps1 (it needs rustup). Installs:
      * Temurin JDK 17 (sdkmanager, avdmanager and apksigner need Java)
      * Android command-line tools, platform-tools, emulator
      * platforms;android-35, build-tools;35.0.1 (what `nsh package --target
        android` asks for), NDK r27 (cross-compiles libnsh.so)
      * the API 35 Google APIs x86_64 system image and an AVD named buwiz_api35
      * Rust targets x86_64-linux-android and aarch64-linux-android
      * LLVM (bindgen's libclang for QuickJS, as the NSH Windows CI uses)
      * the Windows Hypervisor Platform feature (the emulator's acceleration;
        takes effect after a reboot)
    Sets machine env vars ANDROID_HOME, ANDROID_SDK_ROOT, ANDROID_NDK_HOME,
    JAVA_HOME and LIBCLANG_PATH, and adds the SDK tools to the machine PATH.

.PARAMETER SdkRoot
    Where the SDK goes. Default C:\Android.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\setup-android-windows.ps1
    # after a reboot, in a NEW PowerShell window:
    #   emulator -avd buwiz_api35
#>
param([string]$SdkRoot = 'C:\Android')

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$CmdlineToolsUrl = 'https://dl.google.com/android/repository/commandlinetools-win-13114758_latest.zip'
$JdkUrl = 'https://api.adoptium.net/v3/binary/latest/17/ga/windows/x64/jdk/hotspot/normal/eclipse'
$Ndk = 'ndk;27.3.13750724'
$SystemImage = 'system-images;android-35;google_apis;x86_64'
$Packages = @('platform-tools', 'emulator', 'platforms;android-35', 'build-tools;35.0.1', $Ndk, $SystemImage)
$Avd = 'buwiz_api35'

function Write-Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Assert-Native($message) {
    if ($LASTEXITCODE -ne 0) { [Console]::Error.WriteLine($message); exit $LASTEXITCODE }
}

$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Step "Requesting administrator rights (UAC prompt)"
    $elevated = Start-Process -FilePath 'powershell' -Verb RunAs -Wait -PassThru -ArgumentList `
        '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`"",'-SdkRoot',"`"$SdkRoot`""
    exit $elevated.ExitCode
}

New-Item -ItemType Directory -Force -Path $SdkRoot | Out-Null

# 1. JDK 17
$jdk = Join-Path $SdkRoot 'jdk17'
if (-not (Test-Path "$jdk\bin\java.exe")) {
    Write-Step "Installing Temurin JDK 17 into $jdk"
    $zip = Join-Path $env:TEMP 'jdk17.zip'
    $tmp = Join-Path $env:TEMP 'jdk17-extract'
    curl.exe -sSfL -o $zip $JdkUrl; Assert-Native "JDK download failed"
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    Expand-Archive -Path $zip -DestinationPath $tmp
    Move-Item (Get-ChildItem $tmp -Directory | Select-Object -First 1).FullName $jdk
    Remove-Item -Recurse -Force $tmp, $zip
} else { Write-Step "JDK 17 already installed - skipping" }
$env:JAVA_HOME = $jdk
$env:Path = "$jdk\bin;$env:Path"

# 2. Command-line tools
$sdkmanager = Join-Path $SdkRoot 'cmdline-tools\latest\bin\sdkmanager.bat'
if (-not (Test-Path $sdkmanager)) {
    Write-Step "Installing Android command-line tools"
    $zip = Join-Path $env:TEMP 'cmdline-tools.zip'
    $tmp = Join-Path $env:TEMP 'cmdline-tools-extract'
    curl.exe -sSfL -o $zip $CmdlineToolsUrl; Assert-Native "command-line tools download failed"
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    Expand-Archive -Path $zip -DestinationPath $tmp
    New-Item -ItemType Directory -Force -Path (Join-Path $SdkRoot 'cmdline-tools') | Out-Null
    Move-Item (Join-Path $tmp 'cmdline-tools') (Join-Path $SdkRoot 'cmdline-tools\latest')
    Remove-Item -Recurse -Force $tmp, $zip
} else { Write-Step "Android command-line tools already installed - skipping" }

# 3. SDK packages (sdkmanager skips what is installed)
Write-Step "Accepting SDK licenses"
$yes = (1..30 | ForEach-Object { 'y' }) -join "`n"
$yes | & $sdkmanager "--sdk_root=$SdkRoot" --licenses | Out-Null
Write-Step "Installing SDK packages: $($Packages -join ', ')"
& $sdkmanager "--sdk_root=$SdkRoot" @Packages | Out-Null
Assert-Native "sdkmanager failed"

# 4. AVD
$avdmanager = Join-Path $SdkRoot 'cmdline-tools\latest\bin\avdmanager.bat'
$env:ANDROID_HOME = $SdkRoot
$env:ANDROID_SDK_ROOT = $SdkRoot
$avds = & (Join-Path $SdkRoot 'emulator\emulator.exe') -list-avds
if ($avds -notcontains $Avd) {
    Write-Step "Creating AVD $Avd (Pixel 7, $SystemImage)"
    'no' | & $avdmanager create avd -n $Avd -k $SystemImage -d pixel_7 --force | Out-Null
    Assert-Native "avdmanager failed"
} else { Write-Step "AVD $Avd already exists - skipping" }

# 5. LLVM (libclang for bindgen)
if (-not (Test-Path 'C:\Program Files\LLVM\bin\libclang.dll')) {
    Write-Step "Installing LLVM"
    choco install llvm -y --no-progress
    Assert-Native "choco install llvm failed"
} else { Write-Step "LLVM already installed - skipping" }

# 6. Rust Android targets, for the toolchain the repo and the NSH checkout pin
$rustup = Join-Path $env:USERPROFILE '.cargo\bin\rustup.exe'
if (-not (Test-Path $rustup)) { [Console]::Error.WriteLine("rustup not found: run scripts\setup-windows.ps1 first"); exit 1 }
Write-Step "Adding Rust Android targets"
& $rustup target add --toolchain 1.99.0 x86_64-linux-android aarch64-linux-android
Assert-Native "rustup target add failed"

# 7. Machine env vars and PATH
Write-Step "Setting machine env vars"
$ndkDir = Join-Path $SdkRoot ($Ndk -replace ';', '\')
$vars = @{
    ANDROID_HOME     = $SdkRoot
    ANDROID_SDK_ROOT = $SdkRoot
    ANDROID_NDK_HOME = $ndkDir
    JAVA_HOME        = $jdk
    LIBCLANG_PATH    = 'C:\Program Files\LLVM\bin'
}
foreach ($k in $vars.Keys) { [Environment]::SetEnvironmentVariable($k, $vars[$k], 'Machine') }
# Devin blueprint builds pass variables between steps through $ENVRC.
if ($env:ENVRC) { foreach ($k in $vars.Keys) { Add-Content -Path $env:ENVRC -Value "$k=$($vars[$k])" } }
$machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
$add = @("$jdk\bin", "$SdkRoot\platform-tools", "$SdkRoot\emulator", "$SdkRoot\cmdline-tools\latest\bin") |
    Where-Object { ($machinePath -split ';') -notcontains $_ }
if ($add) { [Environment]::SetEnvironmentVariable('Path', (($add -join ';') + ';' + $machinePath), 'Machine') }

# 8. Emulator acceleration
Write-Step "Enabling Windows Hypervisor Platform"
$feature = Get-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform
if ($feature.State -ne 'Enabled') {
    $r = Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -All -NoRestart
    if ($r.RestartNeeded) { Write-Host "Reboot needed before the emulator can start." -ForegroundColor Yellow }
} else { Write-Host "Already enabled." }
& (Join-Path $SdkRoot 'emulator\emulator.exe') -accel-check

Write-Step "Done. In a NEW PowerShell window: emulator -avd $Avd"
