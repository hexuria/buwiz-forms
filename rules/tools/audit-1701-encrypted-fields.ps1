param(
    [string]$SourceDir = 'C:\Mac\Home\Downloads\forms\1701v2018',
    [string]$FormId = '1701-v2018',
    [string]$FilePattern = '00000000000000-1701v2018-122025#*#.xml',
    [string]$RedactedFileName = '00000000000000-1701v2018-122025#<email-redacted>#.xml',
    [string]$ExpectedCiphertextSha256 = '3771c99c191ef5e84b1b5e4c51499911bfbec6002febc3c53dca3f08730e92e3',
    [string]$ExpectedDecryptedSha256 = '95ee42ed78f104335f50168a40e207f8af71ddf8eced9ddd0db1ad42d6366800',
    [int]$ExpectedFieldCount = 838,
    [string]$ExpectedFieldInventorySha256 = '*',
    [string]$ExpectedExtraField = 'frm1701:txtPg1I9Address2',
    [string]$VersionField = 'frm1701:txtVersion',
    [string]$ExpectedXmlVersion = '051414'
)

$ErrorActionPreference = 'Stop'
$candidate = @(Get-ChildItem -LiteralPath $SourceDir -File | Where-Object { $_.Name -like $FilePattern })
if ($candidate.Count -ne 1) { throw "Expected one reviewed encrypted companion; found $($candidate.Count)." }
$path = $candidate[0].FullName
$ciphertext = [IO.File]::ReadAllBytes($path)

$sha = [Security.Cryptography.SHA256]::Create()
$aes = [Security.Cryptography.Aes]::Create()
try {
    $key = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes('T0081gP45sy0rd-To+R3m3m63r!@4/<>'))
    $aes.Mode = [Security.Cryptography.CipherMode]::ECB
    $aes.Padding = [Security.Cryptography.PaddingMode]::None
    $aes.Key = $key

    $zero = New-Object byte[] 16
    $encryptor = $aes.CreateEncryptor()
    $iv = New-Object byte[] 16
    [void]$encryptor.TransformBlock($zero, 0, 16, $iv, 0)

    $compressed = New-Object byte[] $ciphertext.Length
    $cv = [byte[]]$iv.Clone()
    $decryptor = $aes.CreateDecryptor()
    $offset = 0
    while ($offset + 16 -le $ciphertext.Length) {
        $block = New-Object byte[] 16
        [void]$decryptor.TransformBlock($ciphertext, $offset, 16, $block, 0)
        for ($index = 0; $index -lt 16; $index++) {
            $compressed[$offset + $index] = $block[$index] -bxor $cv[$index]
            $cv[$index] = $ciphertext[$offset + $index]
        }
        $offset += 16
    }
    if ($offset -lt $ciphertext.Length) {
        $streamBlock = New-Object byte[] 16
        $tailEncryptor = $aes.CreateEncryptor()
        [void]$tailEncryptor.TransformBlock($cv, 0, 16, $streamBlock, 0)
        for ($index = 0; $index -lt ($ciphertext.Length - $offset); $index++) {
            $compressed[$offset + $index] = $ciphertext[$offset + $index] -bxor $streamBlock[$index]
        }
    }

    $zlibHeader = ('{0:x2}{1:x2}' -f $compressed[0], $compressed[1])
    $deflateBytes = $compressed[2..($compressed.Length - 5)]
    $input = New-Object IO.MemoryStream(,$deflateBytes)
    $deflate = New-Object IO.Compression.DeflateStream($input, [IO.Compression.CompressionMode]::Decompress)
    $output = New-Object IO.MemoryStream
    try { $deflate.CopyTo($output) } finally { $deflate.Dispose(); $input.Dispose() }
    $plaintextBytes = $output.ToArray()
    $output.Dispose()
    $xml = [Text.Encoding]::UTF8.GetString($plaintextBytes)
    $matches = @([regex]::Matches($xml, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>'))
    $keys = @($matches | ForEach-Object { $_.Groups['key'].Value })
    $cipherHash = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    $plainHash = ([BitConverter]::ToString($sha.ComputeHash($plaintextBytes))).Replace('-', '').ToLowerInvariant()
    $inventoryBytes = [Text.Encoding]::UTF8.GetBytes((($keys | Sort-Object) -join "`n"))
    $fieldInventoryHash = ([BitConverter]::ToString($sha.ComputeHash($inventoryBytes))).Replace('-', '').ToLowerInvariant()

    if ($ExpectedCiphertextSha256 -and $ExpectedCiphertextSha256 -ne '*' -and $cipherHash -ne $ExpectedCiphertextSha256) { throw 'Encrypted companion hash changed.' }
    if ($zlibHeader -ne '78da') { throw "Unexpected zlib header: $zlibHeader" }
    if ($ExpectedDecryptedSha256 -and $ExpectedDecryptedSha256 -ne '*' -and $plainHash -ne $ExpectedDecryptedSha256) { throw 'Decrypted payload hash changed.' }
    if ($ExpectedFieldCount -gt 0 -and ($keys.Count -ne $ExpectedFieldCount -or ($keys | Sort-Object -Unique).Count -ne $ExpectedFieldCount)) { throw "Expected $ExpectedFieldCount unique decrypted fields; found $($keys.Count)." }
    if ($ExpectedFieldInventorySha256 -and $ExpectedFieldInventorySha256 -ne '*' -and $fieldInventoryHash -ne $ExpectedFieldInventorySha256) { throw 'Decrypted field inventory hash changed.' }
    if ($ExpectedExtraField -and $ExpectedExtraField -ne '*' -and $keys -notcontains $ExpectedExtraField) { throw 'Expected encrypted-only field is absent.' }
    $versionMatch = $matches | Where-Object { $_.Groups['key'].Value -eq $VersionField } | Select-Object -First 1
    $observedVersion = if ($versionMatch) { $versionMatch.Groups['value'].Value } else { $null }
    if ($ExpectedXmlVersion -and $ExpectedXmlVersion -ne '*' -and $observedVersion -ne $ExpectedXmlVersion) { throw 'Reviewed XML version marker changed.' }

    [ordered]@{
        schema_version = '1.0.0'
        form_id = $FormId
        source_path = (Join-Path $SourceDir $RedactedFileName)
        ciphertext_sha256 = $cipherHash
        zlib_header = $zlibHeader
        decrypted_byte_count = $plaintextBytes.Length
        decrypted_sha256 = $plainHash
        field_count = $keys.Count
        unique_field_count = ($keys | Sort-Object -Unique).Count
        field_inventory_sha256 = $fieldInventoryHash
        encrypted_only_field = if ($ExpectedExtraField -eq '*') { $null } else { $ExpectedExtraField }
        xml_version = $observedVersion
        values_emitted = $false
    } | ConvertTo-Json -Depth 5
} finally {
    $aes.Dispose()
    $sha.Dispose()
}
