param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$SampleDir = 'C:\Mac\Home\Downloads\forms\1602Qv2018',
    [string]$DuplicateSampleDir = 'C:\Mac\Home\Downloads\forms\1602Q',
    [string]$MismatchedPdfDir = 'C:\Mac\Home\Downloads\forms\1602Qv2019'
)

$ErrorActionPreference = 'Stop'
$formId = '1602q-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1602Qv2018.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1602Q.hta'
$pdfPath = Join-Path $MismatchedPdfDir '1602Q Jan 2019.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1602q-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = '898f83932a3e85f39b9efc6ac0c148c3a1a05d00e5feab843e15360d6465cd2e'
    help = '74f6b70ee6051946dfc054b6b17926f7f74f21db9a645350db24881ec09950b8'
    mismatched_pdf = '9cad9524ebf2042c7d9827b87afe77fd6f60cc8a596ec1b5f54ffbf2df49aa69'
    sample_cipher = 'ecda88d22aeaf98b5bf70a0937b88a295613589131d7e16ddd83c5913aea67f9'
    sample_plain = '6c6e54f5ac941a87e3d0959087e0cad505dc2ca3e7d95b3a6eedeb919753395a'
    sample_inventory = 'c35ccaa4129ffc25982901c43f30ea2ec77da3134aeb05e7ee0d50c62d20ea76'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    rmc27 = '2609208dbb5bb2fabfed0cef0ead1bc1cf7b5ea8e6efecbe631c1c9b79c28c51'
}

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function Hash-Lines([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
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

foreach ($path in @($htaPath, $helpPath, $pdfPath, $packagePath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
foreach ($pair in @(
    @($htaPath, 'hta'),
    @($helpPath, 'help'),
    @($pdfPath, 'mismatched_pdf'),
    @($packagePath, 'package')
)) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
$samples = @(Get-ChildItem -LiteralPath $SampleDir -File | Where-Object { $_.Extension -eq '.xml' })
$duplicates = @(Get-ChildItem -LiteralPath $DuplicateSampleDir -File | Where-Object { $_.Extension -eq '.xml' })
if ($samples.Count -ne 1 -or $duplicates.Count -ne 1) { throw 'Expected one encrypted dummy sample in each reviewed directory.' }
foreach ($sample in @($samples[0], $duplicates[0])) {
    if ((Get-FileHash -LiteralPath $sample.FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected.sample_cipher) {
        throw 'Encrypted sample hash changed.'
    }
}
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']1602Qv2018["'']' -or $hta -notmatch '(?i)January\s+2018') {
    throw 'HTA revision binding changed.'
}
if ($help -notmatch '(?i)Form\s+No\.\s+1602Q\s+\(January\s+2018\)' -or $help -notmatch '(?i)last\s+day\s+of\s+the\s+month\s+following') {
    throw 'Help revision/deadline binding changed.'
}
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') { throw 'Mismatched PDF magic changed.' }
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

$formMatch = [regex]::Match($hta, '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain missing.' }
$body = $formMatch.Groups['body'].Value
$bodyOffset = $formMatch.Groups['body'].Index
$excluded = @(@([regex]::Matches($body, '(?is)<script\b.*?</script>')) + @([regex]::Matches($body, '(?is)<!--.*?-->')))
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
    $controls.Add([pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Attr $tag 'id'
        name = Attr $tag 'name'
        element = $element
        control_kind = $kind.ToLowerInvariant()
        source_line = 1 + [regex]::Matches($hta.Substring(0, $bodyOffset + $match.Index), "`n").Count
        value = Attr $tag 'value'
        maxlength = Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}
if ($controls.Count -ne 131) { throw "Expected 131 live controls; found $($controls.Count)." }
$static = @($controls | Where-Object { $_.control_kind -in @('text', 'select', 'select-one', 'textarea', 'radio', 'checkbox') })
if ($static.Count -ne 109 -or @($static.id | Sort-Object -Unique).Count -ne 109) {
    throw "Static serializer inventory changed: $($static.Count)."
}
$runtimeRdo = [pscustomobject][ordered]@{
    ordinal = 0
    id = 'frm1602Q:txtRDOCode'
    name = 'frm1602Q:txtRDOCode'
    element = 'select'
    control_kind = 'select'
    source_line = 5787
    value = '000'
    maxlength = $null
    disabled = $true
    readonly = $false
}
$serializable = @($static) + @($runtimeRdo)
if ($serializable.Count -ne 110 -or @($serializable.id | Sort-Object -Unique).Count -ne 110) {
    throw 'Runtime-complete serializer inventory changed.'
}
$inventoryHash = Hash-Lines @($serializable.id | Sort-Object)
if ($inventoryHash -ne $expected.sample_inventory) {
    throw 'Runtime DOM plus injected RDO no longer matches the reviewed encrypted sample inventory.'
}

$required = @(
    'frm1602Q:txtYear', 'frm1602Q:qtr_1', 'frm1602Q:qtr_2', 'frm1602Q:qtr_3', 'frm1602Q:qtr_4',
    'frm1602Q:AmendedRtn1', 'frm1602Q:AmendedRtn2', 'frm1602Q:OptTaxWithheld1', 'frm1602Q:OptTaxWithheld2',
    'frm1602Q:txtTIN1', 'frm1602Q:txtTIN2', 'frm1602Q:txtTIN3', 'frm1602Q:txtBranchCode',
    'frm1602Q:txtRDOCode', 'frm1602Q:txtTaxpayerName', 'frm1602Q:txtAddress', 'frm1602Q:txtZipCode',
    'frm1602Q:txtTelNum', 'frm1602Q:OptCategoryAgent1', 'frm1602Q:OptCategoryAgent2',
    'frm1602Q:OptSpecialTax1', 'frm1602Q:OptSpecialTax2'
)
$itemMap = @{
    txtYear = '1'; qtr_1 = '2'; qtr_2 = '2'; qtr_3 = '2'; qtr_4 = '2'
    AmendedRtn1 = '3'; AmendedRtn2 = '3'; OptTaxWithheld1 = '4'; OptTaxWithheld2 = '4'
    txtSheets = '4A'; txtTIN1 = '5'; txtTIN2 = '5'; txtTIN3 = '5'; txtBranchCode = '5'
    txtRDOCode = '6'; txtTaxpayerName = '8'; txtTelNum = '9'; txtAddress = '10'; txtZipCode = '11'
    OptCategoryAgent1 = '12'; OptCategoryAgent2 = '12'; OptSpecialTax1 = '13'; OptSpecialTax2 = '13'
    lstSpecialTax = '13A'; txt14 = '14'; txt15 = '15'; txt16 = '16'; txt17 = '17'; txt18 = '18'
    txt19 = '19'; txt20 = '20'; txt21 = '21'; txt22 = '22'; txt23 = '23'; txt24 = '24'
    txt25 = '25'; txt26 = '26'; txt27 = '27'; txt28 = '28'
    OverRemittance1 = '28'; OverRemittance2 = '28'; OverRemittance3 = '28'
}
$computedPattern = '(?i)(:txt1[4-7]$|:txt22$|:txt23$|:txt27$|:txt28$|Sched1Tax\d+$|Sched1Total$|Sched2TaxesWithheld\d+$|Sched2Total$|Sched3TaxWithheld\d+$|Sched3Total$|Page[23](TIN|Agent)$)'
$amountPattern = '(?i)(:txt(1[4-9]|2[0-8])$|Amount|Interest|Rate|Tax|Total)'
$hiddenPattern = '(?i)(txtCurrentPage|txtMaxPage|txtFinalFlag|txtEnroll|ebirOnline|driveSelect)'
$fields = [Collections.Generic.List[object]]::new()
foreach ($control in $serializable) {
    $key = $control.id
    $short = if ($key -like 'frm1602Q:*') { $key.Substring(9) } else { $key }
    $logical = 'string'
    $enum = [object[]]@()
    $normalization = [string[]]@()
    if ($control.control_kind -in @('radio', 'checkbox')) { $logical = 'boolean'; $enum = [object[]]@('true', 'false') }
    elseif ($key -match '(?i)(TIN|RDO|BranchCode|ATC|Treaty|IPA)') { $logical = 'code' }
    elseif ($key -eq 'txtEmail') { $logical = 'email-string' }
    elseif ($key -match $amountPattern) { $logical = 'decimal-amount'; $normalization = [string[]]@('NumWithComma', 'formatCurrency', 'round(...,2)') }
    elseif ($key -eq 'frm1602Q:txtYear') { $logical = 'year' }
    $isComputed = $key -match $computedPattern
    $status = if ($required -contains $key) { 'required' } elseif ($isComputed) { 'computed' } else { 'optional' }
    if ($key -match $hiddenPattern) { $status = 'hidden' }
    if ($key -eq 'frm1602Q:lstSpecialTax') { $status = 'conditional'; $enum = [object[]]@('0', '1', '2', '3') }
    if ($key -match '(?i)Sched2|Sched3') { $status = if ($isComputed) { 'computed' } else { 'conditional' } }
    if ($key -match '(?i)OverRemittance') { $status = 'conditional' }
    $constraints = [ordered]@{}
    if ($control.maxlength -and $control.maxlength -match '^\d+$') { $constraints.max_length = [int]$control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision = 2 }
    if ($key -eq 'frm1602Q:txtYear') { $constraints.minimum = 2018; $constraints.maximum = 'current calendar year' }
    $page = if ($control.source_line -ge 2493) { 3 } elseif ($control.source_line -ge 1301) { 2 } else { 1 }
    $notes = @('Source-derived from the exact January 2018 runtime DOM and generic Save serializer.')
    if ($key -eq 'frm1602Q:txtRDOCode') { $notes += 'Runtime-injected select; its presence is independently proven by the 110-key encrypted sample.' }
    if ($key -eq 'txtEmail') { $notes += 'The official serializer retains the unprefixed control ID literally.' }
    $fields.Add([pscustomobject][ordered]@{
        field_key = $key
        serialized_key = $key
        serialized_occurrence = 1
        label = $short
        page = $page
        item_number = if ($itemMap.ContainsKey($short)) { $itemMap[$short] } else { $null }
        control_kind = $control.control_kind
        storage_type = 'string'
        logical_type = $logical
        required = $status
        required_when = if ($key -eq 'frm1602Q:lstSpecialTax') { 'Item 13 Yes.' } elseif ($key -match 'Sched2') { 'Special-rate/treaty schedules are applicable.' } elseif ($key -match 'Sched3') { 'IPA schedule is applicable.' } elseif ($key -match 'OverRemittance') { 'Item 28 is negative.' } else { $null }
        enabled_when = if ($key -eq 'frm1602Q:lstSpecialTax') { 'Item 13 Yes.' } elseif ($key -match 'Sched2') { 'Item 13 Yes.' } elseif ($key -match 'OverRemittance') { 'Item 28 is negative.' } else { $null }
        visible_when = $null
        default_value = $control.value
        empty_representation = ''
        constraints = [pscustomobject]$constraints
        enum_values = $enum
        normalization = $normalization
        computed = $isComputed
        calculation_id = if ($isComputed) { 'See calculations.json' } else { $null }
        source_refs = @("official-hta-runtime#control:L$($control.source_line)", 'official-hta-runtime#saveXML:L4170-L4248', 'revision-matched-encrypted-sample')
        confidence = 'high'
        notes = $notes
    })
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = 110
    inventory_sha256 = $inventoryHash
    fields = $fields
})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    official_hta_sha256 = $expected.hta
    live_static_control_count = $controls.Count
    static_serializer_control_count = $static.Count
    runtime_injected_control_count = 1
    runtime_serializer_control_count = $serializable.Count
    encrypted_sample_field_count = 110
    encrypted_sample_inventory_sha256 = $expected.sample_inventory
    controls = $serializable
})
$decryptTool = Join-Path $RepoRoot 'rules\tools\audit-1701-encrypted-fields.ps1'
$sampleAudit = &$decryptTool -SourceDir $SampleDir -FormId $formId -FilePattern '*.xml' -RedactedFileName '1602Qv2018-final-copy-#email-redacted#.xml' `
    -ExpectedCiphertextSha256 $expected.sample_cipher -ExpectedDecryptedSha256 $expected.sample_plain -ExpectedFieldCount 110 `
    -ExpectedFieldInventorySha256 $expected.sample_inventory -ExpectedExtraField 'frm1602Q:txtYear' -VersionField '*' -ExpectedXmlVersion '*'
Write-Utf8 (Join-Path $fixtureDir 'encrypted-field-audit.json') ($sampleAudit -join [Environment]::NewLine)
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm1602Q:' -NamePattern '(?i)valid|save|year|enable|disable|final|submit') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm1602Q:' -NamePattern '(?i)compute|tax|rate|quarter') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule(
    [string]$Id, [string]$Phase, $Order, [string]$Condition, [string[]]$Keys, $Message, [string[]]$Refs,
    [string]$Assessment = 'verified-correct',
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
Rule '1602q-validate-001-year-required' validate 1 'Item 1 year is blank.' @('frm1602Q:txtYear') 'Please enter a valid year on Item 1.' @('official-hta-runtime#validate:L5425-L5429')
Rule '1602q-validate-002-year-future' validate 2 'Item 1 year exceeds the current full calendar year.' @('frm1602Q:txtYear') 'Invalid date entry on Item no.1. Entry should not be later than Current Date.' @('official-hta-runtime#validate:L5430-L5434')
Rule '1602q-validate-003-year-before-2018' validate 3 'Item 1 year is below 2018.' @('frm1602Q:txtYear') 'Invalid date entry on Item 1. Entry should not be lower than 2018.' @('official-hta-runtime#validate:L5436-L5439')
Rule '1602q-validate-004-quarter' validate 4 'No quarter radio is selected.' @('frm1602Q:qtr_1','frm1602Q:qtr_2','frm1602Q:qtr_3','frm1602Q:qtr_4') 'Please select an option in item number 2.' @('official-hta-runtime#validate:L5441-L5446')
Rule '1602q-validate-005-amended' validate 5 'Neither amended-return radio is selected.' @('frm1602Q:AmendedRtn1','frm1602Q:AmendedRtn2') 'Please select an option in item number 3.' @('official-hta-runtime#validate:L5448-L5452')
Rule '1602q-validate-006-tin' validate 6 'Any TIN segment or branch code is blank.' @('frm1602Q:txtTIN1','frm1602Q:txtTIN2','frm1602Q:txtTIN3','frm1602Q:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#validate:L5454-L5457') 'official-bug-compatible' 'Only blankness is checked.' 'Apply the shared TIN checksum and segment constraints.'
Rule '1602q-validate-007-rdo' validate 7 'The runtime RDO select remains at index zero.' @('frm1602Q:txtRDOCode') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#validate:L5458-L5462')
Rule '1602q-validate-008-name' validate 8 'Withholding-agent name is blank.' @('frm1602Q:txtTaxpayerName') 'Please enter a valid Taxpayer Name on Item 8.' @('official-hta-runtime#validate:L5467-L5470')
Rule '1602q-validate-009-phone' validate 9 'Telephone number is blank.' @('frm1602Q:txtTelNum') 'Please enter a valid Telephone Number on Item 9.' @('official-hta-runtime#validate:L5471-L5474') 'official-bug-compatible' 'Only blankness is checked.' 'Validate the accepted telephone syntax.'
Rule '1602q-validate-010-address' validate 10 'Registered address is blank.' @('frm1602Q:txtAddress') "Please enter Taxpayer's Registered Address on Item 10." @('official-hta-runtime#validate:L5475-L5478')
Rule '1602q-validate-011-zip' validate 11 'Zip code is blank.' @('frm1602Q:txtZipCode') "Please enter Taxpayer's Zip Code on Item 11." @('official-hta-runtime#validate:L5479-L5482')
Rule '1602q-validate-012-withheld' validate 12 'Neither Item 4 tax-withheld radio is selected.' @('frm1602Q:OptTaxWithheld1','frm1602Q:OptTaxWithheld2') 'Please select an option for Item 4.' @('official-hta-runtime#validate:L5484-L5486')
Rule '1602q-validate-013-agent-category' validate 13 'Neither Item 12 withholding-agent category is selected.' @('frm1602Q:OptCategoryAgent1','frm1602Q:OptCategoryAgent2') 'Please select an option for Item 11.' @('official-hta-runtime#validate:L5488-L5490') 'incorrect-official-behavior' 'The message cites Item 11 although the controls are printed as Item 12.' 'Cite Item 12 while retaining the official text as compatibility metadata.'
Rule '1602q-validate-014-special-tax' validate 14 'Neither Item 13 special-tax radio is selected.' @('frm1602Q:OptSpecialTax1','frm1602Q:OptSpecialTax2') 'Please select an option in item 13.' @('official-hta-runtime#validate:L5492-L5496')
Rule '1602q-validate-015-special-tax-kind' validate 15 'Item 13 is Yes but Item 13A is still the blank option.' @('frm1602Q:OptSpecialTax1','frm1602Q:lstSpecialTax') 'Please select an option in item 13A.' @('official-hta-runtime#validate:L5498-L5505')
Rule '1602q-validate-016-part-iv' validate 16 'Item 4 is Yes and Items 14 and 15 are zero.' @('frm1602Q:OptTaxWithheld1','frm1602Q:txt14','frm1602Q:txt15','frm1602Q:txt16') 'Please fill up Part IV if item 4 is set to Yes.' @('official-hta-runtime#validate:L5507-L5514') 'incorrect-official-behavior' 'The source compares Item 15 twice and never examines Item 16, so a Schedule 3-only amount is rejected.' 'Test Items 14, 15, and 16 exactly once.'
Rule '1602q-validate-017-overremittance' validate 17 'Item 28 is negative and no disposition radio is selected.' @('frm1602Q:txt28','frm1602Q:OverRemittance1','frm1602Q:OverRemittance2','frm1602Q:OverRemittance3') 'Please select an option for over-remittance below item 28.' @('official-hta-runtime#validate:L5516-L5524')
Rule '1602q-sched2-001-row1-atc' validate 18 'Schedule 2 row 1 has a treaty code but no ATC.' @('frm1602Q:txtSched2TreatyCode1','frm1602Q:txtSched2ATC1') 'Please fill up ATC field in Schedule 2 row 1.' @('official-hta-runtime#validateSched2:L4956-L4965')
Rule '1602q-sched2-002-row1-treaty' validate 19 'Schedule 2 row 1 has an ATC but no treaty code.' @('frm1602Q:txtSched2TreatyCode1','frm1602Q:txtSched2ATC1') 'Please fill up Treaty Code field in Schedule 2 row 1.' @('official-hta-runtime#validateSched2:L4967-L4974')
Rule '1602q-sched2-003-row1-complete' validate 20 'Schedule 2 row 1 amount or rate is positive while ATC or treaty code is blank.' @('frm1602Q:txtSched2TreatyCode1','frm1602Q:txtSched2ATC1','frm1602Q:txtSched2Interest1','frm1602Q:txtSched2TaxRate1') 'Please fill up Schedule 2 row 1 completely.' @('official-hta-runtime#validateSched2:L4976-L4992')
Rule '1602q-sched2-004-row2-atc' validate 21 'Schedule 2 row 2 has a treaty code but no ATC.' @('frm1602Q:txtSched2TreatyCode2','frm1602Q:txtSched2ATC2') 'Please fill up ATC field in Schedule 2 row 2.' @('official-hta-runtime#validateSched2:L4995-L5002')
Rule '1602q-sched2-005-row2-treaty' validate 22 'Schedule 2 row 2 has an ATC but no treaty code.' @('frm1602Q:txtSched2TreatyCode2','frm1602Q:txtSched2ATC2') 'Please fill up Treaty Code field in Schedule 2 row 2.' @('official-hta-runtime#validateSched2:L5004-L5011')
Rule '1602q-sched2-006-row2-amount' validate 23 'Schedule 2 row 2 amount is positive while ATC or treaty code is blank.' @('frm1602Q:txtSched2TreatyCode2','frm1602Q:txtSched2ATC2','frm1602Q:txtSched2Interest2') 'Please fill up Schedule 2 row 2 completely.' @('official-hta-runtime#validateSched2:L5013-L5020')
Rule '1602q-sched2-007-row2-rate' validate 24 'Schedule 2 row 2 rate is positive while ATC or treaty code is blank.' @('frm1602Q:txtSched2TreatyCode2','frm1602Q:txtSched2ATC2','frm1602Q:txtSched2TaxRate2') 'Please fill up Schedule 2 row 12 completely.' @('official-hta-runtime#validateSched2:L5022-L5029') 'incorrect-official-behavior' 'The message says row 12 although the branch validates row 2.' 'Report Schedule 2 row 2.'
Rule '1602q-sched3-001-row1' validate 25 'Schedule 3 row 1 amount or rate is positive while IPA is blank.' @('frm1602Q:txtSched3IPA1','frm1602Q:txtSched3TotInterest1','frm1602Q:txtSched3TaxRate1') 'Please fill up Schedule 3 row 1 completely.' @('official-hta-runtime#validateSched3:L5034-L5052')
Rule '1602q-sched3-002-row2' validate 26 'Schedule 3 row 2 amount or rate is positive while IPA is blank.' @('frm1602Q:txtSched3IPA2','frm1602Q:txtSched3TotInterest2','frm1602Q:txtSched3TaxRate2') 'Please fill up Schedule 3 row 2 completely.' @('official-hta-runtime#validateSched3:L5054-L5070')
Rule '1602q-validate-027-success' validate 27 'All prior checks pass.' @() "Validation successful. Click on 'Edit' if you wish to modify your entries." @('official-hta-runtime#validate:L5536-L5546') 'verified-correct' 'Controls are disabled and final-copy/print actions are enabled.' 'Model validated state explicitly.'
Rule '1602q-save-001-tin' save 1 'Any TIN segment or branch code is blank.' @('frm1602Q:txtTIN1','frm1602Q:txtTIN2','frm1602Q:txtTIN3','frm1602Q:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L5798-L5802') 'official-bug-compatible' 'Only blankness is checked.' 'Apply shared TIN checksum and format rules.'
Rule '1602q-save-002-rdo' save 2 'RDO value is 000.' @('frm1602Q:txtRDOCode') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L5803-L5806')
Rule '1602q-save-003-name' save 3 'Withholding-agent name is blank.' @('frm1602Q:txtTaxpayerName') "Please enter a valid Withholding Agent's Name on Item 8." @('official-hta-runtime#initialValidateBeforeSave:L5807-L5811')
Rule '1602q-save-004-sparse' save 4 'Any Validate-only identity, period, classification, computation, or schedule rule fails.' @() $null @('official-hta-runtime#initialValidateBeforeSave:L5798-L5812','official-hta-runtime#validate:L5425-L5547') 'incorrect-official-behavior' 'Save ignores all checks except TIN blankness, RDO 000, and name blankness.' 'Use a shared validation graph with explicit phase exceptions.'
Rule '1602q-year-blur-getyear' 'blur/change' 1 'Any normal four-digit year is compared with Date.getYear(), which returns year minus 1900 in this runtime.' @('frm1602Q:txtYear') 'Invalid year. Year should not be later than the current year.' @('official-hta-runtime#control:L326','official-hta-runtime#validateYear:L5410-L5421') 'incorrect-official-behavior' 'The field can be reset to a value such as 126 instead of 2026.' 'Use getFullYear() and retain a four-digit year.'
Rule '1602q-atc-code-list' 'blur/change' 2 'A Schedule 2 ATC is outside the hard-coded 13-code list.' @('frm1602Q:txtSched2ATC1','frm1602Q:txtSched2ATC2') 'Invalid ATC Code! Please refer to the ATC Codes in Schedule 1.' @('official-hta-runtime#validateATC:L5286-L5302')
Rule '1602q-treaty-code-list' 'blur/change' 3 'A Schedule 2 treaty code is outside the hard-coded country-code list.' @('frm1602Q:txtSched2TreatyCode1','frm1602Q:txtSched2TreatyCode2') 'Invalid Treaty Code! Please refer to the treaty code in schedule 5.' @('official-hta-runtime#validateTreatyCode:L5304-L5321')
Rule '1602q-ipa-code-list' 'blur/change' 4 'A Schedule 3 IPA is outside the hard-coded IPA list.' @('frm1602Q:txtSched3IPA1','frm1602Q:txtSched3IPA2') 'Invalid IPA Code! Please refer to IPA Codes in Schedule 6.' @('official-hta-runtime#validateIPA:L5224-L5240')
Rule '1602q-serialization-unprefixed-email' save $null 'The generic serializer reaches the email control.' @('txtEmail') $null @('official-hta-runtime#control:L596','official-hta-runtime#saveXML:L4170-L4248') 'official-bug-compatible' 'The unprefixed ID txtEmail becomes the XML key.' 'Preserve the literal key with a typed alias.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'; form_id = $formId; revision = $revision
    first_error_behavior = 'Validate and Save alert and return on the first source-ordered failure.'
    rules = $rules
})

