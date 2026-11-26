Add-Type -AssemblyName System.Windows.Forms
foreach ($i in 1..20) { [System.Windows.Forms.SendKeys]::SendWait('{BS}'); Start-Sleep -Milliseconds 60 }
[System.Windows.Forms.SendKeys]::SendWait('{TAB}')
Start-Sleep -Milliseconds 3500
"cleared"
