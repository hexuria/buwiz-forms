param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\1707v2021',
    [string]$SampleDir = 'C:\Mac\Home\Downloads\forms\1707'
)

$ErrorActionPreference = 'Stop'
$formId = '1707-v2021'
$revision = '2021-04-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1707v2021.hta'
$predecessorPath = Join-Path $ExtractedRoot 'forms\BIR-Form1707v2018.hta'
$legacyPath = Join-Path $ExtractedRoot 'forms\BIR-Form1707.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1707v2021.hta'
$pdfPath = Join-Path $OfficialDir '1707 April 2021 ENCS.pdf'
$guidePath = Join-Path $OfficialDir '1707 April 2021 ENCS Guidelines and Instructions.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1707-v2021'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = 'a645463cf3bc7a80f0ec96a08796adde33c76ae65c617b3fefdb478e61496cd4'
    predecessor = '2d995beea0061dfdef2ee74558f2dbf6cedd30c91fc7ac3e18668515dafab158'
    legacy = '66f34ac0ff6ab7f07794bdffa10d4f3074b7b235f712a1f6f1718d950a5c06c7'
    help = 'fe3f17dc77d7b4139d2d037cae37e24813b44446420b8b7de10bc5bf0583e4e0'
    pdf = 'b6bc016f240a8d6233db6fb0065b72b31e75cb665affc2b96bcd2066e7ad257e'
    guide = 'dbb8142e6e67fde5e46dc8b137dab6123153b35c1dae9c95b1603a163b5778f3'
    cipher = '05b1fda3bd618e577439f9c8fe5ee6e52a2fe57d9256a827ce75da9c2b1e4fab'
    plain = '5ee55a46ae6b49be5a7ecbb9a15c24e1186fab0186c1d99d5fd4d0f79b41fe3c'
    inventory = 'ef203d92aaa63420529174f804cb32a08826829f13cb38ddc7e77713f4439fd6'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
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
    }
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

$assetsToCheck = @(
    @($htaPath, 'hta'), @($predecessorPath, 'predecessor'), @($legacyPath, 'legacy'),
    @($helpPath, 'help'), @($pdfPath, 'pdf'), @($guidePath, 'guide'), @($packagePath, 'package')
)
foreach ($pair in $assetsToCheck) {
    if (-not (Test-Path -LiteralPath $pair[0] -PathType Leaf)) { throw "Missing source: $($pair[0])" }
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
$samples = @(Get-ChildItem -LiteralPath $SampleDir -File -Filter '*.xml')
if ($samples.Count -ne 1) { throw "Expected one encrypted sample; found $($samples.Count)." }
if ((Get-FileHash -LiteralPath $samples[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected.cipher) {
    throw 'Encrypted sample hash changed.'
}
foreach ($pdf in @($pdfPath, $guidePath)) {
    $bytes = [IO.File]::ReadAllBytes($pdf)
    if ([Text.Encoding]::ASCII.GetString($bytes[0..4]) -ne '%PDF-') { throw "PDF magic mismatch: $pdf" }
}
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']1707v2021["'']' -or $hta -notmatch '(?i)April\s+2021\s+\(ENCS\)') {
    throw 'April 2021 runtime binding changed.'
}
if ($help -notmatch '(?i)within\s+thirty\s+\(30\)\s+days' -or $help -notmatch '(?i)15%') {
    throw 'Revision-matched help content changed.'
}
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

$keyTool = Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson = & $keyTool -SourcePath $samples[0].FullName `
    -RedactedSourcePath (Join-Path $SampleDir '1707-final-copy-#email-redacted#.xml') `
    -FormId '1707-v1999' -ExpectedCiphertextSha256 $expected.cipher `
    -ExpectedDecryptedSha256 $expected.plain -ExpectedFieldCount 70 `
    -ExpectedFieldInventorySha256 $expected.inventory
$keyAudit = $keyJson | ConvertFrom-Json
$legacyKeys = @($keyAudit.keys)
Write-Utf8 (Join-Path $fixtureDir 'excluded-legacy-encrypted-field-keys-v796.json') ($keyJson -join [Environment]::NewLine)
$legacyIds = @([regex]::Matches([IO.File]::ReadAllText($legacyPath), '(?i)\bid\s*=\s*(["''])(?<id>.*?)\1') | ForEach-Object { $_.Groups['id'].Value } | Where-Object { $_ } | Sort-Object -Unique)
$currentIds = @([regex]::Matches($hta, '(?i)\bid\s*=\s*(["''])(?<id>.*?)\1') | ForEach-Object { $_.Groups['id'].Value } | Where-Object { $_ } | Sort-Object -Unique)
$legacyOverlap = @($legacyKeys | Where-Object { $legacyIds -contains $_ })
$currentOverlap = @($legacyKeys | Where-Object { $currentIds -contains $_ })
if ($legacyOverlap.Count -ne 65 -or $currentOverlap.Count -ne 6) {
    throw "Encrypted-sample revision discrimination changed: legacy=$($legacyOverlap.Count), current=$($currentOverlap.Count)."
}

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
$serial = @($controls | Where-Object { $_.control_kind -in @('text', 'select', 'select-one', 'textarea', 'radio', 'checkbox') })
$staticIds = @($serial.id | Where-Object { $_ } | Sort-Object -Unique)
if ($controls.Count -ne 187 -or $staticIds.Count -ne 142) {
    throw "Expected 187 live controls/142 source-serializable IDs; found $($controls.Count)/$($staticIds.Count)."
}
$byId = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $byId.ContainsKey($control.id)) { $byId[$control.id] = $control }
}
$families = @(
    [pscustomobject]@{ field_pattern = 'frm1707:txtPg2Pt4S2_{N>=1}Col1'; source_line = 2969; logical = 'string' },
    [pscustomobject]@{ field_pattern = 'frm1707:txtPg2Pt4S2_{N>=1}Col2'; source_line = 2972; logical = 'decimal-amount' },
    [pscustomobject]@{ field_pattern = 'frm1707:txtPg2Pt4S2_{N>=1}Col3'; source_line = 2973; logical = 'string' },
    [pscustomobject]@{ field_pattern = 'frm1707:txtPg2Pt4S2_{N>=1}Col4'; source_line = 2974; logical = 'decimal-amount' },
    [pscustomobject]@{ field_pattern = 'frm1707:txtPg2Pt4S3_{N>=1}Col1'; source_line = 3151; logical = 'string' },
    [pscustomobject]@{ field_pattern = 'frm1707:txtPg2Pt4S3_{N>=1}Col2'; source_line = 3152; logical = 'decimal-amount' }
)
foreach ($family in $families) {
    $literal = $family.field_pattern.Replace('{N>=1}', "' + (i + 1) + '")
    if ($hta -notlike "*$literal*") { throw "Dynamic family source changed: $($family.field_pattern)" }
}

