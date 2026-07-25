param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\2200Tv2022',
    [string]$SampleDir = 'C:\Mac\Home\Downloads\forms\2200T'
)

$ErrorActionPreference = 'Stop'
$formId = '2200t-v2020'
$revision = '2020-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form2200Tv2020.hta'
$legacyPath = Join-Path $ExtractedRoot 'forms\BIR-Form2200T.hta'
$legacyHelpPath = Join-Path $ExtractedRoot 'helpfile\Help2200T.hta'
$laterPdfPath = Join-Path $OfficialDir '2200-T Aug 2022 ENCS Final.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'

$expected = @{
    hta = '8ac4eaa1aa8fd5cc323e62133c6841bd9e8696bd719d27e08f1eef1d201dfe78'
    legacy = '10da2116020cb8d911b66defa7451b67060a512272601b7c48412be720a23b75'
    legacy_help = '90cc5e0983b9649bce9b247fe1ed5befade01f20c367ffc4ca09fdb4e822b624'
    later_pdf = 'cea195a413e5aa1ba94da957ed982c0f5f95fd31ad8fa89bc57ce8733dca52fb'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher = 'bcb0c848cc20787659d3418928789db84acd12562d9b4c292917579da246ddf1'
    plain = 'd9d8fece329e40cca428fe4637f3bc62f3edeefc28e5440ddac7e5b5702df899'
    inventory = '1961ac6e982350ea82ab8b79eb0630541197eb038353b9518ffd674562265fb3'
}

function Get-AttributeValue([string]$Tag, [string]$Name) {
    $match = [regex]::Match(
        $Tag,
        ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name))
    )
    if ($match.Success) { return $match.Groups[2].Value }
    return $null
}

