<#
.SYNOPSIS
    Package the x86_64 development APK and cold-boot buwiz_api35 against mock-account.
#>
param([string]$SdkRoot = 'C:\Android', [switch]$SkipIfRebootPending)
$ErrorActionPreference = 'Stop'
function Assert-Native($message) { if ($LASTEXITCODE -ne 0) { throw "$message (exit $LASTEXITCODE)" } }
function Test-HardwareVirtualization {
    try {
        $processors = @(Get-CimInstance Win32_Processor -ErrorAction Stop)
        $processors.Count -gt 0 -and @($processors | Where-Object {
            -not $_.VirtualizationFirmwareEnabled -or -not $_.VMMonitorModeExtensions
        }).Count -eq 0
    } catch { return $false }
}
function Test-WHPXRebootPending {
    $marker = "$env:USERPROFILE\BuwizNsh\android-windows\whpx-reboot-boot-time"
    if (Test-Path $marker) {
        $boot = (Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime().ToString('o')
        $recordedBoot = Get-Content $marker -Raw
        if ($recordedBoot -and $recordedBoot.Trim() -eq $boot) { return $true }
    }
    try {
        (Get-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -ErrorAction Stop).State -eq 'EnablePending'
    } catch { return $false }
}
function Assert-MockAccountListener($listeners) {
    foreach ($listener in $listeners) {
        $process = Get-CimInstance Win32_Process -Filter "ProcessId=$($listener.OwningProcess)"
        if ($process.Name -ne 'mock_account.exe' -or $process.CommandLine -notmatch '(?:^|\s)--auto-approve(?:\s|$)') {
            throw "Port 3008 is owned by PID $($listener.OwningProcess), not mock-account --auto-approve. Stop that listener and rerun this script, or restart it with just mock-account --auto-approve."
        }
    }
}
$repo = Split-Path $PSScriptRoot -Parent
$tools = "$env:USERPROFILE\BuwizNsh\tools"
$work = "$env:USERPROFILE\BuwizNsh\android-windows"
New-Item -ItemType Directory -Force $work | Out-Null
& "$SdkRoot\emulator\emulator.exe" -accel-check
if ($LASTEXITCODE -ne 0) {
    if (Test-WHPXRebootPending) {
        $message = 'WHPX was enabled during setup and Windows must restart first (Restart-Computer), then rerun scripts\run-android-windows.ps1.'
        if (-not (Test-HardwareVirtualization)) {
            $message += ' VT-x could not be verified before the reboot; CPU capability flags can be hidden by a hypervisor. Acceleration must be checked again after restarting.'
        }
        if ($SkipIfRebootPending) { Write-Warning $message; exit 0 }
        throw $message
    }
    Assert-Native 'Hardware acceleration is required; enable nested virtualization and Windows Hypervisor Platform'
}
Assert-MockAccountListener (Get-NetTCPConnection -LocalPort 3008 -State Listen -ErrorAction SilentlyContinue)
$env:Path = "$tools\bin;$env:USERPROFILE\.cargo\bin;$SdkRoot\jdk17\bin;$SdkRoot\platform-tools;" + [Environment]::GetEnvironmentVariable('Path','Machine') + ';' + [Environment]::GetEnvironmentVariable('Path','User')
foreach ($k in 'OPENSSL_DIR', 'OPENSSL_LIB_DIR') {
    [Environment]::SetEnvironmentVariable($k, [Environment]::GetEnvironmentVariable($k, 'User'), 'Process')
}
$env:JAVA_HOME = "$SdkRoot\jdk17"
$env:ANDROID_HOME = $SdkRoot
$env:ANDROID_NDK_HOME = "$SdkRoot\ndk\27.3.13750724"
$env:LIBCLANG_PATH = 'C:\Program Files\LLVM\bin'
$llvm = "$env:ANDROID_NDK_HOME\toolchains\llvm\prebuilt\windows-x86_64\bin"
$env:Path = "C:\tools\msys64\usr\bin;$llvm;$env:Path"
$env:PERL = 'C:/tools/msys64/usr/bin/perl.exe'
$env:X86_64_PC_WINDOWS_MSVC_OPENSSL_NO_VENDOR = '1'
Remove-Item Env:PERL5LIB -ErrorAction SilentlyContinue
$env:GNUMAKEFLAGS = 'CC=clang.exe AR=llvm-ar.exe RANLIB=llvm-ranlib.exe'
$env:RANLIB_x86_64_linux_android = "$llvm\llvm-ranlib.exe"
[Environment]::SetEnvironmentVariable('RANLIB_x86_64-linux-android', "$llvm\llvm-ranlib.exe", 'Process')
$env:RUSTUP_TOOLCHAIN = '1.99.0'
$env:RUST_MIN_STACK = '268435456'
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS = 'true'
$env:BUWIZ_ANDROID_DEV = '1'
$env:BUWIZ_ANDROID_DEV_ENV = 'EBIR_TEST_ENV=1;BUWIZ_NO_KEYCHAIN=1;BIR_SFTP_DRY_RUN=1'
# The CLI resolves NSH_RUNTIME/android/dev; keep the requested bin-dev installation too.
New-Item -ItemType Directory -Force "$tools\runtime\android\dev" | Out-Null
Copy-Item "$tools\android\bin-dev\*" "$tools\runtime\android\dev" -Recurse -Force
New-Item -ItemType Directory -Force "$tools\runtime\android\release" | Out-Null
Copy-Item "$tools\android\bin\*" "$tools\runtime\android\release" -Recurse -Force
$env:NSH_RUNTIME = "$tools\runtime"
Push-Location "$repo\nsh"
try {
    nsh package --target android --abi x86_64 --out "$work\package"
    Assert-Native 'Android packaging failed'
} finally { Pop-Location }
$apk = "$work\package\eBIRForms.apk"
if (-not (Test-Path $apk)) { throw "Packaged APK missing: $apk" }
Add-Type -AssemblyName System.IO.Compression.FileSystem
$extract = "$work\repack"
if (Test-Path $extract) { Remove-Item $extract -Recurse -Force }
[IO.Compression.ZipFile]::ExtractToDirectory($apk, $extract)
Get-ChildItem "$extract\lib" -Directory | Where-Object Name -ne 'x86_64' | Remove-Item -Recurse -Force
if (-not (Test-Path "$extract\lib\x86_64\libnsh.so")) { throw 'APK has no x86_64 runtime' }
Remove-Item "$extract\META-INF" -Recurse -Force -ErrorAction SilentlyContinue
& "$llvm\llvm-strip.exe" --strip-unneeded "$extract\lib\x86_64\libnsh.so"
Assert-Native 'llvm-strip failed'
$unsigned = "$work\unsigned.apk"
Remove-Item $unsigned -Force -ErrorAction SilentlyContinue
& "$env:JAVA_HOME\bin\jar.exe" --create --no-manifest --file $unsigned --no-compress -C $extract .
Assert-Native 'APK repacking failed'
$aligned = "$work\x86_64.apk"
$buildTools = "$SdkRoot\build-tools\35.0.1"
& "$buildTools\zipalign.exe" -f -p 4 $unsigned $aligned
Assert-Native 'zipalign failed'
& "$buildTools\apksigner.bat" sign --ks "$env:USERPROFILE\.android\debug.keystore" --ks-key-alias androiddebugkey --ks-pass pass:android --key-pass pass:android $aligned
Assert-Native 'APK signing failed'
& "$buildTools\apksigner.bat" verify $aligned
Assert-Native 'APK signature verification failed'
Write-Host "x86_64 APK: $aligned ($((Get-Item $aligned).Length) bytes)"
$serial = 'emulator-5554'
$adb = "$SdkRoot\platform-tools\adb.exe"
$devices = & $adb devices
if ($devices -match "$serial\s") {
    $avdName = & $adb -s $serial emu avd name
    Assert-Native 'Could not identify the existing emulator AVD'
    if (-not $avdName -or ($avdName | Select-Object -First 1).Trim() -ne 'buwiz_api35') {
        throw "Refusing to stop $serial because it is not buwiz_api35. Close it or move it to another port before rerunning."
    }
    & $adb -s $serial emu kill
    Assert-Native 'Could not stop existing emulator for cold boot'
    $deadline = (Get-Date).AddMinutes(2)
    while ((& $adb devices) -match "$serial\s") {
        if ((Get-Date) -gt $deadline) { throw 'Existing emulator did not stop' }
        Start-Sleep 2
    }
}
Start-Process "$SdkRoot\emulator\emulator.exe" -ArgumentList '-avd', 'buwiz_api35', '-port', '5554', '-accel', 'on', '-feature', 'WindowsHypervisorPlatform', '-gpu', 'swiftshader_indirect', '-cores', '4', '-memory', '4096', '-no-snapshot', '-no-boot-anim' -RedirectStandardOutput "$work\emulator.log" -RedirectStandardError "$work\emulator.err.log"
$deadline = (Get-Date).AddMinutes(20)
do {
    if ((Get-Date) -gt $deadline) { throw "Emulator boot timed out; see $work\emulator.log" }
    Start-Sleep 5
    $boot = & $env:ComSpec /d /c "`"$adb`" -s $serial shell getprop sys.boot_completed 2>nul"
} until ($boot -and $boot.Trim() -eq '1')
Write-Host 'sys.boot_completed=1'
if (-not (Get-NetTCPConnection -LocalPort 3008 -State Listen -ErrorAction SilentlyContinue)) {
    $savedPerl = $env:PERL
    $savedMakeFlags = $env:GNUMAKEFLAGS
    Remove-Item Env:PERL, Env:GNUMAKEFLAGS
    try {
        Start-Process pwsh -WorkingDirectory $repo -ArgumentList '-NoProfile', '-Command', 'just mock-account --auto-approve' -RedirectStandardOutput "$work\mock-account.log" -RedirectStandardError "$work\mock-account.err.log"
    } finally {
        $env:PERL = $savedPerl
        $env:GNUMAKEFLAGS = $savedMakeFlags
    }
}
$deadline = (Get-Date).AddMinutes(15)
while (-not (Get-NetTCPConnection -LocalPort 3008 -State Listen -ErrorAction SilentlyContinue)) {
    if ((Get-Date) -gt $deadline) { throw "mock-account startup timed out; see $work\mock-account.err.log" }
    Start-Sleep 3
}
Assert-MockAccountListener (Get-NetTCPConnection -LocalPort 3008 -State Listen -ErrorAction SilentlyContinue)
& $adb -s $serial reverse tcp:3008 tcp:3008
Assert-Native 'adb reverse failed'
& $adb -s $serial push $aligned /data/local/tmp/buwiz-x86_64.apk
Assert-Native 'adb push failed'
$install = & $adb -s $serial shell pm install -r /data/local/tmp/buwiz-x86_64.apk
Write-Host $install
Assert-Native 'pm install failed'
if ($install -notcontains 'Success') { throw "pm install: $install" }
& $adb -s $serial shell rm /data/local/tmp/buwiz-x86_64.apk
& $adb -s $serial shell am start -n dev.goldcoders.bir.nsh/com.longbridge.nsh.AppActivity
Assert-Native 'App launch failed'
& "$PSScriptRoot\sign-in-android-windows.ps1" -SdkRoot $SdkRoot -Serial $serial
Write-Host "Emulator and mock-account left running. Logs: $work"
