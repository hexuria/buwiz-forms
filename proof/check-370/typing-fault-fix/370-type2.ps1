param([string]$digits, [int]$delay, [string]$tag)
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
foreach ($i in 1..20) { [System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 60 }
Start-Sleep -Milliseconds 400
if ($delay -eq 0) { [System.Windows.Forms.SendKeys]::SendWait($digits); Start-Sleep -Milliseconds 400 }
else { foreach ($k in $digits.ToCharArray()) { [System.Windows.Forms.SendKeys]::SendWait([string]$k); Start-Sleep -Milliseconds $delay } }
$bmp0 = New-Object System.Drawing.Bitmap 1920,1080
$g0 = [System.Drawing.Graphics]::FromImage($bmp0)
$g0.CopyFromScreen(0,0,0,0,(New-Object System.Drawing.Size 1920,1080))
$c0 = $bmp0.Clone((New-Object System.Drawing.Rectangle 1440,140,480,180), $bmp0.PixelFormat)
$c0.Save("$out\$tag-inbox.png"); $c0.Dispose(); $g0.Dispose(); $bmp0.Dispose()
[System.Windows.Forms.SendKeys]::SendWait('{TAB}')
Start-Sleep -Milliseconds 5000
$bmp = New-Object System.Drawing.Bitmap 1920,1080
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen(0,0,0,0,(New-Object System.Drawing.Size 1920,1080))
$crop = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,180), $bmp.PixelFormat)
$crop.Save("$out\$tag-aftertab.png"); $crop.Dispose(); $g.Dispose(); $bmp.Dispose()
$s = ($py | & "C:\Program Files\Python313\python.exe" -)
"$tag typed $digits delay=$delay -> saved $s" | Out-File "$out\results-fix.txt" -Append
"$tag -> saved $s"