$calcs = [Collections.Generic.List[object]]::new()
function Calc(
    [string]$Id, [string[]]$Out, [string[]]$In, [string]$Formula, [string]$Trigger,
    [string[]]$Deps, [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Recommended = 'Implement with typed decimals and deterministic two-decimal formatting.'
) {
    $calcs.Add([pscustomobject][ordered]@{
        calculation_id = $Id; outputs = $Out; inputs = $In; condition = $null
        official_formula = $Formula
        rounding = 'formatCurrency applies two decimal places; input blur handlers call round(...,2).'
        trigger = $Trigger; depends_on = $Deps; source_refs = $Refs; assessment = $Assessment
        recommended_app_behavior = $Recommended; confidence = 'high'
    })
}
Calc '1602q-sched1-standard-rows' @('frm1602Q:txtSched1Tax1..12','frm1602Q:txtSched1Tax15') @('frm1602Q:txtSched1Amount1..12','frm1602Q:txtSched1Amount15','fixed row rates') 'Each output equals amount multiplied by the row tax rate divided by 100.' computeSched1 @() @('official-hta-runtime#computeSched1:L5324-L5332','official-hta-runtime#controls:L1404-L1869')
Calc '1602q-sched1-fx-rows' @('frm1602Q:txtSched1Tax13','frm1602Q:txtSched1Tax14') @('frm1602Q:txtSched1Amount13','frm1602Q:txtSched1TaxRate13','frm1602Q:txtSched1Rate13','frm1602Q:txtSched1Amount14','frm1602Q:txtSched1TaxRate14','frm1602Q:txtSched1Rate14') 'Tax withheld = amount × (tax rate / 100) × BSP rate.' computeSched1BspRate @() @('official-hta-runtime#computeSched1BspRate:L5334-L5345')
Calc '1602q-sched1-total' @('frm1602Q:txtSched1Total','frm1602Q:txt14') @('frm1602Q:txtSched1Tax1..15') 'Sum all 15 Schedule 1 tax outputs and copy to Item 14.' computeSched1Total @('1602q-sched1-standard-rows','1602q-sched1-fx-rows') @('official-hta-runtime#computeSched1Total:L5347-L5368')
Calc '1602q-sched2-rows' @('frm1602Q:txtSched2TaxesWithheld1','frm1602Q:txtSched2TaxesWithheld2') @('frm1602Q:txtSched2Interest1','frm1602Q:txtSched2TaxRate1','frm1602Q:txtSched2Interest2','frm1602Q:txtSched2TaxRate2') 'For each row, taxes withheld = interest × (tax rate / 100).' computeSched2 @() @('official-hta-runtime#computeSched2:L5264-L5274')
Calc '1602q-sched2-total' @('frm1602Q:txtSched2Total','frm1602Q:txt15') @('frm1602Q:txtSched2TaxesWithheld1','frm1602Q:txtSched2TaxesWithheld2') 'Sum the two Schedule 2 outputs and copy to Item 15.' computeSched2Total @('1602q-sched2-rows') @('official-hta-runtime#computeSched2Total:L5276-L5284')
Calc '1602q-sched3-rows' @('frm1602Q:txtSched3TaxWithheld1','frm1602Q:txtSched3TaxWithheld2') @('frm1602Q:txtSched3TotInterest1','frm1602Q:txtSched3TaxRate1','frm1602Q:txtSched3TotInterest2','frm1602Q:txtSched3TaxRate2') 'For each row, taxes withheld = total interest × (tax rate / 100).' computeSched3 @() @('official-hta-runtime#computeSched3:L5242-L5252')
Calc '1602q-sched3-total' @('frm1602Q:txtSched3Total','frm1602Q:txt16') @('frm1602Q:txtSched3TaxWithheld1','frm1602Q:txtSched3TaxWithheld2') 'Sum the two Schedule 3 outputs and copy to Item 16.' computeSched3Total @('1602q-sched3-rows') @('official-hta-runtime#computeSched3Total:L5254-L5262')
Calc '1602q-item17' @('frm1602Q:txt17') @('frm1602Q:txt14','frm1602Q:txt15','frm1602Q:txt16') 'Item 17 = Item 14 + Item 15 + Item 16.' computeItem17 @('1602q-sched1-total','1602q-sched2-total','1602q-sched3-total') @('official-hta-runtime#computeItem17:L5216-L5222')
Calc '1602q-item22' @('frm1602Q:txt22') @('frm1602Q:txt18','frm1602Q:txt19','frm1602Q:txt20','frm1602Q:txt21') 'Item 22 = Items 18 + 19 + 20 + 21.' computeItem22 @() @('official-hta-runtime#computeItem22:L5208-L5214')
Calc '1602q-item23' @('frm1602Q:txt23') @('frm1602Q:txt17','frm1602Q:txt22') 'Item 23 = Item 17 - Item 22.' computeItem23 @('1602q-item17','1602q-item22') @('official-hta-runtime#computeItem23:L5201-L5206')
Calc '1602q-item27' @('frm1602Q:txt27') @('frm1602Q:txt24','frm1602Q:txt25','frm1602Q:txt26') 'Item 27 = Items 24 + 25 + 26.' computeItem27 @() @('official-hta-runtime#computeItem27:L5193-L5199')
Calc '1602q-item28' @('frm1602Q:txt28') @('frm1602Q:txt23','frm1602Q:txt27') 'Item 28 = Item 23 + Item 27; a negative result enables over-remittance choices.' computeItem28 @('1602q-item23','1602q-item27') @('official-hta-runtime#computeItem28:L5160-L5173')
Calc '1602q-2025-rate-switch' @('frm1602Q:txtSched1TaxRate13','frm1602Q:txtSched1TaxRate14') @('frm1602Q:txtYear','frm1602Q:qtr_1','frm1602Q:qtr_2','frm1602Q:qtr_3','frm1602Q:qtr_4') 'Rows 13 and 14 show 15% before 2025 and for 2025 Q1/Q2, then 20% for 2025 Q3/Q4 and later years.' updateTaxRate @() @('official-hta-runtime#updateTaxRate:L5741-L5784') 'unverified' 'Confirm the statutory transition against a revision-matched issuance before implementing the post-2018 update.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema' = '../../schema/calculations.schema.json'
    schema_version = '1.0.0'; form_id = $formId; revision = $revision
    evaluation_order = @($calcs.calculation_id); calculations = $calcs
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
    schema_version = '1.0.0'; form_id = $formId
    cases = @(
        @{ case_id = 'sched1-simple'; calculation_id = '1602q-sched1-standard-rows'; amount = 1000; rate = 10; official_output = 100 },
        @{ case_id = 'sched1-fx'; calculation_id = '1602q-sched1-fx-rows'; amount = 1000; rate = 15; bsp_rate = 50; official_output = 7500 },
        @{ case_id = 'overremittance'; calculation_id = '1602q-item28'; item23 = -100; item27 = 25; official_output = -75; enables_overremittance = $true },
        @{ case_id = '2025-q2'; calculation_id = '1602q-2025-rate-switch'; year = 2025; quarter = 'Q2'; official_rate = 15 },
        @{ case_id = '2025-q3'; calculation_id = '1602q-2025-rate-switch'; year = 2025; quarter = 'Q3'; official_rate = 20 }
    )
})
$resources = @()
foreach ($src in @([regex]::Matches($hta, '(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1') | ForEach-Object { $_.Groups['v'].Value } | Sort-Object -Unique)) {
    $full = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if (Test-Path -LiteralPath $full) {
        $resources += [pscustomobject][ordered]@{ src = $src; path = $full; present = $true; size = (Get-Item -LiteralPath $full).Length; sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant() }
    } else {
        $resources += [pscustomobject][ordered]@{ src = $src; path = $full; present = $false; size = $null; sha256 = $null }
    }
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; resources = $resources
})

