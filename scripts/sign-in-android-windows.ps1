param([string]$SdkRoot = 'C:\Android', [string]$Serial = 'emulator-5554')
$ErrorActionPreference = 'Stop'

function Get-AccountUiState([xml]$ui) {
    $package = 'dev.goldcoders.bir'
    if (-not $ui.SelectNodes("//*[@package='$package' and (@text!='' or @content-desc!='')]").Count) { return @{ State = 'blocked' } }
    if ($ui.SelectSingleNode("//*[@package='$package' and (@text='Add your first taxpayer' or @content-desc='Add your first taxpayer')]")) { return @{ State = 'ready' } }
    if (-not $ui.SelectSingleNode("//*[@text='Welcome to eBIRForms']")) { return @{ State = 'ready' } }
    $button = $ui.SelectSingleNode("//*[@package='$package' and @content-desc='Sign in' and @clickable='true' and @enabled='true']")
    if (-not $button) { return @{ State = 'blocked' } }
    if ($button.bounds -notmatch '^\[(\d+),(\d+)\]\[(\d+),(\d+)\]$') { throw 'Sign in button has invalid bounds' }
    return @{ State = 'gate'; X = [int](([int]$Matches[1] + [int]$Matches[3]) / 2); Y = [int](([int]$Matches[2] + [int]$Matches[4]) / 2) }
}

$adb = "$SdkRoot\platform-tools\adb.exe"
& $adb -s $Serial shell settings put system screen_off_timeout 1800000
& $adb -s $Serial shell locksettings set-disabled true | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Could not disable the development emulator lock screen' }
& $adb -s $Serial shell input keyevent KEYCODE_WAKEUP
& $adb -s $Serial shell wm dismiss-keyguard
$deadline = (Get-Date).AddMinutes(5)
$started = $false
do {
    & $adb -s $Serial shell am start -n dev.goldcoders.bir/com.longbridge.nsh.AppActivity | Out-Null
    & $adb -s $Serial shell rm -f /sdcard/buwiz-account-ui.xml
    & $env:ComSpec /d /c "`"$adb`" -s $Serial shell uiautomator dump /sdcard/buwiz-account-ui.xml 2>nul" | Out-Null
    $raw = & $env:ComSpec /d /c "`"$adb`" -s $Serial shell cat /sdcard/buwiz-account-ui.xml 2>nul"
    if ($raw -and "$raw".StartsWith('<?xml')) {
        [xml]$ui = $raw -join "`n"
        $state = Get-AccountUiState $ui
        if ($state.State -eq 'ready') {
            Write-Host 'Mock account ready: Android account gate cleared.'
            return
        }
        $systemUiAnr = $ui.SelectSingleNode('//*[@text="System UI isn''t responding"]')
        if ($systemUiAnr) {
            $wait = $ui.SelectSingleNode('//*[@resource-id="android:id/aerr_wait"]')
            if ($wait -and $wait.bounds -match '^\[(\d+),(\d+)\]\[(\d+),(\d+)\]$') {
                & $adb -s $Serial shell input tap ([int](([int]$Matches[1] + [int]$Matches[3]) / 2)) ([int](([int]$Matches[2] + [int]$Matches[4]) / 2))
            }
        } elseif ($state.State -eq 'gate' -and -not $started) {
            & $adb -s $Serial shell input tap $state.X $state.Y
            if ($LASTEXITCODE -ne 0) { throw 'Could not press Android Sign in' }
            $started = $true
        }
    }
    Start-Sleep 2
} while ((Get-Date) -lt $deadline)
throw 'Android mock sign-in timed out; inspect the emulator and mock-account logs.'
