param(
    [Parameter(Mandatory = $true)][string]$HtaPath,
    [Parameter(Mandatory = $true)][string]$SavePath,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$hta = [IO.File]::ReadAllText($HtaPath)
$save = [IO.File]::ReadAllText($SavePath)

function Get-Attribute([string]$tag, [string]$name) {
    $pattern = '(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($name)
    $match = [regex]::Match($tag, $pattern)
    if ($match.Success) { return [Net.WebUtility]::HtmlDecode($match.Groups[2].Value) }
    if ([regex]::IsMatch($tag, "(?i)\b$([regex]::Escape($name))\b")) { return 'true' }
    return $null
}

function Get-Sha256Text([string]$value) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return -join ($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($value)) | ForEach-Object { $_.ToString('x2') })
    } finally { $sha.Dispose() }
}

# Replace comments with equal-length whitespace so source offsets remain stable.
$visible = [regex]::Replace($hta, '<!--.*?-->', { param($m) ' ' * $m.Length }, 'IgnoreCase,Singleline')
$formMatch = [regex]::Match($visible, '(?is)<form\b[^>]*id=[''"]frmMain[''"][^>]*>.*?</form>')
if (-not $formMatch.Success) { throw 'frmMain was not found' }

$candidates = @()
foreach ($match in [regex]::Matches($formMatch.Value, '<(input|select|textarea)\b[^>]*>', 'IgnoreCase,Singleline')) {
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $type = if ($element -eq 'select') { 'select-one' } elseif ($element -eq 'textarea') { 'textarea' } else { Get-Attribute $tag 'type' }
    if (-not $type) { $type = 'text' }
    $type = $type.ToLowerInvariant()
    $id = Get-Attribute $tag 'id'
    if (-not $id -or $type -notin @('text','select-one','radio','checkbox')) { continue }
    $absoluteIndex = $formMatch.Index + $match.Index
    $line = 1 + ([regex]::Matches($visible.Substring(0, $absoluteIndex), "`n")).Count
    $candidates += [ordered]@{
        ordinal = $candidates.Count + 1
        serialized_key = $id
        element = $element
        control_type = $type
        name = Get-Attribute $tag 'name'
        value_attribute = Get-Attribute $tag 'value'
        maxlength = if (Get-Attribute $tag 'maxlength') { [int](Get-Attribute $tag 'maxlength') } else { $null }
        disabled_attribute = [bool](Get-Attribute $tag 'disabled')
        checked_attribute = [bool](Get-Attribute $tag 'checked')
        source_line = $line
    }
}

$totals = @{}
foreach ($control in $candidates) {
    if (-not $totals.ContainsKey($control.serialized_key)) { $totals[$control.serialized_key] = 0 }
    $totals[$control.serialized_key]++
}
$seen = @{}
foreach ($control in $candidates) {
    if (-not $seen.ContainsKey($control.serialized_key)) { $seen[$control.serialized_key] = 0 }
    $seen[$control.serialized_key]++
    $control['occurrence'] = $seen[$control.serialized_key]
    $control['occurrence_count'] = $totals[$control.serialized_key]
    $control['field_key'] = if ($totals[$control.serialized_key] -gt 1) { "$($control.serialized_key)#occurrence-$($seen[$control.serialized_key])" } else { $control.serialized_key }
}

$saveMatches = @([regex]::Matches($save, '<div>(?<key>[^=<>]+)=(?<value>.*?)\k<key>=</div>', 'Singleline'))
$saveEntries = @()
foreach ($match in $saveMatches) {
    $saveEntries += [ordered]@{ ordinal = $saveEntries.Count + 1; serialized_key = $match.Groups['key'].Value; serialized_value = $match.Groups['value'].Value }
}

$sourceKeys = @($candidates | ForEach-Object { $_.serialized_key })
$saveKeys = @($saveEntries | ForEach-Object { $_.serialized_key })
$sequenceEqual = [string]::Join("`n", $sourceKeys) -ceq [string]::Join("`n", $saveKeys)
$orderedKeyText = ([string]::Join("`n", $sourceKeys)) + "`n"

$output = [ordered]@{
    schema_version = '1.0.0'
    form_id = '1702q-v2018c'
    revision = '2018-01-01'
    package_version = '7.9.6.0'
    hta_path = $HtaPath
    hta_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $HtaPath).Hash.ToLowerInvariant()
    save_path = $SavePath
    save_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $SavePath).Hash.ToLowerInvariant()
    runtime_serializable_element_count = $candidates.Count
    unique_serialized_key_count = @($sourceKeys | Sort-Object -Unique).Count
    ordered_serialized_keys_sha256 = Get-Sha256Text $orderedKeyText
    representative_save_element_count = $saveEntries.Count
    representative_save_sequence_matches_source = $sequenceEqual
    duplicate_serialized_keys = @($sourceKeys | Group-Object | Where-Object Count -gt 1 | ForEach-Object { [ordered]@{ serialized_key = $_.Name; occurrences = $_.Count } })
    controls = $candidates
    representative_save = $saveEntries
}

$json = $output | ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($OutputPath, $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
[pscustomobject]@{
    form_id = $output.form_id
    runtime_serializable_element_count = $output.runtime_serializable_element_count
    unique_serialized_key_count = $output.unique_serialized_key_count
    ordered_serialized_keys_sha256 = $output.ordered_serialized_keys_sha256
    representative_save_sequence_matches_source = $output.representative_save_sequence_matches_source
} | ConvertTo-Json
