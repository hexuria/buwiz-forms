param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\0619F'
)

$ErrorActionPreference = 'Stop'
$formId = '0619f-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form0619F.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help0619F.hta'
$pdfPath = Join-Path $OfficialDir '0619-F Jan 2018 rev final.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\0619f-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = '994818ea7b5691656d302d32025a39c6895a032d417871a6080803373436ecca'
    help = '290ec441060ad3bfdd54f0673b704ce05cf4a7b68510efc5a16518bff94cb54e'
    pdf = 'edd7357390b1f0d95f2a38c9bb76252341c15b54b82bffd338bd540452ff15e1'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher = 'd561ce34a44a732e52047552c6d4c0b975b3c45042dc0aba4907abfda89b53fb'
    decrypted = '087116111a5222233d65b9f63bda8bcde4203f072bbb0925ae57c1cadc29c067'
    encrypted_inventory = '2be0270c8b3c61e8d09c8875294885cf52e7ce924a95e85230cd7f91ea978cc2'
    plain = 'f7a1f2481104b8c23b22f92aef263ae02f768227ec6961cb10e4daf0817f8a18'
    plain_inventory = 'eebd0278ed63c59d085ece88eb5d66fe26902522bcc283a9f6ef2dd0ebabe069'
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
    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine),
        [Text.UTF8Encoding]::new($false)
    )
}
function Write-TextFile([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function Get-LineInventoryHash([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes((@($Lines | Sort-Object) -join "`n"))
        return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    }
    finally { $sha.Dispose() }
}
function New-Asset(
    [string]$AssetId,
    [string]$Kind,
    [string]$Path,
    [string]$Binding,
    [string]$DisplayPath = ''
) {
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
    @($htaPath, 'hta'),
    @($helpPath, 'help'),
    @($pdfPath, 'pdf'),
    @($packagePath, 'package')
)) {
    if (-not (Test-Path -LiteralPath $asset[0] -PathType Leaf)) {
        throw "Missing official asset: $($asset[0])"
    }
    if ((Get-Sha256 $asset[0]) -ne $expected[$asset[1]]) {
        throw "Official asset hash changed: $($asset[0])"
    }
}

$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch 'APPLICATIONNAME="0619F"' -or $hta -notmatch 'January 2018') {
    throw 'January 2018 runtime binding changed.'
}
if ($help -notmatch '(?i)Final Income Taxes Withheld') {
    throw '0619F help binding changed.'
}
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') {
    throw '0619F PDF magic mismatch.'
}

$sampleFiles = @(Get-ChildItem -LiteralPath $OfficialDir -File -Filter '*.xml')
$sampleByHash = @{}
foreach ($file in $sampleFiles) {
    $sampleByHash[(Get-Sha256 $file.FullName)] = $file
}
foreach ($name in @('cipher', 'plain')) {
    if (-not $sampleByHash.ContainsKey($expected[$name])) {
        throw "Pinned 0619F sample missing: $name"
    }
}

$keyTool = Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson = & $keyTool `
    -SourcePath $sampleByHash[$expected.cipher].FullName `
    -RedactedSourcePath (Join-Path $OfficialDir '0619F-final-copy-#email-redacted#.xml') `
    -FormId $formId `
    -ExpectedCiphertextSha256 $expected.cipher `
    -ExpectedDecryptedSha256 $expected.decrypted `
    -ExpectedFieldCount 60 `
    -ExpectedFieldInventorySha256 $expected.encrypted_inventory
$keyAudit = $keyJson | ConvertFrom-Json
$keys = @($keyAudit.keys)

$plainText = [IO.File]::ReadAllText($sampleByHash[$expected.plain].FullName)
$plainKeys = @(
    [regex]::Matches($plainText, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') |
        ForEach-Object { $_.Groups['key'].Value }
)
if ($plainKeys.Count -ne 59 -or @($plainKeys | Sort-Object -Unique).Count -ne 59) {
    throw 'Plaintext 0619F inventory changed.'
}
if ((Get-LineInventoryHash $plainKeys) -ne $expected.plain_inventory) {
    throw 'Plaintext 0619F inventory hash changed.'
}
$encryptedOnly = @($keys | Where-Object { $plainKeys -notcontains $_ })
$plainOnly = @($plainKeys | Where-Object { $keys -notcontains $_ })
if (
    $encryptedOnly.Count -ne 1 -or
    $encryptedOnly[0] -ne 'frm0619F:txtAddress2' -or
    $plainOnly.Count -ne 0
) {
    throw '0619F save/final-copy field difference changed.'
}

New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null
Write-TextFile (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') (
    $keyJson -join [Environment]::NewLine
)
Write-JsonFile (Join-Path $fixtureDir 'plaintext-field-audit-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_path = (Join-Path $OfficialDir '0619F-save-#email-redacted#.xml')
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
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) {
            $skip = $true
            break
        }
    }
    if ($skip) { continue }

    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-AttributeValue $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $kind = $kind.ToLowerInvariant()
    $default = Get-AttributeValue $tag 'value'
    if ($kind -in @('radio', 'checkbox')) {
        $default = if ($tag -match '(?i)\bchecked(?:\s*=|\s|>)') { 'true' } else { 'false' }
    }
    $control = [pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Get-AttributeValue $tag 'id'
        name = Get-AttributeValue $tag 'name'
        element = $element
        control_kind = $kind
        source_line = 1 + [regex]::Matches($hta.Substring(0, $match.Index), "`n").Count
        default_value = $default
        maxlength = Get-AttributeValue $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
    [void]$controls.Add($control)
}
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) {
        $controlById[$control.id] = $control
    }
}
$staticMatches = @($keys | Where-Object { $controlById.ContainsKey($_) })
$runtimeRdo = @($keys | Where-Object { $_ -eq 'frm0619F:txtRDOCode' })
$unexplained = @(
    $keys | Where-Object {
        -not $controlById.ContainsKey($_) -and $_ -ne 'frm0619F:txtRDOCode'
    }
)

