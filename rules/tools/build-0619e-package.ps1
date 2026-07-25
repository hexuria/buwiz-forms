param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\0619E'
)

$ErrorActionPreference = 'Stop'
$formId = '0619e-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form0619E.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help0619E.hta'
$pdfPath = Join-Path $OfficialDir '0619-E Jan 2018 rev final.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\0619e-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = '426205a6692a9d11a2a91447446d66b947865257261e4260f91ccb242097d87e'
    help = '1015b35be13e3e76e97149b72ef2fd351c1f7c2deb7400ce25722107a8b25cc4'
    pdf = '0418160d63d4e6f68c34f2bad553273a5d148c3686d8562d338d35fcdd0c5215'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher = '1c49950df1197906bb73ddbb5d0f5f5e1c3f488f376e05b6d53febc1b32016ab'
    decrypted = '6a1911a359efedae7e35fa21c2def9af62c7cc1194e768d4bca5f3193c33fef4'
    encrypted_inventory = '0a4b595e472e1f1b3b6863fcf47d5c1f8f5c616a580c74e36f5fdc5e5aac06be'
    plain = 'a6f21e372a1ce6d707ede13f2447290683ab302d859c3b684a06c55788cbfade'
    plain_inventory = 'e70ca54865959bdf8e5e02e4c68f10d2bd56fae9ab2d082d87fa1a359cd8ecea'
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-AttributeValue([string]$Tag, [string]$Name) {
    $match = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($match.Success) { return $match.Groups[2].Value }
    return $null
}
function Write-JsonFile([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
}
function Write-TextFile([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function Get-LineInventoryHash([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-', '').ToLowerInvariant()
    }
    finally { $sha.Dispose() }
}
function New-Asset([string]$AssetId,[string]$Kind,[string]$Path,[string]$Binding,[string]$DisplayPath='') {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $AssetId
        kind = $Kind
        path = if ($DisplayPath) { $DisplayPath } else { $Path }
        sha256 = Get-Sha256 $Path
        size = $item.Length
        revision_binding = $Binding
    }
}

foreach ($asset in @(
    @($htaPath,'hta'),@($helpPath,'help'),@($pdfPath,'pdf'),@($packagePath,'package')
)) {
    if (-not (Test-Path -LiteralPath $asset[0] -PathType Leaf)) { throw "Missing official asset: $($asset[0])" }
    if ((Get-Sha256 $asset[0]) -ne $expected[$asset[1]]) { throw "Official asset hash changed: $($asset[0])" }
}
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch 'APPLICATIONNAME="0619E"' -or $hta -notmatch 'January 2018') { throw 'January 2018 runtime binding changed.' }
if ($help -notmatch '(?i)Creditable Income Taxes Withheld') { throw '0619E help binding changed.' }
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') { throw '0619E PDF magic mismatch.' }

$sampleFiles = @(Get-ChildItem -LiteralPath $OfficialDir -File -Filter '*.xml')
$sampleByHash = @{}
foreach ($file in $sampleFiles) { $sampleByHash[(Get-Sha256 $file.FullName)] = $file }
foreach ($name in @('cipher','plain')) {
    if (-not $sampleByHash.ContainsKey($expected[$name])) { throw "Pinned 0619E sample missing: $name" }
}
$keyTool = Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson = & $keyTool `
    -SourcePath $sampleByHash[$expected.cipher].FullName `
    -RedactedSourcePath (Join-Path $OfficialDir '0619E-final-copy-#email-redacted#.xml') `
    -FormId $formId `
    -ExpectedCiphertextSha256 $expected.cipher `
    -ExpectedDecryptedSha256 $expected.decrypted `
    -ExpectedFieldCount 59 `
    -ExpectedFieldInventorySha256 $expected.encrypted_inventory
$keyAudit = $keyJson | ConvertFrom-Json
$keys = @($keyAudit.keys)
$plainText = [IO.File]::ReadAllText($sampleByHash[$expected.plain].FullName)
$plainKeys = @(
    [regex]::Matches($plainText, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') |
        ForEach-Object { $_.Groups['key'].Value }
)
if ($plainKeys.Count -ne 58 -or @($plainKeys | Sort-Object -Unique).Count -ne 58) { throw 'Plaintext 0619E inventory changed.' }
if ((Get-LineInventoryHash @($plainKeys | Sort-Object)) -ne $expected.plain_inventory) { throw 'Plaintext 0619E inventory hash changed.' }
$encryptedOnly = @($keys | Where-Object { $plainKeys -notcontains $_ })
$plainOnly = @($plainKeys | Where-Object { $keys -notcontains $_ })
if ($encryptedOnly.Count -ne 1 -or $encryptedOnly[0] -ne 'frm0619E:txtAddress2' -or $plainOnly.Count -ne 0) {
    throw '0619E save/final-copy field difference changed.'
}

New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null
Write-TextFile (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') ($keyJson -join [Environment]::NewLine)
Write-JsonFile (Join-Path $fixtureDir 'plaintext-field-audit-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_path = (Join-Path $OfficialDir '0619E-save-#email-redacted#.xml')
    sha256 = $expected.plain
    field_count = $plainKeys.Count
    unique_field_count = @($plainKeys | Sort-Object -Unique).Count
    field_inventory_sha256 = $expected.plain_inventory
    encrypted_only_keys = $encryptedOnly
    plain_only_keys = $plainOnly
    values_emitted = $false
})

$excludedRanges = @(
    @([regex]::Matches($hta, '(?is)<script\b.*?</script>')) +
    @([regex]::Matches($hta, '(?is)<!--.*?-->'))
)
$controls = [Collections.Generic.List[object]]::new()
$ordinal = 0
foreach ($match in [regex]::Matches($hta, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $skip = $false
    foreach ($range in $excludedRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) { $skip = $true; break }
    }
    if ($skip) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-AttributeValue $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $kind = $kind.ToLowerInvariant()
    $default = Get-AttributeValue $tag 'value'
    if ($kind -in @('radio','checkbox')) { $default = if ($tag -match '(?i)\bchecked(?:\s*=|\s|>)') { 'true' } else { 'false' } }
    $controls.Add([pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Get-AttributeValue $tag 'id'
        name = Get-AttributeValue $tag 'name'
        element = $element
        control_kind = $kind
        source_line = 1 + [regex]::Matches($hta.Substring(0,$match.Index),"`n").Count
        default_value = $default
        maxlength = Get-AttributeValue $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control }
}
$staticMatches = @($keys | Where-Object { $controlById.ContainsKey($_) })
$runtimeRdo = @($keys | Where-Object { $_ -eq 'frm0619E:txtRDOCode' })
$unexplained = @($keys | Where-Object { -not $controlById.ContainsKey($_) -and $_ -ne 'frm0619E:txtRDOCode' })