foreach ($asset in @(
    @($htaPath, 'hta'),
    @($legacyPath, 'legacy'),
    @($legacyHelpPath, 'legacy_help'),
    @($laterPdfPath, 'later_pdf'),
    @($packagePath, 'package')
)) {
    if (-not (Test-Path -LiteralPath $asset[0] -PathType Leaf)) {
        throw "Missing official asset: $($asset[0])"
    }
    $hash = (Get-FileHash -LiteralPath $asset[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -ne $expected[$asset[1]]) {
        throw "Official asset hash changed: $($asset[0])"
    }
}

$hta = [IO.File]::ReadAllText($htaPath)
$legacy = [IO.File]::ReadAllText($legacyPath)
$legacyHelp = [IO.File]::ReadAllText($legacyHelpPath)
if ($hta -notmatch 'APPLICATIONNAME="2200Tv2020"' -or $hta -notmatch 'January 2020') {
    throw 'January 2020 runtime binding changed.'
}
if ($legacy -notmatch 'APPLICATIONNAME="2200T"' -or $legacy -notmatch 'April 2014') {
    throw 'April 2014 predecessor binding changed.'
}
if ($legacyHelp -notmatch '(?i)Excise Tax Returns for Tobacco Products') {
    throw 'Predecessor help binding changed.'
}
$pdfBytes = [IO.File]::ReadAllBytes($laterPdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') {
    throw 'August 2022 reference PDF magic mismatch.'
}

$samples = @(Get-ChildItem -LiteralPath $SampleDir -File -Filter '*.xml')
if ($samples.Count -ne 1) { throw "Expected one encrypted predecessor sample; found $($samples.Count)." }
if ((Get-FileHash -LiteralPath $samples[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected.cipher) {
    throw 'Encrypted predecessor sample hash changed.'
}
$redactedSamplePath = Join-Path $SampleDir '2200T-final-copy-#email-redacted#.xml'
$keyTool = Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson = & $keyTool `
    -SourcePath $samples[0].FullName `
    -RedactedSourcePath $redactedSamplePath `
    -FormId '2200t-v2014-predecessor-excluded' `
    -ExpectedCiphertextSha256 $expected.cipher `
    -ExpectedDecryptedSha256 $expected.plain `
    -ExpectedFieldCount 128 `
    -ExpectedFieldInventorySha256 $expected.inventory
$keyAudit = $keyJson | ConvertFrom-Json
$legacyKeys = @($keyAudit.keys)

$formMatch = [regex]::Match(
    $hta,
    '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>'
)
if (-not $formMatch.Success) { throw 'January 2020 frmMain is missing.' }
$formBody = $formMatch.Groups['body'].Value
$bodyOffset = $formMatch.Groups['body'].Index
$excludedRanges = @(
    @([regex]::Matches($formBody, '(?is)<script\b.*?</script>')) +
    @([regex]::Matches($formBody, '(?is)<!--.*?-->'))
)

$controls = [Collections.Generic.List[object]]::new()
$ordinal = 0
foreach ($match in [regex]::Matches($formBody, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $excluded = $false
    foreach ($range in $excludedRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) {
            $excluded = $true
            break
        }
    }
    if ($excluded) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-AttributeValue $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $kind = $kind.ToLowerInvariant()
    $defaultValue = Get-AttributeValue $tag 'value'
    if ($kind -in @('radio', 'checkbox')) {
        $defaultValue = if ($tag -match '(?i)\bchecked(?:\s*=|\s|>)') { 'true' } else { 'false' }
    }
    $controls.Add([pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Get-AttributeValue $tag 'id'
        name = Get-AttributeValue $tag 'name'
        element = $element
        control_kind = $kind
        source_line = 1 + [regex]::Matches($hta.Substring(0, $bodyOffset + $match.Index), "`n").Count
        default_value = $defaultValue
        maxlength = Get-AttributeValue $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}

$serial = @(
    $controls | Where-Object {
        $_.control_kind -in @('text', 'select', 'select-one', 'textarea', 'radio', 'checkbox') -and $_.id
    }
)
$currentIds = @($serial.id | Sort-Object -Unique)
$legacyIds = @(
    [regex]::Matches($legacy, '(?i)\bid\s*=\s*(["''])(?<id>.*?)\1') |
        ForEach-Object { $_.Groups['id'].Value } |
        Where-Object { $_ } |
        Sort-Object -Unique
)
$directCurrentOverlap = @($legacyKeys | Where-Object { $currentIds -contains $_ })
$legacyStaticOverlap = @($legacyKeys | Where-Object { $legacyIds -contains $_ })
$legacyGeneratedSchedule = @(
    $legacyKeys | Where-Object {
        $_ -match '^frm2200T:txt(?:Pro|Ins)(?:Exempt|Taxable|BasicTaxDue)\d+$'
    }
)
$prefixMappedOverlap = @(
    $legacyKeys |
        Where-Object { $_ -like 'frm2200T:*' } |
        ForEach-Object { $_ -replace '^frm2200T:', 'frm2200Tv2020:' } |
        Where-Object { $currentIds -contains $_ }
)

$discovery = [pscustomobject][ordered]@{
    form_id = $formId
    revision = $revision
    live_controls = $controls.Count
    static_serialized_occurrences = $serial.Count
    static_unique_serialized_ids = $currentIds.Count
    static_control_kind_counts = @(
        $serial |
            Group-Object control_kind |
            Sort-Object Name |
            ForEach-Object { [pscustomobject]@{ kind = $_.Name; count = $_.Count } }
    )
    static_schedule_ids = @($currentIds | Where-Object { $_ -match ':txtSched1_' }).Count
    static_disabled_serialized = @($serial | Where-Object disabled).Count
    predecessor_sample_keys = $legacyKeys.Count
    predecessor_direct_current_overlap = $directCurrentOverlap.Count
    predecessor_legacy_literal_id_overlap = $legacyStaticOverlap.Count
    predecessor_legacy_generated_schedule_match = $legacyGeneratedSchedule.Count
    predecessor_fully_accounted = ($legacyStaticOverlap.Count + $legacyGeneratedSchedule.Count) -eq $legacyKeys.Count
    predecessor_prefix_mapped_current_overlap = $prefixMappedOverlap.Count
    target_has_runtime_families = $hta -match '(?i)createElement|rowTemplate|addRow'
    note = 'January 2020 target inventory; April 2014 encrypted sample is predecessor-only evidence.'
}

$outDir = Join-Path $RepoRoot 'rules\forms\2200t-v2020'
$fixtureDir = Join-Path $outDir 'fixtures'
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

function Write-JsonFile([string]$Path, $Value) {
    $json = ($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine
    [IO.File]::WriteAllText($Path, $json, [Text.UTF8Encoding]::new($false))
}

function Write-TextFile([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}

function Get-LineInventoryHash([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))
        return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

function New-Asset(
    [string]$AssetId,
    [string]$Kind,
    [string]$Path,
    [string]$RevisionBinding,
    [string]$DisplayPath = ''
) {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $AssetId
        kind = $Kind
        path = if ($DisplayPath) { $DisplayPath } else { $Path }
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        size = $item.Length
        revision_binding = $RevisionBinding
    }
}

Write-TextFile (Join-Path $fixtureDir 'excluded-predecessor-encrypted-field-keys-v796.json') (
    $keyJson -join [Environment]::NewLine
)

$selectEnums = @{}
foreach ($selectMatch in [regex]::Matches($formBody, '(?is)<select\b(?<open>[^>]*)>(?<body>.*?)</select>')) {
    $isExcluded = $false
    foreach ($range in $excludedRanges) {
        if ($selectMatch.Index -ge $range.Index -and $selectMatch.Index -lt ($range.Index + $range.Length)) {
            $isExcluded = $true
            break
        }
    }
    if ($isExcluded) { continue }
    $selectId = Get-AttributeValue $selectMatch.Groups['open'].Value 'id'
    if (-not $selectId) { continue }
    $values = [Collections.Generic.List[object]]::new()
    foreach ($option in [regex]::Matches($selectMatch.Groups['body'].Value, '(?is)<option\b(?<open>[^>]*)>(?<text>.*?)</option>')) {
        $optionValue = Get-AttributeValue $option.Groups['open'].Value 'value'
        $optionText = [regex]::Replace($option.Groups['text'].Value, '<[^>]+>', '')
        $optionText = [Net.WebUtility]::HtmlDecode($optionText).Trim()
        $values.Add([pscustomobject][ordered]@{ value = $optionValue; label = $optionText })
    }
    $selectEnums[$selectId] = @($values)
}

$runtimeRdo = 'frm2200Tv2020:rdoCode'
$requiredKeys = @(
    'frm2200Tv2020:txtDateMonth',
    'frm2200Tv2020:txtDateDay',
    'frm2200Tv2020:txtDateYear',
    'frm2200Tv2020:tinA',
    'frm2200Tv2020:tinB',
    'frm2200Tv2020:tinC',
    'frm2200Tv2020:branchCode',
    $runtimeRdo,
    'frm2200Tv2020:registeredName',
    'frm2200Tv2020:registeredAddress',
    'frm2200Tv2020:zipCode',
    'frm2200Tv2020:phoneNumber',
    'frm2200Tv2020:prodCity',
    'frm2200Tv2020:remCity'
)
$computedKeys = @(
    'frm2200Tv2020:txtSched1_TotalDue',
    'frm2200Tv2020:txtExciseDue',
    'frm2200Tv2020:txtLess_Tot',
    'frm2200Tv2020:txtNetTaxDue',
    'frm2200Tv2020:txtStillDue',
    'frm2200Tv2020:txtPen_Tot',
    'frm2200Tv2020:txtAmtPayable',
    'frm2200Tv2020:txtPay_Penalties',
    'frm2200Tv2020:txtPay_Tot',
    'frm2200Tv2020:txtBalance'
)
$labels = @{
    'frm2200Tv2020:txtDateMonth' = 'Return date month'
    'frm2200Tv2020:txtDateDay' = 'Return date day'
    'frm2200Tv2020:txtDateYear' = 'Return date year'
    'frm2200Tv2020:amendedRtn_1' = 'Amended return: Yes'
    'frm2200Tv2020:amendedRtn_2' = 'Amended return: No'
    'frm2200Tv2020:txtSheets' = 'Number of sheets attached'
    'frm2200Tv2020:registeredName' = 'Taxpayer registered name'
    'frm2200Tv2020:registeredAddress' = 'Registered address'
    'frm2200Tv2020:zipCode' = 'ZIP code'
    'frm2200Tv2020:phoneNumber' = 'Telephone number'
    'frm2200Tv2020:txtEmail' = 'Email address'
    'frm2200Tv2020:txtLineBus' = 'Line of business'
    'frm2200Tv2020:prodRegion' = 'Place of production region'
    'frm2200Tv2020:prodProvince' = 'Place of production province'
    'frm2200Tv2020:prodCity' = 'Place of production city'
    'frm2200Tv2020:remRegion' = 'Place of removal region'
    'frm2200Tv2020:remProvince' = 'Place of removal province'
    'frm2200Tv2020:remCity' = 'Place of removal city'
    'frm2200Tv2020:treatyY' = 'Tax relief specification'
    'frm2200Tv2020:paymentOther' = 'Other manner of payment'
}

function Get-ItemNumber([string]$Key) {
    if ($Key -match ':txtSched1_') { return 'Schedule 1' }
    if ($Key -match ':txtDate') { return '1' }
    if ($Key -match ':amendedRtn_') { return '2' }
    if ($Key -match ':txtSheets$') { return '3' }
    if ($Key -match ':(?:tin[ABC]|branchCode)$') { return '4' }
    if ($Key -eq $runtimeRdo) { return '5' }
    if ($Key -match ':registeredName$') { return '6' }
    if ($Key -match ':(?:registeredAddress|zipCode)$') { return '7/7A' }
    if ($Key -match ':phoneNumber$') { return '8' }
    if ($Key -match ':txtEmail$') { return '9' }
    if ($Key -match ':prod(?:Region|Province|City)$') { return '10' }
    if ($Key -match ':rem(?:Region|Province|City)$') { return '11' }
    if ($Key -match ':(?:optTreaty_[12]|treatyY)$') { return '12/12A' }
    if ($Key -match ':optPayment_1$') { return '13' }
    if ($Key -match ':optPayment_2$') { return '14' }
    if ($Key -match ':(?:optPayment_3|paymentOther)$') { return '15' }
    if ($Key -match ':txtExciseDue$') { return '16' }
    if ($Key -match ':txtLess_') { return '17' }
    if ($Key -match ':txtNetTaxDue$') { return '18' }
    if ($Key -match ':txtPrevReturn$') { return '19' }
    if ($Key -match ':txtStillDue$') { return '20' }
    if ($Key -match ':txtPen_') { return '21' }
    if ($Key -match ':txtAmtPayable$') { return '22' }
    if ($Key -match ':txtPay_') { return '23' }
    if ($Key -match ':txtBalance$') { return '24' }
    return $null
}

function New-Field($Control, [string]$Key) {
    $kind = if ($Control) { $Control.control_kind } else { 'runtime-generated-select' }
    $logicalType = if ($kind -in @('radio', 'checkbox')) {
        'boolean'
    }
    elseif ($Key -match '(?i)(tin[ABC]|branchCode|rdoCode|zipCode|DateMonth|DateDay|DateYear|Region|Province|City|ATC)') {
        'code'
    }
    elseif ($Key -match '(?i)(txtSched1_|Excise|Less|Tax|Due|PrevReturn|Still|Pen_|Amt|Pay_|Balance|Rate|Amount)') {
        'decimal-amount'
    }
    else {
        'string'
    }
    $computed = $computedKeys -contains $Key
    $required = if ($computed) {
        'computed'
    }
    elseif ($requiredKeys -contains $Key) {
        'required'
    }
    else {
        'optional'
    }
    $requiredWhen = $null
    if ($Key -eq 'frm2200Tv2020:treatyY') {
        $required = 'conditional'
        $requiredWhen = 'Tax relief Yes is selected.'
    }
    elseif ($Key -eq 'frm2200Tv2020:paymentOther') {
        $required = 'conditional'
        $requiredWhen = 'Other manner of payment is selected.'
    }
    $constraints = [ordered]@{}
    if ($Control -and $Control.maxlength -match '^\d+$') {
        $constraints.max_length = [int]$Control.maxlength
    }
    if ($logicalType -eq 'decimal-amount') {
        $constraints.precision = 2
        $constraints.sign = 'nonnegative after blur for controls wired to blockNegativeNumber; otherwise source-dependent'
    }
    $enumValues = [object[]]::new(0)
    if ($kind -in @('radio', 'checkbox')) {
        $enumValues = [object[]]@('true', 'false')
    }
    elseif ($selectEnums.ContainsKey($Key)) {
        $enumValues = [object[]]@($selectEnums[$Key])
    }
    $normalization = [string[]]::new(0)
    if ($logicalType -eq 'decimal-amount') {
        $normalization = [string[]]@('round(2)', 'blockNegativeNumber', 'amtFormat')
    }
    $page = if (-not $Control) { 1 } elseif ($Control.source_line -lt 1386) { 1 } elseif ($Control.source_line -lt 1969) { 2 } else { 3 }
    $sourceLine = if ($Control) { $Control.source_line } else { 5783 }
    [pscustomobject][ordered]@{
        field_key = $Key
        serialized_key = $Key
        serialized_occurrence = 1
        label = if ($labels.ContainsKey($Key)) { $labels[$Key] } else { $Key }
        page = $page
        item_number = Get-ItemNumber $Key
        control_kind = $kind
        storage_type = 'string'
        logical_type = $logicalType
        required = $required
        required_when = $requiredWhen
        enabled_when = if ($Key -eq 'frm2200Tv2020:txtPrevReturn') { 'Amended return Yes is selected.' } else { $null }
        visible_when = $null
        default_value = if ($Control) { $Control.default_value } else { '000' }
        empty_representation = ''
        constraints = [pscustomobject]$constraints
        enum_values = $enumValues
        normalization = $normalization
        computed = $computed
        calculation_id = if ($computed) { 'See calculations.json' } else { $null }
        source_refs = @(
            'official-hta-runtime#saveXML:L4896-L5158',
            "official-hta-runtime#control:L$sourceLine"
        )
        confidence = 'high'
        notes = @('Source-derived from the hash-pinned January 2020 Offline runtime; no revision-matched final copy is available.')
    }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($control in $serial) {
    $fields.Add((New-Field $control $control.id))
}
$fields.Add((New-Field $null $runtimeRdo))
if ($fields.Count -ne 278) {
    throw "January 2020 typed inventory changed: expected 278, found $($fields.Count)."
}

Write-JsonFile (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = $fields.Count
    inventory_sha256 = Get-LineInventoryHash @($fields.field_key | Sort-Object)
    fields = $fields
})

Write-JsonFile (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    official_hta_sha256 = $expected.hta
    live_control_count = $controls.Count
    static_serialized_occurrence_count = $serial.Count
    static_unique_serialized_id_count = $currentIds.Count
    runtime_generated_scalar_count = 1
    runtime_generated_scalars = @($runtimeRdo)
    runtime_family_count = 0
    predecessor_sample_key_count = $legacyKeys.Count
    predecessor_direct_current_overlap = $directCurrentOverlap.Count
    predecessor_legacy_literal_id_overlap = $legacyStaticOverlap.Count
    predecessor_legacy_generated_schedule_match = $legacyGeneratedSchedule.Count
    predecessor_fully_accounted = ($legacyStaticOverlap.Count + $legacyGeneratedSchedule.Count) -eq $legacyKeys.Count
    controls = $controls
})

$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-TextFile (Join-Path $fixtureDir 'validation-function-inventory-v796.json') (
    (& $functionTool `
        -HtaPath $htaPath `
        -ControlPrefix 'frm2200Tv2020:' `
        -NamePattern '(?i)valid|check|save|date|submit|final|row') -join [Environment]::NewLine
)
Write-TextFile (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') (
    (& $functionTool `
        -HtaPath $htaPath `
        -ControlPrefix 'frm2200Tv2020:' `
        -NamePattern '(?i)comput|total|tax|penalt|balance|format') -join [Environment]::NewLine
)

$rules = [Collections.Generic.List[object]]::new()
function Add-Rule(
    [string]$Id,
    [string]$Phase,
    [int]$Order,
    [string]$Condition,
    [string[]]$FieldKeys,
    $ExactMessage,
    [string[]]$SourceRefs,
    [string]$Assessment = 'verified-correct',
    [string]$OfficialBehavior = 'The branch alerts and stops the active operation.',
    [string]$RecommendedBehavior = 'Retain as a structured revision-aware error.'
) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id = $Id
        form_id = $formId
        revision = $revision
        phase = $Phase
        order = $Order
        condition = $Condition
        fields = $FieldKeys
        accepted_behavior = 'Condition is false; processing continues.'
        rejected_behavior = 'The active operation stops unless official_behavior states otherwise.'
        exact_message = $ExactMessage
        source_refs = $SourceRefs
        evidence_type = @('source')
        assessment = $Assessment
        official_behavior = $OfficialBehavior
        recommended_app_behavior = $RecommendedBehavior
        confidence = 'high'
        unresolved_questions = @()
    })
}