$selectEnums = @{}
foreach ($selectMatch in [regex]::Matches($hta, '(?is)<select\b(?<open>[^>]*)>(?<body>.*?)</select>')) {
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
        $entry = [pscustomobject][ordered]@{
            value = Get-AttributeValue $option.Groups['open'].Value 'value'
            label = [Net.WebUtility]::HtmlDecode(
                [regex]::Replace($option.Groups['text'].Value, '<[^>]+>', '')
            ).Trim()
        }
        [void]$values.Add($entry)
    }
    $selectEnums[$selectId] = @($values)
}

$requiredKeys = @(
    'frm0619F:txtMonth',
    'frm0619F:txtYear',
    'frm0619F:txtDueMonth',
    'frm0619F:txtDueDay',
    'frm0619F:txtDueYear',
    'frm0619F:optWithheld:Y',
    'frm0619F:optWithheld:N',
    'frm0619F:txtTaxTypeCode',
    'frm0619F:txtTIN1',
    'frm0619F:txtTIN2',
    'frm0619F:txtTIN3',
    'frm0619F:txtBranchCode',
    'frm0619F:txtRDOCode',
    'frm0619F:txtTaxpayerName',
    'frm0619F:txtAddress',
    'frm0619F:txtZipCode',
    'frm0619F:txtTelNum',
    'frm0619F:optCategory:P',
    'frm0619F:optCategory:G',
    'txtEmail'
)
$computedKeys = @(
    'frm0619F:txtDueMonth',
    'frm0619F:txtDueYear',
    'frm0619F:txtTax15',
    'frm0619F:txtTax17',
    'frm0619F:txtTax18D',
    'frm0619F:txtTax19'
)

function Get-ItemNumber([string]$Key) {
    if ($Key -match 'txtMonth$|txtYear$') { return '1' }
    if ($Key -match 'txtDue') { return '2' }
    if ($Key -match 'optAmend') { return '3' }
    if ($Key -match 'optWithheld') { return '4' }
    if ($Key -match 'txtTaxTypeCode') { return '5' }
    if ($Key -match 'txtTIN|txtBranchCode') { return '6' }
    if ($Key -match 'txtRDOCode') { return '7' }
    if ($Key -match 'txtTaxpayerName|txtLineBus') { return '8' }
    if ($Key -match 'txtAddress') { return '9' }
    if ($Key -match 'txtZipCode') { return '9A' }
    if ($Key -match 'txtTelNum') { return '10' }
    if ($Key -match 'optCategory') { return '11' }
    if ($Key -eq 'txtEmail') { return '12' }
    if ($Key -match 'txtTax13') { return '13' }
    if ($Key -match 'txtTax14') { return '14' }
    if ($Key -match 'txtTax15') { return '15' }
    if ($Key -match 'txtTax16') { return '16' }
    if ($Key -match 'txtTax17') { return '17' }
    if ($Key -match 'txtTax18') { return ($Key -replace '^.*txtTax', '') }
    if ($Key -match 'txtTax19') { return '19' }
    if ($Key -match '20') { return '20' }
    if ($Key -match '21') { return '21' }
    if ($Key -match '22') { return '22' }
    if ($Key -match '23') { return '23' }
    return $null
}
function Get-Label([string]$Key) {
    $labels = @{
        'frm0619F:txtMonth' = 'Month'
        'frm0619F:txtYear' = 'Year'
        'frm0619F:txtDueMonth' = 'Due date month'
        'frm0619F:txtDueDay' = 'Due date day'
        'frm0619F:txtDueYear' = 'Due date year'
        'frm0619F:optAmend:Y' = 'Amended return: Yes'
        'frm0619F:optAmend:N' = 'Amended return: No'
        'frm0619F:optWithheld:Y' = 'Taxes withheld/remitted: Yes'
        'frm0619F:optWithheld:N' = 'Taxes withheld/remitted: No'
        'frm0619F:txtTaxTypeCode' = 'Tax type code'
        'frm0619F:txtTIN1' = 'TIN segment 1'
        'frm0619F:txtTIN2' = 'TIN segment 2'
        'frm0619F:txtTIN3' = 'TIN segment 3'
        'frm0619F:txtBranchCode' = 'TIN branch code'
        'frm0619F:txtRDOCode' = 'RDO code'
        'frm0619F:txtTaxpayerName' = 'Withholding agent name'
        'frm0619F:txtLineBus' = 'Registered name / line of business'
        'frm0619F:txtAddress' = 'Registered address line 1'
        'frm0619F:txtAddress2' = 'Registered address line 2'
        'frm0619F:txtZipCode' = 'ZIP code'
        'frm0619F:txtTelNum' = 'Contact number'
        'frm0619F:optCategory:P' = 'Category: Private'
        'frm0619F:optCategory:G' = 'Category: Government'
        'txtEmail' = 'Email address'
        'frm0619F:txtTax13' = 'WMF10 remittance'
        'frm0619F:txtTax14' = 'WMF20 remittance'
        'frm0619F:txtTax15' = 'Total of items 13 and 14'
        'frm0619F:txtTax16' = 'Previously remitted amount'
        'frm0619F:txtTax17' = 'Net amount of remittance'
        'frm0619F:txtTax18A' = 'Surcharge'
        'frm0619F:txtTax18B' = 'Interest'
        'frm0619F:txtTax18C' = 'Compromise'
        'frm0619F:txtTax18D' = 'Total penalties'
        'frm0619F:txtTax19' = 'Total amount of remittance'
        'txtTaxAgentNo' = 'Tax agent accreditation or attorney roll number'
        'txtDateIssue' = 'Tax agent accreditation date of issue'
        'txtDateExpiry' = 'Tax agent accreditation date of expiry'
        'txtParticular23' = 'Other payment particulars'
        'txtFinalFlag' = 'Final-copy workflow flag'
        'txtEnroll' = 'Online enrollment flag'
        'ebirOnlineConfirmUsername' = 'Online confirmation username'
        'ebirOnlineUsername' = 'Online username'
        'ebirOnlineSecret' = 'Online secret'
        'driveSelectTPExport' = 'Export drive selection'
    }
    if ($labels.ContainsKey($Key)) { return $labels[$Key] }
    if ($Key -match '^txtAgency(?<item>2[0-3])$') { return "Item $($Matches.item) drawee bank or agency" }
    if ($Key -match '^txtNumber(?<item>2[0-3])$') { return "Item $($Matches.item) payment number" }
    if ($Key -match '^txtDate(?<item>2[0-3])$') { return "Item $($Matches.item) payment date" }
    if ($Key -match '^txtAmount(?<item>2[0-3])$') { return "Item $($Matches.item) payment amount" }
    return $Key
}

