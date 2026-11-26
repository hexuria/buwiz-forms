param([string]$digits,[string]$tag,[string]$mode='type')
# requires: focus already in the target field. captures per-key crops of the field region.
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
$out = "C:\Users\Administrator\main-keytrace\$tag"
New-Item -ItemType Directory -Force $out | Out-Null
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
  $crop.Save("$out\pasted.png")
  $crop.Dispose()
} else {
  $i = 0
  foreach ($k in $digits.ToCharArray()) {
    $i++
    [System.Windows.Forms.SendKeys]::SendWait([string]$k)
    Start-Sleep -Milliseconds 300
    $g.CopyFromScreen(0,0,0,0,$bmp.Size)
    $crop = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,80), $bmp.PixelFormat)
    $crop.Save("$out\key$i.png")
    $crop.Dispose()
  }
}
[System.Windows.Forms.SendKeys]::SendWait('{TAB}')
Start-Sleep -Milliseconds 700
$g.CopyFromScreen(0,0,0,0,$bmp.Size)
$crop2 = $bmp.Clone((New-Object System.Drawing.Rectangle 1440,140,480,180), $bmp.PixelFormat)
$crop2.Save("$out\aftertab.png")
$crop2.Dispose()
$g.Dispose(); $bmp.Dispose()
"$tag done"
