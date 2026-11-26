Add-Type -TypeDefinition '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool SetForegroundWindow(System.IntPtr h); [System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool ShowWindow(System.IntPtr h,int c);' -Name U32c -Namespace W
$p = Get-Process nsh-app-nsh-app-ebirforms-windows-host -ErrorAction SilentlyContinue | Select-Object -First 1
if ($p -and $p.MainWindowHandle -ne 0) { [W.U32c]::ShowWindow($p.MainWindowHandle,9) | Out-Null; [W.U32c]::SetForegroundWindow($p.MainWindowHandle) | Out-Null; 'foregrounded' } else { 'no window' }
