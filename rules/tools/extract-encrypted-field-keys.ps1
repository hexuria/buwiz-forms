param(
    [Parameter(Mandatory = $true)][string]$SourcePath,
    [Parameter(Mandatory = $true)][string]$RedactedSourcePath,
    [Parameter(Mandatory = $true)][string]$FormId,
    [Parameter(Mandatory = $true)][string]$ExpectedCiphertextSha256,
    [string]$ExpectedDecryptedSha256,
    [int]$ExpectedFieldCount = -1,
    [string]$ExpectedFieldInventorySha256,
    [string]$ExpectedOrderedFieldInventorySha256,
    [switch]$Discovery
)

$ErrorActionPreference = 'Stop'
if (-not $Discovery -and (
    [string]::IsNullOrWhiteSpace($ExpectedDecryptedSha256) -or
    $ExpectedFieldCount -lt 0 -or
    [string]::IsNullOrWhiteSpace($ExpectedFieldInventorySha256)
)) {
    throw 'Pinned mode requires the decrypted hash, field count, and inventory hash.'
}
if (-not (Test-Path -LiteralPath $SourcePath -PathType Leaf)) { throw "Missing encrypted source: $SourcePath" }
$ciphertext = [IO.File]::ReadAllBytes($SourcePath)
$cipherHash = (Get-FileHash -LiteralPath $SourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($cipherHash -ne $ExpectedCiphertextSha256) { throw 'Encrypted source hash changed.' }

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
    $chainingValue = [byte[]]$iv.Clone()
    $decryptor = $aes.CreateDecryptor()
    $offset = 0
    while ($offset + 16 -le $ciphertext.Length) {
        $block = New-Object byte[] 16
        [void]$decryptor.TransformBlock($ciphertext, $offset, 16, $block, 0)
        for ($index = 0; $index -lt 16; $index++) {
            $compressed[$offset + $index] = $block[$index] -bxor $chainingValue[$index]
            $chainingValue[$index] = $ciphertext[$offset + $index]
        }
        $offset += 16
    }
    if ($offset -lt $ciphertext.Length) {
        $streamBlock = New-Object byte[] 16
        $tailEncryptor = $aes.CreateEncryptor()
        [void]$tailEncryptor.TransformBlock($chainingValue, 0, 16, $streamBlock, 0)
        for ($index = 0; $index -lt ($ciphertext.Length - $offset); $index++) {
            $compressed[$offset + $index] = $ciphertext[$offset + $index] -bxor $streamBlock[$index]
        }
    }

    $zlibHeader = ('{0:x2}{1:x2}' -f $compressed[0], $compressed[1])
    if ($zlibHeader -ne '78da') { throw "Unexpected zlib header: $zlibHeader" }
    $input = New-Object IO.MemoryStream(, $compressed[2..($compressed.Length - 5)])
    $deflate = New-Object IO.Compression.DeflateStream($input, [IO.Compression.CompressionMode]::Decompress)
    $output = New-Object IO.MemoryStream
    try { $deflate.CopyTo($output) }
    finally { $deflate.Dispose(); $input.Dispose() }
    $plaintextBytes = $output.ToArray()
    $output.Dispose()

    $plainHash = ([BitConverter]::ToString($sha.ComputeHash($plaintextBytes))).Replace('-', '').ToLowerInvariant()
    if (-not $Discovery -and $plainHash -ne $ExpectedDecryptedSha256) { throw 'Decrypted payload hash changed.' }
    $xml = [Text.Encoding]::UTF8.GetString($plaintextBytes)
    $keys = @([regex]::Matches($xml, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') | ForEach-Object { $_.Groups['key'].Value })
    if (-not $Discovery -and ($keys.Count -ne $ExpectedFieldCount -or @($keys | Sort-Object -Unique).Count -ne $ExpectedFieldCount)) {
        throw "Expected $ExpectedFieldCount unique fields; found $($keys.Count)."
    }
    if ($Discovery -and @($keys | Sort-Object -Unique).Count -ne $keys.Count) {
        throw "Discovery found duplicate field keys: $($keys.Count) total versus $(@($keys | Sort-Object -Unique).Count) unique."
    }
    $sortedKeys = @($keys | Sort-Object)
    $inventoryHash = ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($sortedKeys -join "`n"))))).Replace('-', '').ToLowerInvariant()
    if (-not $Discovery -and $inventoryHash -ne $ExpectedFieldInventorySha256) { throw 'Field inventory hash changed.' }
    $orderedInventoryHash = ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($keys -join "`n"))))).Replace('-', '').ToLowerInvariant()
    if (-not [string]::IsNullOrWhiteSpace($ExpectedOrderedFieldInventorySha256) -and
        $orderedInventoryHash -ne $ExpectedOrderedFieldInventorySha256) {
        throw 'Ordered field inventory hash changed.'
    }

    [ordered]@{
        schema_version = '1.0.0'
        form_id = $FormId
        source_path = $RedactedSourcePath
        ciphertext_sha256 = $cipherHash
        decrypted_sha256 = $plainHash
        field_count = $keys.Count
        unique_field_count = $keys.Count
        field_inventory_sha256 = $inventoryHash
        ordered_field_inventory_sha256 = $orderedInventoryHash
        values_emitted = $false
        keys = $keys
    } | ConvertTo-Json -Depth 5
}
finally {
    $aes.Dispose()
    $sha.Dispose()
}