Add-Rule '2200t-input-001-amount-number' input 1 `
    'An amount field wired to blockletter contains a non-number on blur.' `
    @('amount-fields') $null `
    @('official-hta-runtime#blockletter:L5466-L5477') `
    'verified-correct' `
    'The value is normalized to 0.00 without an alert.' `
    'Parse typed decimals and surface invalid input before normalization.'
Add-Rule '2200t-input-002-nonnegative' input 2 `
    'A field wired to blockNegativeNumber is negative on blur.' `
    @('nonnegative-amount-fields') $null `
    @('official-hta-runtime#blockNegativeNumber:L5478-L5484') `
    'verified-correct' `
    'The value is silently replaced with 0.00.' `
    'Reject negative input visibly or normalize only with an explicit user-facing explanation.'
Add-Rule '2200t-input-003-integer' input 3 `
    'A field wired to blockLetterInt contains a non-number.' `
    @('integer-fields') $null `
    @('official-hta-runtime#blockLetterInt:L5496-L5505') `
    'verified-correct' `
    'The value is silently cleared.' `
    'Use a typed integer parser and preserve a structured invalid state.'
Add-Rule '2200t-input-004-date-format' input 4 `
    'validateDate receives malformed MM/DD/YYYY input.' `
    @('date-input') 'Please provide a valid date. (MM/DD/YYYY format)' `
    @('official-hta-runtime#validateDate:L6932-L6989')
Add-Rule '2200t-input-005-date-future' input 5 `
    'validateDate receives a future date.' `
    @('date-input') 'This date cannot be a future date.' `
    @('official-hta-runtime#validateDate:L6990-L6994')
