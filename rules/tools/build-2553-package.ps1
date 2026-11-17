param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\2553v1999',
    [string]$SampleDir = 'C:\Mac\Home\Downloads\forms\2553'
)

$ErrorActionPreference = 'Stop'
$formId = '2553-v1999'
$revision = '1999-07-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form2553.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help2553.hta'
$atcPath = Join-Path $ExtractedRoot 'xml\atcCodes.xml'
$pdfPath = Join-Path $OfficialDir '42792553.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\2553-v1999'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = '67f3bb050668791c87060c0e7f2c20f3215434082ed443f5309bcb3f8e79c2df'
    help = '58d0372fdd015518076278ca2445df208053714d499a015d8957730dd00ccf0c'
    pdf = 'e52f96fe48aba2890078f889930744a4e13a4defe1284aa9c5292e2c702a20e5'
    atc = '16e4db6ce456a6fb0a97f085cf8ef19349c2f6fc183971e97d4e253f63cdd22b'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher = '424578f78773ec1ae84d8e7b3bace1dfb4e0a02291985eae2fc79804e5f8d09d'
    plain = '9510052b73f71d17150c853ae9d89d3e38142a49824619afa712f112488df11e'
    inventory = '9bf7a4b80bd246c737b7e482ad4e7e11e5e83a565e11af24dfdc3ecbd8764403'
}

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function Hash-Lines([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-', '').ToLowerInvariant()
    } finally { $sha.Dispose() }
}
function Attr([string]$Tag, [string]$Name) {
    $match = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($match.Success) { $match.Groups[2].Value } else { $null }
}
function Asset([string]$Id, [string]$Kind, [string]$Path, [string]$Binding, [string]$Display = '') {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $Id
        kind = $Kind
        path = if ($Display) { $Display } else { $Path }
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        size = $item.Length
        revision_binding = $Binding
    }
}

foreach ($pair in @(
    @($htaPath, 'hta'), @($helpPath, 'help'), @($pdfPath, 'pdf'),
    @($atcPath, 'atc'), @($packagePath, 'package')
)) {
    if (-not (Test-Path -LiteralPath $pair[0] -PathType Leaf)) { throw "Missing $($pair[0])" }
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
$samples = @(Get-ChildItem -LiteralPath $SampleDir -File -Filter '*.xml')
if ($samples.Count -ne 1) { throw "Expected one encrypted sample; found $($samples.Count)." }
if ((Get-FileHash -LiteralPath $samples[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected.cipher) {
    throw 'Encrypted sample hash changed.'
}
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') { throw 'Official PDF magic mismatch.' }

$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)July\s+1999\s+\(ENCS\)' -or $hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']2553["'']') {
    throw 'July 1999 runtime binding changed.'
}
if ($help -notmatch '(?i)due date for payment of the tax as stated in the special law' -or
    $help -notmatch '(?i)Certificate of Creditable Tax Withheld at Source') {
    throw 'Packaged help binding changed.'
}

New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null
$keyTool = Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$redactedSample = Join-Path $SampleDir '2553-final-copy-#email-redacted#.xml'
$keyJson = & $keyTool `
    -SourcePath $samples[0].FullName `
    -RedactedSourcePath $redactedSample `
    -FormId $formId `
    -ExpectedCiphertextSha256 $expected.cipher `
    -ExpectedDecryptedSha256 $expected.plain `
    -ExpectedFieldCount 68 `
    -ExpectedFieldInventorySha256 $expected.inventory
$keyAudit = $keyJson | ConvertFrom-Json
Write-Utf8 (Join-Path $fixtureDir 'encrypted-field-keys-v796.json') ($keyJson -join [Environment]::NewLine)

$formMatch = [regex]::Match($hta, '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain missing.' }
$body = $formMatch.Groups['body'].Value
$bodyOffset = $formMatch.Groups['body'].Index
$excluded = @(
    @([regex]::Matches($body, '(?is)<script\b.*?</script>')) +
    @([regex]::Matches($body, '(?is)<!--.*?-->'))
)
$controls = [Collections.Generic.List[object]]::new()
$ordinal = 0
foreach ($match in [regex]::Matches($body, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $skip = $false
    foreach ($range in $excluded) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) { $skip = $true; break }
    }
    if ($skip) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $kind = $kind.ToLowerInvariant()
    $default = Attr $tag 'value'
    if ($kind -in @('radio', 'checkbox')) { $default = if ($tag -match '(?i)\bchecked(?:\s*=|\s|>)') { 'true' } else { 'false' } }
    $controls.Add([pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Attr $tag 'id'
        name = Attr $tag 'name'
        element = $element
        control_kind = $kind
        source_line = 1 + [regex]::Matches($hta.Substring(0, $bodyOffset + $match.Index), "`n").Count
        default_value = $default
        maxlength = Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}