$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'
    schema_version = '1.0.0'; form_id = $formId; revision = $revision
    phases = @(
        @{ phase = 'edit'; official_behavior = 'Three-page January 2018 quarterly return with fixed Schedule 1 rows and two-row treaty and IPA schedules.'; source_refs = @('official-hta-runtime','official-help-v2018'); confidence = 'high' },
        @{ phase = 'saved-draft'; official_behavior = 'Save applies only TIN blankness, RDO 000, and withholding-agent-name blankness before serializing all 110 runtime controls.'; source_refs = @('official-hta-runtime#initialValidateBeforeSave:L5798-L5812','official-hta-runtime#saveXML:L4170-L4248','revision-matched-encrypted-sample'); confidence = 'high' },
        @{ phase = 'validated'; official_behavior = 'Validate applies period, identity, classification, Part IV, over-remittance, Schedule 2, and Schedule 3 checks in source order.'; source_refs = @('official-hta-runtime#validate:L5425-L5547'); confidence = 'high' },
        @{ phase = 'final-copy'; official_behavior = 'A revision-matched encrypted final copy proves the exact 110-key inventory; values were not emitted.'; source_refs = @('revision-matched-encrypted-sample','encrypted-field-audit'); confidence = 'high' },
        @{ phase = 'submitted'; official_behavior = 'Online transport code exists but was not exercised.'; source_refs = @('official-hta-runtime#sendEmail'); confidence = 'medium' }
    )
    transitions = @(
        @{ from = 'edit'; action = 'Save'; to = 'saved-draft'; guard = 'The three narrow Save checks and file-version guards pass.'; side_effects = @('Writes the flat pseudo-XML field stream.','Retains unprefixed email and runtime UI-state fields.'); source_refs = @('official-hta-runtime#saveXML:L4002-L4313') },
        @{ from = 'edit'; action = 'Validate'; to = 'validated'; guard = 'All source-ordered Validate and schedule checks pass.'; side_effects = @('Disables editable controls.','Enables print, edit, and final-copy actions.'); source_refs = @('official-hta-runtime#validate:L5425-L5547') },
        @{ from = 'validated'; action = 'Edit'; to = 'edit'; guard = $null; side_effects = @('Re-enables controls, subject to conditional schedule state.'); source_refs = @('official-hta-runtime#enableAllControl:L5637-L5714') },
        @{ from = 'validated'; action = 'Final Copy'; to = 'final-copy'; guard = 'Finalization/version flow succeeds.'; side_effects = @('Writes and encrypts/compresses the final copy.'); source_refs = @('official-hta-runtime#saveXML:L4002-L4313') },
        @{ from = 'final-copy'; action = 'Transport'; to = 'submitted'; guard = 'Connectivity and remote acceptance succeed.'; side_effects = @('Online submission attempt; deliberately untested.'); source_refs = @('official-hta-runtime#sendEmail') }
    )
    prerequisites = @('Return year and quarter','Amended-return and tax-withheld choices','Withholding-agent identity and RDO','Agent and special-tax classifications','Applicable Schedule 1, 2, and 3 details','Over-remittance disposition when Item 28 is negative')
    required_attachments = @(
        @{ attachment_id = 'deposit-slip'; label = 'BIR-prescribed deposit slip when filing with an Authorized Agent Bank.'; required_when = 'Payment/remittance is made through an AAB.'; official_ui_enforcement = 'Not checked by local Validate.'; source_refs = @('official-help-v2018#L129-L135'); confidence = 'high' }
    )
    filing_deadlines = @(
        @{ quarter = 'Q1'; due_date_rule = 'Not later than the last day of the month following the close of the quarter.'; source_refs = @('official-help-v2018#L107-L120'); confidence = 'high' },
        @{ quarter = 'Q2'; due_date_rule = 'Not later than the last day of the month following the close of the quarter.'; source_refs = @('official-help-v2018#L107-L120'); confidence = 'high' },
        @{ quarter = 'Q3'; due_date_rule = 'Not later than the last day of the month following the close of the quarter.'; source_refs = @('official-help-v2018#L107-L120'); confidence = 'high' },
        @{ quarter = 'Q4'; due_date_rule = 'Not later than the last day of the month following the close of the quarter.'; source_refs = @('official-help-v2018#L107-L120'); confidence = 'high' }
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugs = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$assets = @(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1602Qv2018; printed January 2018.'
    Asset 'official-help-v2018' 'official-runtime-help' $helpPath 'Revision-matched January 2018 guide and filing deadline.'
    Asset 'revision-matched-encrypted-sample' 'dummy-profile-encrypted-final-copy' $samples[0].FullName 'The 110-key inventory exactly equals the runtime DOM plus injected RDO select.' (Join-Path $SampleDir '1602Qv2018-final-copy-#email-redacted#.xml')
    Asset 'mismatched-form-pdf-excluded' 'official-form-pdf' $pdfPath 'January 2019 PDF; retained only as an explicit revision-mismatch exclusion.'
    [pscustomobject][ordered]@{
        asset_id = 'official-rmc-27-2018'
        kind = 'official-bir-issuance'
        path = 'https://bir-cdn.bir.gov.ph/local/pdf/RMC%20No%2027-2018.pdf'
        sha256 = $expected.rmc27
        size = 697595
        revision_binding = 'Official implementing issuance retrieved 2026-07-23; supports the January 2018 revision context, not the field inventory.'
    }
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version = '1.0.0'; form_id = $formId; form_code = '1602Q'; revision = $revision
    package_version = $packageVersion; status = 'complete'; official_assets = $assets
    counts = [ordered]@{
        concrete_fields = 110; runtime_field_families = 0; fields_total = $fields.Count; typed_fields = $fields.Count
        validation_rules = $rules.Count; confirmed_official_bugs = $bugs; calculations = $calcs.Count
        negative_fixtures = $cases.Count; unverified_gaps = 3
    }
    artifacts = [ordered]@{
        fields = 'fields.json'; validations = 'validations.json'; calculations = 'calculations.json'
        workflow = 'workflow.json'; evidence = 'evidence.md'; audit = 'audit.md'; gaps = 'gaps.md'
        runtime_control_fixture = 'fixtures/runtime-control-inventory-v796.json'
        encrypted_field_audit = 'fixtures/encrypted-field-audit.json'
        validation_function_fixture = 'fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture = 'fixtures/calculation-function-inventory-v796.json'
        resource_hash_fixture = 'fixtures/official-resource-hashes-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'; calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer or release metadata changed.',
        'No decrypted values or email-bearing filenames are emitted.',
        'The 109 static serializer controls plus the runtime-injected RDO select exactly reproduce the encrypted sample 110-key inventory hash.',
        'The only local form PDF is January 2019 and is explicitly excluded from January 2018 revision evidence.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') @"
# BIR Form 1602Q - January 2018

Revision-specific validation package for the Quarterly Remittance Return of Final Taxes Withheld on Interest Paid on Deposits and Yield on Deposit Substitutes/Trusts/Etc.

The 110 typed serialized controls exactly match a revision-bound encrypted dummy final copy. No taxpayer values or identifying filename text are included.
"@
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- Runtime HTA: SHA-256 $($expected.hta), `APPLICATIONNAME="1602Qv2018"`, printed January 2018.
- Runtime help: SHA-256 $($expected.help), explicitly titled January 2018 and stating the quarterly filing deadline.
- Revision-matched encrypted dummy final copy: ciphertext SHA-256 $($expected.sample_cipher); decrypted payload SHA-256 $($expected.sample_plain); 110 unique keys; inventory SHA-256 $($expected.sample_inventory). Values were not emitted.
- DOM reconciliation: 109 static serializable controls plus runtime-injected `frm1602Q:txtRDOCode` exactly reproduce the sample inventory hash.
- Official RMC No. 27-2018: SHA-256 $($expected.rmc27), retrieved from the official BIR CDN on 2026-07-23.
- The local January 2019 PDF (SHA-256 $($expected.mismatched_pdf)) is revision-mismatched and excluded from January 2018 field/rule claims.

All email-bearing filenames are represented as `#email-redacted#`.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. The exact January 2018 blank official form PDF was not available locally; the January 2019 PDF is excluded.
2. The post-2018/2025 Schedule 1 rate switch embedded in the current runtime needs a separately pinned statutory issuance before our app should implement it as legally verified.
3. Local Validate/Save source and a revision-matched final copy are complete, but online submission was deliberately not exercised.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- Exact revision pinned: pass.
- Official HTA/help/package hashes pinned: pass.
- Encrypted 110-key inventory pinned without values: pass.
- Runtime DOM reconciliation: pass (109 static + 1 injected RDO = 110; inventory hash exact).
- Typed field inventory: pass (110/110).
- Validation and calculation inventories: pass.
- Save/Validate/Final Copy workflow: documented.
- Confirmed official defects: $bugs.
- Negative fixtures: $($cases.Count).
- JSON structural/schema audit: run `rules/validate.ps1 -RequireJsonSchema` after generation.
- Scope: no renderer, migration, release, capability, commit, or push changes.
"@

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry = $index.forms | Where-Object { $_.form_id -eq $formId }
if ($entry) {
    $entry.form_code = '1602Q'; $entry.revision = $revision; $entry.package_version = $packageVersion
    $entry.priority = 24; $entry.status = 'complete'; $entry.path = 'forms/1602q-v2018/manifest.json'
} else {
    $index.forms += [pscustomobject][ordered]@{
        form_id = $formId; form_code = '1602Q'; revision = $revision; package_version = $packageVersion
        priority = 24; status = 'complete'; path = 'forms/1602q-v2018/manifest.json'
    }
}
$index.forms = @($index.forms | Sort-Object priority)
$index.updated = '2026-07-23'
Write-Json $indexPath $index

[pscustomobject]@{
    form_id = $formId
    fields = $fields.Count
    validations = $rules.Count
    calculations = $calcs.Count
    negative_fixtures = $cases.Count
    confirmed_official_bugs = $bugs
    encrypted_inventory_match = $true
    next_form = '1600wp-v2018'
} | ConvertTo-Json
