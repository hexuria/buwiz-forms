param(
    [Parameter(Mandatory = $true)]
    [string]$Directory,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9A-Za-z-]+$')]
    [string]$FormId
)

$ErrorActionPreference = 'Stop'
$keyTool = Join-Path $PSScriptRoot 'extract-encrypted-field-keys.ps1'

function Get-LineInventoryHash([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes((@($Lines | Sort-Object) -join "`n"))
        ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

$samples = [Collections.Generic.List[object]]::new()
foreach ($file in Get-ChildItem -LiteralPath $Directory -File -Filter '*.xml') {
    $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $bytes = [IO.File]::ReadAllBytes($file.FullName)
    $text = [Text.Encoding]::UTF8.GetString($bytes)
    $plainKeys = @(
        [regex]::Matches($text, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') |
            ForEach-Object { $_.Groups['key'].Value }
    )

    if ($plainKeys.Count -gt 0) {
        $sample = [pscustomobject][ordered]@{
            source_path = Join-Path $Directory '#email-redacted#'
            sha256 = $hash
            encoding = 'plaintext'
            field_count = $plainKeys.Count
            unique_field_count = @($plainKeys | Sort-Object -Unique).Count
            field_inventory_sha256 = Get-LineInventoryHash $plainKeys
            values_emitted = $false
            keys = $plainKeys
        }
    }
    else {
        $json = & $keyTool `
            -SourcePath $file.FullName `
            -RedactedSourcePath (Join-Path $Directory '#email-redacted#') `
            -FormId $FormId `
            -ExpectedCiphertextSha256 $hash `
            -Discovery
        $sample = $json | ConvertFrom-Json
        $sample | Add-Member -NotePropertyName encoding -NotePropertyValue 'encrypted' -Force
    }
    [void]$samples.Add($sample)
}

[pscustomobject][ordered]@{
    form_id = $FormId
    sample_count = $samples.Count
    samples = $samples
} | ConvertTo-Json -Depth 6