$serial = @($controls | Where-Object { $_.control_kind -in @('text', 'select', 'select-one', 'textarea', 'radio', 'checkbox') })
$staticIds = @($serial.id | Where-Object { $_ } | Sort-Object -Unique)
$runtimeRdo = 'frm2553:txtRDOCode'
if ($controls.Count -ne 90 -or $staticIds.Count -ne 67) {
    throw "Expected 90 live controls and 67 static serialized IDs; found $($controls.Count)/$($staticIds.Count)."
}
if ($staticIds -contains $runtimeRdo -or $hta -notmatch [regex]::Escape("<select class='iceSelOneMnu' id='frm2553:txtRDOCode'")) {
    throw 'Runtime RDO selector derivation changed.'
}
$expectedKeys = @($staticIds + $runtimeRdo | Sort-Object)
$sampleKeys = @($keyAudit.keys | Sort-Object)
if (@(Compare-Object $expectedKeys $sampleKeys).Count -ne 0) {
    throw 'Revision-matched final-copy keys no longer equal the source-derived inventory.'
}
$byId = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $byId.ContainsKey($control.id)) { $byId[$control.id] = $control }
}

$required = @(
    'frm2553:itemFiscalStartMonth:_1', 'frm2553:itemFiscalStartMonth:_2',
    'frm2553:itemYearEndMonth', 'frm2553:txtYearEnded',
    'frm2553:optQtr:_1', 'frm2553:optQtr:_2', 'frm2553:optQtr:_3', 'frm2553:optQtr:_4',
    'frm2553:txtTIN1', 'frm2553:txtTIN2', 'frm2553:txtTIN3', 'frm2553:txtBranchCode',
    $runtimeRdo, 'frm2553:txtDescription', 'frm2553:txtTPName', 'frm2553:txtTelNum',
    'frm2553:txtAddress', 'frm2553:txtZipCode'
)
$computed = @(
    'frm2553:txt14E', 'frm2553:txt15E', 'frm2553:txt16E', 'frm2553:txt17E', 'frm2553:txt18E',
    'frm2553:txt19', 'frm2553:txt20C', 'frm2553:txt21', 'frm2553:txt22D', 'frm2553:txt23'
)
$calcByField = @{
    'frm2553:txt14E' = '2553-row-tax'; 'frm2553:txt15E' = '2553-row-tax'
    'frm2553:txt16E' = '2553-row-tax'; 'frm2553:txt17E' = '2553-row-tax'; 'frm2553:txt18E' = '2553-row-tax'
    'frm2553:txt19' = '2553-item19-total-tax'; 'frm2553:txt20C' = '2553-item20c-total-credits'
    'frm2553:txt21' = '2553-item21-tax-payable'; 'frm2553:txt22D' = '2553-item22d-total-penalties'
    'frm2553:txt23' = '2553-item23-total-payable'
}
function Item-For([string]$Key) {
    if ($Key -match 'frm2553:txt(?<n>1[4-9]|2[0-3])') { return $Matches.n }
    $map = @{
        'frm2553:itemFiscalStartMonth:_1' = '1'; 'frm2553:itemFiscalStartMonth:_2' = '1'
        'frm2553:itemYearEndMonth' = '2'; 'frm2553:txtYearEnded' = '2'
        'frm2553:optQtr:_1' = '3'; 'frm2553:optQtr:_2' = '3'; 'frm2553:optQtr:_3' = '3'; 'frm2553:optQtr:_4' = '3'
        'frm2553:optAmended_1' = '4'; 'frm2553:optAmended_2' = '4'; 'frm2553:txtSheets' = '5'
        'frm2553:txtTIN1' = '6'; 'frm2553:txtTIN2' = '6'; 'frm2553:txtTIN3' = '6'; 'frm2553:txtBranchCode' = '6'
        'frm2553:txtRDOCode' = '7'; 'frm2553:txtDescription' = '8'; 'frm2553:txtTPName' = '9'
        'frm2553:txtTelNum' = '10'; 'frm2553:txtAddress' = '11'; 'frm2553:txtZipCode' = '12'
        'frm2553:optTreaty_1' = '13'; 'frm2553:optTreaty_2' = '13'; 'frm2553:lstTaxTreaty' = '13'
        'frm2553:ifoverpay_1' = '23'; 'frm2553:ifoverpay_2' = '23'
    }
    if ($map.ContainsKey($Key)) { $map[$Key] } else { $null }
}
function Label-For([string]$Key) {
    if ($Key -match 'frm2553:txt(?<row>1[4-8])(?<col>[A-E])') {
        $labels = @{ A = 'Taxable transaction or industry classification'; B = 'ATC'; C = 'Taxable amount'; D = 'Tax rate'; E = 'Tax due' }
        return "Item $($Matches.row)$($Matches.col) $($labels[$Matches.col])"
    }
    $labels = @{
        'frm2553:itemFiscalStartMonth:_1' = 'Calendar year'; 'frm2553:itemFiscalStartMonth:_2' = 'Fiscal year'
        'frm2553:itemYearEndMonth' = 'Month ended'; 'frm2553:txtYearEnded' = 'Year ended'
        'frm2553:optQtr:_1' = 'First quarter'; 'frm2553:optQtr:_2' = 'Second quarter'
        'frm2553:optQtr:_3' = 'Third quarter'; 'frm2553:optQtr:_4' = 'Fourth quarter'
        'frm2553:optAmended_1' = 'Amended return Yes'; 'frm2553:optAmended_2' = 'Amended return No'
        'frm2553:txtSheets' = 'Number of sheets attached'; 'frm2553:txtTIN1' = 'TIN segment 1'
        'frm2553:txtTIN2' = 'TIN segment 2'; 'frm2553:txtTIN3' = 'TIN segment 3'
        'frm2553:txtBranchCode' = 'Branch code'; 'frm2553:txtRDOCode' = 'RDO code'
        'frm2553:txtDescription' = 'Line of business or occupation'; 'frm2553:txtTPName' = 'Taxpayer name'
        'frm2553:txtTelNum' = 'Telephone number'; 'frm2553:txtAddress' = 'Registered address'
        'frm2553:txtZipCode' = 'ZIP code'; 'frm2553:optTreaty_1' = 'Special rate or treaty Yes'
        'frm2553:optTreaty_2' = 'Special rate or treaty No'; 'frm2553:lstTaxTreaty' = 'Special rate or treaty basis'
        'frm2553:txt19' = 'Total tax due'; 'frm2553:txt20A' = 'Tax paid in return previously filed'
        'frm2553:txt20B' = 'Creditable tax withheld per BIR Form 2307'; 'frm2553:txt20C' = 'Total tax credits or payments'
        'frm2553:txt21' = 'Tax payable or overpayment'; 'frm2553:txt22A' = 'Surcharge'
        'frm2553:txt22B' = 'Interest'; 'frm2553:txt22C' = 'Compromise'; 'frm2553:txt22D' = 'Total penalties'
        'frm2553:txt23' = 'Total amount payable or overpayment'; 'frm2553:ifoverpay_1' = 'Overpayment to be refunded'
        'frm2553:ifoverpay_2' = 'Overpayment tax credit certificate'
        'txtFinalFlag' = 'Workflow final flag'; 'txtEnroll' = 'Online enrollment state'
        'ebirOnlineConfirmUsername' = 'Online confirmation username'; 'ebirOnlineUsername' = 'Online username'
        'ebirOnlineSecret' = 'Online secret'; 'txtEmail' = 'Online email'; 'driveSelectTPExport' = 'Export drive selector'
    }
    if ($labels.ContainsKey($Key)) { $labels[$Key] } else { $Key }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keyAudit.keys) {
    $control = if ($byId.ContainsKey($key)) { $byId[$key] } else { $null }
    $logical = 'string'
    $normalization = [string[]]@()
    $enumValues = [object[]]@()
    if (($control -and $control.control_kind -in @('radio', 'checkbox')) -or $key -match ':(?:opt|ifoverpay|itemFiscalStartMonth)') {
        $logical = 'boolean'
        $enumValues = [object[]]@('true', 'false')
    } elseif ($key -eq 'frm2553:itemYearEndMonth') {
        $logical = 'month-code'
        $enumValues = [object[]]@('00', '01', '02', '03', '04', '05', '06', '07', '08', '09', '10', '11', '12')
    } elseif ($key -eq 'frm2553:lstTaxTreaty') {
        $logical = 'enum-code'
        $enumValues = [object[]]@('0', '1', '2')
    } elseif ($key -match '(?:TIN|BranchCode|RDOCode|txt\d+[AB]$|ZipCode)') {
        $logical = 'code'
    } elseif ($key -eq 'frm2553:txtYearEnded') {
        $logical = 'year-string'
    } elseif ($key -match 'frm2553:txt(?:1[4-8][CE]|19|20[A-C]|21|22[A-D]|23)$') {
        $logical = 'decimal-amount'
        $normalization = [string[]]@('NumWithComma', 'formatCurrency', 'round(2)')
    } elseif ($key -match 'frm2553:txt1[4-8]D$') {
        $logical = 'percentage'
    }
    $isComputed = $computed -contains $key
    $status = if ($isComputed) { 'computed' } elseif ($required -contains $key) { 'required' } else { 'optional' }
    $requiredWhen = $null
    if ($key -match 'frm2553:txt1[4-8]C$') { $status = 'conditional'; $requiredWhen = 'The corresponding ATC field is nonblank.' }
    if ($key -eq 'frm2553:lstTaxTreaty') { $status = 'conditional'; $requiredWhen = 'Item 13 Yes is selected.' }
    if ($key -match 'frm2553:ifoverpay_[12]') { $status = 'conditional'; $requiredWhen = 'Item 23 is negative and the official formatted-number comparison succeeds.' }
    $constraints = [ordered]@{}
    if ($control -and $control.maxlength -match '^\d+$') { $constraints.max_length = [int]$control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision = 2; $constraints.sign = 'official keypress filtering is incomplete and does not establish a reliable sign bound' }
    $default = if ($control) { $control.default_value } else { '000' }
    if ($key -eq 'frm2553:itemYearEndMonth') { $default = '00' }
    if ($key -eq 'frm2553:lstTaxTreaty') { $default = '0' }
    $sourceLine = if ($control) { $control.source_line } else { 2991 }
    $fields.Add([pscustomobject][ordered]@{
        field_key = $key
        serialized_key = $key
        serialized_occurrence = 1
        label = Label-For $key
        page = if ($key -like 'frm2553:*') { 1 } else { $null }
        item_number = Item-For $key
        control_kind = if ($control) { $control.control_kind } else { 'runtime-generated-select' }
        storage_type = 'string'
        logical_type = $logical
        required = $status
        required_when = $requiredWhen
        enabled_when = if ($key -eq 'frm2553:txt20A') { 'Amended return Yes is selected.' } elseif ($key -eq 'frm2553:lstTaxTreaty') { 'Item 13 Yes is selected.' } else { $null }
        visible_when = $null
        default_value = $default
        empty_representation = ''
        constraints = [pscustomobject]$constraints
        enum_values = $enumValues
        normalization = $normalization
        computed = $isComputed
        calculation_id = if ($calcByField.ContainsKey($key)) { $calcByField[$key] } else { $null }
        source_refs = @('official-hta-runtime#saveXML', "official-hta-runtime#control:L$sourceLine", 'revision-matched-final-copy-keys')
        confidence = 'high'
        notes = @('The key is present in the hash-pinned 68-key July 1999 final-copy inventory; no value is emitted.')
    })
}
if ($fields.Count -ne 68) { throw "Expected 68 fields; found $($fields.Count)." }
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = 68
    inventory_sha256 = Hash-Lines @($fields.field_key | Sort-Object)
    fields = $fields
})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    official_hta_sha256 = $expected.hta
    live_control_count = $controls.Count
    static_serialized_id_count = $staticIds.Count
    runtime_generated_scalar_count = 1
    runtime_generated_scalars = @($runtimeRdo)
    revision_matched_final_copy_key_count = $keyAudit.field_count
    source_and_final_copy_inventories_match = $true
    controls = $controls
})