$discovery = [pscustomobject][ordered]@{
    form_id = $formId
    live_static_controls = $controls.Count
    encrypted_keys = $keys.Count
    plaintext_keys = $plainKeys.Count
    encrypted_only = $encryptedOnly
    static_matches = $staticMatches.Count
    runtime_rdo = $runtimeRdo.Count
    unexplained = $unexplained.Count
}

$selectEnums = @{}
foreach ($selectMatch in [regex]::Matches($hta, '(?is)<select\b(?<open>[^>]*)>(?<body>.*?)</select>')) {
    $isExcluded = $false
    foreach ($range in $excludedRanges) {
        if ($selectMatch.Index -ge $range.Index -and $selectMatch.Index -lt ($range.Index + $range.Length)) { $isExcluded = $true; break }
    }
    if ($isExcluded) { continue }
    $selectId = Get-AttributeValue $selectMatch.Groups['open'].Value 'id'
    if (-not $selectId) { continue }
    $values = [Collections.Generic.List[object]]::new()
    foreach ($option in [regex]::Matches($selectMatch.Groups['body'].Value, '(?is)<option\b(?<open>[^>]*)>(?<text>.*?)</option>')) {
        $values.Add([pscustomobject][ordered]@{
            value = Get-AttributeValue $option.Groups['open'].Value 'value'
            label = [Net.WebUtility]::HtmlDecode([regex]::Replace($option.Groups['text'].Value,'<[^>]+>','')).Trim()
        })
    }
    $selectEnums[$selectId] = @($values)
}
$requiredKeys = @(
    'frm0619E:txtMonth','frm0619E:txtYear','frm0619E:txtDueMonth','frm0619E:txtDueDay','frm0619E:txtDueYear',
    'frm0619E:optWithheld:Y','frm0619E:optWithheld:N',
    'frm0619E:txtTIN1','frm0619E:txtTIN2','frm0619E:txtTIN3','frm0619E:txtBranchCode','frm0619E:txtRDOCode',
    'frm0619E:txtTaxpayerName','frm0619E:txtAddress','frm0619E:txtZipCode','frm0619E:txtTelNum',
    'frm0619E:optCategory:P','frm0619E:optCategory:G','txtEmail'
)
$computedKeys = @(
    'frm0619E:txtDueMonth','frm0619E:txtDueYear','frm0619E:txtTax16','frm0619E:txtTax17D','frm0619E:txtTax18'
)
function Get-ItemNumber([string]$Key) {
    if ($Key -match 'txtMonth$|txtYear$') { return '1' }
    if ($Key -match 'txtDue') { return '2' }
    if ($Key -match 'optAmend') { return '3' }
    if ($Key -match 'optWithheld') { return '4' }
    if ($Key -match 'txtAtc') { return '5' }
    if ($Key -match 'txtTaxTypeCode') { return '6' }
    if ($Key -match 'txtTIN|txtBranchCode') { return '7' }
    if ($Key -match 'txtRDOCode') { return '8' }
    if ($Key -match 'txtTaxpayerName') { return '9' }
    if ($Key -match 'txtAddress') { return '10' }
    if ($Key -match 'txtZipCode') { return '10A' }
    if ($Key -match 'txtTelNum') { return '11' }
    if ($Key -match 'optCategory') { return '12' }
    if ($Key -eq 'txtEmail') { return '13' }
    if ($Key -match 'txtTax14') { return '14' }
    if ($Key -match 'txtTax15') { return '15' }
    if ($Key -match 'txtTax16') { return '16' }
    if ($Key -match 'txtTax17') { return '17' }
    if ($Key -match 'txtTax18') { return '18' }
    if ($Key -match '19') { return '19' }
    if ($Key -match '20') { return '20' }
    if ($Key -match '21') { return '21' }
    if ($Key -match '22') { return '22' }
    return $null
}
function New-Field([string]$Key) {
    $control = if ($controlById.ContainsKey($Key)) { $controlById[$Key] } else { $null }
    $kind = if ($control) { $control.control_kind } else { 'runtime-generated-select' }
    $logical = if ($kind -in @('radio','checkbox')) {
        'boolean'
    }
    elseif ($Key -match '(?i)(TIN|Branch|RDO|Atc|TaxType|Zip|Month|Day|Year)') {
        'code'
    }
    elseif ($Key -match '(?i)(txtTax\d|txtAmount)') {
        'decimal-amount'
    }
    elseif ($Key -match '(?i)Date') {
        'date'
    }
    else {
        'string'
    }
    $computed = $computedKeys -contains $Key
    $required = if ($computed) { 'computed' } elseif ($requiredKeys -contains $Key) { 'required' } else { 'optional' }
    $requiredWhen = $null
    if ($Key -eq 'frm0619E:txtTax14') {
        $required = 'conditional'
        $requiredWhen = 'Taxes withheld/remitted Yes is selected.'
    }
    $constraints = [ordered]@{}
    if ($control -and $control.maxlength -match '^\d+$') { $constraints.max_length = [int]$control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision = 2; $constraints.sign = 'source-dependent' }
    $enumValues = [object[]]::new(0)
    if ($kind -in @('radio','checkbox')) { $enumValues = [object[]]@('true','false') }
    elseif ($selectEnums.ContainsKey($Key)) { $enumValues = [object[]]@($selectEnums[$Key]) }
    $normalization = [string[]]::new(0)
    if ($logical -eq 'decimal-amount') { $normalization = [string[]]@('NumWithComma','formatCurrency') }
    $notes = [Collections.Generic.List[string]]::new()
    $notes.Add('Present in the revision-matched encrypted final-copy inventory; values excluded.')
    if ($Key -eq 'frm0619E:txtAddress2') {
        $notes.Add('Present in encrypted final copy but omitted from the paired plaintext save.')
    }
    [pscustomobject][ordered]@{
        field_key = $Key
        serialized_key = $Key
        serialized_occurrence = 1
        label = $Key
        page = 1
        item_number = Get-ItemNumber $Key
        control_kind = $kind
        storage_type = 'string'
        logical_type = $logical
        required = $required
        required_when = $requiredWhen
        enabled_when = $null
        visible_when = $null
        default_value = if ($control) { $control.default_value } else { '000' }
        empty_representation = ''
        constraints = [pscustomobject]$constraints
        enum_values = $enumValues
        normalization = $normalization
        computed = $computed
        calculation_id = if ($computed) { 'See calculations.json' } else { $null }
        source_refs = @(
            "xml-encrypted#decrypted-field:$Key",
            "official-hta-runtime#control:L$(if($control){$control.source_line}else{2190})"
        )
        confidence = 'high'
        notes = @($notes)
    }
}
$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) { $fields.Add((New-Field $key)) }
if ($fields.Count -ne 59) { throw "0619E typed inventory changed: $($fields.Count)." }
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
    live_static_control_count = $controls.Count
    encrypted_key_count = $keys.Count
    plaintext_key_count = $plainKeys.Count
    static_match_count = $staticMatches.Count
    runtime_rdo_match_count = $runtimeRdo.Count
    unexplained_key_count = $unexplained.Count
    encrypted_only_keys = $encryptedOnly
    controls = $controls
})
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-TextFile (Join-Path $fixtureDir 'validation-function-inventory-v796.json') (
    (& $functionTool -HtaPath $htaPath -ControlPrefix 'frm0619E:' -NamePattern '(?i)valid|check|save|date|submit|final') -join [Environment]::NewLine
)
Write-TextFile (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') (
    (& $functionTool -HtaPath $htaPath -ControlPrefix 'frm0619E:' -NamePattern '(?i)comput|total|tax|penalt|amount|format') -join [Environment]::NewLine
)

