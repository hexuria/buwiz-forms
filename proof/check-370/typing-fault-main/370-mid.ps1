param([string]$keys, [string]$midkeys, [string]$tag)
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
$out = "C:\Users\Administrator\47-matrix"
$py = @'
import sqlite3,json
db=sqlite3.connect(r"C:\Users\Administrator\370main-scratch.db")
rows=list(db.execute("select data_json from form_drafts"))
d=json.loads(rows[-1][0])
s=d["schedule_1"][0]
print(f"{s['taxable_amount']}|{s['tax_due']}")
'@
[System.Windows.Forms.SendKeys]::SendWait('^a'); Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 300
foreach ($k in $keys.ToCharArray()) { [System.Windows.Forms.SendKeys]::SendWait([string]$k); Start-Sleep -Milliseconds 100 }
foreach ($k in $midkeys.Split(',')) { [System.Windows.Forms.SendKeys]::SendWait($k); Start-Sleep -Milliseconds 200 }
[System.Windows.Forms.SendKeys]::SendWait('{TAB}')
Start-Sleep -Milliseconds 4500
$bmp = New-Object System.Drawing.Bitmap 1920,1080
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen(0,0,0,0,(New-Object System.Drawing.Size 1920,1080))
$crop = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,180), $bmp.PixelFormat)
$crop.Save("$out\$tag-aftertab.png"); $crop.Dispose(); $g.Dispose(); $bmp.Dispose()
$s = ($py | & "C:\Program Files\Python313\python.exe" -)
"$tag keys=$keys mid=$midkeys -> saved $s" | Out-File "$out\results-rest.txt" -Append
"$tag -> saved $s"
