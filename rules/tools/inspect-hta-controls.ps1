param(
    [string]$HtaPath,

    [ValidatePattern('^[0-9A-Za-z-]+$')]
    [string]$FormCode
)

$ErrorActionPreference = 'Stop'

if (-not $HtaPath) {
    if (-not $FormCode) {
        throw 'Provide HtaPath or FormCode.'
    }
    $candidates = @(
        Get-ChildItem -LiteralPath 'C:\Users\uriah\AppData\Local\Temp' `
            -Recurse -File -Filter "BIR-Form$FormCode.hta" -ErrorAction SilentlyContinue
    )
    if ($candidates.Count -ne 1) {
        throw "Expected one extracted HTA for $FormCode; found $($candidates.Count)."
    }
    $HtaPath = $candidates[0].FullName
}

function Get-AttributeValue([string]$Tag, [string]$Name) {
    $match = [regex]::Match(
        $Tag,
        ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name))
    )
    if ($match.Success) {
        return $match.Groups[2].Value
    }
    return $null
}

$hta = [IO.File]::ReadAllText($HtaPath)
$excludedRanges = @(
    @([regex]::Matches($hta, '(?is)<script\b.*?</script>')) +
    @([regex]::Matches($hta, '(?is)<!--.*?-->'))
)
$controls = [Collections.Generic.List[object]]::new()
$ordinal = 0

foreach ($match in [regex]::Matches($hta, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $excluded = $false
    foreach ($range in $excludedRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) {
            $excluded = $true
            break
        }
    }
    if ($excluded) {
        continue
    }

    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-AttributeValue $tag 'type' } else { $element }
    if (-not $kind) {
        $kind = 'text'
    }

    $control = [pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Get-AttributeValue $tag 'id'
        name = Get-AttributeValue $tag 'name'
        element = $element
        control_kind = $kind.ToLowerInvariant()
        source_line = 1 + [regex]::Matches($hta.Substring(0, $match.Index), "`n").Count
        value = Get-AttributeValue $tag 'value'
        maxlength = Get-AttributeValue $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
    [void]$controls.Add($control)
}

[pscustomobject][ordered]@{
    hta_path = $HtaPath
    sha256 = (Get-FileHash -LiteralPath $HtaPath -Algorithm SHA256).Hash.ToLowerInvariant()
    control_count = $controls.Count
    controls = $controls
} | ConvertTo-Json -Depth 5
