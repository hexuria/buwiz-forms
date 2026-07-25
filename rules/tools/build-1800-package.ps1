param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$PdfDir = 'C:\Mac\Home\Downloads\forms\1800v2018',
    [string]$LegacySaveDir = 'C:\eBIRForms\savefile',
    [string]$LegacyFinalDir = 'C:\eBIRForms\IAF_RDO_Copy'
)

$ErrorActionPreference = 'Stop'
$formId = '1800-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1800v2018.hta'
$legacyHtaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1800.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1800.hta'
$pdfPath = Join-Path $PdfDir '1800 Jan 2018 ENCS rev final.pdf'
$guidelinesPath = Join-Path $PdfDir '1800 Guidelines.pdf'
$legacySavePath = Join-Path $LegacySaveDir '00000000000000-1800-07222026.xml'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1800-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'

$expected = @{
    hta = '58b1ff5157f6d66f1ed1252c696d13ab9afe090bae0c1793faf42ef104159d37'
    legacy_hta = '0b69656574f35c1b13c0cca56fc59670ace3525453ae3d765c2a0991197a81ed'
    help = '1ef140b7d7584043163c6d37020dc34266ebbd293cff6f9adb341263cb70f9ea'
    pdf = 'e2e837852680196d0e9aa9a513f55c7a6e4924493b5440548e46217166cc085e'
    guidelines = 'b8fa70696bea4a50aae94b9009b4281121112e9928b49eb156f8953949ae7cb8'
    legacy_save = '3a1aca0a1ca402fa408299f000f9672e6bcf48aba65856fedc421a51af0892a5'
    legacy_final = 'ef2d1e1854334ce08e034db74eea80292f99d330c53adfb4a59f38340b78e922'
    legacy_inventory = '2b71d0b7ff2a8de16c0bf406e2a4717e065401e05a1bf8ac31772f0342317c9f'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function Get-HashText([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-', '').ToLowerInvariant()
    } finally { $sha.Dispose() }
}
function Get-Attr([string]$Tag, [string]$Name) {
    $match = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($match.Success) { $match.Groups[2].Value } else { $null }
}
function Asset([string]$Id, [string]$Kind, [string]$Path, [string]$Binding, [string]$DisplayPath = '') {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $Id
        kind = $Kind
        path = if ($DisplayPath) { $DisplayPath } else { $Path }
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        size = $item.Length
        revision_binding = $Binding
    }
}
function Find-FileByHash([string]$Directory, [string]$Hash) {
    $matches = @(Get-ChildItem -LiteralPath $Directory -File | Where-Object {
        (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() -eq $Hash
    })
    if ($matches.Count -ne 1) { throw "Expected one file with SHA-256 $Hash; found $($matches.Count)." }
    $matches[0].FullName
}

foreach ($path in @($htaPath,$legacyHtaPath,$helpPath,$pdfPath,$guidelinesPath,$legacySavePath,$packagePath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
$legacyFinalPath = Find-FileByHash $LegacyFinalDir $expected.legacy_final
foreach ($pair in @(
    @($htaPath,'hta'),@($legacyHtaPath,'legacy_hta'),@($helpPath,'help'),@($pdfPath,'pdf'),
    @($guidelinesPath,'guidelines'),@($legacySavePath,'legacy_save'),@($legacyFinalPath,'legacy_final'),
    @($packagePath,'package')
)) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
foreach ($pdf in @($pdfPath,$guidelinesPath)) {
    $bytes = [IO.File]::ReadAllBytes($pdf)
    if ([Text.Encoding]::ASCII.GetString($bytes[0..4]) -ne '%PDF-') { throw "PDF magic mismatch: $pdf" }
}

$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']1800v2018["'']') { throw 'APPLICATIONNAME mismatch.' }
if ($hta -notmatch '(?i)January\s+2018\s+\(ENCS\)') { throw 'Printed revision label is absent.' }
if ($help -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']1800["'']') { throw 'Expected legacy help binding changed.' }
if ($help -notmatch '(?i)thirty\s+percent\s+\(30%\)') { throw 'Expected pre-2018 tax-rate text changed.' }

New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

$legacyText = [IO.File]::ReadAllText($legacySavePath)
$legacyMatches = @([regex]::Matches($legacyText, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>'))
$legacyKeys = @($legacyMatches | ForEach-Object { $_.Groups['key'].Value })
if ($legacyKeys.Count -ne 89 -or ($legacyKeys | Sort-Object -Unique).Count -ne 89) { throw 'Legacy save key count changed.' }
if ((Get-HashText @($legacyKeys | Sort-Object)) -ne $expected.legacy_inventory) { throw 'Legacy save inventory changed.' }
if (@($legacyKeys | Where-Object { $_ -like 'frm1800v2018:*' }).Count -gt 0 -or
    @($legacyKeys | Where-Object { $_ -like 'frm1800:*' }).Count -eq 0) {
    throw 'Legacy sample revision-prefix classification changed.'
}

$formMatch = [regex]::Match($hta, '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain missing.' }
$body = $formMatch.Groups['body'].Value
$bodyOffset = $formMatch.Groups['body'].Index
$excludedRanges = @(
    @([regex]::Matches($body, '(?is)<script\b.*?</script>'))
    @([regex]::Matches($body, '(?is)<!--.*?-->'))
)
$controls = [Collections.Generic.List[object]]::new()
$ordinal = 0
foreach ($match in [regex]::Matches($body, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $excluded = $false
    foreach ($range in $excludedRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) { $excluded = $true; break }
    }
    if ($excluded) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $controls.Add([pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Get-Attr $tag 'id'
        name = Get-Attr $tag 'name'
        element = $element
        control_kind = $kind.ToLowerInvariant()
        source_line = 1 + [regex]::Matches($hta.Substring(0, $bodyOffset + $match.Index), "`n").Count
        value = Get-Attr $tag 'value'
        maxlength = Get-Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}
if ($controls.Count -ne 184) { throw "Expected 184 live controls after excluding comments/scripts; found $($controls.Count)." }
$staticSerial = @($controls | Where-Object { $_.control_kind -in @('text','select','select-one','textarea','radio','checkbox') })
if ($staticSerial.Count -ne 167 -or @($staticSerial.id | Sort-Object -Unique).Count -ne 167) {
    throw "Expected 167 unique static serializer controls; found $($staticSerial.Count)/$(@($staticSerial.id | Sort-Object -Unique).Count)."
}
$rdoControl = [pscustomobject][ordered]@{
    ordinal = 185; id = 'frm1800v2018:txtRDOCode'; name = 'frm1800v2018:txtRDOCode'
    element = 'select'; control_kind = 'select-one'; source_line = 5059
    value = '000'; maxlength = $null; disabled = $true; readonly = $false
}
$runtimeSerial = @($staticSerial) + @($rdoControl)
$projectedSerial = @($runtimeSerial | Where-Object { $_.id -notin @('frm1800v2018:txtRegAddress2','frm1800v2018:txtResAddress2') })
if ($runtimeSerial.Count -ne 168 -or $projectedSerial.Count -ne 166) { throw 'Projected serializer count changed.' }

$families = @(
    @{ pattern='frm1800v2018:schedA:txtParticulars{index}'; initial_indices='0..4'; kind='string'; source='official-hta-runtime#addScheduleA:L3944-L3969' },
    @{ pattern='frm1800v2018:schedA:txtFairMarketValue{index}'; initial_indices='0..4'; kind='decimal-amount'; source='official-hta-runtime#addScheduleA:L3944-L3969' },
    @{ pattern='frm1800v2018:schedB1:txtOCT{index}'; initial_indices='0..4'; kind='string'; source='official-hta-runtime#addScheduleB1:L4209-L4240' },
    @{ pattern='frm1800v2018:schedB1:txtTaxDecNo{index}'; initial_indices='0..4'; kind='string'; source='official-hta-runtime#addScheduleB1:L4209-L4240' },
    @{ pattern='frm1800v2018:schedB1:txtLocation{index}'; initial_indices='0..4'; kind='string'; source='official-hta-runtime#addScheduleB1:L4209-L4240' },
    @{ pattern='frm1800v2018:schedB1:txtLotImprovement{index}'; initial_indices='0..4'; kind='string'; source='official-hta-runtime#addScheduleB1:L4209-L4240' },
    @{ pattern='frm1800v2018:schedB1:txtClassification{index}'; initial_indices='0..4'; kind='string'; source='official-hta-runtime#addScheduleB1:L4209-L4240' },
    @{ pattern='frm1800v2018:schedB2:txtArea{index}'; initial_indices='0..4'; kind='decimal-or-string'; source='official-hta-runtime#addScheduleB2:L4087-L4118' },
    @{ pattern='frm1800v2018:schedB2:txtFTD{index}'; initial_indices='0..4'; kind='string'; source='official-hta-runtime#addScheduleB2:L4087-L4118' },
    @{ pattern='frm1800v2018:schedB2:txtFMVperTDperBIRZonalValue{index}'; initial_indices='0..4'; kind='string'; source='official-hta-runtime#addScheduleB2:L4087-L4118' },
    @{ pattern='frm1800v2018:schedB2:txtFairMarketValue{index}'; initial_indices='0..4'; kind='decimal-amount'; source='official-hta-runtime#addScheduleB2:L4087-L4118' }
)

$requiredKeys = @(
    'frm1800v2018:txtMonth','frm1800v2018:txtDate','frm1800v2018:txtYear',
    'frm1800v2018:txtTIN1','frm1800v2018:txtTIN2','frm1800v2018:txtTIN3',
    'frm1800v2018:txtBranchCode','frm1800v2018:txtRDOCode','frm1800v2018:txtDonorName'
)
$itemMap = @{
    txtMonth='1'; txtDate='1'; txtYear='1'; AmendedRtn_1='2'; AmendedRtn_2='2'; txtSheets='3'; txtATC='4'
    txtTIN1='5'; txtTIN2='5'; txtTIN3='5'; txtBranchCode='5'; txtRDOCode='6'; txtDonorName='7'
    txtRegAddress='8'; txtZipCode='8'; txtResAddress='9'; txtZipCode2='9'; txtContact='10'; txtEmail='11'
    txtDoneeNameA='12'; txtDoneeTINA='12'; txtDoneeNameB='12'; txtDoneeTINB='12'; txtDoneeNameC='12'; txtDoneeTINC='12'
    txtDoneeNameD='12'; txtDoneeTIND='12'; txtDoneeNameE='12'; txtDoneeTINE='12'
    TaxTreatyYN_Y='13'; TaxTreatyYN_N='13'; TaxTreaty='13A'; TotalNetGifts='14'; DonorTaxDue='16'
    TaxCredit17A='17A'; TaxCredit17B='17B'; TaxCredit17C='17C'; TaxCredit17D='17D'; txtTaxPayable='18'
    Surcharge='19A'; Interest='19B'; Compromise='19C'; TotalPenalties='19D'; TotalAmountPayable='20'
    PersonalProperties='25'; RealProperties='26'; TotalGifts='27'; txtDeductionTitle28='28'; txtDeductionAmount28='28'
    txtDeductionTitle29='29'; txtDeductionAmount29='29'; txtDeductionTitle30='30'; txtDeductionAmount30='30'
    txtDeductionTitle31='31'; txtDeductionAmount31='31'; txtDeductionTitle32='32'; txtDeductionAmount32='32'
    TotalDeductionsAllowed='33'; TotalReturnNetGifts='34'; TotalPriorNetGifts='35'; TotalNetGifts36='36'
    TotalNetGiftsSubjectToTax='38'
}
$computedPattern = '(?i)(TotalNetGifts$|DonorTaxDue|TaxCredit17D|txtTaxPayable|TotalPenalties|TotalAmountPayable|PersonalProperties|RealProperties|TotalGifts$|TotalDeductionsAllowed|TotalReturnNetGifts|TotalNetGifts36|TotalNetGiftsSubjectToTax|txtTotal)'
$amountPattern = '(?i)(Gift|Tax|Credit|Surcharge|Interest|Compromise|Penalt|Amount|Deduction|FairMarketValue|PersonalProperties|RealProperties)'

$fields = [Collections.Generic.List[object]]::new()
foreach ($control in $projectedSerial) {
    $key = $control.id
    $short = if ($key -like 'frm1800v2018:*') { $key.Substring(14) } else { $key }
    $logical = 'string'
    $enum = [object[]]@()
    $normalization = [string[]]@()
    if ($control.control_kind -in @('radio','checkbox')) { $logical='boolean'; $enum=[object[]]@('true','false') }
    elseif ($key -match '(?i)(TIN|RDO|BranchCode|ZipCode)') { $logical='code' }
    elseif ($key -match '(?i)(txtMonth|txtDate|txtYear)$') { $logical='date-component-string' }
    elseif ($key -match $amountPattern) { $logical='decimal-amount'; $normalization=[string[]]@('NumWithComma','formatCurrency','round(...,2)') }
    elseif ($key -match '(?i)Email') { $logical='email-string' }
    $computed = $key -match $computedPattern
    $status = if ($requiredKeys -contains $key) { 'required' } elseif ($computed) { 'computed' } else { 'optional' }
    if ($key -match '(?i)(txtCurrentPage|txtMaxPage|modLabel|numOfDays|rowLocation)') { $status='hidden' }
    if ($key -eq 'frm1800v2018:TaxTreaty') { $status='conditional' }
    $constraints = [ordered]@{}
    if ($control.maxlength -and $control.maxlength -match '^\d+$') { $constraints.max_length=[int]$control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision=2; $constraints.sign='official UI does not consistently reject negative computed results' }
    $item = if ($itemMap.ContainsKey($short)) { $itemMap[$short] } else { $null }
    $page = if ($key -match '(?i)(txtPg2|sched|Deduction|PersonalProperties|RealProperties|TotalGifts|TotalReturn|TotalPrior|TotalNetGifts36|SubjectToTax)') { 2 } else { 1 }
    $notes = @('Source-derived from the January 2018 live DOM and Save serializer; no revision-matched save artifact was available.')
    if ($key -eq 'frm1800v2018:txtRegAddress') { $notes += 'Serialized value concatenates txtRegAddress and txtRegAddress2 under this one key.' }
    if ($key -eq 'frm1800v2018:txtResAddress') { $notes += 'Serialized value concatenates txtResAddress and txtResAddress2 under this one key.' }
    $fields.Add([pscustomobject][ordered]@{
        field_key=$key; serialized_key=$key; serialized_occurrence=1; label=$short; page=$page; item_number=$item
        control_kind=$control.control_kind; storage_type='string'; logical_type=$logical; required=$status
        required_when=if($key -eq 'frm1800v2018:TaxTreaty'){'Item 13 Yes.'}else{$null}
        enabled_when=if($key -eq 'frm1800v2018:TaxTreaty'){'Item 13 Yes.'}elseif($key -eq 'frm1800v2018:TaxCredit17C'){'Amended Return Yes.'}else{$null}
        visible_when=$null; default_value=$control.value; empty_representation=''; constraints=[pscustomobject]$constraints
        enum_values=$enum; normalization=$normalization; computed=$computed
        calculation_id=if($computed){'See calculations.json'}else{$null}
        source_refs=@("official-hta-runtime#control:L$($control.source_line)",'official-hta-runtime#saveXML:L3271-L3430')
        confidence='high'; notes=$notes
    })
}
foreach ($family in $families) {
    $fields.Add([pscustomobject][ordered]@{
        field_key=$family.pattern; serialized_key=$family.pattern; serialized_occurrence=$null
        label=$family.pattern.Substring(14); page=2; item_number=$null; control_kind='runtime-indexed-text'
        storage_type='string'; logical_type=$family.kind; required='optional'; required_when=$null; enabled_when=$null
        visible_when=$null; default_value=$null; empty_representation=''; constraints=[pscustomobject]@{ index='zero-based, unbounded by source Add routine' }
        enum_values=[object[]]@(); normalization=[string[]]@(); computed=$false; calculation_id=$null
        source_refs=@($family.source,'official-hta-runtime#saveXML:L3271-L3430'); confidence='high'
        notes=@("Initial live indices are $($family.initial_indices); Add/Delete rebuild contiguous zero-based rows.")
    })
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema'='../../schema/fields.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    field_count=$fields.Count; runtime_serializable_element_count=166
    inventory_sha256=Get-HashText @($fields.field_key | Sort-Object); fields=$fields
})

Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0'; form_id=$formId; official_hta_sha256=$expected.hta
    live_static_control_count=$controls.Count; static_serializer_control_count=$staticSerial.Count
    runtime_injected_control_count=1; projected_baseline_serializer_entry_count=166
    address_collapse=[ordered]@{
        'frm1800v2018:txtRegAddress'=@('frm1800v2018:txtRegAddress','frm1800v2018:txtRegAddress2')
        'frm1800v2018:txtResAddress'=@('frm1800v2018:txtResAddress','frm1800v2018:txtResAddress2')
    }
    controls=@($controls)+@($rdoControl); dynamic_families=$families
})
Write-Json (Join-Path $fixtureDir 'legacy-artifact-exclusion.json') ([ordered]@{
    schema_version='1.0.0'; target_form_id=$formId
    plaintext_sha256=$expected.legacy_save; encrypted_sha256=$expected.legacy_final
    plaintext_key_count=89; plaintext_inventory_sha256=$expected.legacy_inventory
    observed_prefix='frm1800: plus unprefixed legacy metadata/schedule keys'; required_prefix='frm1800v2018:'
    values_emitted=$false; disposition='excluded from target revision field evidence'
    source_paths=@(
        (Join-Path $LegacySaveDir '00000000000000-1800-07222026.xml'),
        (Join-Path $LegacyFinalDir '1800-final-copy-#email-redacted#.xml')
    )
})

$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1800v2018:' -NamePattern '(?i)valid|check|enable|disable|save|submit|final|date|tin') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1800v2018:' -NamePattern '(?i)compute|schedule|gift|tax|credit|penalt|total') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule(
    [string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$FieldKeys,$Message,[string[]]$Refs,
    [string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',
    [string]$Recommended='Retain as a structured revision-aware error.',[string]$Confidence='high'
) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id; form_id=$formId; revision=$revision; phase=$Phase; order=$Order; condition=$Condition
        fields=$FieldKeys; accepted_behavior='Condition is false; processing continues.'
        rejected_behavior='The active operation stops unless official_behavior states otherwise.'
        exact_message=$Message; source_refs=$Refs; evidence_type=@('source'); assessment=$Assessment
        official_behavior=$Official; recommended_app_behavior=$Recommended; confidence=$Confidence; unresolved_questions=@()
    })
}

Rule '1800-save-001-future-date' 'save' 1 'Donation date compares later than the current date.' @('frm1800v2018:txtMonth','frm1800v2018:txtDate','frm1800v2018:txtYear') 'Date of donation should not be later than the current date.' @('official-hta-runtime#saveXML:L3118','official-hta-runtime#checkDate1:L4620-L4648')
Rule '1800-save-002-date-required' 'save' 2 'Month or day is 00, or year is blank.' @('frm1800v2018:txtMonth','frm1800v2018:txtDate','frm1800v2018:txtYear') 'Please enter a valid date on Item 1.' @('official-hta-runtime#checkDate1:L4636-L4640')
Rule '1800-save-003-year-width' 'save' 3 'Year value has fewer than four characters.' @('frm1800v2018:txtYear') 'Please enter a valid Year(YYYY)' @('official-hta-runtime#checkDate1:L4641-L4644')
Rule '1800-save-004-redundant-return-date' 'save' 4 'All three date values equal the empty string.' @('frm1800v2018:txtMonth','frm1800v2018:txtDate','frm1800v2018:txtYear') 'Please enter a valid Return Date' @('official-hta-runtime#initialValidateBeforeSave:L5072-L5076') 'obsolete' 'checkDate1 runs first and the select defaults are 00, so this branch is not the operative missing-date check.' 'Use one typed date validator.'
Rule '1800-save-005-broken-year-property' 'save' 5 'The code compares the DOM element length property, not txtYear.value.length, with 4.' @('frm1800v2018:txtYear') 'Please enter a valid Year(YYYY)' @('official-hta-runtime#initialValidateBeforeSave:L5077-L5080') 'incorrect-official-behavior' 'The element length is undefined, making this duplicate branch ineffective.' 'Validate the value once.'
Rule '1800-save-006-tin' 'save' 6 'Any donor TIN segment or branch code is blank.' @('frm1800v2018:txtTIN1','frm1800v2018:txtTIN2','frm1800v2018:txtTIN3','frm1800v2018:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L5081-L5085') 'incorrect-official-behavior' 'Only blankness is checked.' 'Validate exact segment lengths, characters, and checksum before finalization.'
Rule '1800-save-007-rdo' 'save' 7 'RDO value is literal 000.' @('frm1800v2018:txtRDOCode') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L5086-L5089')
Rule '1800-save-008-donor-name' 'save' 8 'Donor name is blank.' @('frm1800v2018:txtDonorName') "Please enter a valid Taxpayer's Name on Item 8." @('official-hta-runtime#initialValidateBeforeSave:L5090-L5094') 'incorrect-official-behavior' 'The message cites Item 8, but the donor name is printed Item 7.' 'Use the correct printed item and donor terminology.'
Rule '1800-save-009-version-guard' 'save' 9 'A finalized non-amended version already exists but Amended Return is not Yes.' @('frm1800v2018:AmendedRtn_1') "If your intention is to make an amended return, kindly tick Amended Return to 'Yes' before hitting 'Save' or 'Final Copy' or 'Submit'." @('official-hta-runtime#saveXML:L3195-L3266') 'official-bug-compatible' 'Version/file-state logic blocks overwrite and may create a V-suffixed amended file.' 'Preserve immutable finalized versions with an explicit replacement-version workflow.'
Rule '1800-save-010-address-collapse' 'save' 10 'Save serializes registered and residential address halves.' @('frm1800v2018:txtRegAddress','frm1800v2018:txtRegAddress2','frm1800v2018:txtResAddress','frm1800v2018:txtResAddress2') $null @('official-hta-runtime#saveXML:L3280-L3299') 'official-bug-compatible' 'Each pair is concatenated without a delimiter under the first control ID; the second ID is not emitted.' 'Preserve both typed lines and reproduce the official flattened value only at compatibility boundaries.'

$order=0
function V([string]$Suffix,[string]$Condition,[string[]]$Fields,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The branch alerts and returns.',[string]$Recommended='Retain with revision-aware wording.') {
    $script:order++
    Rule "1800-validate-$Suffix" 'validate' $script:order $Condition $Fields $Message $Refs $Assessment $Official $Recommended
}
V '001-treaty-description' 'Item 13 Yes is checked and Item 13A is blank.' @('frm1800v2018:TaxTreatyYN_Y','frm1800v2018:TaxTreaty') 'Please specify the Special Treaty or International Law the payee is availing in item 11A.' @('official-hta-runtime#validate:L4674-L4678') 'incorrect-official-behavior' 'The check is active, but the message says payee and Item 11A instead of donor and Item 13A.' 'Require Item 13A with accurate form terminology.'
V '002-donor-tin' 'Any donor TIN segment or branch code is blank.' @('frm1800v2018:txtTIN1','frm1800v2018:txtTIN2','frm1800v2018:txtTIN3','frm1800v2018:txtBranchCode') 'Please enter a valid TIN number on Item 4.' @('official-hta-runtime#validate:L4680-L4684') 'incorrect-official-behavior' 'The message cites Item 4; donor TIN is Item 5, and only blankness is checked.' 'Validate shape/checksum and cite Item 5.'
V '003-rdo' 'RDO selectedIndex is zero.' @('frm1800v2018:txtRDOCode') 'Please enter a valid RDO Code on Item 5.' @('official-hta-runtime#validate:L4685-L4690') 'incorrect-official-behavior' 'The message cites Item 5; RDO is Item 6.' 'Cite Item 6.'
V '004-future-date' 'Donation date compares later than the current date.' @('frm1800v2018:txtMonth','frm1800v2018:txtDate','frm1800v2018:txtYear') 'Date of donation should not be later than the current date.' @('official-hta-runtime#checkDate1:L4620-L4635')
V '005-date-required' 'Month or day is 00, or year is blank.' @('frm1800v2018:txtMonth','frm1800v2018:txtDate','frm1800v2018:txtYear') 'Please enter a valid date on Item 1.' @('official-hta-runtime#checkDate1:L4636-L4640')
V '006-year-width' 'Year has fewer than four characters.' @('frm1800v2018:txtYear') 'Please enter a valid Year(YYYY)' @('official-hta-runtime#checkDate1:L4641-L4644')
V '007-success' 'All active checks pass.' @('return-body') "Validation successful. Click on 'Edit' if you wish to modify your entries." @('official-hta-runtime#validate:L4728-L4735')
V '008-calendar-date-gap' 'Month/day is impossible, such as 02/31, but is not future.' @('frm1800v2018:txtMonth','frm1800v2018:txtDate','frm1800v2018:txtYear') $null @('official-hta-runtime#checkDate1:L4620-L4648') 'incorrect-official-behavior' 'The active validator never checks month-specific day bounds or leap years.' 'Parse and validate a real calendar date.'
V '009-year-lower-bound-commented' 'Donation year is before 2018.' @('frm1800v2018:txtYear') 'Please file using the old version of the form.' @('official-hta-runtime#validate:L4654-L4672') 'obsolete' 'The entire revision-year gate is commented out.' 'Bind the form revision to the legally applicable donation date.'
V '010-year-nonnumeric' 'A four-character nonnumeric year is pasted or otherwise reaches validation.' @('frm1800v2018:txtYear') $null @('official-hta-runtime#checkDate1:L4620-L4648','official-hta-runtime#control:L349') 'incorrect-official-behavior' 'Only width and coercive comparisons are checked; the duplicate onkeypress attributes are unreliable evidence of complete numeric enforcement.' 'Parse four ASCII digits explicitly.'
V '011-line-business-commented' 'Line of business/occupation is blank.' @('frm1800v2018:txtLineBus') 'Please enter a valid Line of Business/Occupation on Item 7.' @('official-hta-runtime#validate:L4691-L4695') 'obsolete' 'The branch is commented out and Item 7 is donor name in this revision.' 'Do not revive stale item mappings; determine whether the field belongs to the 2018 filing.'
V '012-other-party-commented' 'No removed Other Party option is selected.' @('AAAAAAAAAA_1','AAAAAAAAAA_2','AAAAAAAAAA_3') 'Please select an option in Item 10.' @('official-hta-runtime#validate:L4697-L4704') 'obsolete' 'The controls and validation are commented out.' 'Exclude removed controls from the active model.'
V '013-donee-tin-commented' 'A removed Other Party branch is selected and donee TIN is blank.' @('frm1800v2018:txtDoneeTINA') 'Please enter a valid TIN in Item 12.' @('official-hta-runtime#validate:L4701-L4704') 'obsolete' 'The branch is commented out.' 'Apply evidence-backed donee identity requirements independently of removed controls.'
V '014-mode-affixture-commented' 'No removed affixture mode is selected.' @('frm1800v2018:optMode_1','frm1800v2018:optMode_2','frm1800v2018:optMode_3') 'Please select an option in Item 13.' @('official-hta-runtime#validate:L4705-L4727') 'obsolete' 'The controls and all related validation are commented out.' 'Exclude the removed DST workflow.'
V '015-donor-name-omitted' 'Donor name is blank during Validate.' @('frm1800v2018:txtDonorName') $null @('official-hta-runtime#validate:L4650-L4736') 'incorrect-official-behavior' 'Validate omits donor name even though Save requires it.' 'Use one shared finalization validator.'
V '016-address-omitted' 'Registered and/or residential address is blank.' @('frm1800v2018:txtRegAddress','frm1800v2018:txtResAddress') $null @('official-hta-runtime#validate:L4650-L4736') 'incorrect-official-behavior' 'Validate checks neither address.' 'Require legally applicable background information.'
V '017-donee-identity-omitted' 'All donee names/TINs are blank or a populated row is incomplete.' @('frm1800v2018:txtDoneeNameA','frm1800v2018:txtDoneeTINA','frm1800v2018:txtDoneeNameB','frm1800v2018:txtDoneeTINB') $null @('official-hta-runtime#validate:L4650-L4736','official-hta-runtime#control:L690-L875') 'incorrect-official-behavior' 'Validate has no active donee-row check.' 'Require at least one donee and validate each populated row consistently.'
V '018-schedules-omitted' 'Gift schedules are empty, partial, malformed, or inconsistent with totals.' @('schedule-a','schedule-b1','schedule-b2') $null @('official-hta-runtime#validate:L4650-L4736','official-hta-runtime#checkDateSched2:L4464-L4511','official-hta-runtime#checkDateSched3:L4512-L4557','official-hta-runtime#checkDateSched4:L4558-L4603') 'incorrect-official-behavior' 'All schedule date validators are block-commented and Validate does not inspect active gift schedules.' 'Validate row completeness, types, valuation dependencies, and calculated totals.'
V '019-tax-computation-omitted' 'Computed Items 14-20 or 25-38 are stale, negative where impermissible, or inconsistent.' @('computation-chain') $null @('official-hta-runtime#validate:L4650-L4736','official-hta-runtime#compute27:L4333-L4367') 'incorrect-official-behavior' 'Validate performs no calculation-consistency checks.' 'Recompute from authoritative inputs before finalization and validate legal bounds.'
V '020-save-button-tin-copy-paste' 'Save-button enablement checks txtTIN1 twice and never checks txtTIN3.' @('frm1800v2018:txtTIN1','frm1800v2018:txtTIN2','frm1800v2018:txtTIN3','frm1800v2018:txtBranchCode') $null @('official-hta-runtime#enableSaveButton:L5153-L5172') 'incorrect-official-behavior' 'A copy/paste error omits the third segment.' 'Centralize eligibility in the typed validator.'
V '021-save-button-final-else' 'Year length is four while earlier date/TIN checks failed.' @('frm1800v2018:txtYear','donor-tin','date') $null @('official-hta-runtime#enableSaveButton:L5153-L5172') 'incorrect-official-behavior' 'The final else re-enables Save based only on year length, overriding earlier disable assignments.' 'Set button state once from the complete validation result.'
V '022-help-rate-mismatch' 'The installed Help1800 text is treated as January 2018 rate guidance.' @('tax-rate-guidance') $null @('legacy-help-supporting-only#tax-rate','official-hta-runtime#compute16:L4358-L4362','official-form-pdf') 'obsolete' 'Help describes schedular relative rates and 30% for strangers, while the 2018 HTA/form uses a flat 6% above the exemption.' 'Use revision-matched January 2018 law/form guidance.'
V '023-calculation-recursion' 'compute18 or computeTotalAmtPayable is invoked.' @('frm1800v2018:txtTaxPayable','frm1800v2018:TotalAmountPayable') $null @('official-hta-runtime#computeTotalAmtPayable:L4323-L4327','official-hta-runtime#compute18:L4363-L4367') 'incorrect-official-behavior' 'compute18 calls computeTotalAmtPayable, which calls compute18 again with no termination condition, causing unbounded mutual recursion.' 'Evaluate the dependency graph once in topological order.'
V '024-negative-tax-base' 'Cumulative net gifts are below the fixed 250,000 exemption.' @('frm1800v2018:TotalNetGifts36','frm1800v2018:TotalNetGiftsSubjectToTax') $null @('official-hta-runtime#compute38:L4352-L4357') 'incorrect-official-behavior' 'The official formula subtracts 250,000 without clamping to zero, so downstream tax can become negative.' 'Clamp taxable net gifts at zero unless authoritative law explicitly permits a negative base.'
V '025-family-unbounded' 'Add Schedule is repeatedly invoked.' @('schedule-a','schedule-b1','schedule-b2') $null @('official-hta-runtime#addScheduleA:L3944-L3969','official-hta-runtime#addScheduleB2:L4087-L4118','official-hta-runtime#addScheduleB1:L4209-L4240') 'official-bug-compatible' 'No source-side maximum row count is enforced.' 'Define tested capacity and preserve all accepted rows losslessly.'
V '026-add-rebuild-name-gap' 'A schedule row is added or rebuilt.' @('schedule-a','schedule-b1','schedule-b2') $null @('official-hta-runtime#addScheduleA:L3944-L3969','official-hta-runtime#addScheduleB2:L4087-L4118','official-hta-runtime#addScheduleB1:L4209-L4240') 'official-bug-compatible' 'Runtime-created row inputs have IDs but no name attributes; the form.elements serializer still enumerates them in the official host.' 'Serialize by the explicit revision contract, not browser name-registration quirks.'
Rule '1800-final-001' 'final-copy' 1 'Final Copy is requested after local validation and file-version checks.' @('txtFinalFlag','return-body') $null @('official-hta-runtime#openAlertEmail:L5784-L5829','official-hta-runtime#saveXML') 'unverified' 'No revision-matched final-copy artifact was available; source shows the same Save serializer followed by encryption/transport workflow.' 'Test locally with dummy data and preserve all baseline plus indexed keys.'
Rule '1800-submit-001' 'submit' 1 'Online transport is invoked.' @('return-body') $null @('official-hta-runtime#saveXMLsubmit:L3449-L3664','official-hta-runtime#sendEmail:L5830-L5920') 'unverified' 'Transport exists but was not exercised.' 'Keep local validation/finalization independently testable.' 'medium'

Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    first_error_behavior='Validate checks treaty description, donor TIN, RDO, then date; it returns on the first failure. Save calls checkDate1 before its narrower initialValidateBeforeSave checks.'
    rules=$rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Outputs,[string[]]$Inputs,[string]$Formula,[string]$Trigger,[string[]]$Depends,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Use decimal arithmetic and recompute from authoritative inputs.',[string]$Condition=$null) {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id=$Id; outputs=$Outputs; inputs=$Inputs; condition=$Condition; official_formula=$Formula
        rounding='Entry handlers call round(...,2); computed values use formatCurrency after NumWithComma parsing.'
        trigger=$Trigger; depends_on=$Depends; source_refs=$Refs; assessment=$Assessment
        recommended_app_behavior=$Recommended; confidence='high'
    })
}
Calc '1800-schedule-a-total' @('frm1800v2018:schedA:txtTotalPayment1','frm1800v2018:PersonalProperties') @('frm1800v2018:schedA:txtFairMarketValue{index}') 'Sum all Schedule A fair-market-value rows; copy the result to Item 25 Personal Properties.' 'computeScheduleA' @() @('official-hta-runtime#computeScheduleA:L4435-L4447')
Calc '1800-schedule-b2-total' @('frm1800v2018:schedB2:txtTotal','frm1800v2018:RealProperties') @('frm1800v2018:schedB2:txtFairMarketValue{index}') 'Sum all Schedule B2 fair-market-value rows; copy the result to Item 26 Real Properties.' 'computeScheduleB2' @() @('official-hta-runtime#computeScheduleB2:L4448-L4463')
Calc '1800-total-gifts' @('frm1800v2018:TotalGifts') @('frm1800v2018:PersonalProperties','frm1800v2018:RealProperties') 'Item 27 = Item 25 + Item 26.' 'compute27' @('1800-schedule-a-total','1800-schedule-b2-total') @('official-hta-runtime#compute27:L4333-L4336')
Calc '1800-total-deductions' @('frm1800v2018:TotalDeductionsAllowed') @('frm1800v2018:DeductionAmt1','frm1800v2018:DeductionAmt2','frm1800v2018:DeductionAmt3','frm1800v2018:DeductionAmt4','frm1800v2018:DeductionAmt5') 'Item 33 = sum of Items 28 through 32.' 'compute33' @() @('official-hta-runtime#compute33:L4337-L4341')
Calc '1800-current-net-gifts' @('frm1800v2018:TotalReturnNetGifts') @('frm1800v2018:TotalGifts','frm1800v2018:TotalDeductionsAllowed') 'Item 34 = Item 27 - Item 33.' 'compute34' @('1800-total-gifts','1800-total-deductions') @('official-hta-runtime#compute34:L4342-L4346')
Calc '1800-cumulative-net-gifts' @('frm1800v2018:TotalNetGifts36') @('frm1800v2018:TotalReturnNetGifts','frm1800v2018:TotalPriorNetGifts') 'Item 36 = Item 34 + Item 35.' 'compute36' @('1800-current-net-gifts') @('official-hta-runtime#compute36:L4347-L4351')
Calc '1800-taxable-net-gifts' @('frm1800v2018:TotalNetGiftsSubjectToTax','frm1800v2018:TotalNetGifts') @('frm1800v2018:TotalNetGifts36') 'Item 38 = Item 36 - 250,000, without a zero floor; Item 14 copies Item 38.' 'compute38' @('1800-cumulative-net-gifts') @('official-hta-runtime#compute38:L4352-L4357') 'incorrect-official-behavior' 'Clamp at zero if required by the revision-matched legal rule.'
Calc '1800-donor-tax-due' @('frm1800v2018:DonorTaxDue') @('frm1800v2018:TotalNetGifts') 'Item 16 = Item 14 × 6%.' 'compute16' @('1800-taxable-net-gifts') @('official-hta-runtime#compute16:L4358-L4362','official-form-pdf')
Calc '1800-tax-credits' @('frm1800v2018:TaxCredit17D') @('frm1800v2018:TaxCredit17A','frm1800v2018:TaxCredit17B','frm1800v2018:TaxCredit17C') 'Item 17D = 17A + 17B + 17C.' 'compute17D' @() @('official-hta-runtime#compute17D:L4328-L4332')
Calc '1800-tax-payable' @('frm1800v2018:txtTaxPayable') @('frm1800v2018:DonorTaxDue','frm1800v2018:TaxCredit17D') 'Item 18 = Item 16 - Item 17D.' 'compute18' @('1800-donor-tax-due','1800-tax-credits') @('official-hta-runtime#compute18:L4363-L4367')
Calc '1800-penalties' @('frm1800v2018:TotalPenalties') @('frm1800v2018:Surcharge','frm1800v2018:Interest','frm1800v2018:Compromise') 'Item 19D = 19A + 19B + 19C.' 'computePenalties' @() @('official-hta-runtime#computePenalties:L4316-L4322')
Calc '1800-total-amount-payable' @('frm1800v2018:TotalAmountPayable') @('frm1800v2018:txtTaxPayable','frm1800v2018:TotalPenalties') 'Item 20 = Item 18 + Item 19D.' 'computeTotalAmtPayable' @('1800-tax-payable','1800-penalties') @('official-hta-runtime#computeTotalAmtPayable:L4323-L4327')
Calc '1800-mutual-recursion-defect' @('frm1800v2018:txtTaxPayable','frm1800v2018:TotalAmountPayable') @('computation-chain') 'compute18 calls computeTotalAmtPayable, which calls compute18 again; no termination condition exists.' 'compute18 or computeTotalAmtPayable' @('1800-tax-payable','1800-total-amount-payable') @('official-hta-runtime#computeTotalAmtPayable:L4323-L4327','official-hta-runtime#compute18:L4363-L4367') 'incorrect-official-behavior' 'Evaluate the acyclic intended graph once and reject cyclic calculation contracts.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema'='../../schema/calculations.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    evaluation_order=@($calculations.calculation_id); calculations=$calculations
})