$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm2553:' -NamePattern '(?i)valid|check|save|enable|date|treaty|submit|final') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm2553:' -NamePattern '(?i)compute|amount|tax|penalt|overpay|format') -join [Environment]::NewLine)

$atcEntries = @(
    [pscustomobject]@{ code = 'OT010'; description = 'PAGCOR'; catalog_rate = '5.0'; runtime_rate = '5.0' }
    [pscustomobject]@{ code = 'OT012'; description = 'OTHERS'; catalog_rate = '5.0'; runtime_rate = '0.0'; rate_editable = $true }
    [pscustomobject]@{ code = 'OT011'; description = 'CLARK DEVELOPMENT CORPORATIONS'; catalog_rate = '5.0'; runtime_rate = '5.0' }
    [pscustomobject]@{ code = 'OT011'; description = 'SPECIAL/REGULAR/ECONOMIC FREE PORT ZONE ENTERPRISES'; catalog_rate = '5.0'; runtime_rate = '5.0' }
)
foreach ($entry in $atcEntries) {
    if ($hta -notmatch [regex]::Escape('if(atc.formType == "2553")') -or
        [IO.File]::ReadAllText($atcPath) -notmatch [regex]::Escape($entry.description)) {
        throw "ATC binding changed for $($entry.code)."
    }
}
Write-Json (Join-Path $fixtureDir 'atc-catalog-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_sha256 = $expected.atc
    entry_count = $atcEntries.Count
    entries = $atcEntries
})

