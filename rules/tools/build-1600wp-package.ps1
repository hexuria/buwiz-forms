param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$PdfDir = 'C:\Mac\Home\Downloads\forms\1600WPv2010',
    [string]$SampleDir = 'C:\Mac\Home\Downloads\forms\1600WP'
)

$ErrorActionPreference = 'Stop'
$formId = '1600wp-v2010'
$revision = '2010-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1600WP.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1600WP.hta'
$pdfPath = Join-Path $PdfDir '1600WP p1ENCS.pdf'
$guidePath = Join-Path $PdfDir '1600WP Guide.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1600wp-v2010'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = 'e33af59157e1083432cdfc360f648e06602caab75167a27fbb89a08853cb9420'
    help = '6f42c395e3892e00af4178031b483dea1e2f92edc53ad611254c3a3c3131884f'
    pdf = '6ea2ef0f6c84a68ef1c50ad63f4ff0e95a68258f52b62b98f305c861c8b75d55'
    guide = '3742f9553251ddd86396a565e156490b5e210487a66c5dfacd83cfc14cbba2f7'
    sample_cipher = '716e33e9ecaf808ec7d52729268c218ae4392cb9e6f6836e89ec5433d75d995a'
    sample_plain = '6d67e080d1ee4c5033351895da3ddf1f64419bd02a439aafb9d2436b252af445'
    sample_inventory = 'a3c9d11faec93a1f7fd54c6b79a61647fa28cb10f146949b7dfdc1cec20968f8'
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