function New-Field([string]$Key) {
    $control = if ($controlById.ContainsKey($Key)) { $controlById[$Key] } else { $null }
    $kind = if ($control) { $control.control_kind } else { 'runtime-generated-select' }
    $logical = if ($kind -in @('radio', 'checkbox')) {
        'boolean'
    }
    elseif ($Key -match '^txtDate|DateIssue|DateExpiry') {
        'date'
    }
    elseif ($Key -match 'txtTax\d|txtAmount') {
        'decimal-amount'
    }
    elseif ($Key -match 'TIN|Branch|RDO|TaxType|Zip|Month|Day|Year|FinalFlag|Enroll') {
        'code'
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
    $enabledWhen = $null
    $visibleWhen = $null
    if ($Key -eq 'frm0619F:txtTax13') {
        $required = 'conditional'
        $requiredWhen = 'Item 4 is Yes and tax type code is WB.'
        $enabledWhen = 'Tax type code is WB.'
    }
    elseif ($Key -eq 'frm0619F:txtTax14') {
        $required = 'conditional'
        $requiredWhen = 'Item 4 is Yes and tax type code is WF.'
        $enabledWhen = 'Tax type code is WF.'
    }
    elseif ($Key -match '^txt(TaxAgentNo|DateIssue|DateExpiry|Agency|Number|Date2|Amount|Particular)') {
        $enabledWhen = 'Official runtime leaves this field disabled in the observed form workflow.'
    }
    elseif ($Key -match '^ebirOnline') {
        $visibleWhen = 'The online-submission dialog is active.'
    }

    $constraints = [ordered]@{}
    if ($control -and $control.maxlength -match '^\d+$') {
        $constraints.max_length = [int]$control.maxlength
    }
    if ($logical -eq 'decimal-amount') {
        $constraints.precision = 2
        $constraints.sign = 'source-dependent'
    }
    if ($logical -eq 'date') {
        $constraints.format = 'MM/DD/YYYY'
    }

    $enumValues = [object[]]::new(0)
    if ($kind -in @('radio', 'checkbox')) {
        $enumValues = [object[]]@('true', 'false')
    }
    elseif ($selectEnums.ContainsKey($Key)) {
        $enumValues = [object[]]@($selectEnums[$Key])
    }
    $normalization = [string[]]::new(0)
    if ($logical -eq 'decimal-amount') {
        $normalization = [string[]]@('NumWithComma', 'round(...,2)', 'formatCurrency')
    }
    elseif ($Key -match 'txtAddress') {
        $normalization = [string[]]@('Profile loading uppercases and splits at 127 characters.')
    }

    $notes = [Collections.Generic.List[string]]::new()
    [void]$notes.Add('Present in the revision-matched encrypted final-copy inventory; values excluded.')
    if ($Key -eq 'frm0619F:txtAddress2') {
        [void]$notes.Add('Present in encrypted final copy but omitted from the paired plaintext save.')
    }
    if ($control -and $control.disabled) {
        [void]$notes.Add('Declared disabled in the static runtime DOM.')
    }

    [pscustomobject][ordered]@{
        field_key = $Key
        serialized_key = $Key
        serialized_occurrence = 1
        label = Get-Label $Key
        page = 1
        item_number = Get-ItemNumber $Key
        control_kind = $kind
        storage_type = 'string'
        logical_type = $logical
        required = $required
        required_when = $requiredWhen
        enabled_when = $enabledWhen
        visible_when = $visibleWhen
        default_value = if ($control) { $control.default_value } else { '000' }
        empty_representation = ''
        constraints = [pscustomobject]$constraints
        enum_values = $enumValues
        normalization = $normalization
        computed = $computed
        calculation_id = if ($computed) { 'See calculations.json' } else { $null }
        source_refs = @(
            "xml-encrypted#decrypted-field:$Key",
            "official-hta-runtime#control:L$(if ($control) { $control.source_line } else { 2447 })"
        )
        confidence = 'high'
        notes = @($notes)
    }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    [void]$fields.Add((New-Field $key))
}
if ($fields.Count -ne 60) {
    throw "0619F typed inventory changed: $($fields.Count)."
}
Write-JsonFile (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = $fields.Count
    inventory_sha256 = Get-LineInventoryHash @($fields.field_key)
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
    (& $functionTool `
        -HtaPath $htaPath `
        -ControlPrefix 'frm0619F:' `
        -NamePattern '(?i)valid|check|save|date|submit|final') -join [Environment]::NewLine
)
Write-TextFile (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') (
    (& $functionTool `
        -HtaPath $htaPath `
        -ControlPrefix 'frm0619F:' `
        -NamePattern '(?i)comput|total|tax|penalt|amount|format') -join [Environment]::NewLine
)

$rules = [Collections.Generic.List[object]]::new()
function Add-Rule(
    [string]$Id,
    [string]$Phase,
    [int]$Order,
    [string]$Condition,
    [string[]]$FieldKeys,
    $Message,
    [string[]]$Refs,
    [string]$Assessment = 'verified-correct',
    [string]$Official = 'The branch alerts and stops the active operation.',
    [string]$Recommended = 'Retain as a structured revision-aware error.'
) {
    $rule = [pscustomobject][ordered]@{
        rule_id = $Id
        form_id = $formId
        revision = $revision
        phase = $Phase
        order = $Order
        condition = $Condition
        fields = $FieldKeys
        accepted_behavior = 'Condition is false; processing continues.'
        rejected_behavior = 'The active operation stops unless official_behavior states otherwise.'
        exact_message = $Message
        source_refs = $Refs
        evidence_type = @('source')
        assessment = $Assessment
        official_behavior = $Official
        recommended_app_behavior = $Recommended
        confidence = 'high'
        unresolved_questions = @()
    }
    [void]$rules.Add($rule)
}

Add-Rule '0619f-input-001-due-date' 'blur/change' 1 `
    'Filing month/year changes.' `
    @('frm0619F:txtMonth', 'frm0619F:txtYear', 'frm0619F:txtDueMonth', 'frm0619F:txtDueYear') `
    $null @('official-hta-runtime#computeDueDate:L2326-L2345') `
    'verified-correct' `
    'Due month becomes the next month; December rolls to January of the next year. Due day remains its default 10.' `
    'Preserve the rollover and explicitly derive the complete due date.'
Add-Rule '0619f-input-002-unused-choice-helper' input 2 `
    'Withheld/remitted or category choice is absent.' @('choice-fields') $null `
    @('official-hta-runtime#checkiftaxwheldisYes:L2259-L2268') 'obsolete' `
    'The helper returns messages, including the stale Item 12 category number, but is never called.' `
    'Use the active Validate branches as the compatibility source.'
Add-Rule '0619f-input-003-helper-case-typo' input 3 `
    'The unused helper requests frm0619F:optcategory:G with a lowercase c.' `
    @('frm0619F:optCategory:G') $null `
    @('official-hta-runtime#checkiftaxwheldisYes:L2264-L2267') 'official-bug-compatible' `
    'The ID does not match the live control, but the function is unreachable.' `
    'Remove the dead helper and keep one correctly cased choice validator.'
Add-Rule '0619f-input-004-tax-type-wb' 'blur/change' 4 `
    'Tax type code is WB.' @('frm0619F:txtTaxTypeCode', 'frm0619F:txtTax13', 'frm0619F:txtTax14') $null `
    @('official-hta-runtime#taxTypeCodeChange:L2310-L2317') 'verified-correct' `
    'Item 14 is reset to 0.00 and disabled; Item 13 is enabled; totals recompute.' `
    'Represent WB/WF as a typed discriminated branch and preserve the reset.'
Add-Rule '0619f-input-005-tax-type-wf' 'blur/change' 5 `
    'Tax type code is not WB (the live alternative is WF).' `
    @('frm0619F:txtTaxTypeCode', 'frm0619F:txtTax13', 'frm0619F:txtTax14') $null `
    @('official-hta-runtime#taxTypeCodeChange:L2317-L2323') 'verified-correct' `
    'Item 13 is reset to 0.00 and disabled; Item 14 is enabled; totals recompute.' `
    'Represent WB/WF as a typed discriminated branch and preserve the reset.'

Add-Rule '0619f-save-006-tin-wrong-item' save 6 `
    'Any TIN segment or branch code is blank.' @('TIN-fields') `
    'Please enter a valid TIN number on Item 7.' `
    @('official-hta-runtime#initialValidateBeforeSave:L2603-L2608') 'official-bug-compatible' `
    'Save reports Item 7, although TIN is Item 6 and Validate reports Item 6.' `
    'Validate the same condition but label the field as Item 6; retain the exact official message for compatibility diagnostics.'
Add-Rule '0619f-save-007-rdo-wrong-item' save 7 `
    'RDO value is 000.' @('frm0619F:txtRDOCode') `
    'Please enter a valid RDO Code on Item 8.' `
    @('official-hta-runtime#initialValidateBeforeSave:L2609-L2612') 'official-bug-compatible' `
    'Save reports Item 8, although RDO is Item 7 and Validate reports Item 7.' `
    'Validate the same condition but label the field as Item 7; retain the exact official message for compatibility diagnostics.'
Add-Rule '0619f-save-008-name-wrong-item' save 8 `
    'Withholding-agent name is blank.' @('frm0619F:txtTaxpayerName') `
    "Please enter a valid Withholding Agent's Name on Item 9." `
    @('official-hta-runtime#initialValidateBeforeSave:L2613-L2617') 'official-bug-compatible' `
    'Save reports Item 9, although the name is Item 8 and Validate reports Item 8.' `
    'Validate the same condition but label the field as Item 8; retain the exact official message for compatibility diagnostics.'
Add-Rule '0619f-save-009-sparse' save 9 `
    'Any field outside TIN/RDO/name is invalid.' @('all-other-form-fields') $null `
    @('official-hta-runtime#initialValidateBeforeSave:L2603-L2619') 'incorrect-official-behavior' `
    'Save ignores filing period, due date, choices, address/contact/email, remittance, and payment details.' `
    'Allow incomplete drafts explicitly, but never equate sparse Save checks with validity.'
Add-Rule '0619f-save-010-amended-version' save 10 `
    'A finalized/versioned return exists and Amended Return is not Yes.' `
    @('frm0619F:optAmend:Y', 'frm0619F:optAmend:N') `
    "If your intention is to make an amended return, kindly tick Amended Return to 'Yes' before hitting 'Save' or 'Final Copy' or 'Submit'." `
    @('official-hta-runtime#saveXML:L1728-L2008')

Add-Rule '0619f-validate-011-month-required' validate 11 `
    'Filing month is blank.' @('frm0619F:txtMonth') `
    'Please enter a valid month on Item 1.' @('official-hta-runtime#validateForm:L2349-L2353')
Add-Rule '0619f-validate-012-current-month' validate 12 `
    'Filing month equals the current month in the current year.' `
    @('frm0619F:txtMonth', 'frm0619F:txtYear') `
    'Invalid month on Item 1. Month should not be a current Date' `
    @('official-hta-runtime#validateForm:L2354-L2358') 'verified-correct' `
    'The source rejects the current month and rewinds the select before recomputing due date.' `
    'Require a completed prior withholding month.'
Add-Rule '0619f-validate-013-future-month' validate 13 `
    'Filing month is after the current month in the current year.' `
    @('frm0619F:txtMonth', 'frm0619F:txtYear') `
    'Invalid month on Item 1. Month should not be a future Date' `
    @('official-hta-runtime#validateForm:L2359-L2363')
Add-Rule '0619f-validate-014-year-required' validate 14 `
    'Filing year is blank.' @('frm0619F:txtYear') `
    'Please enter a valid year on Item 1.' @('official-hta-runtime#validateForm:L2366-L2370')
Add-Rule '0619f-validate-015-year-future' validate 15 `
    'Filing year exceeds current year.' @('frm0619F:txtYear') `
    'Invalid year on Item 1. Year should not be a future Date.' `
    @('official-hta-runtime#validateForm:L2371-L2375')
Add-Rule '0619f-validate-016-year-floor' validate 16 `
    'Filing year is before 2018.' @('frm0619F:txtYear') `
    'Invalid entry on Item 1. Entry should not be a previous year from 2018.' `
    @('official-hta-runtime#validateForm:L2376-L2380')
Add-Rule '0619f-validate-017-due-required' validate 17 `
    'Any due-date component is blank.' @('due-date-fields') `
    'Please enter a valid Date on Item 2' @('official-hta-runtime#validateForm:L2383-L2393')
Add-Rule '0619f-validate-018-due-month' validate 18 `
    'Due month is outside 1..12.' @('frm0619F:txtDueMonth') `
    'Please enter a valid Month on Item 2' @('official-hta-runtime#validateForm:L2394-L2396')
Add-Rule '0619f-validate-019-due-day' validate 19 `
    'Due day is outside 1..31 or violates selected month/leap year.' @('due-date-fields') `
    'Please enter a valid Day on Item 2' @('official-hta-runtime#validateForm:L2397-L2420')
Add-Rule '0619f-validate-020-due-year-future' validate 20 `
    'Due year exceeds current year.' @('frm0619F:txtDueYear') `
    'Year should not be a future Date. Please enter a valid Year on Item 2' `
    @('official-hta-runtime#validateForm:L2400-L2402')
Add-Rule '0619f-validate-021-due-year-floor' validate 21 `
    'Due year is before 2018.' @('frm0619F:txtDueYear') `
    'Previous year from 2018 is not applicable for this Form. Please enter a valid Year on Item 2' `
    @('official-hta-runtime#validateForm:L2403-L2405')
Add-Rule '0619f-validate-022-zero-padding' validate 22 `
    'Due month or day has one character after validation.' `
    @('frm0619F:txtDueMonth', 'frm0619F:txtDueDay') $null `
    @('official-hta-runtime#validateForm:L2423-L2429') 'verified-correct' `
    'The source left-pads each component with zero.' `
    'Normalize to two-digit month/day strings.'
Add-Rule '0619f-validate-023-withheld-choice' validate 23 `
    'Neither withheld/remitted Yes nor No is selected.' `
    @('frm0619F:optWithheld:Y', 'frm0619F:optWithheld:N') `
    'Please select an option for Item 4.' @('official-hta-runtime#validateForm:L2431-L2434')
Add-Rule '0619f-validate-024-category-choice' validate 24 `
    'Neither private nor government category is selected.' `
    @('frm0619F:optCategory:P', 'frm0619F:optCategory:G') `
    'Please select an option for Item 11.' @('official-hta-runtime#validateForm:L2435-L2438')
Add-Rule '0619f-validate-025-tin' validate 25 `
    'Any TIN segment or branch code is blank.' @('TIN-fields') `
    'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#validateForm:L2443-L2446')