$rules = [Collections.Generic.List[object]]::new()
function Add-Rule(
    [string]$Id,[string]$Phase,[int]$Order,[string]$Condition,[string[]]$FieldKeys,$Message,[string[]]$Refs,
    [string]$Assessment='verified-correct',
    [string]$Official='The branch alerts and stops the active operation.',
    [string]$Recommended='Retain as a structured revision-aware error.'
) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$FieldKeys
        accepted_behavior='Condition is false; processing continues.'
        rejected_behavior='The active operation stops unless official_behavior states otherwise.'
        exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment
        official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()
    })
}
Add-Rule '0619e-input-001-due-date' 'blur/change' 1 `
    'Filing month/year changes.' @('frm0619E:txtMonth','frm0619E:txtYear','frm0619E:txtDueMonth','frm0619E:txtDueYear') $null `
    @('official-hta-runtime#computeDueDate:L2246-L2268') 'verified-correct' `
    'Due month becomes the next month; December rolls to January of the next year. Due day remains its default 10.' `
    'Preserve the month rollover and explicitly derive the complete due date.'
Add-Rule '0619e-input-002-unused-choice-helper' input 2 `
    'Withheld/remitted or category choice is absent.' @('choice-fields') $null `
    @('official-hta-runtime#checkiftaxwheldisYes:L2203-L2214') 'obsolete' `
    'The helper returns messages but is never called.' 'Use the active Validate branches as the compatibility source.'
Add-Rule '0619e-input-003-helper-case-typo' input 3 `
    'The unused helper tries to find frm0619E:optcategory:G with a lowercase c.' @('frm0619E:optCategory:G') $null `
    @('official-hta-runtime#checkiftaxwheldisYes:L2208-L2212') 'official-bug-compatible' `
    'The ID does not match the live control, but the function is unreachable.' 'Remove the dead helper and keep one correctly cased choice validator.'