$rules = [Collections.Generic.List[object]]::new()
function Rule(
    [string]$Id, [string]$Phase, $Order, [string]$Condition, [string[]]$Keys, $Message,
    [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Official = 'The branch alerts and stops the active operation.',
    [string]$Recommended = 'Retain as a structured revision-aware error.'
) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id = $Id; form_id = $formId; revision = $revision; phase = $Phase; order = $Order
        condition = $Condition; fields = $Keys
        accepted_behavior = 'Condition is false; processing continues.'
        rejected_behavior = 'The active operation stops unless official_behavior states otherwise.'
        exact_message = $Message; source_refs = $Refs; evidence_type = @('source')
        assessment = $Assessment; official_behavior = $Official; recommended_app_behavior = $Recommended
        confidence = 'high'; unresolved_questions = @()
    })
}
Rule '2553-validate-001-year-type' validate 1 'Neither Calendar nor Fiscal is selected.' @('frm2553:itemFiscalStartMonth:_1','frm2553:itemFiscalStartMonth:_2') 'Please select an option for item 1.' @('official-hta-runtime#validateForm:L2791-L2799')
Rule '2553-validate-002-month' validate 2 'Item 2 month has selectedIndex 0.' @('frm2553:itemYearEndMonth') 'Please select valid month on item 2.' @('official-hta-runtime#validateForm:L2801-L2805')
Rule '2553-validate-003-year-required' validate 3 'Item 2 year is blank.' @('frm2553:txtYearEnded') 'Please enter valid year on item 2.' @('official-hta-runtime#validateForm:L2807-L2811')
Rule '2553-validate-004-year-minimum' validate 4 'Item 2 year is less than 1900.' @('frm2553:txtYearEnded') 'Invalid date entry on Item no.2. Entry should not be lower than 1900.' @('official-hta-runtime#validateForm:L2817-L2821')
Rule '2553-validate-005-quarter' validate 5 'No Item 3 quarter is selected.' @('frm2553:optQtr:_1','frm2553:optQtr:_2','frm2553:optQtr:_3','frm2553:optQtr:_4') 'Select a Quarter on Item no. 3.' @('official-hta-runtime#validateForm:L2822-L2826')
Rule '2553-validate-006-tin' validate 6 'Any TIN segment or branch code is blank.' @('frm2553:txtTIN1','frm2553:txtTIN2','frm2553:txtTIN3','frm2553:txtBranchCode') 'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#validateForm:L2828-L2832')
Rule '2553-validate-007-rdo' validate 7 'RDO selectedIndex is 0.' @($runtimeRdo) 'Please enter a valid RDO Code on Item 7.' @('official-hta-runtime#validateForm:L2833-L2838')
Rule '2553-validate-008-business' validate 8 'Line of business or occupation is blank.' @('frm2553:txtDescription') 'Please enter a valid Line of Business/Occupation on Item 8.' @('official-hta-runtime#validateForm:L2839-L2843')
Rule '2553-validate-009-name' validate 9 'Taxpayer name is blank.' @('frm2553:txtTPName') 'Please enter a valid Taxpayer Name on Item 9.' @('official-hta-runtime#validateForm:L2844-L2848')
Rule '2553-validate-010-telephone' validate 10 'Telephone number is blank.' @('frm2553:txtTelNum') 'Please enter a valid Telephone Number on Item 10.' @('official-hta-runtime#validateForm:L2849-L2853')
Rule '2553-validate-011-address' validate 11 'Registered address is blank.' @('frm2553:txtAddress') "Please enter Taxpayer's Registered Address on Item 11." @('official-hta-runtime#validateForm:L2854-L2858')
Rule '2553-validate-012-zip' validate 12 'ZIP code is blank.' @('frm2553:txtZipCode') "Please enter Taxpayer's Zip Code on Item 12." @('official-hta-runtime#validateForm:L2859-L2863')
Rule '2553-validate-013-overpayment' validate 13 'Item 23 coerces to a negative JavaScript number and neither disposition is selected.' @('frm2553:txt23','frm2553:ifoverpay_1','frm2553:ifoverpay_2') 'Please indicate refund type for overpayment in Item 23.' @('official-hta-runtime#validateForm:L2864-L2870')
Rule '2553-validate-014-atc-amount' validate 14 'For any Item 14-18 row, ATC is nonblank and taxable amount loosely equals zero.' @('frm2553:txt14B','frm2553:txt14C','frm2553:txt18B','frm2553:txt18C') 'Please enter a valid amount for Item {row}C.' @('official-hta-runtime#validateForm:L2872-L2881')
Rule '2553-validate-015-treaty' validate 15 'Item 13 Yes is selected and the treaty selector value is 0.' @('frm2553:optTreaty_1','frm2553:lstTaxTreaty') 'Please select a Tax Treaty from the list.' @('official-hta-runtime#validateForm:L2883-L2886')
Rule '2553-input-016-atc-duplicate' input 1 'Selected ATC code and description duplicate another Item 14-18 row.' @('frm2553:txt14A','frm2553:txt14B','frm2553:txt18A','frm2553:txt18B') 'Invalid input. Selected ATC already defined.' @('official-hta-runtime#getATCCode:L2682-L2697')
Rule '2553-input-017-fiscal-december' 'blur/change' 1 'Fiscal Year is selected while month ended is December.' @('frm2553:itemFiscalStartMonth:_2','frm2553:itemYearEndMonth') 'You have entered invalid month for Fiscal Year' @('official-hta-runtime#dateyear:L3003-L3015')
Rule '2553-input-018-calendar-nondecember' 'blur/change' 2 'Calendar Year is selected while month ended is not December.' @('frm2553:itemFiscalStartMonth:_1','frm2553:itemFiscalStartMonth:_2','frm2553:itemYearEndMonth') 'You have entered a filing year not ending in December. This filing will be considered as a Fiscal Year Filing.' @('official-hta-runtime#datemonth:L3017-L3023')
Rule '2553-input-019-fiscal-december-coerce' 'blur/change' 3 'Fiscal Year is selected while month ended is December.' @('frm2553:itemFiscalStartMonth:_1','frm2553:itemFiscalStartMonth:_2','frm2553:itemYearEndMonth') 'You have entered a filing year ending in December. This filing will be considered as a Calendar Year Filing.' @('official-hta-runtime#datemonth:L3024-L3028')
Rule '2553-save-020-tin' save 1 'Any TIN segment or branch code is blank.' @('frm2553:txtTIN1','frm2553:txtTIN2','frm2553:txtTIN3','frm2553:txtBranchCode') 'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L3031-L3036')
Rule '2553-save-021-rdo' save 2 'RDO value is 000.' @($runtimeRdo) 'Please enter a valid RDO Code on Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L3037-L3040')
Rule '2553-save-022-name' save 3 'Taxpayer name is blank.' @('frm2553:txtTPName') 'Please enter a valid Taxpayer Name on Item 9.' @('official-hta-runtime#initialValidateBeforeSave:L3041-L3045')
Rule '2553-defect-023-future-year' validate 16 'Item 2 year is later than the current year.' @('frm2553:txtYearEnded') 'Invalid date entry on Item no.2. Entry should not be later than Current Date.' @('official-hta-runtime#validateForm:L2812-L2816') 'incorrect-official-behavior' 'The entire future-year branch is commented out, so future years pass.' 'Reject transaction periods after the allowed filing period using a configurable legal-date rule.'
Rule '2553-defect-024-save-sparse' save 4 'Any Validate-only required field is missing.' @('frm2553:txtDescription','frm2553:txtTelNum','frm2553:txtAddress','frm2553:txtZipCode') $null @('official-hta-runtime#initialValidateBeforeSave:L3031-L3047','official-hta-runtime#validateForm:L2791-L2899') 'incorrect-official-behavior' 'Save checks only TIN, RDO, and taxpayer name.' 'Use a shared validation graph with explicit draft exceptions.'
Rule '2553-defect-025-rate-unbounded' input 2 'An OT012 editable tax-rate field contains a negative, nonnumeric, or at-least-100 value.' @('frm2553:txt14D','frm2553:txt15D','frm2553:txt16D','frm2553:txt17D','frm2553:txt18D') $null @('official-hta-runtime#getATCCode:L2668-L2676','official-hta-runtime#computeTaxDue:L2718-L2723') 'incorrect-official-behavior' 'The source makes OT012 rate editable but applies no keypress filter, numeric validation, or range validation.' 'Require a finite nonnegative percentage within the rate authorized by the applicable special law.'
Rule '2553-defect-026-no-tax-row' validate 17 'All Item 14-18 ATC fields are blank.' @('frm2553:txt14B','frm2553:txt15B','frm2553:txt16B','frm2553:txt17B','frm2553:txt18B') $null @('official-hta-runtime#validateForm:L2872-L2881') 'incorrect-official-behavior' 'Validate accepts a return with no taxable transaction row.' 'Require at least one complete applicable tax row unless a legally supported zero-return state is modeled.'
Rule '2553-defect-027-quarter-default' input 3 'Current month is March, June, or September during init.' @('frm2553:optQtr:_1','frm2553:optQtr:_2','frm2553:optQtr:_3','frm2553:optQtr:_4') $null @('official-hta-runtime#init:L2521-L2535') 'official-bug-compatible' 'Three comparisons use month.getMonth without parentheses; the markup-default fourth quarter remains selected in March, June, and September.' 'Derive quarter from an explicit transaction period, not the workstation date.'
Rule '2553-defect-028-overpayment-comma' validate 18 'Item 23 is a formatted negative amount containing a comma.' @('frm2553:txt23','frm2553:ifoverpay_1','frm2553:ifoverpay_2') $null @('official-hta-runtime#validateForm:L2864-L2870','official-hta-runtime#checkOverpayment:L2758-L2771') 'incorrect-official-behavior' 'checkOverpayment strips commas, but Validate uses value*1; a value such as -1,000.00 becomes NaN and bypasses the disposition requirement.' 'Parse formatted money once with the same typed decimal parser in calculation and validation.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    first_error_behavior = 'Validate and Save stop at the first source-ordered failure; Save uses a much narrower graph.'
    rules = $rules
})

