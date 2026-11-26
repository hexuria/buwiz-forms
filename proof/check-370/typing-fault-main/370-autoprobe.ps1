param([string]$tag,[string]$digits,[string]$mode='paste')
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type -TypeDefinition '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetCursorPos(int X,int Y); [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern void mouse_event(int f,int x,int y,int d,int u);' -Name U32 -Namespace W
$out = "C:\Users\Administrator\main-keytrace\$tag"
New-Item -ItemType Directory -Force $out | Out-Null
# click the taxable amount field: real coords 1669,172 (1920x1080)
[W.U32]::SetCursorPos(1669,172) | Out-Null
Start-Sleep -Milliseconds 150
[W.U32]::mouse_event(0x0002,0,0,0,0); Start-Sleep -Milliseconds 60
[W.U32]::mouse_event(0x0004,0,0,0,0)
Start-Sleep -Milliseconds 400
[System.Windows.Forms.SendKeys]::SendWait('^a')
Start-Sleep -Milliseconds 200
[System.Windows.Forms.SendKeys]::SendWait('{BS}')
Start-Sleep -Milliseconds 300
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
if ($mode -eq 'paste') {
  [System.Windows.Forms.Clipboard]::SetText($digits)
  Start-Sleep -Milliseconds 200
  [System.Windows.Forms.SendKeys]::SendWait('^v')
  Start-Sleep -Milliseconds 600
  $g.CopyFromScreen(0,0,0,0,$bmp.Size)
  $crop = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,80), $bmp.PixelFormat)
  $crop.Save("$out\pasted.png"); $crop.Dispose()
} else {
  $i = 0
  foreach ($k in $digits.ToCharArray()) {
    $i++
    [System.Windows.Forms.SendKeys]::SendWait([string]$k)
    Start-Sleep -Milliseconds 300
    $g.CopyFromScreen(0,0,0,0,$bmp.Size)
    $crop = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,80), $bmp.PixelFormat)
    $crop.Save("$out\key$i.png"); $crop.Dispose()
  }
}
[System.Windows.Forms.SendKeys]::SendWait('{TAB}')
Start-Sleep -Milliseconds 700
$g.CopyFromScreen(0,0,0,0,$bmp.Size)
$crop2 = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,180), $bmp.PixelFormat)
$crop2.Save("$out\aftertab.png"); $crop2.Dispose()
$g.Dispose(); $bmp.Dispose()
# read stored value from db via python inline
$py = @'
import sqlite3,json
db=sqlite3.connect(r"C:\Users\Administrator\370main-scratch.db")
rows=list(db.execute("select data_json from form_drafts"))
d=json.loads(rows[-1][0])
s=d["schedule_1"][0]
print(f"stored:{s['taxable_amount']} due:{s['tax_due']}")
'@
$py | & "C:\Program Files\Python313\python.exe" -
