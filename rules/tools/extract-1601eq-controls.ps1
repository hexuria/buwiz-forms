param(
    [Parameter(Mandatory = $true)][string]$HtaPath,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$hta = [IO.File]::ReadAllText($HtaPath)

function Get-Attribute([string]$tag, [string]$name) {
    $pattern = '(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($name)
    $match = [regex]::Match($tag, $pattern)
    if ($match.Success) { return $match.Groups[2].Value }
    if ([regex]::IsMatch($tag, "(?i)\b$([regex]::Escape($name))\b")) { return 'true' }
    return $null
}

$controls = @()
$ordinal = 0
$scriptRanges = @([regex]::Matches($hta, '<script\b.*?</script>', 'IgnoreCase,Singleline'))
foreach ($match in [regex]::Matches($hta, '<(input|select|textarea|button)\b[^>]*>', 'IgnoreCase,Singleline')) {
    $insideScript = $false
    foreach ($scriptRange in $scriptRanges) {
        if ($match.Index -ge $scriptRange.Index -and $match.Index -lt ($scriptRange.Index + $scriptRange.Length)) {
            $insideScript = $true
            break
        }
    }
    if ($insideScript) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $type = if ($element -eq 'input') { Get-Attribute $tag 'type' } else { $element }
    if (-not $type) { $type = 'text' }
    $controls += [ordered]@{
        ordinal = $ordinal
        id = Get-Attribute $tag 'id'
        name = Get-Attribute $tag 'name'
        element = $element
        control_kind = $type.ToLowerInvariant()
        source_line = 1 + ([regex]::Matches($hta.Substring(0, $match.Index), "`n")).Count
        value = Get-Attribute $tag 'value'
        maxlength = Get-Attribute $tag 'maxlength'
        disabled = [bool](Get-Attribute $tag 'disabled')
        readonly = [bool](Get-Attribute $tag 'readonly')
        serializable_by_save_loop = $type.ToLowerInvariant() -notin @('button','hidden','undefined','submit')
    }
}

$dynamic = @(
    [ordered]@{
        id_pattern='frm1601EQ:txtRDOCode'; control_kind='select-one'; minimum_instances=1; maximum_instances=1;
        existence='Injected by getRdo from the runtime RDO catalog.'; source_refs=@('official-hta-runtime#getRdo:L3407-L3420')
    },
    [ordered]@{
        id_pattern='AtcCode{1..N}'; control_kind='checkbox'; minimum_instances=0; maximum_instances=111;
        existence='N is 111 for Private and 96 for Government after Item 11 category filtering.'; source_refs=@('official-hta-runtime#changedrpATCList:L2759-L2782','fixtures/atc-catalog-v796.json')
    },
    [ordered]@{
        id_pattern='frm1601EQ:txtAtcCd{1..N}'; control_kind='text'; minimum_instances=6; maximum_instances=111;
        existence='Rows 1-6 always exist after populateAtcPart2; rows 7-N exist when more than six ATCs are selected.'; source_refs=@('official-hta-runtime#populateAtcPart2:L2861-L2875','official-hta-runtime#getATCCode:L2890-L3031')
    },
    [ordered]@{
        id_pattern='frm1601EQ:txtTaxBase{1..N}'; control_kind='text'; minimum_instances=6; maximum_instances=111;
        existence='Parallel to selected ATC rows; user-editable while in Edit state.'; source_refs=@('official-hta-runtime#populateAtcPart2:L2861-L2875','official-hta-runtime#getATCCode:L2890-L3031')
    },
    [ordered]@{
        id_pattern='frm1601EQ:txtTaxRate{1..N}'; control_kind='text'; minimum_instances=6; maximum_instances=111;
        existence='Parallel to selected ATC rows; catalog/year-derived and normally read-only.'; source_refs=@('official-hta-runtime#populateAtcPart2:L2861-L2875','official-hta-runtime#getATCCode:L2890-L3031')
    },
    [ordered]@{
        id_pattern='frm1601EQ:txtTaxbeWithHeld{1..N}'; control_kind='text'; minimum_instances=6; maximum_instances=111;
        existence='Parallel computed output for each selected ATC row.'; source_refs=@('official-hta-runtime#populateAtcPart2:L2861-L2875','official-hta-runtime#getATCCode:L2890-L3031')
    }
)

$document = [ordered]@{
    schema_version='1.0.0'
    form_id='1601eq-v2018'
    package_version='7.9.6.0'
    official_hta_sha256=(Get-FileHash -Algorithm SHA256 -LiteralPath $HtaPath).Hash.ToLowerInvariant()
    static_control_count=$controls.Count
    static_controls_with_id_count=@($controls | Where-Object { $_.id }).Count
    static_controls_without_id_count=@($controls | Where-Object { -not $_.id }).Count
    static_controls=$controls
    runtime_generated_control_families=$dynamic
    maximum_runtime_control_instances=($controls.Count + 1 + 111 + (4 * 111))
    notes=@(
        'The maximum is a source-derived union, not one observed save: it assumes Private category and all 111 ATCs selected.',
        'frm1601EQ:txtAddress2 is a distinct runtime text control but is concatenated into frm1601EQ:txtAddress during serialization.'
    )
}

$directory = Split-Path -Parent $OutputPath
[IO.Directory]::CreateDirectory($directory) | Out-Null
[IO.File]::WriteAllText($OutputPath, ($document | ConvertTo-Json -Depth 12) + "`n", [Text.UTF8Encoding]::new($false))
Write-Output ([ordered]@{static_controls=$controls.Count;with_id=@($controls|Where-Object{$_.id}).Count;without_id=@($controls|Where-Object{-not $_.id}).Count;maximum_runtime_instances=$document.maximum_runtime_control_instances;output=$OutputPath}|ConvertTo-Json)