Add-Rule '2200t-input-006-date-floor' input 6 `
    'validateDate receives a date before 2018.' `
    @('date-input') 'This date cannot be prior to 2018.' `
    @('official-hta-runtime#validateDate:L6995-L7000') `
    'official-bug-compatible' `
    'The generic helper enforces 2018, while Validate enforces only 1904 and the printed form is January 2020.' `
    'Bind the minimum accepted date to the January 2020 revision.'

Add-Rule '2200t-save-007-return-date' save 7 `
    'All three return-date components are blank.' `
    @('frm2200Tv2020:txtDateMonth','frm2200Tv2020:txtDateDay','frm2200Tv2020:txtDateYear') `
    'Please enter a valid Return Date' `
    @('official-hta-runtime#initialValidateBeforeSave:L5795-L5800') `
    'incorrect-official-behavior' `
    'Save rejects only when all three components are blank; a partially populated or malformed date passes this guard.' `
    'Require a complete valid calendar date before saving a finalizable draft.'
Add-Rule '2200t-save-008-tin' save 8 `
    'Any TIN segment or branch code is blank.' `
    @('frm2200Tv2020:tinA','frm2200Tv2020:tinB','frm2200Tv2020:tinC','frm2200Tv2020:branchCode') `
    'Please enter a valid TIN number on Item 4.' `
    @('official-hta-runtime#initialValidateBeforeSave:L5801-L5805')
Add-Rule '2200t-save-009-rdo' save 9 `
    'RDO value is 000.' `
    @($runtimeRdo) 'Please enter a valid RDO Code on Item 5.' `
    @('official-hta-runtime#initialValidateBeforeSave:L5806-L5809')
Add-Rule '2200t-save-010-name' save 10 `
    'Registered name is blank.' `
    @('frm2200Tv2020:registeredName') 'Please enter a valid Taxpayer Name on Item 7.' `
    @('official-hta-runtime#initialValidateBeforeSave:L5810-L5814') `
    'official-bug-compatible' `
    'The correct field is checked, but the message says Item 7 even though registered name is Item 6.' `
    'Require the field and report Item 6.'
Add-Rule '2200t-save-011-amended-version' save 11 `
    'A finalized/versioned return exists and Amended Return is not Yes.' `
    @('frm2200Tv2020:amendedRtn_1','frm2200Tv2020:amendedRtn_2') `
    "If your intention is to make an amended return, kindly tick Amended Return to 'Yes' before hitting 'Save' or 'Final Copy' or 'Submit'." `
    @('official-hta-runtime#saveXML:L4966-L5030')

Add-Rule '2200t-validate-012-month' validate 12 `
    'Parsed month is blank, below 1, above 12, or NaN/zero.' `
    @('frm2200Tv2020:txtDateMonth') 'Please enter a valid month on Item 1.' `
    @('official-hta-runtime#validateForm:L5529-L5534')
Add-Rule '2200t-validate-013-february' validate 13 `
    'The day exceeds 28 in a non-leap February.' `
    @('frm2200Tv2020:txtDateMonth','frm2200Tv2020:txtDateDay','frm2200Tv2020:txtDateYear') `
    'Please enter a valid date on Item 1. Filing year is not a leap year.' `
    @('official-hta-runtime#validateForm:L5535-L5539')
Add-Rule '2200t-validate-014-day' validate 14 `
    'Parsed day is blank, below 1, above the computed month maximum, or NaN/zero.' `
    @('frm2200Tv2020:txtDateDay') 'Please enter a valid day on Item 1.' `
    @('official-hta-runtime#validateForm:L5540-L5544')
