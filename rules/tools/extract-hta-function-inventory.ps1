param(
    [Parameter(Mandatory = $true)]
    [string]$HtaPath,
    [string]$NamePattern = '.*',
    [string]$NamePrefixes = '',
    [string]$ControlPrefix = 'frm1701MS:'
)

$ErrorActionPreference = 'Stop'
$lines = Get-Content -LiteralPath $HtaPath
$starts = @()
for ($index = 0; $index -lt $lines.Count; $index++) {
    if ($lines[$index] -match '^\s*function\s+([A-Za-z0-9_]+)\s*\(') {
        $starts += [pscustomobject]@{ index = $index; name = $Matches[1] }
    }
}

$inventory = @()
for ($position = 0; $position -lt $starts.Count; $position++) {
    $entry = $starts[$position]
    $prefixMatch = $false
    if ($NamePrefixes) {
        foreach ($prefix in ($NamePrefixes -split ',')) {
            if ($entry.name.StartsWith($prefix.Trim(), [StringComparison]::OrdinalIgnoreCase)) { $prefixMatch = $true; break }
        }
    }
    if (($NamePrefixes -and -not $prefixMatch) -or (-not $NamePrefixes -and $entry.name -notmatch $NamePattern)) { continue }
    $endIndex = if ($position + 1 -lt $starts.Count) { $starts[$position + 1].index - 1 } else { $lines.Count - 1 }
    $body = ($lines[$entry.index..$endIndex] -join "`n")
    $alerts = @([regex]::Matches($body, '(?is)alert\s*\((?<expression>.*?)\)\s*;') | ForEach-Object {
        ($_.Groups['expression'].Value -replace '\s+', ' ').Trim()
    })
    $controlPattern = [regex]::Escape($ControlPrefix) + '[A-Za-z0-9_:]+'
    $controlIds = @([regex]::Matches($body, $controlPattern) | ForEach-Object Value | Sort-Object -Unique)
    $bytes = [Text.Encoding]::UTF8.GetBytes($body)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $bodyHash = ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
    $inventory += [pscustomobject][ordered]@{
        name = $entry.name
        start_line = $entry.index + 1
        end_line = $endIndex + 1
        body_sha256 = $bodyHash
        alerts = $alerts
        control_ids = $controlIds
    }
}

[ordered]@{
    source_sha256 = (Get-FileHash -LiteralPath $HtaPath -Algorithm SHA256).Hash.ToLowerInvariant()
    function_count = $inventory.Count
    functions = $inventory
} | ConvertTo-Json -Depth 8