Add-Rule '0619e-save-004-tin' save 4 `
    'Any TIN segment or branch code is blank.' @('TIN-fields') 'Please enter a valid TIN number on Item 7.' `
    @('official-hta-runtime#initialValidateBeforeSave:L2526-L2534')
Add-Rule '0619e-save-005-rdo' save 5 `
    'RDO value is 000.' @('frm0619E:txtRDOCode') 'Please enter a valid RDO Code on Item 8.' `
    @('official-hta-runtime#initialValidateBeforeSave:L2535-L2541')
Add-Rule '0619e-save-006-name' save 6 `
    'Withholding-agent name is blank.' @('frm0619E:txtTaxpayerName') "Please enter a valid Withholding Agent's Name on Item 9." `
    @('official-hta-runtime#initialValidateBeforeSave:L2542-L2550')
Add-Rule '0619e-save-007-sparse' save 7 `
    'Any field outside TIN/RDO/name is invalid.' @('all-other-form-fields') $null `
    @('official-hta-runtime#initialValidateBeforeSave:L2526-L2552') 'incorrect-official-behavior' `
    'Save ignores filing period, due date, choices, address/contact/email, and remittance amounts.' `
    'Allow incomplete drafts explicitly, but never equate sparse Save checks with validity.'
Add-Rule '0619e-save-008-amended-version' save 8 `
    'A finalized/versioned return exists and Amended Return is not Yes.' @('frm0619E:optAmend:Y','frm0619E:optAmend:N') `
    "If your intention is to make an amended return, kindly tick Amended Return to 'Yes' before hitting 'Save' or 'Final Copy' or 'Submit'." `
    @('official-hta-runtime#saveXML:L1744-L1831')

Add-Rule '0619e-validate-009-month-required' validate 9 `
    'Filing month is blank.' @('frm0619E:txtMonth') 'Please enter a valid month on Item 1.' `
    @('official-hta-runtime#validateForm:L2272-L2276')
Add-Rule '0619e-validate-010-current-month' validate 10 `
    'Filing month equals the current month in the current year.' @('frm0619E:txtMonth','frm0619E:txtYear') `
    'Invalid month on Item 1. Month should not be a current Date' @('official-hta-runtime#validateForm:L2277-L2283') `
    'verified-correct' 'The source rejects the current month and rewinds the select before recomputing due date.' `
    'Require a completed prior withholding month.'
Add-Rule '0619e-validate-011-future-month' validate 11 `
    'Filing month is after the current month in the current year.' @('frm0619E:txtMonth','frm0619E:txtYear') `
    'Invalid month on Item 1. Month should not be a future Date' @('official-hta-runtime#validateForm:L2284-L2290')
Add-Rule '0619e-validate-012-year-required' validate 12 `
    'Filing year is blank.' @('frm0619E:txtYear') 'Please enter a valid year on Item 1.' `
    @('official-hta-runtime#validateForm:L2292-L2298')
Add-Rule '0619e-validate-013-year-future' validate 13 `
    'Filing year exceeds current year.' @('frm0619E:txtYear') 'Invalid year on Item 1. Year should not be a future Date.' `
    @('official-hta-runtime#validateForm:L2299-L2306')
Add-Rule '0619e-validate-014-year-floor' validate 14 `
    'Filing year is before 2018.' @('frm0619E:txtYear') 'Invalid entry on Item 1. Entry should not be a previous year from 2018.' `
    @('official-hta-runtime#validateForm:L2307-L2314')
Add-Rule '0619e-validate-015-due-required' validate 15 `
    'Any due-date component is blank.' @('due-date-fields') 'Please enter a valid Date on Item 2' `
    @('official-hta-runtime#validateForm:L2325-L2329')
Add-Rule '0619e-validate-016-due-month' validate 16 `
    'Due month is outside 1..12.' @('frm0619E:txtDueMonth') 'Please enter a valid Month on Item 2' `
    @('official-hta-runtime#validateForm:L2329-L2332')
Add-Rule '0619e-validate-017-due-day' validate 17 `
    'Due day is outside 1..31 or violates the selected month/leap year.' @('due-date-fields') 'Please enter a valid Day on Item 2' `
    @('official-hta-runtime#validateForm:L2332-L2350')
Add-Rule '0619e-validate-018-due-year-future' validate 18 `
    'Due year exceeds current year.' @('frm0619E:txtDueYear') 'Year should not be a future Date. Please enter a valid Year on Item 2' `
    @('official-hta-runtime#validateForm:L2335-L2338')
Add-Rule '0619e-validate-019-due-year-floor' validate 19 `
    'Due year is before 2018.' @('frm0619E:txtDueYear') 'Previous year from 2018 is not applicable for this Form. Please enter a valid Year on Item 2' `
    @('official-hta-runtime#validateForm:L2338-L2341')
Add-Rule '0619e-validate-020-zero-padding' validate 20 `
    'Due month or day has one character after validation.' @('frm0619E:txtDueMonth','frm0619E:txtDueDay') $null `
    @('official-hta-runtime#validateForm:L2351-L2358') 'verified-correct' `
    'The source left-pads each component with zero.' 'Normalize to two-digit month/day strings.'