$required = @(
    'frm1707:txtPg1I1Month', 'frm1707:txtPg1I1Day', 'frm1707:txtPg1I1Year',
    'frm1707:rdoPg1Pt1I5RDO', 'frm1707:ProfileName', 'frm1707:ProfileAddress1',
    'frm1707:ProfileZipCode', 'frm1707:ProfileContactNum', 'frm1707:ProfileEmailAddr'
)
function Field-Meta([string]$Key, $Control, [bool]$Family, [string]$FamilyLogical = '') {
    $page = $null
    if ($Key -match '(?i)Pg(?<page>\d+)') { $page = [int]$Matches.page }
    $item = $null
    $itemMatches = @([regex]::Matches($Key, '(?i)(?:Itm?|I)(?<item>\d+[a-z]?)'))
    if ($itemMatches.Count) { $item = $itemMatches[-1].Groups['item'].Value }
    $logical = if ($FamilyLogical) { $FamilyLogical } else { 'string' }
    $normalization = [string[]]@()
    $enum = [object[]]@()
    if (($Control -and $Control.control_kind -in @('radio', 'checkbox')) -or $Key -match '(?i):(rdo|chk)') {
        $logical = 'boolean'
        $enum = [object[]]@('true', 'false')
    }
    elseif ($Key -match '(?i)Email') { $logical = 'email-string' }
    elseif ($Key -match '(?i)(Date|Collection)') { $logical = 'date-string-mm-dd-yyyy'; $normalization = [string[]]@('MM/DD/YYYY') }
    elseif ($Key -match '(?i)(TIN|RDO|Zip|ATC)') { $logical = 'code' }
    elseif ($Key -match '(?i)Year$') { $logical = 'integer-year' }
    elseif ($Key -match '(?i)(Amount|Amt|Price|Expense|TaxBase|TaxDue|TaxPaid|TaxPayable|Penalt|Surcharge|Interest|Compromise|Collection|Cost|Mortgage|Installment|NetCptal|AllowExpnse|Tot)') {
        $logical = 'decimal-amount'
        $normalization = [string[]]@('NumWithComma', 'formatCurrency', 'toFixed(2)')
    }
    $computed = $false
    if ($Control -and ($Control.disabled -or $Control.readonly) -and $logical -eq 'decimal-amount') { $computed = $true }
    if ($Key -match '(?i)(PeriodTaxDue|TotAmount|TaxBase|AllowExpnse|NetCptalChnged|TaxDueForTrans|TaxDuePmtPriod|TaxPayable|TotPenalties|TotAmtPyable)$') { $computed = $true }
    $status = if ($required -contains $Key) { 'required' } elseif ($computed) { 'computed' } else { 'optional' }
    if ($Family) { $status = 'conditional'; $computed = $false }
    $constraints = [ordered]@{}
    if ($Control -and $Control.maxlength -match '^\d+$') { $constraints.max_length = [int]$Control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision = 2; $constraints.sign = 'signed unless source validation constrains the field' }
    [pscustomobject]@{ page = $page; item = $item; logical = $logical; enum = $enum; normalization = $normalization; computed = $computed; status = $status; constraints = [pscustomobject]$constraints }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $staticIds) {
    $control = $byId[$key]
    $meta = Field-Meta $key $control $false
    $fields.Add([pscustomobject][ordered]@{
        field_key = $key; serialized_key = $key; serialized_occurrence = 1
        label = if ($key -like 'frm1707:*') { $key.Substring(8) } else { $key }
        page = $meta.page; item_number = $meta.item; control_kind = $control.control_kind
        storage_type = 'string'; logical_type = $meta.logical; required = $meta.status
        required_when = $null; enabled_when = $null; visible_when = $null
        default_value = $control.value; empty_representation = ''; constraints = $meta.constraints
        enum_values = $meta.enum; normalization = $meta.normalization; computed = $meta.computed
        calculation_id = if ($meta.computed) { 'See calculations.json' } else { $null }
        source_refs = @('official-hta-runtime#saveXML:L6003-L6318', "official-hta-runtime#control:L$($control.source_line)")
        confidence = 'high'
        notes = @('Source-derived from the hash-pinned April 2021 runtime; no revision-matched encrypted final copy was available.')
    })
}
foreach ($family in $families) {
    $meta = Field-Meta $family.field_pattern $null $true $family.logical
    $fields.Add([pscustomobject][ordered]@{
        field_key = $family.field_pattern; serialized_key = $null; serialized_occurrence = $null
        label = "Runtime-indexed family $($family.field_pattern)"; page = $meta.page; item_number = $meta.item
        control_kind = 'runtime-indexed-family'; storage_type = 'string'; logical_type = $meta.logical
        required = 'conditional'; required_when = 'The corresponding popup row N exists.'
        enabled_when = 'The row exists.'; visible_when = 'The popup row exists.'
        default_value = $null; empty_representation = ''; constraints = [pscustomobject]@{ index = 'one-based, source-unbounded' }
        enum_values = @(); normalization = $meta.normalization; computed = $false; calculation_id = $null
        source_refs = @("official-hta-runtime#dynamic-id:L$($family.source_line)", 'official-hta-runtime#popup-serialization')
        confidence = 'high'; notes = @('Source-derived unbounded family; the available encrypted sample is legacy and excluded.')
    })
}
if ($fields.Count -ne 148) { throw "Expected 148 fields; found $($fields.Count)." }
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'; schema_version = '1.0.0'; form_id = $formId; revision = $revision
    field_count = $fields.Count; runtime_serializable_element_count = 142
    inventory_sha256 = Hash-Lines @($fields.field_key | Sort-Object); fields = $fields
})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; official_hta_sha256 = $expected.hta
    live_control_count = $controls.Count; static_serialized_id_count = $staticIds.Count
    revision_matched_final_copy_key_count = 0; excluded_legacy_sample_key_count = $legacyKeys.Count
    excluded_legacy_sample_overlap_with_legacy_runtime = $legacyOverlap.Count
    excluded_legacy_sample_overlap_with_current_runtime = $currentOverlap.Count
    active_runtime_family_count = $families.Count; controls = $controls; dynamic_families = $families
})
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1707:' -NamePattern '(?i)valid|check|mandatory|save|enable|disable|date|email|submit|final') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1707:' -NamePattern '(?i)compute|amount|sum|format|tax|penalty|interest') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule(
    [string]$Id, [string]$Phase, $Order, [string]$Condition, [string[]]$Keys, $Message,
    [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Official = 'The branch alerts and stops the active operation.',
    [string]$Recommended = 'Retain as a structured revision-aware error.'
) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id = $Id; form_id = $formId; revision = $revision; phase = $Phase; order = $Order
        condition = $Condition; fields = $Keys; accepted_behavior = 'Condition is false; processing continues.'
        rejected_behavior = 'The active operation stops unless official_behavior states otherwise.'
        exact_message = $Message; source_refs = $Refs; evidence_type = @('source'); assessment = $Assessment
        official_behavior = $Official; recommended_app_behavior = $Recommended; confidence = 'high'; unresolved_questions = @()
    })
}
Rule '1707-validate-001-month' validate 1 'Month equals 00.' @('frm1707:txtPg1I1Month') 'Month field on Page 1 Item 1 is required.' @('official-hta-runtime#checkDatePage1:L3691-L3696')
Rule '1707-validate-002-day' validate 2 'Day is blank.' @('frm1707:txtPg1I1Day') 'Day field on Page 1 Item 1 is required.' @('official-hta-runtime#checkDatePage1:L3699-L3703')
Rule '1707-validate-003-year' validate 3 'Year is blank.' @('frm1707:txtPg1I1Year') 'Year field on Page 1 Item 1 is required.' @('official-hta-runtime#checkDatePage1:L3706-L3710')
Rule '1707-validate-004-date-shape' validate 4 'Transaction date is not a valid MM/DD/YYYY calendar date.' @('frm1707:txtPg1I1Month','frm1707:txtPg1I1Day','frm1707:txtPg1I1Year') 'Please provide a valid date. (MM/DD/YYYY format) in Page 1 Item 1.' @('official-hta-runtime#dateInPage1Item1:L3971-L4044')
Rule '1707-validate-005-date-future' validate 5 'Transaction date is after today.' @('frm1707:txtPg1I1Month','frm1707:txtPg1I1Day','frm1707:txtPg1I1Year') 'Page 1 Item 1 Date cannot be a future date ' @('official-hta-runtime#dateInPage1Item1:L4045-L4050')
Rule '1707-validate-006-version-cutoff' validate 6 'Transaction date is on or before March 31, 2021.' @('frm1707:txtPg1I1Month','frm1707:txtPg1I1Day','frm1707:txtPg1I1Year') 'Year shall not be greater than the present year and not earlier than April 2021.' @('official-hta-runtime#checkVersion:L4270-L4286') 'incorrect-official-behavior' 'The gate is date-based, but the message incorrectly describes only the year.' 'State that this revision applies only to transactions after March 31, 2021.'
Rule '1707-validate-007-atc' validate 7 'Neither individual nor corporation ATC is selected.' @('frm1707:rdoPg1I3ATCIndiv','frm1707:rdoPg1I3ATCCorp') 'ATC Code on Page 1 Item 3 is required.' @('official-hta-runtime#validateAll:L3633-L3638')
Rule '1707-validate-008-corporation-class' validate 8 'Corporate ATC is selected but neither Domestic nor Foreign is selected.' @('frm1707:rdoPg1I3ATCCorp','frm1707:rdoCorpoDomestic','frm1707:rdoCorpoForeign') 'You need to select whether the Corporation is Domestic or Foreign.' @('official-hta-runtime#validateAll:L3639-L3645')
Rule '1707-validate-009-rdo' validate 9 'RDO is 000.' @('frm1707:rdoPg1Pt1I5RDO') 'Please enter a valid RDO Code on Page 1 Item 5.' @('official-hta-runtime#validateAll:L3647')
Rule '1707-validate-010-name' validate 10 'Name is blank.' @('frm1707:ProfileName') 'Name field on Page 1 Item 6 is required.' @('official-hta-runtime#validateAll:L3648')
Rule '1707-validate-011-address' validate 11 'Registered address is blank.' @('frm1707:ProfileAddress1') 'Registered Address field on Page 1 Item 7 is required.' @('official-hta-runtime#validateAll:L3649')
Rule '1707-validate-012-zip' validate 12 'Zip code is blank.' @('frm1707:ProfileZipCode') 'Zip Code field on Page 1 Item 7A is required.' @('official-hta-runtime#validateAll:L3650')
Rule '1707-validate-013-contact' validate 13 'Contact number is blank.' @('frm1707:ProfileContactNum') 'Contact Number field on Page 1 Item 8 is required.' @('official-hta-runtime#validateAll:L3651')
Rule '1707-validate-014-email-required' validate 14 'Email is blank.' @('frm1707:ProfileEmailAddr') 'E-mail address on page 1 item 9 is required.' @('official-hta-runtime#validateAll:L3652')
Rule '1707-validate-015-seller-row' validate 15 'Seller row 1 is empty, or any seller row mixes blank identity fields with a populated TIN.' @('frm1707:txtPg1I6SellerName0','frm1707:txtPg1I6SellerAddr0','frm1707:txtPg1I6SellerTIN0') 'Seller''s information in Page 1 should have at least one row. / Please complete Item #{row} in Seller''s information in Page 1.' @('official-hta-runtime#checkPage1BuyerAndSeller:L3754-L3811')
Rule '1707-validate-016-buyer-row' validate 16 'Buyer row 1 is empty, or any buyer row mixes blank identity fields with a populated TIN.' @('frm1707:txtPg1I7BuyerName0','frm1707:txtPg1I7BuyerAddr0','frm1707:txtPg1I7BuyerTIN0') 'Buyer''s information in Page 1 should have at least one row. / Please complete Item #{row} in Buyer''s information in Page 1.' @('official-hta-runtime#checkPage1BuyerAndSeller:L3754-L3811')
Rule '1707-validate-017-seller-tin' validate 17 'A nonblank seller TIN has length 11 or less.' @('frm1707:txtPg1I6SellerTIN0','frm1707:txtPg1I6SellerTIN1','frm1707:txtPg1I6SellerTIN2') 'Please check TIN number in Seller''s Information Page 1 row #{row}.' @('official-hta-runtime#validatePageOneTIN:L3842-L3880') 'official-bug-compatible' 'The source accepts any nonblank value longer than 11 characters and does not validate TIN structure.' 'Normalize and validate the official TIN shape.'
Rule '1707-validate-018-buyer-tin' validate 18 'A nonblank buyer TIN has length 11 or less.' @('frm1707:txtPg1I7BuyerTIN0','frm1707:txtPg1I7BuyerTIN1','frm1707:txtPg1I7BuyerTIN2') 'Please check TIN number in Buyer''s Information Page 1 row #{row}.' @('official-hta-runtime#validatePageOneTIN:L3842-L3880') 'official-bug-compatible' 'The source accepts any nonblank value longer than 11 characters and does not validate TIN structure.' 'Normalize and validate the official TIN shape.'
Rule '1707-validate-019-tax-relief-spec' validate 19 'Tax Relief Yes is selected and specification is blank.' @('frm1707:rdoPg1I8TaxReliefYes','frm1707:txtPg1I8TaxReliefSpec') 'Specify Tax Relief field on Page 1 Item 8A is required.' @('official-hta-runtime#validateAll:L3664-L3669')
Rule '1707-validate-020-tax-relief-choice' validate 20 'Neither Tax Relief Yes nor No is selected.' @('frm1707:rdoPg1I8TaxReliefYes','frm1707:rdoPg1I8TaxReliefNo') $null @('official-hta-runtime#validateAll:L3659-L3663') 'incorrect-official-behavior' 'The mandatory-choice validation is commented out, so neither selection passes.' 'Require an explicit Yes/No selection.'
Rule '1707-validate-021-transaction' validate 21 'No transaction-description radio is selected.' @('frm1707:rdoPg1I9TransDescCash','frm1707:rdoPg1I9TransDescInstallment','frm1707:rdoPg1I9TransDescForeclosure','frm1707:rdoPg1I9TransDescOthers') 'Description of Transaction on Page 1 Item 9 is required.' @('official-hta-runtime#validateAll:L3671-L3678')
Rule '1707-validate-022-transaction-other' validate 22 'Others is selected and its description is blank.' @('frm1707:rdoPg1I9TransDescOthers','frm1707:txtPg1I9TransDescSpec') 'Specific description of transaction field on Page 1 Item 9 is required.' @('official-hta-runtime#validateAll:L3680-L3685')
Rule '1707-validate-023-schedule2-row' validate 23 'A Schedule 2 row has incomplete corporation/stock-certificate text or nonpositive shares/tax base.' @('frm1707:txtPg2P4Sch2NameOfCorpStock1','frm1707:txtPg2P4Sch2StockCertNo1','frm1707:txtPg2P4Sch2NoOfShares1','frm1707:txtPg2P4Sch2TaxBaseSellPrice1') 'Please complete Item #{row} in Part IV Sched 2 Page 2.' @('official-hta-runtime#checkPartIVSched2Fields:L3813-L3838')
Rule '1707-validate-024-schedule3-row' validate 24 'A Schedule 3 row has particulars without a positive amount or an amount without particulars.' @('frm1707:txtPg2P4Sch3Particulars1','frm1707:txtPg2P4Sch3Amount1') 'Please complete Item #{row} in Part IV Sched 3 Page 2.' @('official-hta-runtime#checkPartIVSched3Fields:L3840-L3867')
Rule '1707-popup-025-schedule2' input 1 'On popup Save or Add Row, any Schedule 2 popup row lacks description, positive shares, stock certificate, or positive tax base.' @('frm1707:txtPg2Pt4S2_{N>=1}Col1','frm1707:txtPg2Pt4S2_{N>=1}Col2','frm1707:txtPg2Pt4S2_{N>=1}Col3','frm1707:txtPg2Pt4S2_{N>=1}Col4') 'Cannot {method}. You have an empty data in D.{row}' @('official-hta-runtime#CheckEmptyDesc:L2720-L2752')
Rule '1707-popup-026-schedule3' input 2 'On popup Save or Add Row, any Schedule 3 popup row lacks particulars or a positive amount.' @('frm1707:txtPg2Pt4S3_{N>=1}Col1','frm1707:txtPg2Pt4S3_{N>=1}Col2') 'Cannot {method}. You have an empty data in D.{row}' @('official-hta-runtime#CheckEmptyDesc2:L2761-L2782')
Rule '1707-email-027-unreachable' validate 25 'Email fails the source regex.' @('frm1707:ProfileEmailAddr') 'Please enter a valid e-mail address on page 1 item 10' @('official-hta-runtime#validateEmail:L4130-L4142','official-hta-runtime#validateAll:L3625-L3689') 'incorrect-official-behavior' 'validateEmail is defined but never called by Validate or the email control.' 'Invoke format validation from the shared validation graph.'
Rule '1707-date-028-invalid' 'blur/change' 1 'A date control contains an invalid MM/DD/YYYY date.' @('date-controls') 'Please provide a valid date. (MM/DD/YYYY format)' @('official-hta-runtime#validateDate:L3882-L3968')
Rule '1707-date-029-future-return' 'blur/change' 2 'A valid date is after today.' @('date-controls') 'This date cannot be a future date.' @('official-hta-runtime#validateDate:L3951-L3956') 'incorrect-official-behavior' 'The field clears, but isValid remains true and the function returns true.' 'Set false and return false after clearing.'
Rule '1707-date-030-pre2018-return' 'blur/change' 3 'A valid date has a year before 2018.' @('date-controls') 'This date cannot be prior to 2018.' @('official-hta-runtime#validateDate:L3957-L3962') 'incorrect-official-behavior' 'The field clears, but isValid remains true and the function returns true.' 'Set false and return false after clearing.'
Rule '1707-rate-031-upper-bound' 'blur/change' 4 'A tax rate is at least 100.' @('frm1707:txtPg1P2I13AppTaxRate','frm1707:txtPg2P4Sch1I6ApplcRate') 'Tax rate should be below 100.' @('official-hta-runtime#checkRate:L4248-L4254')
Rule '1707-rate-032-negative' 'blur/change' 5 'A tax rate is negative.' @('frm1707:txtPg1P2I13AppTaxRate','frm1707:txtPg2P4Sch1I6ApplcRate') $null @('official-hta-runtime#checkRate:L4248-L4254') 'incorrect-official-behavior' 'checkRate enforces only the upper bound, so a negative rate is accepted.' 'Require a rate from zero through less than 100.'
Rule '1707-save-033-full-graph' save 1 'Any validateAll rule fails.' @() 'First validateAll message.' @('official-hta-runtime#initialValidateBeforeSave:L6593-L6607') 'verified-correct' 'Save invokes the full Validate graph, then redundantly checks RDO again.' 'Use the same shared graph without the redundant RDO branch.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'; schema_version = '1.0.0'; form_id = $formId; revision = $revision
    first_error_behavior = 'Validate and Save stop at the first source-ordered failure; popup row checks run when adding or saving popup rows.'
    rules = $rules
})