Add-Rule '2200t-validate-015-year-required' validate 15 `
    'Year is the empty string.' `
    @('frm2200Tv2020:txtDateYear') 'Please enter a valid year on Item 1.' `
    @('official-hta-runtime#validateForm:L5545-L5549')
Add-Rule '2200t-validate-016-year-floor' validate 16 `
    'Year coerces below 1904.' `
    @('frm2200Tv2020:txtDateYear') 'Invalid date entry on Item 1. Entry should not be lower than 1904.' `
    @('official-hta-runtime#validateForm:L5550-L5554') `
    'incorrect-official-behavior' `
    'The January 2020 form accepts years back to 1904; the separate validateDate helper uses 2018.' `
    'Enforce a revision-supported return period beginning January 2020.'
Add-Rule '2200t-validate-017-nonnumeric-year' validate 17 `
    'Year is nonempty and nonnumeric after a manipulated load or programmatic edit.' `
    @('frm2200Tv2020:txtDateYear') $null `
    @('official-hta-runtime#validateForm:L5512-L5559') `
    'incorrect-official-behavior' `
    'JavaScript comparisons with NaN are false, so the nonnumeric year can pass the year-floor and future-date branches.' `
    'Require a four-digit numeric year before constructing the date.'
Add-Rule '2200t-validate-018-future' validate 18 `
    'The composed return date is after the current local date.' `
    @('frm2200Tv2020:txtDateMonth','frm2200Tv2020:txtDateDay','frm2200Tv2020:txtDateYear') `
    'Invalid date entry on Item 1. Date cannot be after the current date.' `
    @('official-hta-runtime#validateForm:L5555-L5559')
Add-Rule '2200t-validate-019-tin-presence' validate 19 `
    'Any TIN segment or branch code is blank.' `
    @('frm2200Tv2020:tinA','frm2200Tv2020:tinB','frm2200Tv2020:tinC','frm2200Tv2020:branchCode') `
    'Please enter a valid TIN number on Item 4.' `
    @('official-hta-runtime#validateForm:L5560-L5564')
Add-Rule '2200t-validate-020-tin-checksum-omitted' validate 20 `
    'TIN segments are merely nonblank; no TIN checksum is invoked by validateForm.' `
    @('frm2200Tv2020:tinA','frm2200Tv2020:tinB','frm2200Tv2020:tinC','frm2200Tv2020:branchCode') `
    $null @('official-hta-runtime#validateForm:L5560-L5564') `
    'incorrect-official-behavior' `
    'The source labels the check as valid TIN but tests presence only.' `
    'Apply the shared evidence-backed TIN checksum and branch-code rules.'
Add-Rule '2200t-validate-021-rdo' validate 21 `
    'RDO selectedIndex is zero.' `
    @($runtimeRdo) 'Please enter a valid RDO Code on Item 5.' `
    @('official-hta-runtime#validateForm:L5565-L5569')
Add-Rule '2200t-validate-022-name' validate 22 `
    'Registered name is exactly empty.' `
    @('frm2200Tv2020:registeredName') "Please enter a valid Taxpayer's Name on Item 6." `
    @('official-hta-runtime#validateForm:L5570-L5574')
Add-Rule '2200t-validate-023-address' validate 23 `
    'Registered address is exactly empty.' `
    @('frm2200Tv2020:registeredAddress') "Please enter Taxpayer's Registered Address on Item 7." `
    @('official-hta-runtime#validateForm:L5575-L5579')
Add-Rule '2200t-validate-024-zip' validate 24 `
    'ZIP code is exactly empty.' `
    @('frm2200Tv2020:zipCode') "Please enter Taxpayer's Zip Code on Item 7A." `
    @('official-hta-runtime#validateForm:L5580-L5584')
Add-Rule '2200t-validate-025-phone' validate 25 `
    'Telephone number is exactly empty.' `
    @('frm2200Tv2020:phoneNumber') 'Please enter a valid Telephone Number on Item 8.' `
    @('official-hta-runtime#validateForm:L5585-L5589')
Add-Rule '2200t-validate-026-production-city' validate 26 `
    'Place-of-production city selectedIndex is zero.' `
    @('frm2200Tv2020:prodCity') 'Please enter a valid Place of Production on Item 10.' `
    @('official-hta-runtime#validateForm:L5590-L5594')
Add-Rule '2200t-validate-027-removal-city' validate 27 `
    'Place-of-removal city selectedIndex is zero.' `
    @('frm2200Tv2020:remCity') 'Please enter a valid Place of Removal on Item 11.' `
    @('official-hta-runtime#validateForm:L5595-L5599')
Add-Rule '2200t-validate-028-relief-specification' validate 28 `
    'Tax relief Yes is checked and the specification is empty.' `
    @('frm2200Tv2020:optTreaty_1','frm2200Tv2020:treatyY') 'Please specify a Tax Relief on Item 12A.' `
    @('official-hta-runtime#validateForm:L5600-L5604')
Add-Rule '2200t-validate-029-relief-choice-omitted' validate 29 `
    'Neither tax-relief radio is checked.' `
    @('frm2200Tv2020:optTreaty_1','frm2200Tv2020:optTreaty_2') $null `
    @('official-hta-runtime#validateForm:L5600-L5604') `
    'incorrect-official-behavior' `
    'Validate accepts the form because only the Yes-plus-empty-specification case is tested.' `
    'Require an explicit Yes or No choice.'
Add-Rule '2200t-validate-030-payment-choice' validate 30 `
    'None of the three manner-of-payment radios is checked.' `
    @('frm2200Tv2020:optPayment_1','frm2200Tv2020:optPayment_2','frm2200Tv2020:optPayment_3') `
    'Please enter a Manner of Payment on Part II.' `
    @('official-hta-runtime#validateForm:L5605-L5612')
Add-Rule '2200t-validate-031-payment-other' validate 31 `
    'Other manner of payment is checked and its description is empty.' `
    @('frm2200Tv2020:optPayment_3','frm2200Tv2020:paymentOther') 'Please specify a Scheme on Item 15.' `
    @('official-hta-runtime#validateForm:L5613-L5617')
Add-Rule '2200t-validate-032-email-omitted' validate 32 `
    'Email is blank or malformed.' `
    @('frm2200Tv2020:txtEmail') $null `
    @('official-hta-runtime#validateForm:L5507-L5632','official-hta-runtime#openAlertEmail:L6351-L6412') `
    'incorrect-official-behavior' `
    'Validate performs no email check even though Final Copy asks the taxpayer to ensure a valid email address.' `
    'Validate email syntax before enabling Final Copy.'
Add-Rule '2200t-validate-033-line-business-omitted' validate 33 `
    'Line of business is blank.' `
    @('frm2200Tv2020:txtLineBus') $null `
    @('official-hta-runtime#validateForm:L5507-L5632') `
    'ambiguous' `
    'Validate does not inspect the printed line-of-business field.' `
    'Preserve it losslessly and require it only if the January 2020 legal instructions establish that obligation.'
Add-Rule '2200t-validate-034-schedule-omitted' validate 34 `
    'Schedule 1 contains no positive tax base or excise-due amount.' `
    @('schedule-1-fields') $null `
    @('official-hta-runtime#validateForm:L5507-L5632','official-hta-runtime#calculate_Sched1_TotalDue:L7061-L7076') `
    'incorrect-official-behavior' `
    'Validate never inspects Schedule 1 and accepts an all-zero schedule.' `
    'Require the applicable tobacco line and validate the selected effective-year row.'