Add-Rule '0619e-validate-021-withheld-choice' validate 21 `
    'Neither withheld/remitted Yes nor No is selected.' @('frm0619E:optWithheld:Y','frm0619E:optWithheld:N') `
    'Please select an option for Item 4.' @('official-hta-runtime#validateForm:L2360-L2364')
Add-Rule '0619e-validate-022-category-choice' validate 22 `
    'Neither private nor government category is selected.' @('frm0619E:optCategory:P','frm0619E:optCategory:G') `
    'Please select an option for Item 12.' @('official-hta-runtime#validateForm:L2365-L2369')
Add-Rule '0619e-validate-023-tin' validate 23 `
    'Any TIN segment or branch code is blank.' @('TIN-fields') 'Please enter a valid TIN number on Item 7.' `
    @('official-hta-runtime#validateForm:L2374-L2378')
Add-Rule '0619e-validate-024-tin-checksum-omitted' validate 24 `
    'TIN segments are nonblank but fail shared checksum/branch semantics.' @('TIN-fields') $null `
    @('official-hta-runtime#validateForm:L2374-L2378') 'incorrect-official-behavior' `
    'The source tests presence only.' 'Apply the shared evidence-backed TIN validation.'
Add-Rule '0619e-validate-025-rdo' validate 25 `
    'RDO selectedIndex is zero.' @('frm0619E:txtRDOCode') 'Please enter a valid RDO Code on Item 8.' `
    @('official-hta-runtime#validateForm:L2379-L2383')
Add-Rule '0619e-validate-026-name' validate 26 `
    'Taxpayer/withholding-agent name is blank.' @('frm0619E:txtTaxpayerName') 'Please enter a valid Taxpayer Name on Item 9.' `
    @('official-hta-runtime#validateForm:L2384-L2388')
Add-Rule '0619e-validate-027-contact' validate 27 `
    'Contact number is blank.' @('frm0619E:txtTelNum') 'Please enter a valid Contact Number on Item 11.' `
    @('official-hta-runtime#validateForm:L2389-L2393')
Add-Rule '0619e-validate-028-address' validate 28 `
    'Primary registered-address line is blank.' @('frm0619E:txtAddress') "Please enter Taxpayer's Registered Address on Item 10." `
    @('official-hta-runtime#validateForm:L2394-L2398')
Add-Rule '0619e-validate-029-zip' validate 29 `
    'ZIP code is blank.' @('frm0619E:txtZipCode') "Please enter Taxpayer's Zip Code on Item 10A." `
    @('official-hta-runtime#validateForm:L2399-L2403')
Add-Rule '0619e-validate-030-email' validate 30 `
    'Email is blank.' @('txtEmail') 'Please enter valid Email Address on Item 13.' `
    @('official-hta-runtime#validateForm:L2404-L2408')
Add-Rule '0619e-validate-031-email-format-omitted' validate 31 `
    'Email is nonblank but malformed.' @('txtEmail') $null @('official-hta-runtime#validateForm:L2404-L2408') `
    'incorrect-official-behavior' 'Validate checks only blankness.' 'Apply evidence-backed email syntax validation.'
Add-Rule '0619e-validate-032-tax14-conditional' validate 32 `
    'Item 4 Yes is selected and Item 14 equals numeric zero.' @('frm0619E:optWithheld:Y','frm0619E:txtTax14') `
    'Please fill up Part II - Tax Remittance if item 4 is set to Yes.' @('official-hta-runtime#validateForm:L2409-L2415')
Add-Rule '0619e-validate-033-amended-choice-omitted' validate 33 `
    'Neither Amended Return Yes nor No is selected.' @('frm0619E:optAmend:Y','frm0619E:optAmend:N') $null `
    @('official-hta-runtime#validateForm:L2269-L2415') 'incorrect-official-behavior' `
    'Validate accepts the missing choice.' 'Require an explicit Yes/No amended-return state.'