Add-Rule '0619f-validate-026-tin-checksum-omitted' validate 26 `
    'TIN segments are nonblank but fail shared checksum/branch semantics.' @('TIN-fields') $null `
    @('official-hta-runtime#validateForm:L2443-L2446') 'incorrect-official-behavior' `
    'The source tests presence only.' 'Apply the shared evidence-backed TIN validation.'
Add-Rule '0619f-validate-027-rdo' validate 27 `
    'RDO selectedIndex is zero.' @('frm0619F:txtRDOCode') `
    'Please enter a valid RDO Code on Item 7.' @('official-hta-runtime#validateForm:L2447-L2450')
Add-Rule '0619f-validate-028-name' validate 28 `
    'Taxpayer/withholding-agent name is blank.' @('frm0619F:txtTaxpayerName') `
    'Please enter a valid Taxpayer Name on Item 8.' @('official-hta-runtime#validateForm:L2451-L2454')
Add-Rule '0619f-validate-029-contact' validate 29 `
    'Contact number is blank.' @('frm0619F:txtTelNum') `
    'Please enter a valid Contact Number on Item 10.' @('official-hta-runtime#validateForm:L2455-L2458')
Add-Rule '0619f-validate-030-address' validate 30 `
    'Primary registered-address line is blank.' @('frm0619F:txtAddress') `
    "Please enter Taxpayer's Registered Address on Item 9." `
    @('official-hta-runtime#validateForm:L2459-L2462')