$calcs = [Collections.Generic.List[object]]::new()
function Calc(
    [string]$Id, [string[]]$Outputs, [string[]]$Inputs, [string]$Formula, [string]$Trigger,
    [string[]]$Depends, [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Recommended = 'Implement with typed decimals and the official two-decimal rounding order.'
) {
    $calcs.Add([pscustomobject][ordered]@{
        calculation_id = $Id; outputs = $Outputs; inputs = $Inputs; condition = $null
        official_formula = $Formula; rounding = 'Source applies toFixed(2) and formatCurrency at each displayed output.'
        trigger = $Trigger; depends_on = $Depends; source_refs = $Refs; assessment = $Assessment
        recommended_app_behavior = $Recommended; confidence = 'high'
    })
}
Calc '1707-rate-selection' @('frm1707:txtPg1P2I13AppTaxRate','frm1707:txtPg2P4Sch1I6ApplcRate') @('frm1707:rdoPg1I9TransDescInstallment') 'Installment sets Schedule 1 rate to 15 and clears Item 13 rate; other transaction types set Item 13 rate to 15 and clear Schedule 1 rate.' transactionType @() @('official-hta-runtime#transactionType:L4476-L4497')
Calc '1707-schedule2-popup-subtotal' @('Pg2Pt4S2SubTotal') @('frm1707:txtPg2Pt4S2_{N>=1}Col4') 'Sum popup Schedule 2 tax-base amounts.' Sum_Pg2Pt4S2 @() @('official-hta-runtime#Sum_Pg2Pt4S2:L2980-L2996')
Calc '1707-schedule2-total' @('frm1707:txtPg2P4Sch2TotAmount') @('frm1707:txtPg2P4Sch2TaxBaseSellPrice1','frm1707:txtPg2P4Sch2TaxBaseSellPrice2','frm1707:txtPg2P4Sch2TaxBaseSellPrice3','frm1707:txtPg2P4Sch2TaxBaseSellPrice4') 'Sum fixed Schedule 2 rows; row 4 carries the popup subtotal when multiple Others rows exist.' pageTwoSched2Comp @('1707-schedule2-popup-subtotal') @('official-hta-runtime#pageTwoSched2Comp:L4387-L4397')
Calc '1707-schedule3-popup-subtotal' @('Pg2Pt4S3SubTotal') @('frm1707:txtPg2Pt4S3_{N>=1}Col2') 'Sum popup Schedule 3 amounts.' Sum_Pg2Pt4S3 @() @('official-hta-runtime#Sum_Pg2Pt4S3:L3158-L3174')
Calc '1707-schedule3-total' @('frm1707:txtPg2P4Sch3TotAmount') @('frm1707:txtPg2P4Sch3Amount1','frm1707:txtPg2P4Sch3Amount2','frm1707:txtPg2P4Sch3Amount3','frm1707:txtPg2P4Sch3Amount4') 'Sum fixed Schedule 3 rows; row 4 carries the popup subtotal when multiple Others rows exist.' pageTwoSched3Comp @('1707-schedule3-popup-subtotal') @('official-hta-runtime#pageTwoSched3Comp:L4399-L4409')
Calc '1707-installment-period-tax' @('frm1707:txtPg2P4Sch1I7PeriodTaxDue') @('frm1707:txtPg2P4Sch1I5InstallmentAmt','frm1707:txtPg2P4Sch1I6ApplcRate') 'Schedule 1 Item 7 = installment amount × applicable rate / 100.' pageOneComputation @('1707-rate-selection') @('official-hta-runtime#pageOneComputation:L4289-L4295')
Calc '1707-item10-tax-base' @('frm1707:txtPg1P2I10TaxBase') @('frm1707:txtPg2P4Sch2TotAmount') 'Item 10 copies Schedule 2 total tax base.' pageOneComputation @('1707-schedule2-total') @('official-hta-runtime#pageOneComputation:L4297-L4300')
Calc '1707-item11-expenses' @('frm1707:txtPg1P2I11AllowExpnse') @('frm1707:txtPg2P4Sch3TotAmount') 'Item 11 copies Schedule 3 allowable expenses.' pageOneComputation @('1707-schedule3-total') @('official-hta-runtime#pageOneComputation:L4302-L4305')
Calc '1707-item12-net-capital-gain' @('frm1707:txtPg1P2I12NetCptalChnged') @('frm1707:txtPg1P2I10TaxBase','frm1707:txtPg1P2I11AllowExpnse') 'Item 12 = Item 10 - Item 11.' pageOneComputation @('1707-item10-tax-base','1707-item11-expenses') @('official-hta-runtime#pageOneComputation:L4307-L4311')
Calc '1707-item13-tax-due' @('frm1707:txtPg1P2I13TaxDueForTrans') @('frm1707:txtPg1P2I12NetCptalChnged','frm1707:txtPg1P2I13AppTaxRate') 'For cash, foreclosure, or other transactions, Item 13 = max(0, Item 12 × rate / 100).' pageOneComputation @('1707-rate-selection','1707-item12-net-capital-gain') @('official-hta-runtime#pageOneComputation:L4312-L4343')
Calc '1707-item14-installment-tax' @('frm1707:txtPg1P2I14TaxDuePmtPriod') @('frm1707:txtPg2P4Sch1I7PeriodTaxDue') 'For installment transactions, Item 14 copies Schedule 1 Item 7; otherwise it is zero.' pageOneComputation @('1707-installment-period-tax') @('official-hta-runtime#pageOneComputation:L4345-L4357')
Calc '1707-item16-tax-payable' @('frm1707:txtPg1P2I16TaxPayable') @('frm1707:txtPg1P2I13TaxDueForTrans','frm1707:txtPg1P2I14TaxDuePmtPriod','frm1707:txtPg1P2I15TaxPaidInPrevRtrn') 'Installment: Item 14 - Item 15; otherwise Item 13 - Item 15.' pageOneComputation @('1707-item13-tax-due','1707-item14-installment-tax') @('official-hta-runtime#pageOneComputation:L4364-L4376')
Calc '1707-item17-penalties' @('frm1707:txtPg1P2I17TotPenalties') @('frm1707:txtPg1P2I17Surcharge','frm1707:txtPg1P2I17Interest','frm1707:txtPg1P2I17Compromise') 'Item 17 = surcharge + interest + compromise.' pageOneComputation @() @('official-hta-runtime#pageOneComputation:L4378-L4383')
Calc '1707-item18-payable' @('frm1707:txtPg1P2I18TotAmtPyable') @('frm1707:txtPg1P2I16TaxPayable','frm1707:txtPg1P2I17TotPenalties') 'If tax payable is negative and penalties are positive, Item 18 equals penalties; otherwise Item 18 = tax payable + penalties.' pageOneComputation @('1707-item16-tax-payable','1707-item17-penalties') @('official-hta-runtime#pageOneComputation:L4385-L4398') 'incorrect-official-behavior' 'When tax payable is negative and penalties are zero, the source exposes a negative total amount payable; represent the credit separately and payable as zero.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema' = '../../schema/calculations.schema.json'; schema_version = '1.0.0'; form_id = $formId; revision = $revision
    evaluation_order = @($calcs.calculation_id); calculations = $calcs
})