foreach ($path in @($htaPath, $helpPath, $pdfPath, $guidePath, $packagePath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
foreach ($pair in @(
    @($htaPath, 'hta'), @($helpPath, 'help'), @($pdfPath, 'pdf'),
    @($guidePath, 'guide'), @($packagePath, 'package')
)) {
    if ((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected[$pair[1]]) {
        throw "Hash changed: $($pair[0])"
    }
}
$samples = @(Get-ChildItem -LiteralPath $SampleDir -File | Where-Object { $_.Extension -eq '.xml' })
if ($samples.Count -ne 1) { throw "Expected one reviewed encrypted sample; found $($samples.Count)." }
if ((Get-FileHash -LiteralPath $samples[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected.sample_cipher) {
    throw 'Encrypted sample hash changed.'
}
foreach ($pdf in @($pdfPath, $guidePath)) {
    $bytes = [IO.File]::ReadAllBytes($pdf)
    if ([Text.Encoding]::ASCII.GetString($bytes[0..4]) -ne '%PDF-') { throw "PDF magic mismatch: $pdf" }
}
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']1600WP["'']' -or $hta -notmatch '(?i)January\s+2010\s+\(ENCS\)') {
    throw 'HTA revision binding changed.'
}
if ($help -notmatch '(?i)BIR\s+Form\s+No\.\s+1600\s*WP' -or $help -notmatch '(?i)Alphabetical\s+list\s+of\s+payees') {
    throw 'Help identity/attachment binding changed.'
}
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
        ordinal = $ordinal; id = Attr $tag 'id'; name = Attr $tag 'name'; element = $element
        control_kind = $kind.ToLowerInvariant()
        source_line = 1 + [regex]::Matches($hta.Substring(0, $bodyOffset + $match.Index), "`n").Count
        value = Attr $tag 'value'; maxlength = Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}
if ($controls.Count -ne 132) { throw "Expected 132 live controls; found $($controls.Count)." }
$static = @($controls | Where-Object { $_.control_kind -in @('text','select','select-one','textarea','radio','checkbox') })
if ($static.Count -ne 110 -or @($static.id | Sort-Object -Unique).Count -ne 110) {
    throw "Static serializer inventory changed: $($static.Count)."
}
$injected = @(
    [pscustomobject][ordered]@{ ordinal=0; id='frm1600WP:txtRDOCode'; name='frm1600WP:txtRDOCode'; element='select'; control_kind='select'; source_line=3122; value='000'; maxlength=$null; disabled=$true; readonly=$false },
    [pscustomobject][ordered]@{ ordinal=0; id='AtcCd1'; name='AtcCd1'; element='input'; control_kind='checkbox'; source_line=3549; value=$null; maxlength=$null; disabled=$false; readonly=$false },
    [pscustomobject][ordered]@{ ordinal=0; id='AtcCd2'; name='AtcCd2'; element='input'; control_kind='checkbox'; source_line=3549; value=$null; maxlength=$null; disabled=$false; readonly=$false },
    [pscustomobject][ordered]@{ ordinal=0; id='SchedIIAtcCde1'; name='SchedIIAtcCde'; element='input'; control_kind='radio'; source_line=3553; value=$null; maxlength=$null; disabled=$false; readonly=$false },
    [pscustomobject][ordered]@{ ordinal=0; id='SchedIIAtcCde2'; name='SchedIIAtcCde'; element='input'; control_kind='radio'; source_line=3553; value=$null; maxlength=$null; disabled=$false; readonly=$false }
)
$concrete = @($static) + $injected
if ($concrete.Count -ne 115 -or @($concrete.id | Sort-Object -Unique).Count -ne 115) { throw 'Concrete runtime inventory changed.' }
$sampleProjectionHash = Hash-Lines @($concrete.id | Sort-Object)
if ($sampleProjectionHash -ne $expected.sample_inventory) {
    throw 'Concrete runtime control inventory no longer equals the reviewed encrypted sample.'
}
$families = @(
    @{ pattern='frm1600WP:txtAtcCode{index}'; kind='code'; control='runtime-indexed-text'; computed=$true; source='getATCCode:L3716-L3723' },
    @{ pattern='frm1600WP:txtTaxBase{index}'; kind='decimal-amount'; control='runtime-indexed-text'; computed=$false; source='getATCCode:L3716-L3723' },
    @{ pattern='frm1600WP:txtTaxRate{index}'; kind='decimal-rate'; control='runtime-indexed-text'; computed=$true; source='getATCCode:L3716-L3723' },
    @{ pattern='frm1600WP:txtTaxbeWithHeld{index}'; kind='decimal-amount'; control='runtime-indexed-text'; computed=$true; source='getATCCode:L3716-L3723' }
)

$required = @(
    'frm1600WP:DateWithholdingMonth','frm1600WP:DateWithholdingDay','frm1600WP:DateWithholdingYear',
    'frm1600WP:AmendedReturn_1','frm1600WP:AmendedReturn_2','frm1600WP:AnyTaxHeld_1','frm1600WP:AnyTaxHeld_2',
    'frm1600WP:txtTIN1','frm1600WP:txtTIN2','frm1600WP:txtTIN3','frm1600WP:txtBranchCode',
    'frm1600WP:txtRDOCode','frm1600WP:CategoryAgent_P','frm1600WP:CategoryAgent_G',
    'frm1600WP:txtTaxpayerName','frm1600WP:txtAddress','frm1600WP:txtZipCode',
    'frm1600WP:SpecialLaw_1','frm1600WP:SpecialLaw_2'
)
$itemMap = @{
    DateWithholdingMonth='1';DateWithholdingDay='1';DateWithholdingYear='1'
    DateWithholdingToMonth='1';DateWithholdingToDay='1';DateWithholdingToYear='1'
    AmendedReturn_1='2';AmendedReturn_2='2';txtSheets='3';AnyTaxHeld_1='4';AnyTaxHeld_2='4'
    txtTIN1='5';txtTIN2='5';txtTIN3='5';txtBranchCode='5';txtRDOCode='6'
    CategoryAgent_P='7';CategoryAgent_G='7';txtTaxpayerName='8';txtAddress='9';txtZipCode='10'
    SpecialLaw_1='11';SpecialLaw_2='11';SpecialLawSelect='11A'
    txtTax12='12';txtTax13='13';txtTax14='14';txtTax15A='15A';txtTax15B='15B'
    txtTax15C='15C';txtTax15D='15D';txtTax16='16'
}
$computedPattern = '(?i)(txtTax12$|txtTax14$|txtTax15D$|txtTax16$|dtSched:taxWithheld|dtSched:TotaltaxWithheld|DateWithholdingTo|AtcCd|SchedIIAtcCde)'
$amountPattern = '(?i)(txtTax1[2-6]|amount\d+$|RatePercent|taxWithheld|Total)'
$hiddenPattern = '(?i)(hPartIITableSize|txtFinalFlag|txtEnroll|ebirOnline|driveSelect)'
$fields = [Collections.Generic.List[object]]::new()
foreach ($control in $concrete) {
    $key = $control.id
    $short = if ($key -like 'frm1600WP:*') { $key.Substring(10) } else { $key }
    $logical = 'string'; $enum = [object[]]@(); $normalization = [string[]]@()
    if ($control.control_kind -in @('radio','checkbox')) { $logical='boolean'; $enum=[object[]]@('true','false') }
    elseif ($key -match '(?i)(TIN|RDO|BranchCode|Atc)') { $logical='code' }
    elseif ($key -eq 'txtEmail') { $logical='email-string' }
    elseif ($key -match $amountPattern) { $logical='decimal-amount'; $normalization=[string[]]@('NumWithComma','formatCurrency','round(...,2)') }
    elseif ($key -match 'DateWithholding.*(Month|Day)$') { $logical='integer-string' }
    elseif ($key -match 'DateWithholding.*Year$') { $logical='year' }
    $isComputed = $key -match $computedPattern
    $status = if ($required -contains $key) { 'required' } elseif ($isComputed) { 'computed' } else { 'optional' }
    if ($key -match $hiddenPattern) { $status='hidden' }
    if ($key -eq 'frm1600WP:SpecialLawSelect') { $status='conditional'; $enum=[object[]]@('0','1','2','3') }
    if ($key -match 'dtSched:') { $status = if ($isComputed) { 'computed' } else { 'conditional' } }
    $constraints = [ordered]@{}
    if ($control.maxlength -and $control.maxlength -match '^\d+$') { $constraints.max_length=[int]$control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision=2 }
    if ($key -match 'DateWithholdingYear$') { $constraints.minimum=1900 }
    $notes = @('Source-derived from the exact January 2010 runtime DOM and generic serializer.')
    if ($injected.id -contains $key) { $notes += 'Runtime-generated control independently proven by the reviewed encrypted sample.' }
    if ($key -eq 'txtEmail') { $notes += 'The official serializer retains the unprefixed control ID literally.' }
    $fields.Add([pscustomobject][ordered]@{
        field_key=$key;serialized_key=$key;serialized_occurrence=1;label=$short
        page=if($key -match 'dtSched:'){2}else{1}
        item_number=if($itemMap.ContainsKey($short)){$itemMap[$short]}else{$null}
        control_kind=$control.control_kind;storage_type='string';logical_type=$logical;required=$status
        required_when=if($key-eq'frm1600WP:SpecialLawSelect'){'Item 11 Yes.'}elseif($key-match'dtSched:'){'Item 4 Yes and a Schedule II row is used.'}else{$null}
        enabled_when=if($key-match'dtSched:'){'Item 4 Yes.'}else{$null};visible_when=$null
        default_value=$control.value;empty_representation='';constraints=[pscustomobject]$constraints
        enum_values=$enum;normalization=$normalization;computed=$isComputed
        calculation_id=if($isComputed){'See calculations.json'}else{$null}
        source_refs=@("official-hta-runtime#control:L$($control.source_line)",'official-hta-runtime#saveXML:L2580-L2660','revision-matched-encrypted-sample')
        confidence='high';notes=$notes
    })
}
foreach ($family in $families) {
    $fields.Add([pscustomobject][ordered]@{
        field_key=$family.pattern;serialized_key=$family.pattern;serialized_occurrence=$null;label=$family.pattern
        page=1;item_number='Part II';control_kind=$family.control;storage_type='string';logical_type=$family.kind
        required=if($family.computed){'computed'}else{'conditional'}
        required_when='Item 4 Yes and the corresponding ATC is selected.';enabled_when='The corresponding ATC is selected.'
        visible_when='The corresponding ATC is selected.';default_value=if($family.kind-like'decimal*'){'0.00'}else{$null}
        empty_representation='';constraints=[pscustomobject]@{index='one-based contiguous selected-ATC row index'}
        enum_values=@();normalization=@(if($family.kind-like'decimal*'){'round(...,2)';'formatCurrency'})
        computed=[bool]$family.computed;calculation_id=if($family.computed){'See calculations.json'}else{$null}
        source_refs=@("official-hta-runtime#$($family.source)",'official-hta-runtime#saveXML:L2580-L2660')
        confidence='high';notes=@('Conditional Part II controls are absent from the reviewed zero-selected-ATC sample but are serialized whenever generated.')
    })
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    field_count=$fields.Count;runtime_serializable_element_count=115
    inventory_sha256=Hash-Lines @($fields.field_key|Sort-Object);fields=$fields
})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta
    live_static_control_count=$controls.Count;static_serializer_control_count=$static.Count
    runtime_injected_concrete_count=$injected.Count;concrete_serializer_control_count=$concrete.Count
    conditional_part_ii_family_count=$families.Count
    encrypted_sample_field_count=115;encrypted_sample_inventory_sha256=$expected.sample_inventory
    concrete_projection_matches_sample=$true;controls=$concrete;conditional_families=$families
})
$decryptTool = Join-Path $RepoRoot 'rules\tools\audit-1701-encrypted-fields.ps1'
Write-Utf8 (Join-Path $fixtureDir 'encrypted-field-audit.json') ((&$decryptTool -SourceDir $SampleDir -FormId $formId -FilePattern '*.xml' `
    -RedactedFileName '1600WP-final-copy-#email-redacted#.xml' -ExpectedCiphertextSha256 $expected.sample_cipher `
    -ExpectedDecryptedSha256 $expected.sample_plain -ExpectedFieldCount 115 -ExpectedFieldInventorySha256 $expected.sample_inventory `
    -ExpectedExtraField 'frm1600WP:DateWithholdingMonth' -VersionField '*' -ExpectedXmlVersion '*') -join [Environment]::NewLine)
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm1600WP:' -NamePattern '(?i)valid|save|date|enable|disable|final|submit') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm1600WP:' -NamePattern '(?i)compute|withheld|penalt|atc') -join [Environment]::NewLine)

$rules=[Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Keys,$Message,[string[]]$Refs,
    [string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',
    [string]$Recommended='Retain as a structured revision-aware error.') {
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Keys
        accepted_behavior='Condition is false; processing continues.'
        rejected_behavior='The active operation stops unless official_behavior states otherwise.'
        exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment
        official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()
    })
}
Rule '1600wp-validate-001-month-required' validate 1 'From-month is blank.' @('frm1600WP:DateWithholdingMonth') 'Please enter a valid month on Item 1.' @('official-hta-runtime#validate:L3157-L3167')
Rule '1600wp-validate-002-month-width' validate 2 'From-month has one character.' @('frm1600WP:DateWithholdingMonth') 'Please enter a valid month on item 1. Format should be MM/DD/YYYY.' @('official-hta-runtime#validate:L3168-L3172')
Rule '1600wp-validate-003-day-required' validate 3 'From-day is blank.' @('frm1600WP:DateWithholdingDay') 'Please enter a valid day on Item 1.' @('official-hta-runtime#validate:L3173-L3177')
Rule '1600wp-validate-004-day-width' validate 4 'From-day has one character.' @('frm1600WP:DateWithholdingDay') 'Please enter a valid day on item 1. Format should be MM/DD/YYYY.' @('official-hta-runtime#validate:L3178-L3182')
Rule '1600wp-validate-005-year-required' validate 5 'From-year is blank.' @('frm1600WP:DateWithholdingYear') 'Please enter a valid year on Item 1.' @('official-hta-runtime#validate:L3183-L3187')
Rule '1600wp-validate-006-positive-day' validate 6 'From-day is below 1.' @('frm1600WP:DateWithholdingDay') 'Invalid date entry on item 1.' @('official-hta-runtime#validate:L3188-L3192')
Rule '1600wp-validate-007-nonleap-feb29' validate 7 'Date is February 29 in a non-leap year.' @('frm1600WP:DateWithholdingMonth','frm1600WP:DateWithholdingDay','frm1600WP:DateWithholdingYear') 'Filing year is not a leap year.' @('official-hta-runtime#validate:L3193-L3198')
Rule '1600wp-validate-008-calendar-day' validate 8 'Day exceeds the allowed value for a recognized month.' @('frm1600WP:DateWithholdingMonth','frm1600WP:DateWithholdingDay','frm1600WP:DateWithholdingYear') 'Invalid date entry on item 1.' @('official-hta-runtime#validate:L3199-L3219')
Rule '1600wp-validate-009-year-floor' validate 9 'Year is below 1900.' @('frm1600WP:DateWithholdingYear') 'Invalid date entry on Item no.1. Entry should not be lower than 1900.' @('official-hta-runtime#validate:L3235-L3239')
Rule '1600wp-validate-010-invalid-month-gap' validate 10 'Month is a two-character value outside 01 through 12.' @('frm1600WP:DateWithholdingMonth') $null @('official-hta-runtime#validate:L3163-L3219') 'incorrect-official-behavior' 'The value bypasses both month arrays and is accepted.' 'Parse and validate a real calendar date.'
Rule '1600wp-validate-011-future-date-gap' validate 11 'Return date is later than the current date.' @('frm1600WP:DateWithholdingMonth','frm1600WP:DateWithholdingDay','frm1600WP:DateWithholdingYear') $null @('official-hta-runtime#validate:L3220-L3234') 'incorrect-official-behavior' 'All future-date checks are commented out.' 'Reject dates beyond the allowed return period.'
Rule '1600wp-validate-012-amended' validate 12 'Neither amended-return radio is selected.' @('frm1600WP:AmendedReturn_1','frm1600WP:AmendedReturn_2') 'Please choose amended return on item 2.' @('official-hta-runtime#validate:L3240-L3244')
Rule '1600wp-validate-013-tax-held' validate 13 'Neither Item 4 radio is selected.' @('frm1600WP:AnyTaxHeld_1','frm1600WP:AnyTaxHeld_2') 'Please select an option for Item 4.' @('official-hta-runtime#validate:L3245-L3249')
Rule '1600wp-validate-014-tin' validate 14 'Any withholding-agent TIN segment or branch code is blank.' @('frm1600WP:txtTIN1','frm1600WP:txtTIN2','frm1600WP:txtTIN3','frm1600WP:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#validate:L3251-L3255') 'official-bug-compatible' 'Only blankness is checked.' 'Apply the shared TIN checksum and segment constraints.'
Rule '1600wp-validate-015-rdo' validate 15 'The runtime RDO select remains at index zero.' @('frm1600WP:txtRDOCode') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#validate:L3256-L3261')
Rule '1600wp-validate-016-category' validate 16 'Neither withholding-agent category is selected.' @('frm1600WP:CategoryAgent_P','frm1600WP:CategoryAgent_G') 'Please select an option for Item 7.' @('official-hta-runtime#validate:L3262-L3266')
Rule '1600wp-validate-017-name' validate 17 'Withholding-agent name is blank.' @('frm1600WP:txtTaxpayerName') "Please enter a valid Withholding Agent's Name on Item 8." @('official-hta-runtime#validate:L3267-L3271')
Rule '1600wp-validate-018-address' validate 18 'Registered address is blank.' @('frm1600WP:txtAddress') "Please enter Taxpayer's Registered Address on Item 9." @('official-hta-runtime#validate:L3272-L3276')
Rule '1600wp-validate-019-zip' validate 19 'Zip code is blank.' @('frm1600WP:txtZipCode') "Please enter Taxpayer's Zip Code on Item 10." @('official-hta-runtime#validate:L3278-L3282')
Rule '1600wp-validate-020-category-duplicate' validate 20 'Neither withholding-agent category is selected after the identical earlier check passed.' @('frm1600WP:CategoryAgent_P','frm1600WP:CategoryAgent_G') 'Please select an option for Item 7.' @('official-hta-runtime#validate:L3284-L3288') 'obsolete' 'This branch is unreachable because the identical condition already returned at order 16.' 'Keep one category check.'
Rule '1600wp-validate-021-partii-required' validate 21 'Item 4 is Yes but the Part II table has no selected ATC rows.' @('frm1600WP:AnyTaxHeld_1','frm1600WP:txtAtcCode{index}') 'Please fill up Part II Computation of Tax if item 4 is set to Yes.' @('official-hta-runtime#validate:L3289-L3295')
Rule '1600wp-validate-022-partii-base-blank' validate 22 'A Part II ATC row has blank tax base.' @('frm1600WP:txtAtcCode{index}','frm1600WP:txtTaxBase{index}') 'Please enter a valid value for tax base for {ATC}.' @('official-hta-runtime#validate:L3296-L3306')
Rule '1600wp-validate-023-partii-base-positive' validate 23 'A Part II ATC row has tax base at or below zero.' @('frm1600WP:txtAtcCode{index}','frm1600WP:txtTaxBase{index}') 'Please enter Tax Base for ATC <{ATC}>.' @('official-hta-runtime#validate:L3307-L3310')
Rule '1600wp-validate-024-partii-atc' validate 24 'A generated Part II row has blank ATC.' @('frm1600WP:txtAtcCode{index}') 'Please fill up Part II Computation of Tax if item 4 is set to Yes.' @('official-hta-runtime#validate:L3311-L3314')
Rule '1600wp-validate-025-schedule-tin' validate 25 'A used Schedule II row has blank or shorter-than-12 TIN.' @('frm1600WP:dtSched:txtTin{index}') 'Please enter a valid TIN Number for Sequence {index}.' @('official-hta-runtime#validate:L3316-L3324') 'official-bug-compatible' 'Only length is checked; checksum and structure are not.' 'Apply payee TIN structure/checksum rules.'
Rule '1600wp-validate-026-schedule-name' validate 26 'A used Schedule II row has blank payee name.' @('frm1600WP:dtSched:txtFullname{index}') 'Please enter a valid Name of Individual/Corporation for Sequence {index}.' @('official-hta-runtime#validate:L3324-L3327')
Rule '1600wp-validate-027-schedule-atc' validate 27 'A used Schedule II row has blank ATC or nature of payment.' @('frm1600WP:dtSched:drpAtcCode{index}','frm1600WP:dtSched:naturePayment{index}') 'Please select an ATC from the list for Sequence {index}.' @('official-hta-runtime#validate:L3327-L3330')
Rule '1600wp-validate-028-schedule-amount' validate 28 'A used Schedule II row has amount at or below zero.' @('frm1600WP:dtSched:amount{index}','frm1600WP:dtSched:drpAtcCode{index}') 'Please enter Tax Base for ATC {ATC}. Value must be greater than 0.' @('official-hta-runtime#validate:L3330-L3333')
Rule '1600wp-validate-029-schedule-trigger-gap' validate 29 'A Schedule II row contains only rate or computed tax-withheld data.' @('frm1600WP:dtSched:txtRatePercent{index}','frm1600WP:dtSched:taxWithheld{index}') $null @('official-hta-runtime#validate:L3316-L3321') 'incorrect-official-behavior' 'The row-use predicate repeats naturePayment and omits rate and tax-withheld.' 'Trigger row validation on any non-empty serialized row field.'
Rule '1600wp-validate-030-success' validate 30 'All prior checks pass.' @() 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validate:L3362-L3367') 'verified-correct' 'Controls are disabled after hPartIITableSize is updated.' 'Model validated state explicitly.'
Rule '1600wp-save-001-tin' save 1 'Any withholding-agent TIN segment or branch code is blank.' @('frm1600WP:txtTIN1','frm1600WP:txtTIN2','frm1600WP:txtTIN3','frm1600WP:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L3751-L3756') 'official-bug-compatible' 'Only blankness is checked.' 'Apply shared TIN validation.'
Rule '1600wp-save-002-rdo' save 2 'RDO value is 000.' @('frm1600WP:txtRDOCode') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L3757-L3760')
Rule '1600wp-save-003-name' save 3 'Withholding-agent name is blank.' @('frm1600WP:txtTaxpayerName') "Please enter a valid Withholding Agent's Name on Item 8." @('official-hta-runtime#initialValidateBeforeSave:L3761-L3765')
Rule '1600wp-save-004-sparse' save 4 'Any period, classification, address, Part II, or Schedule II Validate-only rule fails.' @() $null @('official-hta-runtime#initialValidateBeforeSave:L3751-L3767','official-hta-runtime#validate:L3157-L3368') 'incorrect-official-behavior' 'Save ignores every check except TIN blankness, RDO 000, and name blankness.' 'Use a shared validation graph with explicit phase exceptions.'
Rule '1600wp-schedule-rounding' 'blur/change' 1 'Schedule II computes amount × (rate/100 rounded to two decimals).' @('frm1600WP:dtSched:amount{index}','frm1600WP:dtSched:txtRatePercent{index}','frm1600WP:dtSched:taxWithheld{index}') $null @('official-hta-runtime#computeDtShedTaxWithheld:L3517-L3527') 'incorrect-official-behavior' 'Rates with fractional percentage points are distorted; e.g. 2.5% becomes 3% before multiplication.' 'Divide the exact decimal rate by 100, multiply, then round only the monetary result.'
Rule '1600wp-serialization-runtime-selectors' save $null 'The generic serializer reaches transient ATC-modal selectors.' @('AtcCd1','AtcCd2','SchedIIAtcCde1','SchedIIAtcCde2') $null @('official-hta-runtime#changedrpATCList:L3529-L3556','official-hta-runtime#saveXML:L2580-L2660','revision-matched-encrypted-sample') 'official-bug-compatible' 'Transient selector state is retained as ordinary XML fields.' 'Preserve exact keys for round-trip compatibility but keep them outside the typed tax model.'
Rule '1600wp-serialization-unprefixed-email' save $null 'The generic serializer reaches email.' @('txtEmail') $null @('official-hta-runtime#control:L1795','official-hta-runtime#saveXML:L2580-L2660') 'official-bug-compatible' 'The unprefixed ID txtEmail becomes the XML key.' 'Preserve the literal key with a typed alias.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    first_error_behavior='Validate and Save alert and return on the first source-ordered failure; row messages interpolate sequence or ATC.'
    rules=$rules
})

$calcs=[Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Out,[string[]]$In,[string]$Formula,[string]$Trigger,[string[]]$Deps,[string[]]$Refs,
    [string]$Assessment='verified-correct',[string]$Recommended='Implement with typed decimals and deterministic two-decimal formatting.') {
    $calcs.Add([pscustomobject][ordered]@{
        calculation_id=$Id;outputs=$Out;inputs=$In;condition=$null;official_formula=$Formula
        rounding='formatCurrency applies two decimals unless the formula documents an earlier rounding defect.'
        trigger=$Trigger;depends_on=$Deps;source_refs=$Refs;assessment=$Assessment
        recommended_app_behavior=$Recommended;confidence='high'
    })
}
Calc '1600wp-return-period-to' @('frm1600WP:DateWithholdingToMonth','frm1600WP:DateWithholdingToDay','frm1600WP:DateWithholdingToYear') @('frm1600WP:DateWithholdingMonth','frm1600WP:DateWithholdingDay','frm1600WP:DateWithholdingYear') 'Each To component mirrors its corresponding From component on blur.' 'inline onblur' @() @('official-hta-runtime#controls:L291-L301')
Calc '1600wp-partii-row-tax' @('frm1600WP:txtTaxbeWithHeld{index}') @('frm1600WP:txtTaxBase{index}','frm1600WP:txtTaxRate{index}') 'Tax required to be withheld = tax base × (rate / 100).' getRequiredWithheld @() @('official-hta-runtime#getRequiredWithheld:L3445-L3448')
Calc '1600wp-item12-total' @('frm1600WP:txtTax12') @('frm1600WP:txtTaxbeWithHeld{index}') 'Item 12 is the sum of all generated Part II tax-withheld rows.' computeofTotalWithheldTax @('1600wp-partii-row-tax') @('official-hta-runtime#computeofTotalWithheldTax:L3501-L3511')
Calc '1600wp-item14-balance' @('frm1600WP:txtTax14') @('frm1600WP:txtTax12','frm1600WP:txtTax13') 'Item 14 = Item 12 - Item 13.' computeofTotalWithheldTax @('1600wp-item12-total') @('official-hta-runtime#computeofTotalWithheldTax:L3510-L3513')
Calc '1600wp-penalty-total' @('frm1600WP:txtTax15D') @('frm1600WP:txtTax15A','frm1600WP:txtTax15B','frm1600WP:txtTax15C') 'Item 15D = surcharge + interest + compromise.' computePenalties @() @('official-hta-runtime#computePenalties:L3485-L3492')
Calc '1600wp-total-amount-due' @('frm1600WP:txtTax16') @('frm1600WP:txtTax14','frm1600WP:txtTax15D') 'Item 16 = Item 14 + Item 15D.' computeOfTotalAmtDue @('1600wp-item14-balance','1600wp-penalty-total') @('official-hta-runtime#computeOfTotalAmtDue:L3493-L3499')
Calc '1600wp-schedule-row-tax' @('frm1600WP:dtSched:taxWithheld1..10') @('frm1600WP:dtSched:amount1..10','frm1600WP:dtSched:txtRatePercent1..10') 'For each row, tax = amount × round(rate / 100, 2), then the money result is rounded to two decimals.' computeDtShedTaxWithheld @() @('official-hta-runtime#computeDtShedTaxWithheld:L3517-L3527') 'incorrect-official-behavior' 'Use exact decimal rate / 100 and round only the monetary output.'
Calc '1600wp-schedule-total' @('frm1600WP:dtSched:TotaltaxWithheld') @('frm1600WP:dtSched:taxWithheld1..10') 'Sum the ten Schedule II tax-withheld outputs, rounding the running total to two decimals each iteration.' computeDtShedTaxWithheld @('1600wp-schedule-row-tax') @('official-hta-runtime#computeDtShedTaxWithheld:L3519-L3527') 'official-bug-compatible' 'Sum exact row-money values once and round the final total unless compatibility requires iterative rounding.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    evaluation_order=@($calcs.calculation_id);calculations=$calcs
})
$cases=@();$number=0
foreach($rule in @($rules|Where-Object{$_.exact_message})) {
    $number++
    $cases += [pscustomobject][ordered]@{
        case_id=('case-{0:d2}-{1}'-f$number,$rule.rule_id);phase=$rule.phase
        mutations=@{synthetic_condition=$rule.condition};expected_message=$rule.exact_message
        expected_behavior=$rule.official_behavior;rule_id=$rule.rule_id
    }
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases
})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;cases=@(
        @{case_id='partii-2.5-percent';calculation_id='1600wp-partii-row-tax';base=1000;rate=2.5;official_output=25},
        @{case_id='schedule-2.5-percent-defect';calculation_id='1600wp-schedule-row-tax';base=1000;rate=2.5;official_output=30;recommended_output=25},
        @{case_id='item14-overremittance';calculation_id='1600wp-item14-balance';item12=100;item13=125;official_output=-25},
        @{case_id='penalties';calculation_id='1600wp-penalty-total';surcharge=25;interest=10;compromise=5;official_output=40}
    )
})
$resources=@()
foreach($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1')|ForEach-Object{$_.Groups['v'].Value}|Sort-Object -Unique)) {
    $full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if(Test-Path -LiteralPath $full) {
        $resources += [pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}
    } else {
        $resources += [pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}
    }
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$workflow=[ordered]@{
    '$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    phases=@(
        @{phase='edit';official_behavior='January 2010 race-track-operator return with dynamic Part II ATC rows and ten fixed Schedule II payee rows.';source_refs=@('official-hta-runtime','official-form-pdf','official-help-runtime');confidence='high'},
        @{phase='saved-draft';official_behavior='Save checks only TIN blankness, RDO 000, and name blankness, then serializes every current form control including transient ATC selectors and generated Part II rows.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L3751-L3767','official-hta-runtime#saveXML:L2417-L2723');confidence='high'},
        @{phase='validated';official_behavior='Validate checks the return date, identity/classification, selected Part II ATCs, and used Schedule II rows in source order.';source_refs=@('official-hta-runtime#validate:L3157-L3368');confidence='high'},
        @{phase='final-copy';official_behavior='The revision-matched encrypted dummy copy proves 115 concrete keys with no selected Part II rows.';source_refs=@('revision-matched-encrypted-sample','encrypted-field-audit');confidence='high'},
        @{phase='submitted';official_behavior='Online transport exists but was not exercised.';source_refs=@('official-hta-runtime#sendEmail');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='Three narrow Save checks and file-version guards pass.';side_effects=@('Writes flat pseudo-XML.','Serializes generated ATC selectors and Part II rows currently in the form.');source_refs=@('official-hta-runtime#saveXML:L2417-L2723')},
        @{from='edit';action='Validate';to='validated';guard='All source-ordered checks pass.';side_effects=@('Stores the Part II row count.','Disables editable controls.');source_refs=@('official-hta-runtime#validate:L3157-L3368')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables controls and applicable ATC selection actions.');source_refs=@('official-hta-runtime#enableAllControl:L3065-L3118')},
        @{from='validated';action='Final Copy';to='final-copy';guard='Finalization/version flow succeeds.';side_effects=@('Writes and encrypts/compresses the final copy.');source_refs=@('official-hta-runtime#saveXML:L2417-L2723')},
        @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and remote acceptance succeed.';side_effects=@('Online submission attempt; deliberately untested.');source_refs=@('official-hta-runtime#sendEmail')}
    )
    prerequisites=@('Return date','Amended-return and tax-withheld choices','Withholding-agent identity/RDO/category','Part II ATC and tax base when tax was withheld','Complete used Schedule II payee rows')
    required_attachments=@(
        @{attachment_id='alphabetical-payee-list';label='Alphabetical list of payees with tax period, TIN, name, ATC, nature, amount, rate, and tax withheld when the return cannot accommodate all entries.';required_when='The payee list cannot be accommodated in the return.';official_ui_enforcement='Ten rows are validated locally; overflow attachment presence is not checked.';source_refs=@('official-help-runtime#L182-L196');confidence='high'},
        @{attachment_id='deposit-slip';label='BIR-prescribed deposit slip when filing with an Authorized Agent Bank.';required_when='Payment is made through an AAB.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-runtime#L97-L108');confidence='high'}
    )
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='This is not a quarterly return; the reviewed runtime help omits the actual ordinary filing deadline after repeating its heading.';source_refs=@('official-help-runtime#L97-L116');confidence='low'},
        @{quarter='Q2';due_date_rule='This is not a quarterly return; the reviewed runtime help omits the actual ordinary filing deadline after repeating its heading.';source_refs=@('official-help-runtime#L97-L116');confidence='low'},
        @{quarter='Q3';due_date_rule='This is not a quarterly return; the reviewed runtime help omits the actual ordinary filing deadline after repeating its heading.';source_refs=@('official-help-runtime#L97-L116');confidence='low'},
        @{quarter='Q4';due_date_rule='This is not a quarterly return; the reviewed runtime help omits the actual ordinary filing deadline after repeating its heading.';source_refs=@('official-help-runtime#L97-L116');confidence='low'}
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugs=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1600WP; printed January 2010 ENCS.'
    Asset 'official-help-runtime' 'official-runtime-help' $helpPath 'Form-specific who-must-file and attachment guidance.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2010 ENCS blank form.'
    Asset 'official-guide-pdf' 'official-guide-pdf' $guidePath 'Local official 1600WP guide.'
    Asset 'revision-matched-encrypted-sample' 'dummy-profile-encrypted-final-copy' $samples[0].FullName '115-key concrete inventory exactly matches runtime controls; no selected Part II rows.' (Join-Path $SampleDir '1600WP-final-copy-#email-redacted#.xml')
)
$manifest=[ordered]@{
    '$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='1600WP'
    revision=$revision;package_version=$packageVersion;status='complete';official_assets=$assets
    counts=[ordered]@{
        concrete_fields=115;runtime_field_families=4;fields_total=$fields.Count;typed_fields=$fields.Count
        validation_rules=$rules.Count;confirmed_official_bugs=$bugs;calculations=$calcs.Count
        negative_fixtures=$cases.Count;unverified_gaps=3
    }
    artifacts=[ordered]@{
        fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json'
        evidence='evidence.md';audit='audit.md';gaps='gaps.md'
        runtime_control_fixture='fixtures/runtime-control-inventory-v796.json'
        encrypted_field_audit='fixtures/encrypted-field-audit.json'
        validation_function_fixture='fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture='fixtures/calculation-function-inventory-v796.json'
        resource_hash_fixture='fixtures/official-resource-hashes-v796.json'
        negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'
    }
    scope_notes=@(
        'Research only; no renderer or release metadata changed.',
        'No decrypted values or email-bearing filenames are emitted.',
        'The 115 concrete runtime keys exactly reproduce the encrypted sample inventory hash.',
        'Four selected-ATC Part II families preserve conditional controls absent from the zero-selected-ATC sample.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') @"
# BIR Form 1600WP - January 2010 ENCS

Revision-specific validation package for the Remittance Return of Percentage Tax on Winnings and Prizes Withheld by Race Track Operators.

The package preserves 115 concrete serialized keys plus four conditional Part II families. No taxpayer values or identifying filename text are included.
"@
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- Runtime HTA SHA-256: $($expected.hta); `APPLICATIONNAME="1600WP"`; printed January 2010 ENCS.
- Runtime help SHA-256: $($expected.help); identifies Form 1600WP, filer population, AAB deposit slip, and alphabetical payee-list attachment.
- Official blank form PDF SHA-256: $($expected.pdf); valid PDF magic.
- Official guide PDF SHA-256: $($expected.guide); valid PDF magic.
- Revision-matched encrypted dummy final copy: ciphertext SHA-256 $($expected.sample_cipher); decrypted SHA-256 $($expected.sample_plain); 115 unique keys; inventory SHA-256 $($expected.sample_inventory). Values were not emitted.
- DOM reconciliation: 110 static serialized controls plus RDO and four generated ATC selectors exactly reproduce the sample inventory.
- Four conditional Part II families are source-proven but absent from the sample because it contains no selected ATC rows.

All email-bearing filenames are represented as `#email-redacted#`.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. The runtime help repeats the “When and Where” heading but omits the ordinary filing deadline; no unsupported deadline is inferred.
2. The reviewed encrypted sample has no selected Part II ATC rows, so the four generated field families are source-proven rather than sample-observed.
3. Online submission was deliberately not exercised.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- Exact January 2010 revision pinned: pass.
- HTA/help/form/guide/package hashes pinned: pass.
- Encrypted 115-key inventory pinned without values: pass.
- Runtime reconciliation: pass (110 static + 5 injected = 115; exact hash).
- Conditional Part II families preserved: pass (4).
- Typed inventory: pass ($($fields.Count)).
- Validation and calculation inventories: pass.
- Confirmed official defects: $bugs.
- Negative fixtures: $($cases.Count).
- JSON structural/schema audit: run `rules/validate.ps1 -RequireJsonSchema` after generation.
- Scope: no renderer, migration, release, capability, commit, or push changes.
"@

$indexPath=Join-Path $RepoRoot 'rules\index.json'
$index=Get-Content -Raw -LiteralPath $indexPath|ConvertFrom-Json
$entry=$index.forms|Where-Object{$_.form_id-eq$formId}
if($entry) {
    $entry.form_code='1600WP';$entry.revision=$revision;$entry.package_version=$packageVersion
    $entry.priority=25;$entry.status='complete';$entry.path='forms/1600wp-v2010/manifest.json'
} else {
    $index.forms += [pscustomobject][ordered]@{
        form_id=$formId;form_code='1600WP';revision=$revision;package_version=$packageVersion
        priority=25;status='complete';path='forms/1600wp-v2010/manifest.json'
    }
}
$index.forms=@($index.forms|Sort-Object priority);$index.updated='2026-07-23'
Write-Json $indexPath $index

[pscustomobject]@{
    form_id=$formId;concrete_fields=115;field_families=4;typed_fields=$fields.Count
    validations=$rules.Count;calculations=$calcs.Count;negative_fixtures=$cases.Count
    confirmed_official_bugs=$bugs;encrypted_inventory_match=$true;next_form='1702ex'
}|ConvertTo-Json