Add-Rule '0619f-validate-031-zip' validate 31 `
    'ZIP code is blank.' @('frm0619F:txtZipCode') `
    "Please enter Taxpayer's Zip Code on Item 9A." @('official-hta-runtime#validateForm:L2463-L2466')
Add-Rule '0619f-validate-032-email' validate 32 `
    'Email is blank.' @('txtEmail') `
    'Please enter valid Email Address on Item 12.' @('official-hta-runtime#validateForm:L2467-L2470')
Add-Rule '0619f-validate-033-email-format-omitted' validate 33 `
    'Email is nonblank but malformed.' @('txtEmail') $null `
    @('official-hta-runtime#validateForm:L2467-L2470') 'incorrect-official-behavior' `
    'Validate checks only blankness.' 'Apply evidence-backed email syntax validation.'
Add-Rule '0619f-validate-034-remittance-conditional' validate 34 `
    'Item 4 Yes is selected and both Items 13 and 14 equal numeric zero.' `
    @('frm0619F:optWithheld:Y', 'frm0619F:txtTax13', 'frm0619F:txtTax14') `
    'Please fill up Part II - Tax Remittance if item 4 is set to Yes.' `
    @('official-hta-runtime#validateForm:L2471-L2476')
Add-Rule '0619f-validate-035-amended-choice-omitted' validate 35 `
    'Neither Amended Return Yes nor No is selected.' `
    @('frm0619F:optAmend:Y', 'frm0619F:optAmend:N') $null `
    @('official-hta-runtime#validateForm:L2349-L2476') 'incorrect-official-behavior' `
    'Validate accepts the missing choice.' 'Require an explicit Yes/No amended-return state.'
Add-Rule '0619f-validate-036-line-business-omitted' validate 36 `
    'Registered name / line of business is blank.' @('frm0619F:txtLineBus') $null `
    @('official-hta-runtime#validateForm:L2349-L2476') 'ambiguous' `
    'Validate never inspects the field.' `
    'Preserve it and require it only when supported by revision-matched instructions.'
Add-Rule '0619f-validate-037-address2-save-loss' validate 37 `
    'Second registered-address line exists in the final copy.' @('frm0619F:txtAddress2') $null `
    @('xml-encrypted', 'xml-plaintext', 'official-hta-runtime#saveXML:L1728-L2008') `
    'official-bug-compatible' `
    'The encrypted final copy retains the field, while the paired plaintext save omits it.' `
    'Preserve the field losslessly across every save/final-copy transition.'