$negativeRules = @($rules | Where-Object { $_.exact_message } | Select-Object -First 40)
$cases = @()
$caseNumber=0
foreach ($rule in $negativeRules) {
    $caseNumber++
    $cases += [pscustomobject][ordered]@{
        case_id=('case-{0:d2}-{1}' -f $caseNumber,$rule.rule_id); phase=$rule.phase
        mutations=@{synthetic_condition=$rule.condition}; expected_message=$rule.exact_message
        expected_behavior=$rule.official_behavior; rule_id=$rule.rule_id
    }
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version='1.0.0'; form_id=$formId; synthetic_only=$true; cases=$cases
})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version='1.0.0'; form_id=$formId; cases=@(
        @{case_id='below-exemption';calculation_id='1800-taxable-net-gifts';inputs=@{cumulative=100000};official_output=-150000;recommended_output=0},
        @{case_id='at-exemption';calculation_id='1800-taxable-net-gifts';inputs=@{cumulative=250000};official_output=0},
        @{case_id='flat-six-percent';calculation_id='1800-donor-tax-due';inputs=@{taxable_net_gifts=1000000};official_output=60000},
        @{case_id='tax-credits';calculation_id='1800-tax-credits';inputs=@{a=100;b=200;c=300};official_output=600},
        @{case_id='recursion';calculation_id='1800-mutual-recursion-defect';entry='compute18';official_result='unbounded mutual recursion / host stack failure'},
        @{case_id='schedule-sum';calculation_id='1800-schedule-a-total';inputs=@{rows=@(100,200,300)};official_output=600}
    )
})