Add-Rule '0619e-validate-034-line-business-omitted' validate 34 `
    'Line of business is blank.' @('frm0619E:txtLineBus') $null @('official-hta-runtime#validateForm:L2269-L2415') `
    'ambiguous' 'Validate never inspects the field.' 'Preserve it and require it only when supported by revision-matched instructions.'
Add-Rule '0619e-validate-035-address2-save-loss' validate 35 `
    'Second registered-address line exists in the final copy.' @('frm0619E:txtAddress2') $null `
    @('xml-encrypted','xml-plaintext','official-hta-runtime#saveXML:L1671-L1953') 'official-bug-compatible' `
    'The encrypted final copy retains the field, while the paired plaintext save omits it.' `
    'Preserve the field losslessly across every save/final-copy transition.'
Add-Rule '0619e-validate-036-negative-net' validate 36 `
    'Item 15 exceeds Item 14, producing a negative Item 16 and potentially negative remittance.' @('frm0619E:txtTax14','frm0619E:txtTax15','frm0619E:txtTax16') $null `
    @('official-hta-runtime#computeNetAmtRem:L2228-L2233','official-hta-runtime#validateForm:L2269-L2415') `
    'incorrect-official-behavior' 'The source computes the negative result and does not reject it.' `
    'Validate the legally permitted relationship between remittance and adjustment.'
Add-Rule '0619e-validate-037-success' validate 37 `
    'All active Validate branches pass.' @('frm0619E:cmdValidate','frm0619E:cmdEdit') `
    'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validateForm:L2416-L2424') `
    'verified-correct' 'Validate disables controls and enables Edit/Final Copy.' 'Tie validation state to the exact field snapshot.'

Write-JsonFile (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    first_error_behavior='Save and Validate alert the first matching active branch and return.'
    rules=$rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Add-Calculation($Id,[string[]]$Outputs,[string[]]$Inputs,$Formula,$Trigger,[string[]]$Deps,[string[]]$Refs,$Assessment='verified-correct') {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id=$Id;outputs=$Outputs;inputs=$Inputs;condition=$null;official_formula=$Formula
        rounding='formatCurrency after NumWithComma conversion; displayed to two decimal places.'
        trigger=$Trigger;depends_on=$Deps;source_refs=$Refs;assessment=$Assessment
        recommended_app_behavior='Use typed decimals and preserve the source dependency order except where the source is defective.'
        confidence='high'
    })
}
Add-Calculation '0619e-due-date' `
    @('frm0619E:txtDueMonth','frm0619E:txtDueDay','frm0619E:txtDueYear') `
    @('frm0619E:txtMonth','frm0619E:txtYear') `
    'Due date is the 10th day of the following month; December rolls to January of year + 1.' `
    'computeDueDate' @() @('official-hta-runtime#computeDueDate:L2246-L2268')
Add-Calculation '0619e-item16-net' `
    @('frm0619E:txtTax16') @('frm0619E:txtTax14','frm0619E:txtTax15') `
    '16 = 14 - 15.' 'computeNetAmtRem' @() @('official-hta-runtime#computeNetAmtRem:L2228-L2233') `
    'incorrect-official-behavior'
Add-Calculation '0619e-item17d-penalties' `
    @('frm0619E:txtTax17D') @('frm0619E:txtTax17A','frm0619E:txtTax17B','frm0619E:txtTax17C') `
    '17D = 17A + 17B + 17C.' 'computePenalties' @() @('official-hta-runtime#computePenalties:L2234-L2240')
