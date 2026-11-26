param([string]$speed='slow')
# types each value into the focused-window field after self-clicking it; reads saved value from draft db.
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type -TypeDefinition '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetCursorPos(int X,int Y); [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern void mouse_event(int f,int x,int y,int d,int u);' -Name U32 -Namespace W
$out = "C:\Users\Administrator\47-matrix"
New-Item -ItemType Directory -Force $out | Out-Null
$results = "$out\results-$speed.txt"
"" | Out-File $results
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
function Click-Field($x,$y) {
  [W.U32]::SetCursorPos($x,$y) | Out-Null
  Start-Sleep -Milliseconds 120
  [W.U32]::mouse_event(0x0002,0,0,0,0); Start-Sleep -Milliseconds 50
  [W.U32]::mouse_event(0x0004,0,0,0,0)
  Start-Sleep -Milliseconds 400
}
function Shot($name) {
  $bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen(0,0,0,0,$bmp.Size)
  $crop = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,180), $bmp.PixelFormat)
  $crop.Save("$out\$name.png")
  $crop.Dispose(); $g.Dispose(); $bmp.Dispose()
}
function Saved() {
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
$values = @('99999999.99','12345678.90','123456789.12','1000000.005','1234567.89')
foreach ($v in $values) {
  Click-Field 1669 172
  [System.Windows.Forms.SendKeys]::SendWait('^a')
  Start-Sleep -Milliseconds 150
  [System.Windows.Forms.SendKeys]::SendWait('{BS}')
  Start-Sleep -Milliseconds 250
  if ($speed -eq 'burst') {
    [System.Windows.Forms.SendKeys]::SendWait($v)
    Start-Sleep -Milliseconds 400
  } else {
    foreach ($k in $v.ToCharArray()) {
      [System.Windows.Forms.SendKeys]::SendWait([string]$k)
      Start-Sleep -Milliseconds 120
    }
  }
  Shot("$speed-$v-inbox")
  [System.Windows.Forms.SendKeys]::SendWait('{TAB}')
  Start-Sleep -Milliseconds 700
  Shot("$speed-$v-aftertab")
  $s = Saved
  "$speed typed $v -> saved $s" | Out-File $results -Append
}
# middle-edit probes: type 1234567890 (10 digits), then Left x2 + Backspace; then fresh, Left x2 + type 5
Click-Field 1669 172
[System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 250
foreach ($k in '1234567890'.ToCharArray()) { [System.Windows.Forms.SendKeys]::SendWait([string]$k); Start-Sleep -Milliseconds 120 }
[System.Windows.Forms.SendKeys]::SendWait('{LEFT}'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{LEFT}'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 300
Shot("$speed-midbackspace-inbox")
[System.Windows.Forms.SendKeys]::SendWait('{TAB}'); Start-Sleep -Milliseconds 700
Shot("$speed-midbackspace-aftertab")
$s = Saved
"$speed 1234567890 then Left Left Backspace -> saved $s" | Out-File $results -Append
Click-Field 1669 172
[System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 250
foreach ($k in '1234567890'.ToCharArray()) { [System.Windows.Forms.SendKeys]::SendWait([string]$k); Start-Sleep -Milliseconds 120 }
[System.Windows.Forms.SendKeys]::SendWait('{LEFT}'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{LEFT}'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('5'); Start-Sleep -Milliseconds 300
Shot("$speed-middigit-inbox")
[System.Windows.Forms.SendKeys]::SendWait('{TAB}'); Start-Sleep -Milliseconds 700
Shot("$speed-middigit-aftertab")
$s = Saved
"$speed 1234567890 then Left Left 5 -> saved $s" | Out-File $results -Append
"matrix $speed done"