Add-Rule '2200t-validate-035-row-formula-omitted' validate 35 `
    'A Schedule 1 Due cell disagrees with taxable base multiplied by the printed rate.' `
    @('schedule-1-due-fields') $null `
    @('official-hta-runtime#schedule-controls:L1478-L2340','official-hta-runtime#calculate_Sched1_TotalDue:L7061-L7076') `
    'incorrect-official-behavior' `
    'Due cells are editable inputs; the source only sums them and never computes or cross-checks the printed tax rates.' `
    'Compute each due amount from the revision-specific rate and taxable base, preserving any official rounding rule.'
Add-Rule '2200t-validate-036-success' validate 36 `
    'All preceding Validate branches pass.' `
    @('frm2200Tv2020:cmdValidate','frm2200Tv2020:cmdEdit') `
    'Validation successful. Click on Edit if you wish to modify your entries.' `
    @('official-hta-runtime#validateForm:L5619-L5631') `
    'verified-correct' `
    'Validate disables itself, enables Print/Edit/Final Copy, and disables editable controls.' `
    'Transition to a validated state while retaining an explicit Edit transition.'

Add-Rule '2200t-final-037-validation-state' final-copy 37 `
    'The form has not reached the successful Validate state.' `
    @('frm2200Tv2020:cmdValidate','frm2200Tv2020:btnFinalCopy') $null `
    @('official-hta-runtime#init:L5396-L5417','official-hta-runtime#validateForm:L5619-L5631') `
    'verified-correct' `
    'The Final Copy button remains disabled until Validate succeeds.' `
    'Require a validation snapshot tied to the exact current field state.'
Add-Rule '2200t-final-038-confirmation' final-copy 38 `
    'The user declines the submission/final-copy confirmation.' `
    @('frm2200Tv2020:btnFinalCopy') `
    "Please ensure that you have INTERNET access and a VALID email address is indicated in your tax return.`n`nAre you sure you want to submit?" `
    @('official-hta-runtime#openAlertEmail:L6401-L6404') `
    'verified-correct' `
    'The confirm dialog cancels when the user does not affirm.' `
    'Keep Final Copy and online Submit as separately named, explicit actions.'
Add-Rule '2200t-final-039-offline' final-copy 39 `
    'The connection check fails.' `
    @('frm2200Tv2020:btnFinalCopy') "The system detected that you have no internet connection.`nPlease contact your internet service provider." `
    @('official-hta-runtime#openAlertEmail:L6405-L6412') `
    'ambiguous' `
    'The source sets final flag 3 and still saves an encrypted profile for offline handling.' `
    'Allow an explicitly offline Final Copy without representing it as an online submission.'

Write-JsonFile (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    first_error_behavior = 'Validate and Save alert the first matching branch and return; successful Validate then disables editing.'
    rules = $rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Add-Calculation(
    [string]$Id,
    [string[]]$Outputs,
    [string[]]$Inputs,
    [string]$Formula,
    [string]$Trigger,
    [string[]]$DependsOn,
    [string[]]$SourceRefs,
    [string]$Assessment = 'verified-correct',
    [string]$RecommendedBehavior = 'Use typed decimals and preserve the source dependency order.'
) {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id = $Id
        outputs = $Outputs
        inputs = $Inputs
        condition = $null
        official_formula = $Formula
        rounding = 'amtFormat uses Number.toFixed(2) and inserts thousands separators.'
        trigger = $Trigger
        depends_on = $DependsOn
        source_refs = $SourceRefs
        assessment = $Assessment
        recommended_app_behavior = $RecommendedBehavior
        confidence = 'high'
    })
}

