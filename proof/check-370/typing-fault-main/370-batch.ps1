# runs probe list through 370-one.ps1 logic inline; writes results-batch.txt
$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type -TypeDefinition '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetCursorPos(int X,int Y); [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern void mouse_event(int f,int x,int y,int d,int u); [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr h); [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr h,int c);' -Name U32 -Namespace W
$out = "C:\Users\Administrator\47-matrix"
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
function Focus-Field {
  $proc = Get-Process nsh-app-nsh-app-ebirforms-windows-host -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($proc -and $proc.MainWindowHandle -ne 0) {
    [W.U32]::ShowWindow($proc.MainWindowHandle, 9) | Out-Null
    [W.U32]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
    Start-Sleep -Milliseconds 400
  }
  foreach ($n in 1..2) {
    [W.U32]::SetCursorPos(1669,172) | Out-Null
    Start-Sleep -Milliseconds 150
    [W.U32]::mouse_event(0x0002,0,0,0,0); Start-Sleep -Milliseconds 50
    [W.U32]::mouse_event(0x0004,0,0,0,0)
    Start-Sleep -Milliseconds 450
  }
}
function Saved {
$py = @'
import sqlite3,json
db=sqlite3.connect(r"C:\Users\Administrator\370main-scratch.db")
rows=list(db.execute("select data_json from form_drafts"))
d=json.loads(rows[-1][0])
s=d["schedule_1"][0]
print(f"{s['taxable_amount']}|{s['tax_due']}")
'@
  ($py | & "C:\Program Files\Python313\python.exe" -)
}
function Shot($name) {
  $bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen(0,0,0,0,$bmp.Size)
  $crop = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,180), $bmp.PixelFormat)
  $crop.Save("$out\$name.png"); $crop.Dispose(); $g.Dispose(); $bmp.Dispose()
}
function Probe($digits,$tag,$delay) {
  Focus-Field
  [System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 150
  [System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 300
  if ($delay -eq 0) {
    [System.Windows.Forms.SendKeys]::SendWait($digits); Start-Sleep -Milliseconds 400
  } else {
    foreach ($k in $digits.ToCharArray()) { [System.Windows.Forms.SendKeys]::SendWait([string]$k); Start-Sleep -Milliseconds $delay }
  }
  [System.Windows.Forms.SendKeys]::SendWait('{TAB}')
  Start-Sleep -Milliseconds 1600
  Shot("$tag-aftertab")
  $s = Saved
  "$tag typed $digits -> saved $s" | Out-File "$out\results-batch.txt" -Append
}
$values = @('99999999.99','12345678.90','123456789.12','1000000.005','1234567.89')
foreach ($v in $values) { Probe $v "sl-$v" 120 }
foreach ($v in $values) { Probe $v "bu-$v" 0 }
# mid-string probes
Focus-Field
[System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 300
foreach ($k in '1234567890'.ToCharArray()) { [System.Windows.Forms.SendKeys]::SendWait([string]$k); Start-Sleep -Milliseconds 120 }
[System.Windows.Forms.SendKeys]::SendWait('{LEFT}'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{LEFT}'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait('{TAB}'); Start-Sleep -Milliseconds 1600
Shot("midbs-aftertab")
$s = Saved
"slow 1234567890 Left Left Backspace -> saved $s" | Out-File "$out\results-batch.txt" -Append
Focus-Field
[System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 300
foreach ($k in '1234567890'.ToCharArray()) { [System.Windows.Forms.SendKeys]::SendWait([string]$k); Start-Sleep -Milliseconds 120 }
[System.Windows.Forms.SendKeys]::SendWait('{LEFT}'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{LEFT}'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('5'); Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait('{TAB}'); Start-Sleep -Milliseconds 1600
Shot("mid5-aftertab")
$s = Saved
"slow 1234567890 Left Left 5 -> saved $s" | Out-File "$out\results-batch.txt" -Append
"batch done"
