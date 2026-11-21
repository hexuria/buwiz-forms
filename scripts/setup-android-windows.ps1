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
param([string]$SdkRoot = 'C:\Android', [switch]$Run)

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

function Test-AndroidRuntime($path, $pin) {
    if (-not (Test-Path "$path\classes.dex") -or -not (Test-Path "$path\lib\x86_64\libnsh.so") -or -not (Test-Path "$path\nsh-ref")) { return $false }
    $builtRef = Get-Content "$path\nsh-ref" -Raw
    $builtRef -and $builtRef.Trim() -eq $pin
}

$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Step "Requesting administrator rights (UAC prompt)"
    $elevationArgs = @('-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`"",'-SdkRoot',"`"$SdkRoot`"")
    if ($Run) { $elevationArgs += '-Run' }
    $elevated = Start-Process -FilePath 'powershell' -Verb RunAs -Wait -PassThru -ArgumentList $elevationArgs
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
& $rustup target add --toolchain 1.99.0 x86_64-linux-android aarch64-linux-android armv7-linux-androideabi
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
foreach ($k in $vars.Keys) {
    [Environment]::SetEnvironmentVariable($k, $vars[$k], 'Machine')
    [Environment]::SetEnvironmentVariable($k, $vars[$k], 'Process')
}
# Devin blueprint builds pass variables between steps through $ENVRC.
if ($env:ENVRC) {
    $envrcPath = $env:ENVRC
    if ($envrcPath -match '^/([a-zA-Z])/') {
        $envrcPath = $envrcPath -replace '^/([a-zA-Z])/', '$1:\'
        $envrcPath = $envrcPath.Replace('/', '\')
    }
    foreach ($k in $vars.Keys) {
        $value = $vars[$k].Replace('\', '/')
        if ($value.Contains("'")) { throw 'ENVRC paths cannot contain single quotes' }
        Add-Content -Path $envrcPath -Value "export $k='$value'"
    }
}
$machinePath = [Environment]::GetEnvironmentVariable('Path', 'Machine')
$add = @("$jdk\bin", "$SdkRoot\platform-tools", "$SdkRoot\emulator", "$SdkRoot\cmdline-tools\latest\bin") |
    Where-Object { ($machinePath -split ';') -notcontains $_ }
if ($add) { [Environment]::SetEnvironmentVariable('Path', (($add -join ';') + ';' + $machinePath), 'Machine') }

# 8. Emulator acceleration
Write-Step "Enabling Windows Hypervisor Platform"
$feature = Get-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform
$restartNeeded = $false
if ($feature.State -eq 'EnablePending') {
    $restartNeeded = $true
} elseif ($feature.State -ne 'Enabled') {
    $r = Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -All -NoRestart
    $restartNeeded = $r.RestartNeeded
    if ($r.RestartNeeded) { Write-Host "Reboot needed before the emulator can start." -ForegroundColor Yellow }
} else { Write-Host "Already enabled." }
if ($restartNeeded) {
    $rebootState = "$env:USERPROFILE\BuwizNsh\android-windows"
    New-Item -ItemType Directory -Force $rebootState | Out-Null
    Set-Content "$rebootState\whpx-reboot-boot-time" (Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime().ToString('o')
}
& (Join-Path $SdkRoot 'emulator\emulator.exe') -accel-check

if ($LASTEXITCODE -ne 0 -and $restartNeeded) {
    Write-Warning 'Emulator acceleration awaits a reboot; continuing CLI and Android runtime provisioning. Reboot before running Android.'
} else {
    Assert-Native 'Emulator acceleration unavailable; enable nested virtualization and reboot'
}

$repo = Split-Path $PSScriptRoot -Parent
$checkout = Join-Path (Split-Path $repo -Parent) 'nsh'
$pin = (Select-String -Path "$repo\.github\workflows\nsh.yml" -Pattern '^\s*NSH_PINNED_REF:\s*([0-9a-f]+)').Matches[0].Groups[1].Value
if (-not $pin) { throw 'NSH_PINNED_REF not found' }
if (-not (Test-Path $checkout)) {
    git clone https://github.com/hexuria/nsh $checkout
    Assert-Native 'NSH clone failed'
}
if (git -C $checkout status --porcelain) { throw 'NSH checkout has local changes' }
if ((git -C $checkout rev-parse HEAD) -ne $pin) {
    git -C $checkout fetch origin $pin
    Assert-Native 'NSH fetch failed'
    git -C $checkout checkout --detach $pin
    Assert-Native 'NSH checkout failed'
}
$tools = "$env:USERPROFILE\BuwizNsh\tools"
$env:RUSTUP_TOOLCHAIN = '1.99.0'
$env:RUST_MIN_STACK = '268435456'
foreach ($k in 'OPENSSL_DIR', 'OPENSSL_LIB_DIR') {
    [Environment]::SetEnvironmentVariable($k, [Environment]::GetEnvironmentVariable($k, 'User'), 'Process')
}
$env:Path = "$env:USERPROFILE\.cargo\bin;$jdk\bin;$SdkRoot\platform-tools;$env:Path"
$env:X86_64_PC_WINDOWS_MSVC_OPENSSL_NO_VENDOR = '1'
$have = if (Test-Path "$tools\bin\nsh.exe") { & "$tools\bin\nsh.exe" --version }
if ("$have" -notlike "*($($pin.Substring(0,7)))*" -or -not (Test-Path "$tools\nsh-ref") -or (Get-Content "$tools\nsh-ref" -Raw).Trim() -ne $pin) {
    cargo install --locked --path "$checkout\crates\nsh-cli" --root $tools --force
    Assert-Native 'NSH CLI install failed'
    Set-Content "$tools\nsh-ref" $pin
}
Write-Step 'Installing MSYS2 Perl and make for Android vendored OpenSSL'
if (-not (Test-Path 'C:\tools\msys64\usr\bin\pacman.exe')) {
    choco install msys2 -y --no-progress
    Assert-Native 'MSYS2 install failed'
}
& C:\tools\msys64\usr\bin\pacman.exe -S --needed --noconfirm perl make
Assert-Native 'MSYS2 Perl/make install failed'
$llvm = "$ndkDir\toolchains\llvm\prebuilt\windows-x86_64\bin"
$env:Path = "C:\tools\msys64\usr\bin;$llvm;$env:Path"
$env:PERL = 'C:/tools/msys64/usr/bin/perl.exe'
Remove-Item Env:PERL5LIB -ErrorAction SilentlyContinue
$env:GNUMAKEFLAGS = 'CC=clang.exe AR=llvm-ar.exe RANLIB=llvm-ranlib.exe'
foreach ($triple in 'x86_64-linux-android', 'aarch64-linux-android', 'armv7-linux-androideabi') {
    [Environment]::SetEnvironmentVariable("RANLIB_$triple", "$llvm\llvm-ranlib.exe", 'Process')
}
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:X86_64_PC_WINDOWS_MSVC_OPENSSL_NO_VENDOR = '1'
$variants = @(@('debug-dev', 'bin-dev', 'dev', '--dev'), @('release', 'bin', 'release', '--release'))
foreach ($variant in $variants) {
    $source = "$checkout\target\android\$($variant[0])"
    $dest = "$tools\android\$($variant[1])"
    $runtime = "$tools\runtime\android\$($variant[2])"
    if ((Test-AndroidRuntime $dest $pin) -and (Test-AndroidRuntime $runtime $pin)) { continue }
    Push-Location $checkout
    try {
        Write-Step "Building Android $($variant[2]) runtime"
        cargo xtask android $($variant[3])
        Assert-Native "Android $($variant[2]) runtime build failed"
    } finally { Pop-Location }
    New-Item -ItemType Directory -Force $dest, $runtime | Out-Null
    foreach ($install in $dest, $runtime) {
        Remove-Item "$install\nsh-ref" -Force -ErrorAction SilentlyContinue
        Copy-Item "$source\*" $install -Recurse -Force
        Set-Content "$install\nsh-ref" $pin
    }
}
if ($Run) { & "$PSScriptRoot\run-android-windows.ps1" -SdkRoot $SdkRoot }
Write-Step 'Done. Use scripts\run-android-windows.ps1 to launch Android against mock-account.'
