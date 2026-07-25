param(
    [Parameter(Mandatory = $true)][string]$CatalogPath,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$text = [IO.File]::ReadAllText($CatalogPath)
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $CatalogPath).Hash.ToLowerInvariant()
$slots = @{ P = 0; G = 0 }
$entries = @()

foreach ($match in [regex]::Matches($text, '<div>atc(?<index>\d+):(?<value>.*?)atc\d+:</div>')) {
    $value = $match.Groups['value'].Value
    if ($value.IndexOf('1600PTv2018', [StringComparison]::Ordinal) -lt 0) { continue }
    $parts = $value -split '~'
    $category = $parts[3]
    $slot = $null
    $reachable = $category -in @('P', 'G', 'PG')
    if ($category -eq 'P') { $slots.P++; $slot = [ordered]@{ private = $slots.P; government = $null } }
    elseif ($category -eq 'G') { $slots.G++; $slot = [ordered]@{ private = $null; government = $slots.G } }
    elseif ($category -eq 'PG') { $slots.P++; $slots.G++; $slot = [ordered]@{ private = $slots.P; government = $slots.G } }
    $rate = $parts[2]
    $entries += [ordered]@{
        source_index = [int]$match.Groups['index'].Value
        category = $category
        runtime_slot = $slot
        code = $parts[0]
        description = $parts[1]
        catalog_rate = $rate
        reachable_from_category_ui = $reachable
        parse_issue = if ($reachable -and $rate -match '^\d+(\.\d+)?$') { $null } elseif (-not $reachable) { 'Category is not P, G, or shared PG.' } else { 'Rate is not numeric.' }
    }
}

$document = [ordered]@{
    schema_version = '1.0.0'
    form_id = '1600pt-v2018'
    package_version = '7.9.6.0'
    source_sha256 = $hash
    selection_semantics = 'AtcCodeN is the Nth entry after filtering by Item 10 category; PG records appear in both ordered lists, so slot meanings differ between P and G.'
    private_slot_count = $slots.P
    government_slot_count = $slots.G
    unreachable_record_count = @($entries | Where-Object { -not $_.reachable_from_category_ui }).Count
    entry_count = $entries.Count
    entries = $entries
}

$directory = Split-Path -Parent $OutputPath
[IO.Directory]::CreateDirectory($directory) | Out-Null
[IO.File]::WriteAllText($OutputPath, ($document | ConvertTo-Json -Depth 10) + "`n", [Text.UTF8Encoding]::new($false))
Write-Output ([ordered]@{entry_count=$entries.Count;private_slots=$slots.P;government_slots=$slots.G;unreachable=@($entries | Where-Object { -not $_.reachable_from_category_ui }).Count;output=$OutputPath} | ConvertTo-Json)