$calcs = [Collections.Generic.List[object]]::new()
function Calc(
    [string]$Id, [string[]]$Outputs, [string[]]$Inputs, [string]$Formula, [string]$Trigger,
    [string[]]$Dependencies, [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Recommended = 'Implement with typed decimals and the official two-decimal display order.'
) {
    $calcs.Add([pscustomobject][ordered]@{
        calculation_id = $Id; outputs = $Outputs; inputs = $Inputs; condition = $null
        official_formula = $Formula; rounding = 'formatCurrency after source arithmetic; editable inputs round to two decimals on blur where wired.'
        trigger = $Trigger; depends_on = $Dependencies; source_refs = $Refs; assessment = $Assessment
        recommended_app_behavior = $Recommended; confidence = 'high'
    })
}
Calc '2553-row-tax' @('frm2553:txt14E','frm2553:txt15E','frm2553:txt16E','frm2553:txt17E','frm2553:txt18E') @('frm2553:txt14C','frm2553:txt14D','frm2553:txt18C','frm2553:txt18D') 'For each Item 14-18 row: tax due = taxable amount / 100 * tax rate.' computeTaxDue @() @('official-hta-runtime#computeTaxDue:L2718-L2723')
Calc '2553-item19-total-tax' @('frm2553:txt19') @('frm2553:txt14E','frm2553:txt15E','frm2553:txt16E','frm2553:txt17E','frm2553:txt18E') 'Item 19 = sum of Item 14E through Item 18E.' computeTotalTaxDue @('2553-row-tax') @('official-hta-runtime#computeTotalTaxDue:L2725-L2736')
Calc '2553-item20c-total-credits' @('frm2553:txt20C') @('frm2553:txt20A','frm2553:txt20B') 'Item 20C = Item 20A + Item 20B.' computeTotalTaxCreditPayments @() @('official-hta-runtime#computeTotalTaxCreditPayments:L2773-L2778')
Calc '2553-item21-tax-payable' @('frm2553:txt21') @('frm2553:txt19','frm2553:txt20C') 'Item 21 = Item 19 - Item 20C.' computeTaxPayable @('2553-item19-total-tax','2553-item20c-total-credits') @('official-hta-runtime#computeTaxPayable:L2738-L2742')
Calc '2553-item22d-total-penalties' @('frm2553:txt22D') @('frm2553:txt22A','frm2553:txt22B','frm2553:txt22C') 'Item 22D = Item 22A + Item 22B + Item 22C.' computePenalties @() @('official-hta-runtime#computePenalties:L2744-L2748')
Calc '2553-item23-total-payable' @('frm2553:txt23') @('frm2553:txt21','frm2553:txt22D') 'Item 23 = Item 21 + Item 22D.' computeTotalAmountPayable @('2553-item21-tax-payable','2553-item22d-total-penalties') @('official-hta-runtime#computeTotalAmountPayable:L2750-L2756')
Calc '2553-overpayment-ui-state' @('frm2553:ifoverpay_1','frm2553:ifoverpay_2') @('frm2553:txt23') 'If parsed Item 23 is negative, enable and clear both disposition radios; otherwise disable and clear both.' checkOverpayment @('2553-item23-total-payable') @('official-hta-runtime#checkOverpayment:L2758-L2771') 'official-bug-compatible' 'Represent overpayment disposition as a conditional enum without clearing an existing valid choice on every recomputation.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema' = '../../schema/calculations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    evaluation_order = @($calcs.calculation_id)
    calculations = $calcs
})

