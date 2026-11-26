param([string]$digits,[string]$tag,[int]$delay=120)
# focus must already be in the field (caller clicked). types, captures, reads saved after 1.6s settle.
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
$out = "C:\Users\Administrator\47-matrix"
[System.Windows.Forms.SendKeys]::SendWait('^a')
Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{BS}')
Start-Sleep -Milliseconds 300
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$g = [System.Drawing.Graphics]::FromImage($bmp)
if ($delay -eq 0) {
  [System.Windows.Forms.SendKeys]::SendWait($digits)
  Start-Sleep -Milliseconds 400
} else {
  foreach ($k in $digits.ToCharArray()) {
    [System.Windows.Forms.SendKeys]::SendWait([string]$k)
    Start-Sleep -Milliseconds $delay
  }
}
$g.CopyFromScreen(0,0,0,0,$bmp.Size)
$crop = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,80), $bmp.PixelFormat)
$crop.Save("$out\$tag-inbox.png"); $crop.Dispose()
[System.Windows.Forms.SendKeys]::SendWait('{TAB}')
Start-Sleep -Milliseconds 4000
$g.CopyFromScreen(0,0,0,0,$bmp.Size)
$crop2 = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,180), $bmp.PixelFormat)
$crop2.Save("$out\$tag-aftertab.png"); $crop2.Dispose()
$g.Dispose(); $bmp.Dispose()
$py = @'
import sqlite3,json
db=sqlite3.connect(r"C:\Users\Administrator\370main-scratch.db")
rows=list(db.execute("select data_json from form_drafts"))
d=json.loads(rows[-1][0])
s=d["schedule_1"][0]
print(f"saved {s['taxable_amount']}|{s['tax_due']}")
'@
$r = ($py | & "C:\Program Files\Python313\python.exe" -)
"$tag typed $digits -> $r" | Out-File "$out\results2.txt" -Append
"$tag -> $r"
