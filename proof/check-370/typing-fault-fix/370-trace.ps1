param([string]$digits, [int]$delay, [string]$tag, [string]$prekeys = '')
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
$out = "C:\Users\Administrator\47-matrix"
$py = @'
import sqlite3,json
db=sqlite3.connect(r"C:\Users\Administrator\370fix-scratch.db")
rows=list(db.execute("select data_json from form_drafts"))
d=json.loads(rows[-1][0])
s=d["schedule_1"][0]
print(f"{s['taxable_amount']}|{s['tax_due']}")
'@
function Crop($name) {
  $bmp = New-Object System.Drawing.Bitmap 1920,1080
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen(0,0,0,0,(New-Object System.Drawing.Size 1920,1080))
  $c = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,90), $bmp.PixelFormat)
  $c.Save("$out\$name.png"); $c.Dispose(); $g.Dispose(); $bmp.Dispose()
}
if ($prekeys -ne '') { foreach ($k in $prekeys.Split(',')) { [System.Windows.Forms.SendKeys]::SendWait($k); Start-Sleep -Milliseconds 250 } }
Crop("$tag-key0-start")
$i = 0
foreach ($k in $digits.ToCharArray()) {
  $i++
  [System.Windows.Forms.SendKeys]::SendWait([string]$k)
  Start-Sleep -Milliseconds $delay
  Crop("$tag-key$i-$k")
}
[System.Windows.Forms.SendKeys]::SendWait('{TAB}')
Start-Sleep -Milliseconds 5000
Crop("$tag-aftertab")
$s = ($py | & "C:\Program Files\Python313\python.exe" -)
"$tag typed $digits prekeys=$prekeys -> saved $s" | Out-File "$out\results-fix.txt" -Append
"$tag -> saved $s"