$cases = @()
$caseNumber = 0
foreach ($rule in @($rules | Where-Object { $_.exact_message })) {
    $caseNumber++
    $cases += [pscustomobject][ordered]@{
        case_id = ('case-{0:d2}-{1}' -f $caseNumber, $rule.rule_id)
        phase = $rule.phase
        mutations = @{ synthetic_condition = $rule.condition }
        expected_message = $rule.exact_message
        expected_behavior = $rule.official_behavior
        rule_id = $rule.rule_id
    }
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; synthetic_only = $true; cases = $cases
})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    cases = @(
        @{ case_id = 'five-percent-row'; calculation_id = '2553-row-tax'; taxable_amount = 100000; rate = 5; official_output = 5000 }
        @{ case_id = 'manual-ot012-rate'; calculation_id = '2553-row-tax'; taxable_amount = 100000; rate = 2.5; official_output = 2500 }
        @{ case_id = 'overpayment-with-penalty'; calculation_id = '2553-item23-total-payable'; tax_payable = -1000; penalties = 250; official_output = -750 }
    )
})

$resources = @()
foreach ($src in @([regex]::Matches($hta, '(?i)<script[^>]+src\s*=\s*(["''])(?<value>.*?)\1') | ForEach-Object { $_.Groups['value'].Value } | Sort-Object -Unique)) {
    $full = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if (Test-Path -LiteralPath $full) {
        $resources += [pscustomobject][ordered]@{
            src = $src; path = $full; present = $true; size = (Get-Item -LiteralPath $full).Length
            sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    } else {
        $resources += [pscustomobject][ordered]@{ src = $src; path = $full; present = $false; size = $null; sha256 = $null }
    }
}
$resources += [pscustomobject][ordered]@{
    src = 'xml/atcCodes.xml'; path = $atcPath; present = $true; size = (Get-Item -LiteralPath $atcPath).Length; sha256 = $expected.atc
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; resources = $resources
})