$resources=@()
foreach ($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1') | ForEach-Object {$_.Groups['v'].Value} | Sort-Object -Unique)) {
    $full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if(Test-Path -LiteralPath $full){$resources += [pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}}
    else{$resources += [pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}}
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$workflow=[ordered]@{
    '$schema'='../../schema/workflow.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    phases=@(
        @{phase='edit';official_behavior='January 2018 donor tax return; one return per donor per donation date, with prior same-year net gifts carried into the computation.';source_refs=@('official-form-pdf','official-guidelines-pdf');confidence='high'},
        @{phase='saved-draft';official_behavior='Save first runs checkDate1, then checks donor TIN blankness, RDO 000, and donor name; it serializes a 166-entry five-row baseline plus any added indexed schedule rows.';source_refs=@('official-hta-runtime#saveXML:L3113-L3447','official-hta-runtime#initialValidateBeforeSave:L5072-L5096');confidence='high'},
        @{phase='validated';official_behavior='Validate actively checks only treaty description, donor TIN blankness, RDO, and donation date before disabling controls.';source_refs=@('official-hta-runtime#validate:L4650-L4736');confidence='high'},
        @{phase='final-copy';official_behavior='Source shows final-copy encryption after the same serializer, but no frm1800v2018 revision-matched final-copy sample was available.';source_refs=@('official-hta-runtime#saveXML','legacy-artifact-exclusion');confidence='medium'},
        @{phase='submitted';official_behavior='Online transport exists but was not exercised.';source_refs=@('official-hta-runtime#saveXMLsubmit','official-hta-runtime#sendEmail');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='checkDate1 and narrow Save checks pass, and version-file guards permit the write.';side_effects=@('Writes flat pseudo-XML.','Collapses two address pairs.','Forces txtCurrentPage to 1.');source_refs=@('official-hta-runtime#saveXML:L3113-L3447')},
        @{from='edit';action='Validate';to='validated';guard='All active source-ordered checks pass.';side_effects=@('Disables controls.','Shows the exact success alert.');source_refs=@('official-hta-runtime#validate:L4650-L4736')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables applicable controls and preserves conditional disabled state.');source_refs=@('official-hta-runtime#enableAllControl:L4852-L4968')},
        @{from='validated';action='Final Copy';to='final-copy';guard='File-version and connectivity/finalization flow permits progress.';side_effects=@('Creates an encrypted/compressed final copy.');source_refs=@('official-hta-runtime#openAlertEmail:L5784-L5829')},
        @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and remote acceptance succeed.';side_effects=@('Attempts online submission; untested.');source_refs=@('official-hta-runtime#sendEmail:L5830-L5920')}
    )
    prerequisites=@(
        'Donation date and donor identity/RDO',
        'At least one donee and complete background information',
        'Gift schedules and valuation support',
        'Allowable deductions, prior same-year gifts, credits, and penalties as applicable'
    )
    required_attachments=@(
        @{attachment_id='relationship-statement';label='Sworn statement of donor-donee relationship.';required_when='Applicable filing.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'},
        @{attachment_id='tax-credit-proof';label='Proof of claimed tax credit.';required_when='Tax credit is claimed.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'},
        @{attachment_id='title-copy';label='Owner copy with photocopy or certified true copy of TCT/CCT/OCT.';required_when='Real property is donated.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'},
        @{attachment_id='tax-declaration';label='Certified true copy of latest tax declaration for lot and/or improvement.';required_when='Applicable real property.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'},
        @{attachment_id='no-improvement';label='Assessor Certificate of No Improvement.';required_when='Donated real property has no declared improvement.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'},
        @{attachment_id='stock-valuation';label='Proof of share valuation at donation date.';required_when='Shares are donated.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'},
        @{attachment_id='other-property-valuation';label='Proof of valuation of other personal property.';required_when='Applicable.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'},
        @{attachment_id='deduction-proof';label='Proof of claimed deductions.';required_when='Deductions are claimed.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'},
        @{attachment_id='tax-debit-memo';label='Tax Debit Memo used as payment.';required_when='Applicable payment mode.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#attachments');confidence='high'}
    )
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='Event-based: within 30 days after the donation date.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#deadline');confidence='high'},
        @{quarter='Q2';due_date_rule='Not quarterly; the same 30-day donation-relative deadline applies.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#deadline');confidence='high'},
        @{quarter='Q3';due_date_rule='Not quarterly; the same 30-day donation-relative deadline applies.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#deadline');confidence='high'},
        @{quarter='Q4';due_date_rule='Not quarterly; the same 30-day donation-relative deadline applies.';source_refs=@('official-guidelines-pdf','legacy-help-supporting-only#deadline');confidence='high'}
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount=@($rules|Where-Object {$_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME/formTyp 1800v2018 and printed January 2018 ENCS.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 Form 1800.'
    Asset 'official-guidelines-pdf' 'official-guidelines-pdf' $guidelinesPath 'Local guidelines distributed with the January 2018 form assets.'
    Asset 'legacy-help-supporting-only' 'official-runtime-help-legacy' $helpPath 'APPLICATIONNAME 1800; pre-2018 schedular/30% rate text contradicts the 2018 flat-rate form and is supporting only.'
    Asset 'legacy-hta-excluded' 'runtime-extracted-hta-legacy' $legacyHtaPath 'Legacy APPLICATIONNAME 1800; excluded from the 1800v2018 rule contract.'
    Asset 'legacy-editable-save-excluded' 'dummy-profile-editable-save-legacy' $legacySavePath '89 frm1800-prefixed keys; revision mismatch; values excluded.'
    Asset 'legacy-final-copy-excluded' 'dummy-profile-encrypted-final-copy-legacy' $legacyFinalPath 'Legacy 1800 final copy; revision mismatch; values excluded.' (Join-Path $LegacyFinalDir '1800-final-copy-#email-redacted#.xml')
)
$manifest=[ordered]@{
    '$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='1800';revision=$revision
    revision_label='January 2018 ENCS';package_version=$packageVersion;status='complete';official_assets=$assets
    counts=[ordered]@{
        concrete_fields=166;runtime_field_families=11;fields_total=$fields.Count;typed_fields=$fields.Count
        validation_rules=$rules.Count;confirmed_official_bugs=$bugCount;calculations=$calculations.Count
        negative_fixtures=$cases.Count;unverified_gaps=3
    }
    artifacts=[ordered]@{
        fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json'
        evidence='evidence.md';audit='audit.md';gaps='gaps.md'
        runtime_control_fixture='fixtures/runtime-control-inventory-v796.json'
        legacy_artifact_exclusion='fixtures/legacy-artifact-exclusion.json'
        validation_function_fixture='fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture='fixtures/calculation-function-inventory-v796.json'
        resource_hash_fixture='fixtures/official-resource-hashes-v796.json'
        negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'
    }
    scope_notes=@(
        'Research only; no renderer, typed model, migration, capability, or release metadata changed.',
        'No source values or email-bearing filenames are copied.',
        'The concrete inventory is source-derived from the exact Save serializer: 167 live static controls plus one injected RDO, minus two collapsed address continuation controls, for 166 baseline entries.',
        'Eleven indexed families cover unbounded Schedule A, B1, and B2 rows; initial concrete indices 0 through 4 are already included in the baseline.',
        'The available 89-key plaintext save and encrypted final copy are legacy frm1800 artifacts and are explicitly excluded from 1800v2018 evidence.',
        'Installed Help1800 is supporting-only because its pre-2018 rate text contradicts the pinned January 2018 form.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1800 - January 2018 ENCS`n`nRevision-specific Offline eBIRForms rule package with 166 source-derived baseline serializer entries and 11 expandable indexed families. Legacy frm1800 saves are excluded.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- January 2018 HTA SHA-256: $($expected.hta); APPLICATIONNAME 1800v2018; printed January 2018 ENCS.
- Official form PDF SHA-256: $($expected.pdf); guidelines PDF SHA-256: $($expected.guidelines); valid PDF magic.
- Installed Help1800 SHA-256: $($expected.help); APPLICATIONNAME 1800 and pre-2018 schedular/30% tax text, so supporting only.
- Live DOM inventory: 184 controls; 167 static serializable controls; one runtime-injected RDO select.
- Save projection: 166 baseline entries after registered/residential address-pair collapse; 11 indexed Schedule A/B1/B2 families can add rows beyond the five initial rows.
- Legacy plaintext SHA-256: $($expected.legacy_save); 89 unique frm1800 keys; inventory $($expected.legacy_inventory). Legacy encrypted copy SHA-256: $($expected.legacy_final). Both are excluded from January 2018 field evidence.
- No existing typed 1800 model was found under crates/bir-core/src/forms.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. No plaintext or encrypted final-copy sample with frm1800v2018 keys was available; the 166-entry baseline and 11 families are proven from the exact serializer/runtime DOM but not compared with a saved January 2018 artifact.
2. Online submission was not exercised.
3. Attachment presence and the official calculation recursion failure were not black-box exercised; both are source-proven, with attachment enforcement absent from Validate.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- Revision/assets: **pass** - exact 1800v2018 HTA, January 2018 PDF, guidelines, package, and contradictory legacy help are pinned.
- Fields: **pass with explicit observation gap** - 166 baseline serialized entries and 11 indexed families are source-derived; revision-mismatched 89-key legacy saves are excluded.
- Controls/functions: **pass** - comment/script filtering, runtime RDO injection, address collapse, function inventories, and resource hashes captured.
- Rules/workflow: **pass** - exact Save/Validate order and messages, final-copy/version guards, deadline, and attachments captured.
- Calculations: **pass** - schedule totals, gifts, deductions, exemption, flat 6% tax, credits, penalties, and total are recorded.
- Official defects: **pass** - $bugCount bug-compatible/incorrect/obsolete rules include wrong item references, sparse validation, impossible dates, broken Save enablement, negative tax base, stale help, and unbounded mutual recursion.
- Privacy: **pass** - no values or email-bearing filenames copied.
- Revision-matched saved artifact, online transport, and attachment presence: **unverified** and explicit gaps.
"@
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 20: 1800-v2018. Next: 1801.`n"

$indexPath=Join-Path $RepoRoot 'rules\index.json'
$index=Get-Content -LiteralPath $indexPath -Raw|ConvertFrom-Json
$entry=[pscustomobject][ordered]@{form_id=$formId;form_code='1800';revision=$revision;package_version=$packageVersion;priority=20;status='complete';path='forms/1800-v2018/manifest.json'}
$index.forms=@(@($index.forms|Where-Object {$_.form_id -ne $formId})+$entry|Sort-Object priority)
$index.updated=(Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index

"Generated ${formId}: fields=$($fields.Count), concrete=166, families=$($families.Count), rules=$($rules.Count), calculations=$($calculations.Count), negative_cases=$($cases.Count), bugs=$bugCount, controls=$($controls.Count), static_serial=$($staticSerial.Count)"