Add-Rule '0619f-validate-038-negative-net' validate 38 `
    'Item 16 exceeds Item 15, producing a negative Item 17 and potentially negative remittance.' `
    @('frm0619F:txtTax15', 'frm0619F:txtTax16', 'frm0619F:txtTax17') $null `
    @('official-hta-runtime#computeNetAmtRem:L2291-L2294', 'official-hta-runtime#validateForm:L2349-L2476') `
    'incorrect-official-behavior' `
    'The source computes the negative result and does not reject it.' `
    'Validate the legally permitted relationship between prior remittance and current total.'
Add-Rule '0619f-validate-039-disabled-payment-metadata' validate 39 `
    'Tax-agent accreditation and Part III payment-detail fields contain any value.' `
    @('txtTaxAgentNo', 'txtDateIssue', 'txtDateExpiry', 'payment-detail-fields') $null `
    @('official-hta-runtime#controls:L835-L929', 'official-hta-runtime#validateForm:L2349-L2476') `
    'ambiguous' `
    'The fields are statically disabled, serialized when present, and never validated by this form.' `
    'Preserve imported values losslessly; do not invent validation without revision-matched evidence.'
Add-Rule '0619f-validate-040-success' validate 40 `
    'All active Validate branches pass.' @('frm0619F:cmdValidate', 'frm0619F:cmdEdit') `
    'Validation successful. Click on Edit if you wish to modify your entries.' `
    @('official-hta-runtime#validateForm:L2478-L2485') 'verified-correct' `
    'Validate disables controls and enables Edit, upload, and Final Copy.' `
    'Tie validation state to the exact field snapshot.'

Write-JsonFile (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    first_error_behavior = 'Save and Validate alert the first matching active branch and return.'
    rules = $rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Add-Calculation(
    [string]$Id,
    [string[]]$Outputs,
    [string[]]$Inputs,
    [string]$Formula,
    [string]$Trigger,
    [string[]]$Dependencies,
    [string[]]$Refs,
    [string]$Assessment = 'verified-correct'
) {
    $calculation = [pscustomobject][ordered]@{
        calculation_id = $Id
        outputs = $Outputs
        inputs = $Inputs
        condition = $null
        official_formula = $Formula
        rounding = 'formatCurrency after NumWithComma conversion; displayed to two decimal places.'
        trigger = $Trigger
        depends_on = $Dependencies
        source_refs = $Refs
        assessment = $Assessment
        recommended_app_behavior = 'Use typed decimals and preserve the source dependency order except where the source is defective.'
        confidence = 'high'
    }
    [void]$calculations.Add($calculation)
}
Add-Calculation '0619f-due-date' `
    @('frm0619F:txtDueMonth', 'frm0619F:txtDueDay', 'frm0619F:txtDueYear') `
    @('frm0619F:txtMonth', 'frm0619F:txtYear') `
    'Due date is the 10th day of the following month; December rolls to January of year + 1.' `
    'computeDueDate' @() @('official-hta-runtime#computeDueDate:L2326-L2345')
Add-Calculation '0619f-item15-total-atc' `
    @('frm0619F:txtTax15') @('frm0619F:txtTax13', 'frm0619F:txtTax14') `
    '15 = 13 + 14.' 'computeTotalAtc' @() @('official-hta-runtime#computeTotalAtc:L2284-L2288')
Add-Calculation '0619f-item17-net' `
    @('frm0619F:txtTax17') @('frm0619F:txtTax15', 'frm0619F:txtTax16') `
    '17 = 15 - 16.' 'computeNetAmtRem' @('0619f-item15-total-atc') `
    @('official-hta-runtime#computeNetAmtRem:L2291-L2294') 'incorrect-official-behavior'
Add-Calculation '0619f-item18d-penalties' `
    @('frm0619F:txtTax18D') `
    @('frm0619F:txtTax18A', 'frm0619F:txtTax18B', 'frm0619F:txtTax18C') `
    '18D = 18A + 18B + 18C.' 'computePenalties' @() `
    @('official-hta-runtime#computePenalties:L2297-L2301')