$deadline = 'On or before the due date for payment of the tax stated in the applicable special law.'
$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    phases = @(
        @{ phase = 'edit'; official_behavior = 'July 1999 return for percentage tax payable under special laws.'; source_refs = @('official-hta-runtime','official-form-pdf','packaged-help'); confidence = 'high' }
        @{ phase = 'saved-draft'; official_behavior = 'Save checks only TIN, RDO, and taxpayer name, then serializes 68 controls.'; source_refs = @('official-hta-runtime#initialValidateBeforeSave:L3031-L3047','revision-matched-final-copy-keys'); confidence = 'high' }
        @{ phase = 'validated'; official_behavior = 'Validate checks period, identity, conditional overpayment, populated-row amounts, and treaty selection.'; source_refs = @('official-hta-runtime#validateForm:L2791-L2899'); confidence = 'high' }
        @{ phase = 'final-copy'; official_behavior = 'The supplied encrypted final copy proves the complete 68-key serialization without exposing values.'; source_refs = @('revision-matched-final-copy-keys','official-hta-runtime#saveEncryptedProfile'); confidence = 'high' }
        @{ phase = 'submitted'; official_behavior = 'Online transport code exists but was not exercised.'; source_refs = @('official-hta-runtime#sendEmail','official-hta-runtime#uploadXMLFile'); confidence = 'medium' }
    )
    transitions = @(
        @{ from = 'edit'; action = 'Save'; to = 'saved-draft'; guard = 'Sparse Save checks pass.'; side_effects = @('Writes flat pseudo-XML.','Preserves all 68 controls.'); source_refs = @('official-hta-runtime#saveXML') }
        @{ from = 'edit'; action = 'Validate'; to = 'validated'; guard = 'All source-ordered Validate checks pass.'; side_effects = @('Disables editable controls.','Enables print, edit, upload, and final copy.'); source_refs = @('official-hta-runtime#validateForm','official-hta-runtime#enableDisabledFields') }
        @{ from = 'validated'; action = 'Edit'; to = 'edit'; guard = $null; side_effects = @('Re-enables controls conditionally.'); source_refs = @('official-hta-runtime#editForm') }
        @{ from = 'validated'; action = 'Final Copy'; to = 'final-copy'; guard = 'Finalization succeeds.'; side_effects = @('Encrypts and compresses the copy.'); source_refs = @('official-hta-runtime#saveEncryptedProfile') }
        @{ from = 'final-copy'; action = 'Transport'; to = 'submitted'; guard = 'Connectivity and acceptance succeed.'; side_effects = @('Untested online attempt.'); source_refs = @('official-hta-runtime#sendEmail') }
    )
    prerequisites = @('Transaction period','TIN and RDO','Taxpayer identity and address','Complete populated tax rows','Treaty basis when applicable','Overpayment disposition when applicable')
    required_attachments = @(
        @{ attachment_id = 'bir-2307'; label = 'Certificate of Creditable Tax Withheld at Source (BIR Form 2307), if applicable.'; required_when = 'Creditable withholding applies.'; official_ui_enforcement = 'Not enforced.'; source_refs = @('packaged-help#L218-L227'); confidence = 'high' }
        @{ attachment_id = 'tax-debit-memo'; label = 'Duly approved Tax Debit Memo, if applicable.'; required_when = 'A tax debit memo is used.'; official_ui_enforcement = 'Not enforced.'; source_refs = @('packaged-help#L218-L227'); confidence = 'high' }
        @{ attachment_id = 'amended-return-support'; label = 'Proof of payment and the return previously filed.'; required_when = 'The return is amended.'; official_ui_enforcement = 'Not enforced.'; source_refs = @('packaged-help#L218-L227'); confidence = 'high' }
    )
    filing_deadlines = @(
        @{ quarter = 'Q1'; due_date_rule = $deadline; source_refs = @('packaged-help#L146-L158'); confidence = 'high' }
        @{ quarter = 'Q2'; due_date_rule = $deadline; source_refs = @('packaged-help#L146-L158'); confidence = 'high' }
        @{ quarter = 'Q3'; due_date_rule = $deadline; source_refs = @('packaged-help#L146-L158'); confidence = 'high' }
        @{ quarter = 'Q4'; due_date_rule = $deadline; source_refs = @('packaged-help#L146-L158'); confidence = 'high' }
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$assets = @(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'July 1999 ENCS runtime.'
    Asset 'packaged-help' 'official-runtime-help' $helpPath 'Packaged Form 2553 instructions.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'Local official July 1999 form PDF.'
    Asset 'official-atc-catalog' 'official-runtime-data' $atcPath 'Package ATC catalog filtered by runtime formType 2553.'
    Asset 'revision-matched-final-copy' 'dummy-profile-encrypted-final-copy' $samples[0].FullName 'July 1999 final copy; all 68 source-derived keys match.' $redactedSample
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    form_code = '2553'
    revision = $revision
    package_version = $packageVersion
    status = 'complete'
    official_assets = $assets
    counts = [ordered]@{
        concrete_fields = 68; runtime_field_families = 0; fields_total = $fields.Count; typed_fields = $fields.Count
        validation_rules = $rules.Count; confirmed_official_bugs = $bugCount; calculations = $calcs.Count
        negative_fixtures = $cases.Count; unverified_gaps = 2
    }
    artifacts = [ordered]@{
        fields = 'fields.json'; validations = 'validations.json'; calculations = 'calculations.json'
        workflow = 'workflow.json'; evidence = 'evidence.md'; audit = 'audit.md'; gaps = 'gaps.md'
        encrypted_keys = 'fixtures/encrypted-field-keys-v796.json'
        runtime_controls = 'fixtures/runtime-control-inventory-v796.json'
        validation_functions = 'fixtures/validation-function-inventory-v796.json'
        calculation_functions = 'fixtures/calculation-function-inventory-v796.json'
        atc_catalog = 'fixtures/atc-catalog-v796.json'
        resources = 'fixtures/official-resource-hashes-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'
        calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer, release evidence, migration status, or capability metadata changed.'
        'No decrypted values or email-bearing filenames emitted.'
        'The revision-matched final copy proves 67 static serialized controls plus one runtime RDO selector.'
        'No unbounded serialized field families exist in this form.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 2553 - July 1999`n`nRevision-specific package with 68 concrete fields and no dynamic serialized families.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- July 1999 runtime: $($expected.hta).
- Packaged help: $($expected.help).
- Official form PDF: $($expected.pdf).
- ATC catalog: $($expected.atc).
- Sample: ciphertext $($expected.cipher), decrypted $($expected.plain), 68 keys, inventory $($expected.inventory); values never emitted.
- The sample keys equal the source-derived inventory: 67 static controls plus runtime RDO selector `frm2553:txtRDOCode`.
- The official sample filename is represented only as `#email-redacted#`.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. Validate/Save behavior is source-proven and final-copy serialization is sample-proven, but no live UI black-box matrix was needed or executed for the remaining source-explicit branches.
2. Online submission was not exercised.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- July 1999 revision binding: pass.
- Revision-matched final-copy inventory: 68/68 keys match the source-derived inventory.
- Typed inventory: 68 concrete fields, zero families.
- Validations: $($rules.Count); calculations: $($calcs.Count); negative fixtures: $($cases.Count); official defects: $bugCount.
- Full structural/schema audit must run after generation.
- No renderer/release/capability/commit/push changes.
"@

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry = $index.forms | Where-Object { $_.form_id -eq $formId }
if ($entry) {
    $entry.form_code = '2553'; $entry.revision = $revision; $entry.package_version = $packageVersion
    $entry.priority = 30; $entry.status = 'complete'; $entry.path = 'forms/2553-v1999/manifest.json'
} else {
    $index.forms += [pscustomobject][ordered]@{
        form_id = $formId; form_code = '2553'; revision = $revision; package_version = $packageVersion
        priority = 30; status = 'complete'; path = 'forms/2553-v1999/manifest.json'
    }
}
$index.forms = @($index.forms | Sort-Object priority)
$index.updated = '2026-07-23'
Write-Json $indexPath $index

[pscustomobject]@{
    form_id = $formId
    concrete_fields = $fields.Count
    families = 0
    typed_fields = $fields.Count
    validations = $rules.Count
    calculations = $calcs.Count
    negative_fixtures = $cases.Count
    confirmed_official_bugs = $bugCount
    next_form = '2200A'
} | ConvertTo-Json