$cases = @()
$caseNumber = 0
foreach ($rule in @($rules | Where-Object { $_.exact_message })) {
    $caseNumber++
    $cases += [pscustomobject][ordered]@{
        case_id = ('case-{0:d2}-{1}' -f $caseNumber, $rule.rule_id); phase = $rule.phase
        mutations = @{ synthetic_condition = $rule.condition }; expected_message = $rule.exact_message
        expected_behavior = $rule.official_behavior; rule_id = $rule.rule_id
    }
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{ schema_version = '1.0.0'; form_id = $formId; synthetic_only = $true; cases = $cases })
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; cases = @(
        @{ case_id = 'regular-fifteen-percent'; calculation_id = '1707-item13-tax-due'; net_capital_gain = 1000; rate = 15; official_output = 150 },
        @{ case_id = 'negative-gain-floor'; calculation_id = '1707-item13-tax-due'; net_capital_gain = -100; rate = 15; official_output = 0 },
        @{ case_id = 'installment-tax'; calculation_id = '1707-installment-period-tax'; installment_amount = 1000; rate = 15; official_output = 150 },
        @{ case_id = 'negative-payable-defect'; calculation_id = '1707-item18-payable'; tax_payable = -100; penalties = 0; official_output = -100; recommended_output = 0 }
    )
})
$resources = @()
foreach ($src in @([regex]::Matches($hta, '(?i)<script[^>]+src\s*=\s*(["''])(?<value>.*?)\1') | ForEach-Object { $_.Groups['value'].Value } | Sort-Object -Unique)) {
    $full = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if (Test-Path -LiteralPath $full) {
        $resources += [pscustomobject][ordered]@{ src = $src; path = $full; present = $true; size = (Get-Item -LiteralPath $full).Length; sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    else {
        $resources += [pscustomobject][ordered]@{ src = $src; path = $full; present = $false; size = $null; sha256 = $null }
    }
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{ schema_version = '1.0.0'; form_id = $formId; resources = $resources })

$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'; schema_version = '1.0.0'; form_id = $formId; revision = $revision
    phases = @(
        @{ phase = 'edit'; official_behavior = 'April 2021 capital-gains return for shares not traded through the local stock exchange.'; source_refs = @('official-hta-runtime','official-help-runtime'); confidence = 'high' },
        @{ phase = 'saved-draft'; official_behavior = 'Save invokes the full validation graph before serializing static and popup controls.'; source_refs = @('official-hta-runtime#initialValidateBeforeSave:L6593-L6607','official-hta-runtime#saveXML:L6003-L6318'); confidence = 'high' },
        @{ phase = 'validated'; official_behavior = 'Validate stops at the first invoked error and then disables the form for printing/final-copy actions.'; source_refs = @('official-hta-runtime#validateAll:L3625-L3689','official-hta-runtime#validate'); confidence = 'high' },
        @{ phase = 'final-copy'; official_behavior = 'The runtime defines encrypted final-copy creation, but no revision-matched April 2021 final copy is available.'; source_refs = @('official-hta-runtime#saveXML:L6003-L6318'); confidence = 'medium' },
        @{ phase = 'submitted'; official_behavior = 'Online/EFPS transports exist but were not exercised.'; source_refs = @('official-hta-runtime#submitOnline','official-hta-runtime#submitToEFPS'); confidence = 'medium' }
    )
    transitions = @(
        @{ from = 'edit'; action = 'Save'; to = 'saved-draft'; guard = 'validateAll and the redundant RDO check pass.'; side_effects = @('Writes flat pseudo-XML.','Serializes runtime popup rows.'); source_refs = @('official-hta-runtime#saveXML:L6003-L6318') },
        @{ from = 'edit'; action = 'Validate'; to = 'validated'; guard = 'All invoked validateAll checks pass.'; side_effects = @('Disables controls.','Enables print/final-copy actions.'); source_refs = @('official-hta-runtime#validate') },
        @{ from = 'validated'; action = 'Edit'; to = 'edit'; guard = $null; side_effects = @('Re-enables controls subject to transaction-type conditions.'); source_refs = @('official-hta-runtime#enableAllControl') },
        @{ from = 'validated'; action = 'Final Copy'; to = 'final-copy'; guard = 'Finalization/version flow succeeds.'; side_effects = @('Encrypts and compresses the final copy.'); source_refs = @('official-hta-runtime#saveXML:L6003-L6318') },
        @{ from = 'final-copy'; action = 'Transport'; to = 'submitted'; guard = 'Connectivity and remote acceptance succeed.'; side_effects = @('Online/EFPS attempt; untested.'); source_refs = @('official-hta-runtime#submitOnline','official-hta-runtime#submitToEFPS') }
    )
    prerequisites = @('Transaction date after March 31, 2021','ATC and corporate classification when applicable','RDO and taxpayer identity','Seller and buyer information','Transaction description','Complete applicable schedules')
    required_attachments = @(
        @{ attachment_id = 'ecar-mandatory-documents'; label = 'Mandatory documents for securing the Electronic Certificate Authorizing Registration, with additional photocopies.'; required_when = 'For eCAR processing.'; official_ui_enforcement = 'Not enforced by local Validate.'; source_refs = @('official-help-runtime#L318-L342'); confidence = 'high' },
        @{ attachment_id = 'ecar-additional-documents'; label = 'Additional eCAR requirements when applicable.'; required_when = 'The corresponding transaction circumstance applies.'; official_ui_enforcement = 'Not enforced by local Validate.'; source_refs = @('official-help-runtime#L343-L363'); confidence = 'high' }
    )
    filing_deadlines = @(
        @{ quarter = 'Q1'; due_date_rule = 'Transaction-based, not quarterly: file and pay within thirty (30) days after each disposition; for installment sales, within thirty (30) days after the first down payment and each subsequent installment receipt.'; source_refs = @('official-help-runtime#L177-L194'); confidence = 'high' },
        @{ quarter = 'Q2'; due_date_rule = 'Transaction-based, not quarterly: file and pay within thirty (30) days after each disposition; for installment sales, within thirty (30) days after the first down payment and each subsequent installment receipt.'; source_refs = @('official-help-runtime#L177-L194'); confidence = 'high' },
        @{ quarter = 'Q3'; due_date_rule = 'Transaction-based, not quarterly: file and pay within thirty (30) days after each disposition; for installment sales, within thirty (30) days after the first down payment and each subsequent installment receipt.'; source_refs = @('official-help-runtime#L177-L194'); confidence = 'high' },
        @{ quarter = 'Q4'; due_date_rule = 'Transaction-based, not quarterly: file and pay within thirty (30) days after each disposition; for installment sales, within thirty (30) days after the first down payment and each subsequent installment receipt.'; source_refs = @('official-help-runtime#L177-L194'); confidence = 'high' }
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow
$bugs = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$officialAssets = @(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1707v2021; April 2021 ENCS.'
    Asset 'predecessor-2018-excluded' 'runtime-extracted-hta' $predecessorPath 'January 2018 predecessor; excluded from April 2021 rules.'
    Asset 'legacy-1999-excluded' 'runtime-extracted-hta' $legacyPath 'July 1999 predecessor; excluded from April 2021 rules.'
    Asset 'official-help-runtime' 'official-runtime-help' $helpPath 'Revision-matched April 2021 filing, rate, penalty, and attachment guidance.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'April 2021 ENCS form.'
    Asset 'official-guide-pdf' 'official-guidelines-pdf' $guidePath 'April 2021 ENCS guidelines and instructions.'
    Asset 'legacy-encrypted-sample-excluded' 'dummy-profile-encrypted-final-copy' $samples[0].FullName 'Excluded: 65 of 70 keys overlap the July 1999 runtime, while only 6 overlap April 2021.' (Join-Path $SampleDir '1707-final-copy-#email-redacted#.xml')
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'; schema_version = '1.0.0'; form_id = $formId
    form_code = '1707'; revision = $revision; package_version = $packageVersion; status = 'complete'
    official_assets = $officialAssets
    counts = [ordered]@{
        concrete_fields = 142; runtime_field_families = 6; fields_total = $fields.Count; typed_fields = $fields.Count
        validation_rules = $rules.Count; confirmed_official_bugs = $bugs; calculations = $calcs.Count
        negative_fixtures = $cases.Count; unverified_gaps = 2
    }
    artifacts = [ordered]@{
        fields = 'fields.json'; validations = 'validations.json'; calculations = 'calculations.json'
        workflow = 'workflow.json'; evidence = 'evidence.md'; audit = 'audit.md'; gaps = 'gaps.md'
        excluded_legacy_encrypted_keys = 'fixtures/excluded-legacy-encrypted-field-keys-v796.json'
        runtime_controls = 'fixtures/runtime-control-inventory-v796.json'
        validation_functions = 'fixtures/validation-function-inventory-v796.json'
        calculation_functions = 'fixtures/calculation-function-inventory-v796.json'
        resources = 'fixtures/official-resource-hashes-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'; calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer/release metadata changed.',
        'No decrypted values or email-bearing filenames are emitted.',
        'The available encrypted sample is proven legacy and excluded.',
        '142 source-serializable controls plus 6 source-unbounded popup families are preserved.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1707 - April 2021`n`nRevision-specific rule package for runtime `1707v2021`, with 142 source-serializable controls and 6 unbounded popup families.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- April 2021 runtime HTA SHA-256: $($expected.hta); `APPLICATIONNAME="1707v2021"` and April 2021 ENCS print identity.
- January 2018 predecessor SHA-256: $($expected.predecessor); excluded.
- July 1999 predecessor SHA-256: $($expected.legacy); excluded.
- Revision-matched help SHA-256: $($expected.help).
- April 2021 form PDF SHA-256: $($expected.pdf).
- April 2021 guide PDF SHA-256: $($expected.guide).
- Available encrypted sample: ciphertext $($expected.cipher); decrypted payload $($expected.plain); 70 unique keys; inventory $($expected.inventory). Values were never emitted.
- Revision discrimination: 65/70 keys occur in the July 1999 runtime, but only 6/70 occur in April 2021. The sample is excluded.
- April 2021 inventory: 142 `saveXML`-serializable controls plus six source-unbounded popup families.

All email-bearing filenames are represented as `#email-redacted#`.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. No revision-matched April 2021 encrypted final copy is available; the supplied sample is proven to match the July 1999 runtime and is excluded.`n2. Online and EFPS submission were deliberately not exercised.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- April 2021 revision and predecessor separation: pass.`n- Legacy-sample mismatch detected and excluded: 65/70 legacy overlap versus 6/70 current overlap.`n- Official asset and encrypted inventory hashes: pass.`n- Typed inventory: 142 concrete + 6 families = $($fields.Count).`n- Validation rules: $($rules.Count); calculations: $($calcs.Count); negative fixtures: $($cases.Count).`n- Confirmed official defects: $bugs.`n- Full JSON structural/schema audit must run after generation.`n- No renderer/release/capability/commit/push changes.`n"

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry = $index.forms | Where-Object { $_.form_id -eq $formId }
if ($entry) {
    $entry.form_code = '1707'; $entry.revision = $revision; $entry.package_version = $packageVersion
    $entry.priority = 27; $entry.status = 'complete'; $entry.path = 'forms/1707-v2021/manifest.json'
}
else {
    $index.forms += [pscustomobject][ordered]@{
        form_id = $formId; form_code = '1707'; revision = $revision; package_version = $packageVersion
        priority = 27; status = 'complete'; path = 'forms/1707-v2021/manifest.json'
    }
}
$index.forms = @($index.forms | Sort-Object priority)
$index.updated = '2026-07-23'
Write-Json $indexPath $index
[pscustomobject]@{
    form_id = $formId; concrete_fields = 142; families = 6; typed_fields = $fields.Count
    validations = $rules.Count; calculations = $calcs.Count; negative_fixtures = $cases.Count
    confirmed_official_bugs = $bugs; next_form = '1707A'
} | ConvertTo-Json