Add-Calculation '2200t-schedule-total' `
    @('frm2200Tv2020:txtSched1_TotalDue','frm2200Tv2020:txtExciseDue') `
    @('frm2200Tv2020:txtSched1_*_Due') `
    'Sum every input whose ID starts frm2200Tv2020:txtSched1_ and ends _Due; copy the formatted total to Item 16.' `
    'Blur of any Schedule 1 Due cell.' @() `
    @('official-hta-runtime#calculate_Sched1_TotalDue:L7061-L7076') `
    'incorrect-official-behavior' `
    'Compute each row due from its pinned rate and taxable base before summing; do not trust editable due cells.'
Add-Calculation '2200t-item17-total' `
    @('frm2200Tv2020:txtLess_Tot') `
    @('frm2200Tv2020:txtLess_Balance','frm2200Tv2020:txtLess_Excise') `
    '17C = 17A + 17B.' 'calculate_Part3' @() `
    @('official-hta-runtime#calculate_Part3:L7044-L7046')
Add-Calculation '2200t-item18-net' `
    @('frm2200Tv2020:txtNetTaxDue') `
    @('frm2200Tv2020:txtExciseDue','frm2200Tv2020:txtLess_Tot') `
    '18 = 16 - 17C.' 'calculate_Part3' @('2200t-schedule-total','2200t-item17-total') `
    @('official-hta-runtime#calculate_Part3:L7046-L7047')
Add-Calculation '2200t-item20-still-due' `
    @('frm2200Tv2020:txtStillDue') `
    @('frm2200Tv2020:txtNetTaxDue','frm2200Tv2020:txtPrevReturn') `
    '20 = 18 - 19.' 'calculate_Part3' @('2200t-item18-net') `
    @('official-hta-runtime#calculate_Part3:L7047-L7048')
Add-Calculation '2200t-item21-penalties' `
    @('frm2200Tv2020:txtPen_Tot') `
    @('frm2200Tv2020:txtPen_Surcharge','frm2200Tv2020:txtPen_Interest','frm2200Tv2020:txtPen_Compromise') `
    '21D = 21A + 21B + 21C.' 'calculate_Part3' @() `
    @('official-hta-runtime#calculate_Part3:L7049-L7054')
Add-Calculation '2200t-item22-payable' `
    @('frm2200Tv2020:txtAmtPayable') `
    @('frm2200Tv2020:txtStillDue','frm2200Tv2020:txtPen_Tot') `
    '22 = 20 + 21D.' 'calculate_Part3' @('2200t-item20-still-due','2200t-item21-penalties') `
    @('official-hta-runtime#calculate_Part3:L7055-L7055')
Add-Calculation '2200t-item23b-penalties' `
    @('frm2200Tv2020:txtPay_Penalties') `
    @('frm2200Tv2020:txtPen_Tot') `
    '23B copies 21D unconditionally.' 'calculate_Part3' @('2200t-item21-penalties') `
    @('official-hta-runtime#calculate_Part3:L7056-L7056')
Add-Calculation '2200t-item23-total' `
    @('frm2200Tv2020:txtPay_Tot') `
    @('frm2200Tv2020:txtPay_TaxPayment','frm2200Tv2020:txtPay_Penalties') `
    '23C = 23A + 23B.' 'calculate_Part3' @('2200t-item23b-penalties') `
    @('official-hta-runtime#calculate_Part3:L7057-L7057')
Add-Calculation '2200t-item24-balance' `
    @('frm2200Tv2020:txtBalance') `
    @('frm2200Tv2020:txtAmtPayable','frm2200Tv2020:txtPay_Tot') `
    '24 = 22 - 23C.' 'calculate_Part3' @('2200t-item22-payable','2200t-item23-total') `
    @('official-hta-runtime#calculate_Part3:L7058-L7058')

Write-JsonFile (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema' = '../../schema/calculations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    evaluation_order = @($calculations.calculation_id)
    calculations = $calculations
})

$negativeCases = [Collections.Generic.List[object]]::new()
$caseNumber = 0
foreach ($rule in @($rules | Where-Object { $_.exact_message })) {
    $caseNumber++
    $negativeCases.Add([pscustomobject][ordered]@{
        case_id = ('case-{0:d2}-{1}' -f $caseNumber, $rule.rule_id)
        phase = $rule.phase
        mutations = @{ synthetic_condition = $rule.condition }
        expected_message = $rule.exact_message
        expected_behavior = $rule.official_behavior
        rule_id = $rule.rule_id
    })
}
Write-JsonFile (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    synthetic_only = $true
    cases = $negativeCases
})
Write-JsonFile (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    cases = @(
        @{
            case_id = 'schedule-total'
            calculation_id = '2200t-schedule-total'
            due_values = @(100.25, 200.25)
            official_output = 300.50
        },
        @{
            case_id = 'row-due-not-computed'
            calculation_id = '2200t-schedule-total'
            taxable_base = 100
            printed_rate = 2.31
            entered_due = 0
            official_output = 0
            recommended_output = 231
        },
        @{
            case_id = 'part3-balance'
            calculation_id = '2200t-item24-balance'
            item22 = 1200
            item23c = 1100
            official_output = 100
        }
    )
})

$resources = [Collections.Generic.List[object]]::new()
foreach ($src in @(
    [regex]::Matches($hta, '(?i)<script[^>]+src\s*=\s*(["''])(?<value>.*?)\1') |
        ForEach-Object { $_.Groups['value'].Value } |
        Sort-Object -Unique
)) {
    $fullPath = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if (Test-Path -LiteralPath $fullPath) {
        $resources.Add([pscustomobject][ordered]@{
            src = $src
            path = $fullPath
            present = $true
            size = (Get-Item -LiteralPath $fullPath).Length
            sha256 = (Get-FileHash -LiteralPath $fullPath -Algorithm SHA256).Hash.ToLowerInvariant()
        })
    }
    else {
        $resources.Add([pscustomobject][ordered]@{
            src = $src
            path = $fullPath
            present = $false
            size = $null
            sha256 = $null
        })
    }
}
Write-JsonFile (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    resources = $resources
})

$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    phases = @(
        @{
            phase = 'edit'
            official_behavior = 'January 2020 three-page tobacco excise return with a fully static Schedule 1.'
            source_refs = @('official-hta-runtime')
            confidence = 'high'
        },
        @{
            phase = 'saved-draft'
            official_behavior = 'Save checks only all-date-components-blank, TIN segment presence, RDO, and registered name before serializing the flat control state.'
            source_refs = @('official-hta-runtime#initialValidateBeforeSave:L5795-L5814','official-hta-runtime#saveXML:L4896-L5158')
            confidence = 'high'
        },
        @{
            phase = 'validated'
            official_behavior = 'Validate runs the ordered 17-branch main-form check but performs no Schedule 1 validation or row calculation.'
            source_refs = @('official-hta-runtime#validateForm:L5507-L5632')
            confidence = 'high'
        },
        @{
            phase = 'final-copy'
            official_behavior = 'After Validate, Final Copy prompts for internet/email readiness and can save an encrypted offline copy when connectivity fails.'
            source_refs = @('official-hta-runtime#openAlertEmail:L6351-L6412','official-hta-runtime#saveEncryptedProfile:L4733-L4817')
            confidence = 'high'
        },
        @{
            phase = 'submitted'
            official_behavior = 'Online transport code exists but was not exercised.'
            source_refs = @('official-hta-runtime#sendEmail:L6419-L6526')
            confidence = 'medium'
        }
    )
    transitions = @(
        @{
            from = 'edit'
            action = 'Save'
            to = 'saved-draft'
            guard = 'Sparse Save checks pass.'
            side_effects = @('Writes a flat pseudo-XML serialization of control state.')
            source_refs = @('official-hta-runtime#saveXML:L4896-L5158')
        },
        @{
            from = 'edit'
            action = 'Validate'
            to = 'validated'
            guard = 'The ordered Validate branches pass.'
            side_effects = @('Disables editable controls.','Enables Print, Edit, and Final Copy.')
            source_refs = @('official-hta-runtime#validateForm:L5507-L5632')
        },
        @{
            from = 'validated'
            action = 'Edit'
            to = 'edit'
            guard = $null
            side_effects = @('Re-enables editable controls.','Disables Final Copy.')
            source_refs = @('official-hta-runtime#editForm:L5694-L5708')
        },
        @{
            from = 'validated'
            action = 'Submit / Final Copy'
            to = 'final-copy'
            guard = 'The user confirms; an online or offline final-copy path completes.'
            side_effects = @('Updates final flag.','Writes encrypted/compressed copy.')
            source_refs = @('official-hta-runtime#openAlertEmail:L6351-L6412')
        },
        @{
            from = 'final-copy'
            action = 'Online transport'
            to = 'submitted'
            guard = 'Connectivity and remote acceptance succeed.'
            side_effects = @('Untested online attempt.')
            source_refs = @('official-hta-runtime#sendEmail:L6419-L6526')
        }
    )
    prerequisites = @(
        'Return date',
        'TIN and RDO',
        'Registered identity and address',
        'Production/removal locations',
        'Tax-relief and payment choices',
        'Schedule 1 and Part III amounts'
    )
    required_attachments = @()
    filing_deadlines = @(
        @{
            quarter = 'Q1'
            due_date_rule = 'For any covered removal in this quarter, the predecessor packaged help says tax is paid before removal from the place of production; no revision-matched January 2020 guide is locally pinned.'
            source_refs = @('predecessor-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q2'
            due_date_rule = 'For any covered removal in this quarter, the predecessor packaged help says tax is paid before removal from the place of production; no revision-matched January 2020 guide is locally pinned.'
            source_refs = @('predecessor-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q3'
            due_date_rule = 'For any covered removal in this quarter, the predecessor packaged help says tax is paid before removal from the place of production; no revision-matched January 2020 guide is locally pinned.'
            source_refs = @('predecessor-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q4'
            due_date_rule = 'For any covered removal in this quarter, the predecessor packaged help says tax is paid before removal from the place of production; no revision-matched January 2020 guide is locally pinned.'
            source_refs = @('predecessor-help')
            confidence = 'medium'
        }
    )
}
Write-JsonFile (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @(
    $rules | Where-Object {
        $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete')
    }
).Count
$assets = @(
    New-Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    New-Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'January 2020 Offline runtime; authoritative for this package.'
    New-Asset 'predecessor-hta-runtime' 'runtime-extracted-hta-predecessor' $legacyPath 'April 2014 predecessor; excluded from January 2020 rule derivation.'
    New-Asset 'predecessor-help' 'official-runtime-help-predecessor' $legacyHelpPath 'Packaged unversioned predecessor help; deadline context only.'
    New-Asset 'later-form-pdf' 'official-form-pdf-later-revision' $laterPdfPath 'August 2022 PDF; later revision recorded as a contradiction boundary, not merged into January 2020 rules.'
    New-Asset 'predecessor-encrypted-sample' 'dummy-profile-encrypted-final-copy-predecessor' $samples[0].FullName 'April 2014-family 128-key dummy final copy; excluded from January 2020 inventory.' $redactedSamplePath
)

$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    form_code = '2200T'
    revision = $revision
    package_version = $packageVersion
    status = 'complete'
    official_assets = $assets
    counts = [ordered]@{
        concrete_fields = $fields.Count
        runtime_field_families = 0
        fields_total = $fields.Count
        typed_fields = $fields.Count
        validation_rules = $rules.Count
        confirmed_official_bugs = $bugCount
        calculations = $calculations.Count
        negative_fixtures = $negativeCases.Count
        unverified_gaps = 3
    }
    artifacts = [ordered]@{
        fields = 'fields.json'
        validations = 'validations.json'
        calculations = 'calculations.json'
        workflow = 'workflow.json'
        evidence = 'evidence.md'
        audit = 'audit.md'
        gaps = 'gaps.md'
        predecessor_encrypted_field_audit = 'fixtures/excluded-predecessor-encrypted-field-keys-v796.json'
        runtime_controls = 'fixtures/runtime-control-inventory-v796.json'
        validation_functions = 'fixtures/validation-function-inventory-v796.json'
        calculation_functions = 'fixtures/calculation-function-inventory-v796.json'
        resources = 'fixtures/official-resource-hashes-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'
        calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer or release metadata changed.',
        'No decrypted values or email-bearing filenames are emitted.',
        'The January 2020 runtime has 277 static serializable controls plus one runtime-generated RDO select and no runtime field families.',
        'The April 2014 encrypted sample and August 2022 PDF are revision boundaries, not January 2020 inventory sources.'
    )
}
Write-JsonFile (Join-Path $outDir 'manifest.json') $manifest

Write-TextFile (Join-Path $outDir 'README.md') @"
# BIR Form 2200T - January 2020

Revision-specific Offline eBIRForms rules for the January 2020 tobacco excise return: 278 concrete serialized controls and no runtime field families.
"@
Write-TextFile (Join-Path $outDir 'evidence.md') @"
# Evidence

- January 2020 runtime SHA-256: $($expected.hta).
- Installed package SHA-256: $($expected.package).
- Runtime inventory: 300 live controls, 277 static serializable controls, one runtime-generated RDO select, and no dynamic families.
- The 128-key encrypted dummy copy is an April 2014-family predecessor: ciphertext $($expected.cipher), decrypted payload $($expected.plain), inventory $($expected.inventory). Its 83 literal IDs plus 45 generated product/schedule keys account for all 128 keys. Values are never emitted.
- The only local form PDF is August 2022, SHA-256 $($expected.later_pdf). It is recorded as a later revision and is not merged into January 2020 rules.
- All email-bearing filenames are represented only as ``#email-redacted#``.
"@
Write-TextFile (Join-Path $outDir 'gaps.md') @"
# Gaps

1. No revision-matched January 2020 final-copy sample is locally available; the encrypted sample is explicitly predecessor-only.
2. No revision-matched January 2020 PDF or guide is locally pinned; the packaged help is predecessor material and the local PDF is August 2022.
3. Online submission was not exercised.
"@
Write-TextFile (Join-Path $outDir 'audit.md') @"
# Audit

- January 2020 runtime binding: pass.
- Revision discrimination: pass; April 2014 sample and August 2022 PDF are excluded from January 2020 field/rule derivation.
- Typed inventory: 277 static controls + 1 runtime RDO = 278; no runtime families.
- Validations: $($rules.Count); calculations: $($calculations.Count); negatives: $($negativeCases.Count); confirmed official defects: $bugCount.
- Focused and full strict structural/schema audits must run after generation.
- No renderer/release/capability/commit/push changes.
"@

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry = $index.forms | Where-Object { $_.form_id -eq $formId }
if ($entry) {
    $entry.form_code = '2200T'
    $entry.revision = $revision
    $entry.package_version = $packageVersion
    $entry.priority = 37
    $entry.status = 'complete'
    $entry.path = 'forms/2200t-v2020/manifest.json'
}
else {
    $index.forms += [pscustomobject][ordered]@{
        form_id = $formId
        form_code = '2200T'
        revision = $revision
        package_version = $packageVersion
        priority = 37
        status = 'complete'
        path = 'forms/2200t-v2020/manifest.json'
    }
}
$index.forms = @($index.forms | Sort-Object priority)
$index.updated = '2026-07-23'
Write-JsonFile $indexPath $index

$actualCounts = [ordered]@{
    live_controls = $controls.Count
    static_serialized = $serial.Count
    static_unique = $currentIds.Count
    runtime_scalars = 1
    typed_fields = $fields.Count
    predecessor_keys = $legacyKeys.Count
    predecessor_literal = $legacyStaticOverlap.Count
    predecessor_generated = $legacyGeneratedSchedule.Count
    validations = $rules.Count
    calculations = $calculations.Count
    negative_fixtures = $negativeCases.Count
    confirmed_official_bugs = $bugCount
}
$expectedCounts = [ordered]@{
    live_controls = 300
    static_serialized = 277
    static_unique = 277
    runtime_scalars = 1
    typed_fields = 278
    predecessor_keys = 128
    predecessor_literal = 83
    predecessor_generated = 45
    validations = 39
    calculations = 9
    negative_fixtures = 28
    confirmed_official_bugs = 10
}
foreach ($name in $expectedCounts.Keys) {
    if ($actualCounts[$name] -ne $expectedCounts[$name]) {
        throw "2200T fail-closed count changed: $name expected $($expectedCounts[$name]), found $($actualCounts[$name])."
    }
}

[pscustomobject][ordered]@{
    form_id = $formId
    live_controls = $controls.Count
    static_serialized = $serial.Count
    runtime_scalars = 1
    typed_fields = $fields.Count
    predecessor_keys = $legacyKeys.Count
    predecessor_fully_accounted = ($legacyStaticOverlap.Count + $legacyGeneratedSchedule.Count) -eq $legacyKeys.Count
    validations = $rules.Count
    calculations = $calculations.Count
    negative_fixtures = $negativeCases.Count
    confirmed_official_bugs = $bugCount
    next_form = '0605 audit'
} | ConvertTo-Json