Add-Calculation '0619e-item18-total' `
    @('frm0619E:txtTax18') @('frm0619E:txtTax16','frm0619E:txtTax17D') `
    '18 = 16 + 17D.' 'computeTotalAmtRem' @('0619e-item16-net','0619e-item17d-penalties') `
    @('official-hta-runtime#computeTotalAmtRem:L2241-L2245')
Write-JsonFile (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    evaluation_order=@($calculations.calculation_id);calculations=$calculations
})

$negativeCases = [Collections.Generic.List[object]]::new()
$caseNumber=0
foreach($rule in @($rules|Where-Object{$_.exact_message})){
    $caseNumber++
    $negativeCases.Add([pscustomobject][ordered]@{
        case_id=('case-{0:d2}-{1}'-f$caseNumber,$rule.rule_id);phase=$rule.phase
        mutations=@{synthetic_condition=$rule.condition};expected_message=$rule.exact_message
        expected_behavior=$rule.official_behavior;rule_id=$rule.rule_id
    })
}
Write-JsonFile (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$negativeCases
})
Write-JsonFile (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;cases=@(
        @{case_id='december-rollover';calculation_id='0619e-due-date';filing_month=12;filing_year=2025;official_output='01/10/2026'},
        @{case_id='negative-net';calculation_id='0619e-item16-net';item14=100;item15=125;official_output=-25;recommended_behavior='reject or apply the legally correct adjustment rule'},
        @{case_id='total-remittance';calculation_id='0619e-item18-total';item16=1000;item17d=60;official_output=1060}
    )
})

$workflow=[ordered]@{
    '$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    phases=@(
        @{phase='edit';official_behavior='January 2018 monthly remittance form for creditable income taxes withheld (expanded).';source_refs=@('official-hta-runtime','official-form-pdf','packaged-help');confidence='high'},
        @{phase='saved-draft';official_behavior='Save checks only TIN, RDO, and withholding-agent name before serializing the draft.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L2526-L2552');confidence='high'},
        @{phase='validated';official_behavior='Validate runs filing-period, due-date, choice, identity/contact, and conditional remittance checks.';source_refs=@('official-hta-runtime#validateForm:L2269-L2415');confidence='high'},
        @{phase='final-copy';official_behavior='Final Copy is enabled after Validate and writes an encrypted/compressed copy with 59 keys.';source_refs=@('official-hta-runtime#saveEncryptedProfile:L1507-L1593','xml-encrypted');confidence='high'},
        @{phase='submitted';official_behavior='Online transport code exists but was not exercised.';source_refs=@('official-hta-runtime#sendEmail');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='Sparse Save checks pass.';side_effects=@('Writes flat pseudo-XML.');source_refs=@('official-hta-runtime#saveXML:L1671-L1953')},
        @{from='edit';action='Validate';to='validated';guard='All ordered active validation branches pass.';side_effects=@('Disables controls.','Enables Edit and Final Copy.');source_refs=@('official-hta-runtime#validateForm:L2269-L2415')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables editable controls.');source_refs=@('official-hta-runtime#enableAllControl')},
        @{from='validated';action='Final Copy';to='final-copy';guard='Final-copy save succeeds.';side_effects=@('Writes encrypted/compressed copy.');source_refs=@('official-hta-runtime#saveEncryptedProfile:L1507-L1593')},
        @{from='final-copy';action='Online transport';to='submitted';guard='Connectivity and remote acceptance succeed.';side_effects=@('Untested online attempt.');source_refs=@('official-hta-runtime#sendEmail')}
    )
    prerequisites=@('Completed prior withholding month','Derived due date','Withheld/remitted choice','TIN/RDO and identity','Category/contact/address/email','Conditional Part II remittance')
    required_attachments=@()
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='For each covered month, the source derives the due date as the 10th day of the following month.';source_refs=@('official-hta-runtime#computeDueDate:L2246-L2268');confidence='high'},
        @{quarter='Q2';due_date_rule='For each covered month, the source derives the due date as the 10th day of the following month.';source_refs=@('official-hta-runtime#computeDueDate:L2246-L2268');confidence='high'},
        @{quarter='Q3';due_date_rule='For each covered month, the source derives the due date as the 10th day of the following month.';source_refs=@('official-hta-runtime#computeDueDate:L2246-L2268');confidence='high'},
        @{quarter='Q4';due_date_rule='For each covered month, the source derives the due date as the 10th day of the following month.';source_refs=@('official-hta-runtime#computeDueDate:L2246-L2268');confidence='high'}
    )
}
Write-JsonFile (Join-Path $outDir 'workflow.json') $workflow

$bugCount=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    New-Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    New-Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'January 2018 runtime.'
    New-Asset 'packaged-help' 'official-runtime-help' $helpPath 'Packaged January 2018 instructions.'
    New-Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 official form.'
    New-Asset 'xml-encrypted' 'dummy-profile-encrypted-final-copy' $sampleByHash[$expected.cipher].FullName 'Revision-matched 59-key dummy final copy; values excluded.' (Join-Path $OfficialDir '0619E-final-copy-#email-redacted#.xml')
    New-Asset 'xml-plaintext' 'dummy-profile-plaintext-save' $sampleByHash[$expected.plain].FullName 'Revision-matched 58-key dummy save; values excluded.' (Join-Path $OfficialDir '0619E-save-#email-redacted#.xml')
)
$manifest=[ordered]@{
    '$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='0619E';revision=$revision;package_version=$packageVersion;status='complete'
    official_assets=$assets
    counts=[ordered]@{concrete_fields=$fields.Count;runtime_field_families=0;fields_total=$fields.Count;typed_fields=$fields.Count;validation_rules=$rules.Count;confirmed_official_bugs=$bugCount;calculations=$calculations.Count;negative_fixtures=$negativeCases.Count;unverified_gaps=1}
    artifacts=[ordered]@{
        fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md'
        encrypted_field_audit='fixtures/encrypted-field-audit-v796.json';plaintext_field_audit='fixtures/plaintext-field-audit-v796.json'
        runtime_controls='fixtures/runtime-control-inventory-v796.json';validation_functions='fixtures/validation-function-inventory-v796.json';calculation_functions='fixtures/calculation-function-inventory-v796.json'
        negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'
    }
    scope_notes=@('Research only; no renderer or release metadata changed.','No decrypted values or email-bearing filenames emitted.','The 59-key final-copy inventory is lossless; the paired plaintext save omits txtAddress2.')
}
Write-JsonFile (Join-Path $outDir 'manifest.json') $manifest
Write-TextFile (Join-Path $outDir 'README.md') "# BIR Form 0619E - January 2018`n`nRevision-specific Offline eBIRForms rules with 59 concrete serialized keys and no runtime field families.`n"
Write-TextFile (Join-Path $outDir 'evidence.md') "# Evidence`n`n- January 2018 runtime: $($expected.hta); help: $($expected.help); PDF: $($expected.pdf).`n- Encrypted final copy: 59 unique keys, inventory $($expected.encrypted_inventory); values excluded.`n- Plaintext save: 58 unique keys, inventory $($expected.plain_inventory); values excluded.`n- The only difference is ``frm0619E:txtAddress2``, retained by final copy and omitted by plaintext Save.`n- Key accounting: 58 static controls + 1 runtime RDO; zero unexplained keys.`n- All email-bearing filenames use ``#email-redacted#``.`n"
Write-TextFile (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. Online submission was not exercised.`n"
Write-TextFile (Join-Path $outDir 'audit.md') "# Audit`n`n- January 2018 binding: pass.`n- Lossless final-copy inventory: 59 keys; plaintext Save omission documented.`n- Typed inventory: 59 concrete keys, no families, zero unexplained.`n- Validations $($rules.Count); calculations $($calculations.Count); negatives $($negativeCases.Count); defects $bugCount.`n- Focused and full strict audits must run.`n- No renderer/release/capability/commit/push changes.`n"

$indexPath=Join-Path $RepoRoot 'rules\index.json';$index=Get-Content -Raw -LiteralPath $indexPath|ConvertFrom-Json
$entry=$index.forms|Where-Object{$_.form_id-eq$formId}
if($entry){$entry.form_code='0619E';$entry.revision=$revision;$entry.package_version=$packageVersion;$entry.priority=39;$entry.status='complete';$entry.path='forms/0619e-v2018/manifest.json'}
else{$index.forms+=[pscustomobject][ordered]@{form_id=$formId;form_code='0619E';revision=$revision;package_version=$packageVersion;priority=39;status='complete';path='forms/0619e-v2018/manifest.json'}}
$index.forms=@($index.forms|Sort-Object priority);$index.updated='2026-07-23';Write-JsonFile $indexPath $index

$actual=[ordered]@{live_controls=$controls.Count;encrypted_keys=$keys.Count;plaintext_keys=$plainKeys.Count;static_matches=$staticMatches.Count;runtime_rdo=$runtimeRdo.Count;unexplained=$unexplained.Count;fields=$fields.Count;validations=$rules.Count;calculations=$calculations.Count;negatives=$negativeCases.Count;bugs=$bugCount}
$expectedCounts=[ordered]@{live_controls=85;encrypted_keys=59;plaintext_keys=58;static_matches=58;runtime_rdo=1;unexplained=0;fields=59;validations=37;calculations=4;negatives=26;bugs=8}
foreach($name in $expectedCounts.Keys){if($actual[$name]-ne$expectedCounts[$name]){throw "0619E fail-closed count changed: $name expected $($expectedCounts[$name]), found $($actual[$name])."}}
[pscustomobject][ordered]@{form_id=$formId;live_controls=$controls.Count;encrypted_keys=$keys.Count;plaintext_keys=$plainKeys.Count;static_matches=$staticMatches.Count;runtime_rdo=$runtimeRdo.Count;unexplained=$unexplained.Count;typed_fields=$fields.Count;validations=$rules.Count;calculations=$calculations.Count;negative_fixtures=$negativeCases.Count;confirmed_official_bugs=$bugCount;next_form='0619F'}|ConvertTo-Json