Add-Calculation '0619f-item19-total' `
    @('frm0619F:txtTax19') @('frm0619F:txtTax17', 'frm0619F:txtTax18D') `
    '19 = 17 + 18D.' 'computeTotalAmtRem' `
    @('0619f-item17-net', '0619f-item18d-penalties') `
    @('official-hta-runtime#computeTotalAmtRem:L2304-L2307')

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
    $negativeCase = [pscustomobject][ordered]@{
        case_id = ('case-{0:d2}-{1}' -f $caseNumber, $rule.rule_id)
        phase = $rule.phase
        mutations = @{ synthetic_condition = $rule.condition }
        expected_message = $rule.exact_message
        expected_behavior = $rule.official_behavior
        rule_id = $rule.rule_id
    }
    [void]$negativeCases.Add($negativeCase)
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
            case_id = 'december-rollover'
            calculation_id = '0619f-due-date'
            filing_month = 12
            filing_year = 2025
            official_output = '01/10/2026'
        },
        @{
            case_id = 'wb-branch'
            rule_id = '0619f-input-004-tax-type-wb'
            item13 = 100
            item14_before = 75
            official_item14_after = 0
            official_item15 = 100
        },
        @{
            case_id = 'wf-branch'
            rule_id = '0619f-input-005-tax-type-wf'
            item13_before = 100
            item14 = 75
            official_item13_after = 0
            official_item15 = 75
        },
        @{
            case_id = 'negative-net'
            calculation_id = '0619f-item17-net'
            item15 = 100
            item16 = 125
            official_output = -25
            recommended_behavior = 'reject or apply the legally correct amended-return rule'
        },
        @{
            case_id = 'total-remittance'
            calculation_id = '0619f-item19-total'
            item17 = 1000
            item18d = 60
            official_output = 1060
        }
    )
})

$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    phases = @(
        @{
            phase = 'edit'
            official_behavior = 'January 2018 monthly remittance form for final income taxes withheld.'
            source_refs = @('official-hta-runtime', 'official-form-pdf', 'packaged-help')
            confidence = 'high'
        },
        @{
            phase = 'saved-draft'
            official_behavior = 'Save checks only TIN, RDO, and withholding-agent name, with stale item numbers, before serializing the draft.'
            source_refs = @('official-hta-runtime#initialValidateBeforeSave:L2603-L2619')
            confidence = 'high'
        },
        @{
            phase = 'validated'
            official_behavior = 'Validate runs filing-period, due-date, choice, identity/contact, and conditional remittance checks.'
            source_refs = @('official-hta-runtime#validateForm:L2349-L2485')
            confidence = 'high'
        },
        @{
            phase = 'final-copy'
            official_behavior = 'Final Copy is enabled after Validate and writes an encrypted/compressed copy with 60 keys.'
            source_refs = @('official-hta-runtime#saveEncryptedProfile:L1564-L1650', 'xml-encrypted')
            confidence = 'high'
        },
        @{
            phase = 'submitted'
            official_behavior = 'Online transport code exists but was not exercised.'
            source_refs = @('official-hta-runtime#sendEmail')
            confidence = 'medium'
        }
    )
    transitions = @(
        @{
            from = 'edit'
            action = 'Save'
            to = 'saved-draft'
            guard = 'Sparse Save checks pass.'
            side_effects = @('Writes flat pseudo-XML with 59 keys in the paired sample.')
            source_refs = @('official-hta-runtime#saveXML:L1728-L2008', 'xml-plaintext')
        },
        @{
            from = 'edit'
            action = 'Validate'
            to = 'validated'
            guard = 'All ordered active validation branches pass.'
            side_effects = @('Disables controls.', 'Enables Edit, upload, and Final Copy.')
            source_refs = @('official-hta-runtime#validateForm:L2349-L2485')
        },
        @{
            from = 'validated'
            action = 'Edit'
            to = 'edit'
            guard = $null
            side_effects = @('Re-enables editable controls according to tax-type branch.')
            source_refs = @('official-hta-runtime#enableAllControl:L2539-L2560')
        },
        @{
            from = 'validated'
            action = 'Final Copy'
            to = 'final-copy'
            guard = 'Final-copy save succeeds.'
            side_effects = @('Writes encrypted/compressed copy with 60 keys.')
            source_refs = @('official-hta-runtime#saveEncryptedProfile:L1564-L1650', 'xml-encrypted')
        },
        @{
            from = 'final-copy'
            action = 'Online transport'
            to = 'submitted'
            guard = 'Connectivity and remote acceptance succeed.'
            side_effects = @('Untested online attempt.')
            source_refs = @('official-hta-runtime#sendEmail')
        }
    )
    prerequisites = @(
        'Completed prior withholding month',
        'Derived due date',
        'Withheld/remitted choice and WB/WF tax type',
        'TIN/RDO and identity',
        'Category/contact/address/email',
        'Conditional Part II remittance'
    )
    required_attachments = @()
    filing_deadlines = @(
        @{
            quarter = 'Q1'
            due_date_rule = 'For each covered month, the source derives the due date as the 10th day of the following month.'
            source_refs = @('official-hta-runtime#computeDueDate:L2326-L2345')
            confidence = 'high'
        },
        @{
            quarter = 'Q2'
            due_date_rule = 'For each covered month, the source derives the due date as the 10th day of the following month.'
            source_refs = @('official-hta-runtime#computeDueDate:L2326-L2345')
            confidence = 'high'
        },
        @{
            quarter = 'Q3'
            due_date_rule = 'For each covered month, the source derives the due date as the 10th day of the following month.'
            source_refs = @('official-hta-runtime#computeDueDate:L2326-L2345')
            confidence = 'high'
        },
        @{
            quarter = 'Q4'
            due_date_rule = 'For each covered month, the source derives the due date as the 10th day of the following month.'
            source_refs = @('official-hta-runtime#computeDueDate:L2326-L2345')
            confidence = 'high'
        }
    )
}
Write-JsonFile (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @(
    $rules | Where-Object {
        $_.assessment -in @(
            'official-bug-compatible',
            'incorrect-official-behavior',
            'obsolete'
        )
    }
).Count
$assets = @(
    New-Asset 'package-7.9.6' 'official-package-executable' $packagePath `
        'Installed Offline eBIRForms package 7.9.6.0.'
    New-Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath `
        'January 2018 runtime.'
    New-Asset 'packaged-help' 'official-runtime-help' $helpPath `
        'Packaged January 2018 instructions.'
    New-Asset 'official-form-pdf' 'official-form-pdf' $pdfPath `
        'January 2018 official form.'
    New-Asset 'xml-encrypted' 'dummy-profile-encrypted-final-copy' `
        $sampleByHash[$expected.cipher].FullName `
        'Revision-matched 60-key dummy final copy; values excluded.' `
        (Join-Path $OfficialDir '0619F-final-copy-#email-redacted#.xml')
    New-Asset 'xml-plaintext' 'dummy-profile-plaintext-save' `
        $sampleByHash[$expected.plain].FullName `
        'Revision-matched 59-key dummy save; values excluded.' `
        (Join-Path $OfficialDir '0619F-save-#email-redacted#.xml')
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    form_code = '0619F'
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
        unverified_gaps = 1
    }
    artifacts = [ordered]@{
        fields = 'fields.json'
        validations = 'validations.json'
        calculations = 'calculations.json'
        workflow = 'workflow.json'
        evidence = 'evidence.md'
        audit = 'audit.md'
        gaps = 'gaps.md'
        encrypted_field_audit = 'fixtures/encrypted-field-audit-v796.json'
        plaintext_field_audit = 'fixtures/plaintext-field-audit-v796.json'
        runtime_controls = 'fixtures/runtime-control-inventory-v796.json'
        validation_functions = 'fixtures/validation-function-inventory-v796.json'
        calculation_functions = 'fixtures/calculation-function-inventory-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'
        calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer or release metadata changed.',
        'No decrypted values or email-bearing filenames emitted.',
        'The 60-key final-copy inventory is lossless; the paired plaintext save omits txtAddress2.',
        'Save has copied Item 7/8/9 messages for fields printed as Items 6/7/8.'
    )
}
Write-JsonFile (Join-Path $outDir 'manifest.json') $manifest
Write-TextFile (Join-Path $outDir 'README.md') @"
# BIR Form 0619F - January 2018

