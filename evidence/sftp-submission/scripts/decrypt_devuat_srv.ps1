# Reproduces ebfSFTP.Program.Decrypt against the client's own DEV/UAT `srv` blob.
# Validates the credential-wrap crypto with a NON-PRODUCTION vector.
# Expected output: ftp2.birgovph.com  (17 bytes, no BOM)
$pass = 'Carlo*TSSD2!018'
$b64  = '25s+rBZx/AO+YuDjzPzIBx81hOVx4fhdnOHNysmXar3RpmRnduhtuxoasmUEANldVjKUeaebvHyefVvj5aJQ/+hCjBdF+xwd7GFdWWSbqL8='
$all  = [Convert]::FromBase64String($b64)
$salt = $all[0..31]; $iv = $all[32..47]; $ct = $all[48..($all.Length-1)]
$kdf  = New-Object System.Security.Cryptography.Rfc2898DeriveBytes($pass, [byte[]]$salt, 100000)  # PBKDF2-HMAC-SHA1
$aes  = [System.Security.Cryptography.Aes]::Create()
$aes.KeySize = 256; $aes.BlockSize = 128
$aes.Mode = 'CBC'; $aes.Padding = 'PKCS7'
$aes.Key = $kdf.GetBytes(32); $aes.IV = [byte[]]$iv
$pt = $aes.CreateDecryptor().TransformFinalBlock([byte[]]$ct, 0, $ct.Length)
Write-Output ("srv = '{0}'  (len {1}, BOM={2})" -f `
  [Text.Encoding]::UTF8.GetString($pt), $pt.Length, `
  ($pt.Length -ge 3 -and $pt[0] -eq 0xEF -and $pt[1] -eq 0xBB -and $pt[2] -eq 0xBF))