Revision-specific Offline eBIRForms rules with 60 concrete serialized keys and
no runtime field families.
"@
Write-TextFile (Join-Path $outDir 'evidence.md') @"
# Evidence

- January 2018 runtime: $($expected.hta); help: $($expected.help); PDF: $($expected.pdf).
- Encrypted final copy: 60 unique keys, inventory $($expected.encrypted_inventory); values excluded.
- Plaintext save: 59 unique keys, inventory $($expected.plain_inventory); values excluded.
- The only difference is ``frm0619F:txtAddress2``, retained by final copy and omitted by plaintext Save.
- Key accounting: 59 static controls + 1 runtime RDO; zero unexplained keys.
- The runtime has 86 live static controls and tax-type choices WB and WF.
- All email-bearing filenames use ``#email-redacted#``.
"@
Write-TextFile (Join-Path $outDir 'gaps.md') @"
# Gaps

1. Online submission was not exercised.
"@
Write-TextFile (Join-Path $outDir 'audit.md') @"
# Audit

- January 2018 binding: pass.
- Lossless final-copy inventory: 60 keys; plaintext Save omission documented.
- Typed inventory: 60 concrete keys, no families, zero unexplained.
- Validations $($rules.Count); calculations $($calculations.Count); negatives $($negativeCases.Count); defects $bugCount.
- Focused and full strict audits must run.
- No renderer/release/capability/commit/push changes.
"@

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry = $index.forms | Where-Object { $_.form_id -eq $formId }
if ($entry) {
    $entry.form_code = '0619F'
    $entry.revision = $revision
    $entry.package_version = $packageVersion
    $entry.priority = 40
    $entry.status = 'complete'
    $entry.path = 'forms/0619f-v2018/manifest.json'
}
else {
    $index.forms += [pscustomobject][ordered]@{
        form_id = $formId
        form_code = '0619F'
        revision = $revision
        package_version = $packageVersion
        priority = 40
        status = 'complete'
        path = 'forms/0619f-v2018/manifest.json'
    }
}
$index.forms = @($index.forms | Sort-Object priority)
$index.updated = '2026-07-23'
Write-JsonFile $indexPath $index

$actual = [ordered]@{
    live_controls = $controls.Count
    encrypted_keys = $keys.Count
    plaintext_keys = $plainKeys.Count
    static_matches = $staticMatches.Count
    runtime_rdo = $runtimeRdo.Count
    unexplained = $unexplained.Count
    fields = $fields.Count
    validations = $rules.Count
    calculations = $calculations.Count
    negatives = $negativeCases.Count
    bugs = $bugCount
}
$expectedCounts = [ordered]@{
    live_controls = 86
    encrypted_keys = 60
    plaintext_keys = 59
    static_matches = 59
    runtime_rdo = 1
    unexplained = 0
    fields = 60
    validations = 40
    calculations = 5
    negatives = 26
    bugs = 11
}
foreach ($name in $expectedCounts.Keys) {
    if ($actual[$name] -ne $expectedCounts[$name]) {
        throw "0619F fail-closed count changed: $name expected $($expectedCounts[$name]), found $($actual[$name])."
    }
}

[pscustomobject][ordered]@{
    form_id = $formId
    live_controls = $controls.Count
    encrypted_keys = $keys.Count
    plaintext_keys = $plainKeys.Count
    static_matches = $staticMatches.Count
    runtime_rdo = $runtimeRdo.Count
    unexplained = $unexplained.Count
    typed_fields = $fields.Count
    validations = $rules.Count
    calculations = $calculations.Count
    negative_fixtures = $negativeCases.Count
    confirmed_official_bugs = $bugCount
    next_form = '1601C'
} | ConvertTo-Json
