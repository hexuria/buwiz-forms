param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$SourceDir = 'C:\Mac\Home\Downloads\forms\1702RTv2018c'
)

$ErrorActionPreference = 'Stop'
$formId = '1702rt-v2018c'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1702RTv2018C.hta'
$sharedPath = Join-Path $ExtractedRoot 'js\lib\1702RT.js'
$plainPath = Join-Path $SourceDir '00000000000000-1702RTv2018C-122025.xml'
$pdfPath = Join-Path $SourceDir '1702-RT Jan 2018 ENCS Final v3.pdf'
$outDir = Join-Path $RepoRoot 'rules\forms\1702rt-v2018c'
$fixtureDir = Join-Path $outDir 'fixtures'
$encryptedCandidates = @(Get-ChildItem -LiteralPath $SourceDir -File | Where-Object { $_.Name -like '00000000000000-1702RTv2018C-122025#*#.xml' })
if ($encryptedCandidates.Count -ne 1) { throw "Expected exactly one target-revision encrypted companion; found $($encryptedCandidates.Count)." }
$encryptedPath = $encryptedCandidates[0].FullName
foreach ($path in @($htaPath,$sharedPath,$plainPath,$encryptedPath,$pdfPath,'C:\eBIRForms\BIRForms.exe')) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

function Write-Json([string]$Path,$Value) {
    [IO.File]::WriteAllText($Path,(($Value | ConvertTo-Json -Depth 50) + [Environment]::NewLine),[Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path,[string]$Value) {
    [IO.File]::WriteAllText($Path,$Value,[Text.UTF8Encoding]::new($false))
}
function Get-Attr([string]$Tag,[string]$Name) {
    $match = [regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($match.Success) { $match.Groups[2].Value } else { $null }
}
function Get-HashText([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-','').ToLowerInvariant()
    } finally { $sha.Dispose() }
}
function Asset([string]$Id,[string]$Kind,[string]$Path,[string]$Binding) {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id=$Id; kind=$Kind; path=$Path
        sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        size=$item.Length; revision_binding=$Binding
    }
}

$expectedHashes = @{
    hta       = 'fbbaa75784bb054f3658ab0a357ad50b65807c59c437c2d010e69aa1ad64081a'
    shared    = 'fdb74a14ebf1813bb8c466e8416ff6c5c703592b2207c887f3047f6fa01fab6e'
    plain     = 'a5316d974ffca1db2359d92208fd4f6b15533e5330fcfc73922becd6b2c29299'
    encrypted = 'e45db05bb89c2513054e7f075e41a09e9ec35c9590982619dcfb1dfb57602501'
    decrypted = 'fc0ee1febf0e80a4116e7f274d49a12c939f5140ce592f5e2af48577d1206c99'
    inventory = '2e294eb7da3dfeff23dff785b8a971fba2100b4ecba4569c5a18044b6d0caced'
    pdf       = 'd9a6a8a13e0114934261151c4eb269a1573042e7ce670eaf12b15f169d308d2d'
    package   = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}
foreach ($pair in @(
    @($htaPath,'hta'),@($sharedPath,'shared'),@($plainPath,'plain'),
    @($encryptedPath,'encrypted'),@($pdfPath,'pdf'),@('C:\eBIRForms\BIRForms.exe','package')
)) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expectedHashes[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') { throw 'Official PDF magic mismatch.' }

$hta = [IO.File]::ReadAllText($htaPath)
$plain = [IO.File]::ReadAllText($plainPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*"1702RTv2018C"') { throw 'APPLICATIONNAME mismatch.' }
if ($hta -notmatch '(?i)January\s+2018') { throw 'Printed revision marker mismatch.' }
function Save-Keys([string]$Text) {
    @([regex]::Matches($Text,'<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') | ForEach-Object { $_.Groups['key'].Value })
}
$keys = Save-Keys $plain
if ($keys.Count -ne 258 -or ($keys | Sort-Object -Unique).Count -ne 258) { throw "Expected 258 unique plaintext keys; found $($keys.Count)." }
if ((Get-HashText @($keys | Sort-Object)) -ne $expectedHashes.inventory) { throw 'Plaintext field inventory hash changed.' }

$formMatch = [regex]::Match($hta,'(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain not found.' }
$formBody = $formMatch.Groups['body'].Value
$formOffset = $formMatch.Groups['body'].Index
$scriptRanges = @([regex]::Matches($formBody,'(?is)<script\b.*?</script>'))
$controls = @()
$ordinal = 0
foreach ($match in [regex]::Matches($formBody,'(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $insideScript = $false
    foreach ($range in $scriptRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) { $insideScript=$true; break }
    }
    if ($insideScript) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind='text' }
    $controls += [pscustomobject][ordered]@{
        ordinal=$ordinal; id=Get-Attr $tag 'id'; name=Get-Attr $tag 'name'; element=$element
        control_kind=$kind.ToLowerInvariant()
        source_line=1+[regex]::Matches($hta.Substring(0,$formOffset+$match.Index),"`n").Count
        value=Get-Attr $tag 'value'; maxlength=Get-Attr $tag 'maxlength'
        disabled=$tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly=$tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
}
$serializable = @($controls | Where-Object { $_.control_kind -in @('text','select','select-one','radio','checkbox','hidden') })
if ($controls.Count -ne 351 -or $serializable.Count -ne 292) { throw "Expected 351 controls/292 serializer candidates; found $($controls.Count)/$($serializable.Count)." }
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id]=$control }
}

$required = @(
    'frm1702RT:rdoPg1I1Calendar','frm1702RT:rdoPg1I1Fiscal','frm1702RT:ddlPg1I2Month','frm1702RT:txtPg1I2Year',
    'frm1702RT:drpPg1Pt1I7RDOCode','frm1702RT:txtPg1Pt1I8Name1','frm1702RT:txtPg1Pt1I9Address1',
    'frm1702RT:txtPg1Pt1I10','frm1702RT:txtPg1Pt1I11Contact','frm1702RT:txtPg1Pt1I12Email'
)
$computedPattern = '(?i)(Subtotal|Total|NetTax|IncomeTaxDue|MinimumCorporate|TotalIncomeTax|TotalTaxCredits|NetOperatingLoss|TotalNOLCO|TotalExcessMCIT|NetTaxableIncome)$'
function Get-Meta([string]$Key,$Control,[bool]$Family) {
    $page=$null; $item=$null; $logical='string'; $status='optional'; $enum=[object[]]@(); $normalization=[string[]]@()
    if ($Key -match '(?i)Pg(?<page>[1-4])') { $page=[int]$Matches.page }
    if ($Key -match '(?i)(?:Pt\d+|Sc\d+A?)I(?<item>\d+[a-z]?)') { $item=$Matches.item }
    if ($Key -match '(?i)(rdo|chk|checkbox)') { $logical='boolean'; $enum=[object[]]@('true','false') }
    elseif ($Key -match '(?i)(TIN|Branch|RDO|ATC|PSIC)') { $logical='code' }
    elseif ($Key -match '(?i)Email') { $logical='email-string' }
    elseif ($Key -match '(?i)(Contact|Tel)') { $logical='phone-string' }
    elseif ($Key -match '(?i)(Date|I10$)') { $logical='date-string'; $normalization=[string[]]@('MM/DD/YYYY where a date helper is bound') }
    elseif ($Key -match '(?i)(Year|C1$)' -and $Key -match '(?i)(Sc3A|I2Year)') { $logical='integer' }
    elseif ($Key -match '(?i)(Amount|Income|Tax|Sales|Revenue|Receipt|Deduct|Loss|Cost|Payment|Credit|Penalt|Surcharge|Interest|Compromise|Rate|C[2-8]$|I\d+$)') {
        $logical='whole-peso-amount'; $normalization=[string[]]@('NumWithComma/NumWithParenthesis','whole-peso display')
    }
    if ($required -contains $Key) { $status='required' }
    $isComputed = $Key -match $computedPattern
    if ($isComputed) { $status='computed' }
    if ($Key -match '^(txtFinalFlag|txtEnroll|ebirOnline|driveSelectTPExport)') { $status='hidden' }
    if ($Family) { $status='conditional' }
    $constraints=[ordered]@{}
    if ($Control -and $Control.maxlength -and $Control.maxlength -match '^\d+$') { $constraints.max_length=[int]$Control.maxlength }
    if ($Family) { $constraints.index='N >= 1; modal add/save code enforces no maximum.' }
    [pscustomobject]@{
        page=$page; item=$item; logical=$logical; status=$status; enum=$enum; normalization=$normalization
        computed=$isComputed; calculation=if($isComputed){'See calculations.json'}else{$null}; constraints=[pscustomobject]$constraints
    }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    $control = if ($controlById.ContainsKey($key)) { $controlById[$key] } else { $null }
    $meta = Get-Meta $key $control $false
    $refs = @("xml-editable-v1#field:$key")
    if ($control) { $refs += "official-hta-runtime#control:L$($control.source_line)" }
    else { $refs += 'official-hta-runtime#saveXML/runtime-injection' }
    $fields.Add([pscustomobject][ordered]@{
        field_key=$key; serialized_key=$key; serialized_occurrence=1; label=$key
        page=$meta.page; item_number=$meta.item
        control_kind=if($control){$control.control_kind}else{'runtime-injected-control'}
        storage_type='string'; logical_type=$meta.logical; required=$meta.status
        required_when=$null; enabled_when=$null; visible_when=$null
        default_value=if($control){$control.value}else{$null}; empty_representation=''
        constraints=$meta.constraints; enum_values=$meta.enum; normalization=$meta.normalization
        computed=$meta.computed; calculation_id=$meta.calculation; source_refs=$refs
        confidence=if($control){'high'}else{'medium'}
        notes=@('Observed in both reviewed plaintext and decrypted encrypted 258-key inventories; source values are excluded.')
    })
}
$familySpecs = @(
    @{base='frm1702RT:txtPg2Pt4I54';columns=2;page=2;item='54';types=@('string','whole-peso-amount')},
    @{base='frm1702RT:txtPg3Sc1I17i';columns=2;page=3;item='17i';types=@('string','whole-peso-amount')},
    @{base='frm1702RT:txtPg3Sc2I4';columns=3;page=3;item='4';types=@('string','string','whole-peso-amount')},
    @{base='frm1702RT:txtPg4Sc3AI7';columns=6;page=4;item='7';types=@('integer','whole-peso-amount','whole-peso-amount','whole-peso-amount','whole-peso-amount','whole-peso-amount')},
    @{base='frm1702RT:txtPg4Sc5I3';columns=2;page=4;item='3';types=@('string','whole-peso-amount')},
    @{base='frm1702RT:txtPg4Sc5I6';columns=2;page=4;item='6';types=@('string','whole-peso-amount')},
    @{base='frm1702RT:txtPg4Sc5I8';columns=2;page=4;item='8';types=@('string','whole-peso-amount')}
)
$families = @()
foreach ($spec in $familySpecs) {
    for ($column=1; $column -le $spec.columns; $column++) {
        $key = "$($spec.base).{N>=1}C$column"
        $families += $key
        [string[]]$familyNormalization = if ($spec.types[$column-1] -eq 'string') {
            @('trim checked for modal validity')
        } else {
            @('whole-number key filter','currency formatting')
        }
        $fields.Add([pscustomobject][ordered]@{
            field_key=$key; serialized_key=$null; serialized_occurrence=$null
            label="Runtime modal row field $key"; page=$spec.page; item_number=$spec.item
            control_kind='runtime-indexed-family'; storage_type='string'; logical_type=$spec.types[$column-1]
            required='conditional'; required_when='The corresponding modal row N exists.'
            enabled_when='The source row and Add more control are enabled.'
            visible_when='The modal is open or the row is retained in its repository table.'
            default_value=$null; empty_representation=''; constraints=[pscustomobject]@{index='N >= 1; no runtime maximum'}
            enum_values=[object[]]@(); normalization=$familyNormalization
            computed=($spec.columns -eq 6 -and $column -eq 6)
            calculation_id=if($spec.columns -eq 6 -and $column -eq 6){'1702rt-nolco-row-balance'}else{$null}
            source_refs=@('official-hta-runtime#loadModalTable:L5144-L5328','official-hta-runtime#addRowModalTable:L5329-L5443','official-hta-runtime#saveModalTable:L5444-L5688')
            confidence='high'; notes=@('Unbounded row ID uses a literal dot before the one-based row number.')
        })
    }
}
if ($families.Count -ne 19 -or $fields.Count -ne 277 -or ($fields.field_key | Sort-Object -Unique).Count -ne 277) {
    throw "Expected 258 concrete fields plus 19 families; found $($fields.Count)."
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema'='../../schema/fields.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    field_count=$fields.Count; runtime_serializable_element_count=258
    inventory_sha256=Get-HashText @($fields.field_key | Sort-Object); fields=$fields
})

$staticIds = @($serializable.id | Where-Object { $_ } | Sort-Object -Unique)
$plainOnly = @(Compare-Object $staticIds @($keys | Sort-Object -Unique) -PassThru | Where-Object { $_.SideIndicator -eq '=>' })
$staticOnly = @(Compare-Object $staticIds @($keys | Sort-Object -Unique) -PassThru | Where-Object { $_.SideIndicator -eq '<=' })
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0'; form_id=$formId; official_hta_sha256=$expectedHashes.hta
    form_control_count=$controls.Count; static_serializer_candidate_count=$serializable.Count
    static_serializer_unique_id_count=$staticIds.Count; reviewed_plaintext_key_count=$keys.Count
    runtime_modal_family_count=$families.Count
    serializer_set_differences=[ordered]@{runtime_injected_observed=$plainOnly; static_not_in_plain_snapshot=$staticOnly}
    controls=$controls; dynamic_families=$families
})

$decryptTool = Join-Path $RepoRoot 'rules\tools\audit-1701-encrypted-fields.ps1'
$decryptAudit = (& $decryptTool -SourceDir $SourceDir -FormId $formId -FilePattern '00000000000000-1702RTv2018C-122025#*#.xml' -RedactedFileName '00000000000000-1702RTv2018C-122025#email-redacted#.xml' -ExpectedCiphertextSha256 $expectedHashes.encrypted -ExpectedDecryptedSha256 $expectedHashes.decrypted -ExpectedFieldCount 258 -ExpectedFieldInventorySha256 $expectedHashes.inventory -ExpectedExtraField '*' -VersionField '' -ExpectedXmlVersion '*') -join [Environment]::NewLine
Write-Utf8 (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') $decryptAudit
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1702RT:' -NamePattern '(?i)valid|check|process|enable|disable|save|submit|final') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1702RT:' -NamePattern '(?i)compute|calc|populate|sum|difference|product') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'shared-1702rt-function-inventory-v796.json') ((& $functionTool -HtaPath $sharedPath -ControlPrefix 'frm1702RT:' -NamePattern '(?i)populate|generate|process|compute|tax|submit|save') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$FieldKeys,[string]$Accepted,[string]$Rejected,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The first active branch alerts and returns.',[string]$Recommended='Retain as a structured revision-aware field error.',[string]$Confidence='high') {
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id; form_id=$formId; revision=$revision; phase=$Phase; order=$Order; condition=$Condition; fields=$FieldKeys
        accepted_behavior=$Accepted; rejected_behavior=$Rejected; exact_message=$Message; source_refs=$Refs
        evidence_type=@('source'); assessment=$Assessment; official_behavior=$Official
        recommended_app_behavior=$Recommended; confidence=$Confidence; unresolved_questions=@()
    })
}
Rule '1702rt-save-001' 'save' 1 'RDO value is literal 000.' @('frm1702RT:drpPg1Pt1I7RDOCode') 'Any other string passes.' 'Save is blocked.' 'Please select an RDO Code (Part I Item 7).' @('official-hta-runtime#initialValidateBeforeSave:L10969-L10980') 'official-bug-compatible' 'Equality-only test permits blank or unknown non-000 values.' 'Require a catalog RDO before finalization while permitting lossless drafts.'
Rule '1702rt-save-002' 'save' 2 'Registered name is blank.' @('frm1702RT:txtPg1Pt1I8Name1') 'Nonblank passes.' 'Save is blocked.' 'Please provide a Registered Name (Part I Item 9).' @('official-hta-runtime#initialValidateBeforeSave:L10981-L10984') 'incorrect-official-behavior' 'Message says Item 9, but the field is printed Item 8.' 'Report the correct Item 8.'
Rule '1702rt-save-003' 'save' 3 'Registered address line 1 is blank.' @('frm1702RT:txtPg1Pt1I9Address1') 'Nonblank passes.' 'Save is blocked.' 'Please provide a Registered Address (Part I Item 10).' @('official-hta-runtime#initialValidateBeforeSave:L10985-L10988') 'incorrect-official-behavior' 'Message says Item 10, but the field is printed Item 9.' 'Report the correct Item 9.'
Rule '1702rt-save-004' 'save' 4 'Contact number is blank.' @('frm1702RT:txtPg1Pt1I11Contact') 'Nonblank passes.' 'Save is blocked.' 'Please provide a Contact Number (Part I Item 11).' @('official-hta-runtime#initialValidateBeforeSave:L10989-L10992')
Rule '1702rt-save-005' 'save' 5 'Email is blank.' @('frm1702RT:txtPg1Pt1I12Email') 'Nonblank passes without format checking here.' 'Save is blocked.' 'Please provide an Email Address (Part I Item 12).' @('official-hta-runtime#initialValidateBeforeSave:L10993-L10996') 'official-bug-compatible' 'Save checks presence only.' 'Validate format before finalization.'
Rule '1702rt-save-006' 'save' 6 'Any other field is incomplete or invalid.' @('return-body') 'Save proceeds after five narrow checks.' 'No rejection occurs.' $null @('official-hta-runtime#initialValidateBeforeSave:L10969-L11012') 'official-bug-compatible' 'PSIC validation is commented out and taxpayer TIN is not checked by Save.' 'Preserve drafts losslessly and show completeness separately.'

$validateOrder = 0
function V([string]$Suffix,[string]$Condition,[string[]]$FieldKeys,$Message,[string]$Lines,[string]$Assessment='verified-correct',[string]$Official='The branch alerts, changes page, and returns.',[string]$Recommended='Retain with revision-aware wording.') {
    $script:validateOrder++
    Rule "1702rt-validate-$Suffix" 'validate' $script:validateOrder $Condition $FieldKeys 'Condition is false; validation continues.' 'Validation stops.' $Message @("official-hta-runtime#validate:L$Lines") $Assessment $Official $Recommended
}
V '001' 'Neither Calendar nor Fiscal is selected.' @('frm1702RT:rdoPg1I1Calendar','frm1702RT:rdoPg1I1Fiscal') 'Please select if you are filing for Calendar or Fiscal year on Page 1 Item 1' '10659-L10665'
V '002' 'Return month or year is numerically zero.' @('frm1702RT:ddlPg1I2Month','frm1702RT:txtPg1I2Year') 'Please provide a valid Year Ended on Page 1 Item 2' '10666-L10672'
V '003' 'RDO value is numerically zero.' @('frm1702RT:drpPg1Pt1I7RDOCode') 'Please select a RDO code on Page 1 Part 1 Item 7' '10673-L10679'
V '004' 'Date of incorporation is blank.' @('frm1702RT:txtPg1Pt1I10') 'Please provide a valid Date of Incorporation/Organization on Page 1 Part I Item 10' '10680-L10686'
V '005' 'Registered name is blank.' @('frm1702RT:txtPg1Pt1I8Name1') 'Please provide a Registered Name on Page 1 Part I Item 8' '10687-L10693'
V '006' 'Registered address line 1 is blank.' @('frm1702RT:txtPg1Pt1I9Address1') 'Please provide a Registered Address on Page 1 Part I Item 9' '10694-L10700'
V '007' 'Contact number is blank.' @('frm1702RT:txtPg1Pt1I11Contact') 'Please provide a valid Contact Number on Page 1 Part I Item 11' '10701-L10707'
V '008' 'Email is blank.' @('frm1702RT:txtPg1Pt1I12Email') 'Please provide a valid Email Address on Page 1 Part I Item 12' '10708-L10714' 'official-bug-compatible' 'Validate checks presence only; format is a separate change handler.' 'Run the same email-format rule during final validation.'
V '009' 'Item 16 is negative and no overpayment disposition is selected.' @('frm1702RT:txtPg1Pt2I16NetTax','frm1702RT:rdoPg1Pt2I21OverpaymentRefunded','frm1702RT:rdoPg1Pt2I21OverpaymentIssued','frm1702RT:rdoPg1Pt2I21OverpaymentCarried') 'Please select an option for Overpayment Radio Button on Page 1 Part II ' '10717-L10725'
V '010' 'Item 21 is negative and no overpayment disposition is selected.' @('frm1702RT:txtPg1Pt2I21TotalAmount','frm1702RT:rdoPg1Pt2I21OverpaymentRefunded','frm1702RT:rdoPg1Pt2I21OverpaymentIssued','frm1702RT:rdoPg1Pt2I21OverpaymentCarried') 'Please select an option for Overpayment Radio Button on Page 1 Part' '10726-L10734' 'official-bug-compatible' 'This duplicates Item 16 logic with truncated wording.' 'Evaluate disposition once against the authoritative overpayment amount.'
V '011' 'Item 23 amount is nonzero and any linked payment detail is blank.' @('frm1702RT:txtPg1Pt3I23DebitMemoC1','frm1702RT:txtPg1Pt3I23DebitMemoC2','frm1702RT:txtPg1Pt3I23DebitMemoC3Date','frm1702RT:txtPg1Pt3I23DebitMemoC4Amount') 'Please provide data on Page 1 Part III Item 26.' '10737-L10743' 'incorrect-official-behavior' 'The helper is given Item 26 for printed Item 23.' 'Report Item 23.'
V '012' 'Item 24 amount is nonzero and any linked payment detail is blank.' @('frm1702RT:txtPg1Pt3I24CheckC1','frm1702RT:txtPg1Pt3I24CheckC2','frm1702RT:txtPg1Pt3I24CheckC3Date','frm1702RT:txtPg1Pt3I24CheckC4Amount') 'Please provide data on Page 1 Part III Item 27.' '10744-L10750' 'incorrect-official-behavior' 'The helper is given Item 27 for printed Item 24.' 'Report Item 24.'
V '013' 'Item 25 amount is nonzero and any linked payment detail is blank.' @('frm1702RT:txtPg1Pt3I25TaxDebitC2','frm1702RT:txtPg1Pt3I25TaxDebitDate','frm1702RT:txtPg1Pt3I25TaxDebitC4Amount') 'Please provide data on Page 1 Part III Item 28.' '10751-L10757' 'incorrect-official-behavior' 'The helper is given Item 28 for printed Item 25.' 'Report Item 25.'
V '014' 'Item 26 amount is nonzero and any linked payment detail is blank.' @('frm1702RT:txtPg1Pt3I26Others','frm1702RT:txtPg1Pt3I26OthersC1','frm1702RT:txtPg1Pt3I26OthersC2','frm1702RT:txtPg1Pt3I26OthersC3Date','frm1702RT:txtPg1Pt3I26OthersC4Amount') 'Please provide data on Page 1 Part III Item 29.' '10758-L10764' 'incorrect-official-behavior' 'The helper is given Item 29 for printed Item 26.' 'Report Item 26.'
V '015' 'Nonzero payment-detail sum differs from Total Amount Payable.' @('frm1702RT:txtPg1Pt3I23DebitMemoC4Amount','frm1702RT:txtPg1Pt3I24CheckC4Amount','frm1702RT:txtPg1Pt3I25TaxDebitC4Amount','frm1702RT:txtPg1Pt3I26OthersC4Amount','frm1702RT:txtPg1Pt2I21TotalAmount') 'Sum of Amount fields in Details of Payment (Page 1 Part III) Segment must be equal to TOTAL AMOUNT PAYABLE' '10765-L10773'

$descriptionRules = @(
    @('016','txtPg2Pt4I53C2','txtPg2Pt4I53C1','Page 2 Part 4 Item 53','10776-L10782'),
    @('017','txtPg2Pt4I54C2','txtPg2Pt4I54C1','Page 2 Part 4 Item 54','10783-L10789'),
    @('019','txtPg3Sc1I17dC2','txtPg3Sc1I17dC1','Page 3 Schedule 1 Item 17d','10798-L10804'),
    @('020','txtPg3Sc1I17eC2','txtPg3Sc1I17eC1','Page 3 Schedule 1 Item 17e','10805-L10811'),
    @('021','txtPg3Sc1I17fC2','txtPg3Sc1I17fC1','Page 3 Schedule 1 Item 17f','10812-L10818'),
    @('022','txtPg3Sc1I17gC2','txtPg3Sc1I17gC1','Page 3 Schedule 1 Item 17g','10819-L10825'),
    @('023','txtPg3Sc1I17hC2','txtPg3Sc1I17hC1','Page 3 Schedule 1 Item 17g','10826-L10832'),
    @('024','txtPg3Sc1I17iC2','txtPg3Sc1I17iC1','Page 3 Schedule 1 Item 17i','10833-L10839'),
    @('025','txtPg3Sc2I1C3','txtPg3Sc2I1C1,txtPg3Sc2I1C2','Page 3 Schedule 2 Item 1','10840-L10846'),
    @('026','txtPg3Sc2I2C3','txtPg3Sc2I2C1,txtPg3Sc2I2C2','Page 3 Schedule 2 Item 2','10847-L10853'),
    @('027','txtPg3Sc2I3C3','txtPg3Sc2I3C1,txtPg3Sc2I3C2','Page 3 Schedule 2 Item 3','10854-L10860'),
    @('028','txtPg3Sc2I4C3','txtPg3Sc2I4C1,txtPg3Sc2I4C2','Page 3 Schedule 2 Item 4','10861-L10867'),
    @('030','txtPg4Sc3AI4C2','txtPg4Sc3AI4C1','Page 4 Schedule 3A Item 4','10876-L10882'),
    @('031','txtPg4Sc3AI5C2','txtPg4Sc3AI5C1','Page 4 Schedule 3A Item 5','10883-L10889'),
    @('032','txtPg4Sc3AI6C2','txtPg4Sc3AI6C1','Page 4 Schedule 3A Item 6','10890-L10896'),
    @('033','txtPg4Sc3AI7C2','txtPg4Sc3AI7C1','Page 4 Schedule 3A Item 7','10897-L10903'),
    @('034','txtPg4Sc5I2C2','txtPg4Sc5I2C1','Page 4 Schedule 5 Item 2','10904-L10910'),
    @('035','txtPg4Sc5I3C2','txtPg4Sc5I3C1','Page 4 Schedule 5 Item 3','10911-L10917'),
    @('036','txtPg4Sc5I5C2','txtPg4Sc5I5C1','Page 4 Schedule 5 Item 5','10918-L10924'),
    @('037','txtPg4Sc5I6C2','txtPg4Sc5I6C1','Page 4 Schedule 5 Item 6','10925-L10931'),
    @('038','txtPg4Sc5I7C2','txtPg4Sc5I7C1','Page 4 Schedule 5 Item 7','10932-L10938'),
    @('039','txtPg4Sc5I8C2','txtPg4Sc5I8C1','Page 4 Schedule 5 Item 8','10939-L10945')
)
foreach ($entry in $descriptionRules) {
    $assessment = if ($entry[0] -eq '023') { 'incorrect-official-behavior' } else { 'verified-correct' }
    $recommended = if ($entry[0] -eq '023') { 'Report Schedule 1 Item 17h, not 17g.' } else { 'Require linked description/legal-basis cells when amount is nonzero.' }
    V $entry[0] "Amount is nonzero and one or more linked description fields are blank ($($entry[3]))." @(($entry[1..2] -join ',') -split ',') "Please provide data on $($entry[3])." $entry[4] $assessment 'validate_nullDescription emits the message and returns false.' $recommended
}
V '018' 'Item 35 is positive, Part V Item 57 is zero, and itemized deduction is selected.' @('frm1702RT:txtPg2Pt4I35SpecialAllowable','frm1702RT:txtPg2Pt5I57SpecialAllowable','frm1702RT:rdoPg1Pt1I13ItemizedDeduction') 'Please provide a value for this field (Part V Item 57)' '10790-L10797'
V '029' 'Schedule 3 gross income is greater than ordinary allowable deductions.' @('frm1702RT:txtPg4Sc3I1GrossIncome','frm1702RT:txtPg4Sc3I2TotalDeductions') 'In Page 4 Schedule 3 Computation for NOLCO, Gross Income should not be greater than Ordinary Allowable Deductions.' '10868-L10875' 'official-bug-compatible' 'The wording describes the opposite relationship from the usual net-operating-loss condition even though the branch checks gross > deductions.' 'Re-derive the NOLCO eligibility condition from the governing revision.'
V '040' 'Page 2 Item 39 and Page 4 Schedule V Item 10 strings are not exactly equal.' @('frm1702RT:txtPg2Pt4I39NetTaxable','frm1702RT:txtPg4Sc5I10NetTaxableIncome') 'Item Page 4 Schedule V Item 10 must be equal Item 39 on Page 2 Part IV' '10946-L10953' 'official-bug-compatible' 'The code compares formatted strings rather than parsed numeric values.' 'Compare normalized decimal values.'
foreach ($row in 1..3) {
    V ("04{0}" -f $row) "Schedule IV row $row computed balance is negative." @("frm1702RT:txtPg4Sc4I${row}C4","frm1702RT:txtPg4Sc4I${row}C5","frm1702RT:txtPg4Sc4I${row}C6","frm1702RT:txtPg4Sc4I${row}C7","frm1702RT:txtPg4Sc4I${row}C8") 'Page 4, Schedule IV: The sum of Columns D,E & F should not be greater than the amount in Column C. Please re-enter the correct values.' ("1095{0}-L109{1}" -f (3+(($row-1)*11)),(63+(($row-1)*11)))
}
Rule '1702rt-validate-success' 'validate' 44 'All prior ordered checks pass.' @('return-body') 'Controls are disabled; Edit, upload, Final Copy and Add-more buttons are enabled.' 'No rejection.' 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validate:L10986-L10967')

Rule '1702rt-input-email' 'blur/change' $null 'Email helper regex rejects the value.' @('frm1702RT:txtPg1Pt1I12Email') 'Value passes the regex.' 'Alert is shown and the field is cleared.' 'You have entered an invalid email address format!' @('official-hta-runtime#validateEmail:L6852-L6862')
Rule '1702rt-input-date-format' 'blur/change' $null 'Date fails the official parser or round-trip check.' @('date-fields') 'MM/DD/YYYY value round-trips.' 'Alert is shown and value is cleared.' 'Please provide a valid date. (MM/DD/YYYY format)' @('official-hta-runtime#validateDate:L6994-L7060')
Rule '1702rt-input-date-future' 'blur/change' $null 'Parsed date is after current system date.' @('date-fields') 'Date is not in the future.' 'Alert is shown and value is cleared.' 'This date cannot be a future date.' @('official-hta-runtime#validateDate:L6994-L7060')
Rule '1702rt-input-tin-short' 'blur/change' $null 'A nonblank TIN segment contains fewer than three characters and checkFull is false.' @('tin-segment-fields') 'Three-character segment passes.' 'Alert is shown and the segment is selected, but the helper still returns true.' 'Please provide a valid TIN (must have 3 numbers per box).' @('official-hta-runtime#validateTIN:L7191-L7223') 'incorrect-official-behavior' 'The assignment isValid=false is commented out.' 'Reject malformed segments and validate the complete taxpayer TIN checksum.'
Rule '1702rt-validate-tin-omitted' 'validate' $null 'Validate is invoked with any taxpayer TIN shape or checksum.' @('taxpayer-tin-fields') 'Validate does not inspect taxpayer TIN.' 'No TIN-specific rejection occurs.' $null @('official-hta-runtime#validate:L10657-L10968','official-hta-runtime#validateTIN:L7191-L7223') 'incorrect-official-behavior' 'No validateTIN call exists in the ordered Validate function.' 'Require exact segment/branch shape and checksum before finalization.'
Rule '1702rt-input-ctc-year' 'blur/change' $null 'The stale CTC helper would reject an Item 23 year different from current system year.' @('nonexistent-ctc-fields') 'No active call site or matching control exists.' 'Unreachable.' $null @('official-hta-runtime#validateDateOfIssue:L7313-L7385','official-hta-runtime#no-call-site') 'obsolete' 'The helper references fields absent from the 258-key inventory and is never called.' 'Do not implement this stale branch for 1702RTv2018C.'
Rule '1702rt-input-year-minimum' 'blur/change' $null 'Two-digit filing year resolves below 2018.' @('frm1702RT:txtPg1I2Year') '2018 or later continues.' 'Alert and clear.' 'Invalid Year. Year should not be earlier than 2018.' @('official-hta-runtime#validateYearEnd:L7517-L7619')
Rule '1702rt-input-fiscal-future' 'blur/change' $null 'Fiscal return end is after the current month/year.' @('frm1702RT:rdoPg1I1Fiscal','frm1702RT:ddlPg1I2Month','frm1702RT:txtPg1I2Year') 'Not future.' 'Alert and clear.' 'Date (Page 1 Item 2) cannot be greater than current date when filing for Fiscal Year.' @('official-hta-runtime#validateYearEnd:L7517-L7619')
Rule '1702rt-input-fiscal-december' 'blur/change' $null 'Fiscal return month is December.' @('frm1702RT:rdoPg1I1Fiscal','frm1702RT:ddlPg1I2Month') 'January through November.' 'Alert and clear.' 'Date (Page 1 Item 2) Month cannot be equal to December.' @('official-hta-runtime#validateYearEnd:L7517-L7619')
Rule '1702rt-input-calendar-regular-current' 'blur/change' $null 'Calendar, not short period, and year is current or later.' @('frm1702RT:rdoPg1I1Calendar','frm1702RT:rdoPg1I4ShortPeriodNo','frm1702RT:txtPg1I2Year') 'Year is before current.' 'Alert and clear.' 'Year (Page 1 Item 2) cannot be greater than or equal to current year when filing for Calendar Year.' @('official-hta-runtime#validateYearEnd:L7517-L7619')
Rule '1702rt-input-calendar-short-future' 'blur/change' $null 'Calendar short-period end is later than current month/year.' @('frm1702RT:rdoPg1I1Calendar','frm1702RT:rdoPg1I4ShortPeriodYes','frm1702RT:ddlPg1I2Month','frm1702RT:txtPg1I2Year') 'Not future.' 'Alert and clear.' 'Month (Page 1 Item 2) cannot be greater than current month date when filing for Calendar Year and Short Period Return.' @('official-hta-runtime#validateYearEnd:L7517-L7619')
Rule '1702rt-input-calendar-short-december' 'blur/change' $null 'Calendar short-period month is December.' @('frm1702RT:rdoPg1I1Calendar','frm1702RT:rdoPg1I4ShortPeriodYes','frm1702RT:ddlPg1I2Month') 'January through November.' 'Alert and clear.' 'Month (Page 1 Item 2) cannot be equal to december when filing for Calendar Year and Short Period Return.' @('official-hta-runtime#validateYearEnd:L7517-L7619')
Rule '1702rt-input-year-tax-rate-state' 'blur/change' $null 'Resolved taxable year crosses 2020.' @('frm1702RT:txtPg1I2Year','frm1702RT:Pg2Pt4I40IncomeTaxRate','frm1702RT:txtPg2Pt4I41IncomeTaxDue','frm1702RT:txtPg2Pt4I42MinimumCorporate','frm1702RT:txtPg2Pt5I57SpecialAllowable') 'For 2020+, four rate/tax fields are enabled; before 2020 they are disabled.' 'No alert.' $null @('official-hta-runtime#global-taxableYear:L7502-L7516','official-hta-runtime#validateYearEnd:L7517-L7619') 'official-bug-compatible' 'Global taxableYear starts as current two-digit year minus one; load declares a different block-scoped local variable, so state depends on later handlers.' 'Derive tax treatment solely from normalized return period, not system-time globals.'

Rule '1702rt-input-incorp-after-filing' 'blur/change' $null 'Date of incorporation month/year is after filing period.' @('frm1702RT:txtPg1Pt1I10','frm1702RT:ddlPg1I2Month','frm1702RT:txtPg1I2Year') 'Incorporation is not after filing period.' 'Alert, clear, focus.' 'Date of Incorporation cannot be greater than Page 1 Item 2 Date.' @('official-hta-runtime#checkDateOfIncorporation:L7736-L7810')
Rule '1702rt-input-incorp-fourth-year' 'blur/change' $null 'Filing year minus incorporation year equals four and IC 055 is unchecked.' @('frm1702RT:txtPg1Pt1I10','frm1702RT:rdoPg1I5Atc') 'IC 055 is checked.' 'Alert and automatic check.' $null @('official-hta-runtime#checkDateOfIncorporation:L7736-L7810') 'official-bug-compatible' 'Dynamic message says the ATC will be marked; elapsed time ignores month in this branch.' 'Use the legally defined MCIT applicability date.'
Rule '1702rt-input-incorp-less-four' 'blur/change' $null 'Filing year minus incorporation year is below four.' @('frm1702RT:txtPg1Pt1I10','frm1702RT:rdoPg1I5Atc') 'IC 055 remains disabled and unchecked.' 'Alert and false return.' $null @('official-hta-runtime#checkDateOfIncorporation:L7736-L7810') 'official-bug-compatible' 'The branch condition additionally requires IC 055 already false, and wording says a mark will be removed.' 'Use a deterministic applicability calculation.'
Rule '1702rt-input-incorp-four-plus' 'blur/change' $null 'Elapsed time is at least four years and IC 055 is unchecked.' @('frm1702RT:txtPg1Pt1I10','frm1702RT:rdoPg1I5Atc') 'IC 055 is checked.' 'Alert and automatic check.' $null @('official-hta-runtime#checkDateOfIncorporation:L7736-L7810') 'official-bug-compatible' 'Dynamic message says IC 055 will be marked.' 'Use a deterministic applicability calculation.'

Rule '1702rt-modal-two-description' 'input' $null 'A two-column modal row description is blank.' @('two-column-modal-C1-families') 'Description is nonblank.' 'Add/Save is blocked.' 'Page {current page} Item {item}.{row} Description should not be blank.' @('official-hta-runtime#addRowModalTable:L5329-L5443','official-hta-runtime#saveModalTable:L5444-L5688')
Rule '1702rt-modal-two-amount' 'input' $null 'A two-column modal row amount is zero.' @('two-column-modal-C2-families') 'Amount is nonzero.' 'Add/Save is blocked.' 'Page {current page} Item {item}.{row} Amount should not be zero.' @('official-hta-runtime#addRowModalTable:L5329-L5443','official-hta-runtime#saveModalTable:L5444-L5688')
Rule '1702rt-modal-three-description' 'input' $null 'Schedule 2 Item 4 modal description is blank.' @('frm1702RT:txtPg3Sc2I4.{N>=1}C1') 'Description is nonblank.' 'Add/Save is blocked.' 'Page {current page} Item {item}.{row} Description should not be blank.' @('official-hta-runtime#addRowModalTable:L5329-L5443','official-hta-runtime#saveModalTable:L5444-L5688')
Rule '1702rt-modal-three-basis' 'input' $null 'Schedule 2 Item 4 legal basis is blank.' @('frm1702RT:txtPg3Sc2I4.{N>=1}C2') 'Legal basis is nonblank.' 'Add/Save is blocked.' 'Page {current page} Item {item}.{row} Legal Basis should not be blank.' @('official-hta-runtime#addRowModalTable:L5329-L5443','official-hta-runtime#saveModalTable:L5444-L5688')
Rule '1702rt-modal-three-amount' 'input' $null 'Schedule 2 Item 4 amount is zero.' @('frm1702RT:txtPg3Sc2I4.{N>=1}C3') 'Amount is nonzero.' 'Add/Save is blocked.' 'Page {current page} Item {item}.{row} Amount should not be zero.' @('official-hta-runtime#addRowModalTable:L5329-L5443','official-hta-runtime#saveModalTable:L5444-L5688')
Rule '1702rt-modal-six-add' 'input' $null 'NOLCO six-column Add Row sees blank C1.' @('frm1702RT:txtPg4Sc3AI7.{N>=1}C1') 'C1 is nonblank.' 'Add is blocked.' 'Please fill-up all fields before you can add more.' @('official-hta-runtime#addRowModalTable:L5329-L5443') 'official-bug-compatible' 'Despite the wording, only C1 is checked.' 'Validate each required column explicitly.'
Rule '1702rt-modal-six-save' 'input' $null 'NOLCO six-column Save sees C1 nonblank but C2-C5 blank/invalid.' @('frm1702RT:txtPg4Sc3AI7.{N>=1}C1','frm1702RT:txtPg4Sc3AI7.{N>=1}C2','frm1702RT:txtPg4Sc3AI7.{N>=1}C3','frm1702RT:txtPg4Sc3AI7.{N>=1}C4','frm1702RT:txtPg4Sc3AI7.{N>=1}C5') 'Row is retained based on C1.' 'No validation rejection occurs.' $null @('official-hta-runtime#saveModalTable:L5444-L5688') 'incorrect-official-behavior' 'Save does not validate C2-C5 before computing C6.' 'Validate all NOLCO row inputs and computed balance.'
Rule '1702rt-modal-success' 'input' $null 'Modal Save completes.' @('modal-row-families') 'Repository arrays and subtotals update.' 'No rejection.' 'Data has been saved successfully!' @('official-hta-runtime#saveModalTable:L5444-L5688')
Rule '1702rt-obsolete-page8-modal' 'input' $null 'Dead Page 8 Schedule 12/13 branches are inspected.' @('nonexistent-page8-fields') 'No matching controls or active call sites exist in this four-page HTA.' 'Unreachable.' $null @('official-hta-runtime#AddrowChecking_ModalColumnRT:L6069-L6292','official-hta-runtime#four-page-form') 'obsolete' 'Stale validation code is retained from another form/revision.' 'Do not create fields or validations for unreachable controls.'
Rule '1702rt-resource-gserializer' 'input' $null 'HTA requests ../js/gserializer.js.' @('runtime-package') 'Resource would load if present.' 'File is absent in the extracted package.' $null @('official-hta-runtime#script-src:gserializer.js','runtime-package#missing:js/gserializer.js') 'official-bug-compatible' 'Package continues using other serializer code.' 'Do not depend on the missing resource.'
Rule '1702rt-resource-jxboxprinting' 'input' $null 'HTA requests ../js/jxboxprinting.js.' @('runtime-package') 'Resource would load if present.' 'File is absent in the extracted package.' $null @('official-hta-runtime#script-src:jxboxprinting.js','runtime-package#missing:js/jxboxprinting.js') 'official-bug-compatible' 'Package continues using other print code.' 'Record the missing dependency and avoid relying on it.'
Rule '1702rt-resource-testjs' 'input' $null 'HTA requests ../js/lib/test.js.' @('runtime-package') 'Resource would load if present.' 'File is absent in the extracted package.' $null @('official-hta-runtime#script-src:lib/test.js','runtime-package#missing:js/lib/test.js') 'official-bug-compatible' 'A test resource is linked by the production HTA but absent.' 'Do not reproduce the dependency.'
Rule '1702rt-final-001' 'final-copy' 1 'Final Copy is requested after successful validation.' @('return-body','txtFinalFlag') 'Encryption/profile flow proceeds.' 'Confirmation, profile, or connectivity can stop it.' $null @('official-hta-runtime#saveEncryptedProfile','official-hta-runtime#btnFinalCopy') 'official-bug-compatible' 'Final copy is coupled to profile/connectivity behavior.' 'Separate deterministic offline finalization from transport.'
Rule '1702rt-submit-001' 'submit' 1 'Online send is invoked.' @('return-body') 'Encrypted payload is prepared.' 'Online submission was not exercised.' $null @('official-hta-runtime#submit-functions','shared-1702rt-js') 'unverified' 'Source-derived only; no online submission was attempted.' 'Keep local validation independently testable.' 'medium'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    first_error_behavior='Save accumulates five narrow errors but alerts only errorList[0]. Validate executes in source order and returns on the first active failure. Blur/change/modal handlers can alert and mutate state before either phase.'
    rules=$rules
})

$calcs = [Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Outputs,[string[]]$Inputs,[string]$Formula,[string]$Trigger,[string[]]$Depends,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Use decimal/integer arithmetic and recompute from authoritative inputs.') {
    $calcs.Add([pscustomobject][ordered]@{
        calculation_id=$Id; outputs=$Outputs; inputs=$Inputs; condition=$null; official_formula=$Formula
        rounding='Official helpers strip commas/parentheses and generally format whole pesos; exact helper-specific behavior is retained in source references.'
        trigger=$Trigger; depends_on=$Depends; source_refs=$Refs; assessment=$Assessment
        recommended_app_behavior=$Recommended; confidence='high'
    })
}
Calc '1702rt-item16-net-tax' @('frm1702RT:txtPg1Pt2I16NetTax') @('frm1702RT:txtPg1Pt2I14IncomeTax','frm1702RT:txtPg1Pt2I15TotalTaxCredits') '16 = 14 - 15.' 'computeP1Pt2I16' @() @('official-hta-runtime#computeP1Pt2I16:L8176-L8180')
Calc '1702rt-item20-penalties' @('frm1702RT:txtPg1Pt2I20TotalPenalties') @('frm1702RT:txtPg1Pt2I17Surcharge','frm1702RT:txtPg1Pt2I18Interest','frm1702RT:txtPg1Pt2I19Compromise') '20 = 17 + 18 + 19.' 'computeP1Pt2I20' @() @('official-hta-runtime#computeP1Pt2I20:L8181-L8185')
Calc '1702rt-item21-total-payable' @('frm1702RT:txtPg1Pt2I21TotalAmount') @('frm1702RT:txtPg1Pt2I16NetTax','frm1702RT:txtPg1Pt2I20TotalPenalties') 'If Item 16 >= 0, 21 = 16 + 20; if 16 < 0 and penalties > 0, 21 = penalties; otherwise 21 = negative Item 16.' 'computeP1Pt2I21' @('1702rt-item16-net-tax','1702rt-item20-penalties') @('official-hta-runtime#computeP1Pt2I21:L8186-L8216') 'official-bug-compatible' 'Review whether penalties-only output and overpayment disposition should be represented separately.'
Calc '1702rt-item29-gross-profit' @('frm1702RT:txtPg2Pt4I29NetSales') @('frm1702RT:txtPg2Pt4I27Sales','frm1702RT:txtPg2Pt4I28LessSales') '29 = 27 - 28.' 'computeP2Pt4I29' @() @('official-hta-runtime#computeP2Pt4I29:L8217-L8225')
Calc '1702rt-item31-net-other-income' @('frm1702RT:txtPg2Pt4I31GrossIncome') @('frm1702RT:txtPg2Pt4I29NetSales','frm1702RT:txtPg2Pt4I30LessCost') '31 = 29 - 30.' 'computeP2Pt4I31' @('1702rt-item29-gross-profit') @('official-hta-runtime#computeP2Pt4I31:L8226-L8231')
Calc '1702rt-item33-total-gross' @('frm1702RT:txtPg2Pt4I33TotalGross') @('frm1702RT:txtPg2Pt4I31GrossIncome','frm1702RT:txtPg2Pt4I32AddOtherTaxable') '33 = 31 + 32; non-loss state clears NOLCO source fields.' 'computeP2Pt4I33' @('1702rt-item31-net-other-income') @('official-hta-runtime#computeP2Pt4I33:L8232-L8253')
Calc '1702rt-schedule1-copy-and-loss' @('frm1702RT:txtPg2Pt4I34OrdinaryAllowable','frm1702RT:txtPg4Sc3I1GrossIncome','frm1702RT:txtPg4Sc3I2TotalDeductions','frm1702RT:txtPg4Sc3I3NetOperatingLoss') @('frm1702RT:txtPg3Sc1I18TotalOrdinaryAllowable','frm1702RT:txtPg2Pt4I33TotalGross') 'Copy Schedule 1 Item 18 to Item 34/deductions, copy Item 33 to gross, and compute net operating loss as deductions minus gross when positive.' 'computeItem18and33' @('1702rt-item33-total-gross','1702rt-schedule1-total') @('official-hta-runtime#computeItem18and33:L8254-L8276')
Calc '1702rt-item37-itemized-total' @('frm1702RT:txtPg2Pt4I37TotalItemized') @('frm1702RT:txtPg2Pt4I34OrdinaryAllowable','frm1702RT:txtPg2Pt4I35SpecialAllowable','frm1702RT:txtPg2Pt4I36Nolco') 'When itemized, 37 = 34 + 35 + 36; otherwise zero.' 'computeP2Pt4I37' @('1702rt-schedule1-copy-and-loss','1702rt-schedule2-total','1702rt-nolco-total') @('official-hta-runtime#computeP2Pt4I37:L8277-L8290')
Calc '1702rt-item38-osd' @('frm1702RT:txtPg2Pt4I38OptionalStandard') @('frm1702RT:txtPg2Pt4I33TotalGross') 'When optional standard deduction is selected, 38 = 40% of Item 33; otherwise zero.' 'computeP2Pt4I38' @('1702rt-item33-total-gross') @('official-hta-runtime#computeP2Pt4I38:L8291-L8305')
Calc '1702rt-item39-net-taxable' @('frm1702RT:txtPg2Pt4I39NetTaxable','frm1702RT:txtPg4Sc3AI4C5','frm1702RT:txtPg4Sc3AI4C6') @('frm1702RT:txtPg2Pt4I33TotalGross','frm1702RT:txtPg2Pt4I37TotalItemized','frm1702RT:txtPg2Pt4I38OptionalStandard') '39 = 33 - 38 under OSD, otherwise 33 - 37; also updates current-year NOLCO cells.' 'computeP2Pt4I39' @('1702rt-item37-itemized-total','1702rt-item38-osd') @('official-hta-runtime#computeP2Pt4I39:L8306-L8326')
Calc '1702rt-item41-rate-tax' @('frm1702RT:txtPg2Pt4I41IncomeTaxDue') @('frm1702RT:txtPg2Pt4I39NetTaxable','frm1702RT:Pg2Pt4I40IncomeTaxRate') 'For 2020+, tax due uses user-visible rate / 100 times Item 39, with nonpositive net forced to zero; before 2020 it uses 30%.' 'computeP2Pt4I40/computeP2Pt4I41' @('1702rt-item39-net-taxable') @('official-hta-runtime#computeP2Pt4I40:L8327-L8334','official-hta-runtime#computeP2Pt4I41:L8335-L8358') 'official-bug-compatible' 'Bind revision-appropriate statutory rates rather than trusting an editable rate field.'
Calc '1702rt-item42-mcit' @('frm1702RT:txtPg2Pt4I42MinimumCorporate') @('frm1702RT:txtPg2Pt4I33TotalGross','frm1702RT:txtPg2Pt4I39NetTaxable','frm1702RT:rdoPg1I5Atc') 'Before 2020 and when IC 055 is selected, MCIT is 2% of gross; 2020+ leaves the enabled value user-controlled except nonpositive net forces zero.' 'computeP2Pt4I42' @('1702rt-item33-total-gross','1702rt-item39-net-taxable') @('official-hta-runtime#computeP2Pt4I42:L8359-L8379') 'official-bug-compatible' 'Derive MCIT rate/applicability from the tax period and law.'
Calc '1702rt-item43-total-income-tax' @('frm1702RT:txtPg2Pt4I43TotalIncomeTax','frm1702RT:txtPg1Pt2I14IncomeTax') @('frm1702RT:txtPg2Pt4I41IncomeTaxDue','frm1702RT:txtPg2Pt4I42MinimumCorporate') '43 is the greater of regular income tax and MCIT, then copies to Page 1 Item 14.' 'computeP2Pt4I43' @('1702rt-item41-rate-tax','1702rt-item42-mcit') @('official-hta-runtime#computeP2Pt4I43:L8380-L8415')
Calc '1702rt-item55-tax-credits' @('frm1702RT:txtPg2Pt4I55TotalTaxCredits','frm1702RT:txtPg1Pt2I15TotalTaxCredits') @('page2-items-44-through-54') '55 = sum Items 44 through 54 and copy to Page 1 Item 15.' 'computeP2Pt4I55TotalTaxCredits' @() @('official-hta-runtime#computeP2Pt4I55TotalTaxCredits:L8416-L8423')
Calc '1702rt-item56-net-tax' @('frm1702RT:txtPg2Pt4I56NetTax','frm1702RT:txtPg1Pt2I16NetTax') @('frm1702RT:txtPg2Pt4I43TotalIncomeTax','frm1702RT:txtPg2Pt4I55TotalTaxCredits') '56 = 43 - 55 and copy to Page 1 Item 16.' 'computeP2Pt4I56' @('1702rt-item43-total-income-tax','1702rt-item55-tax-credits') @('official-hta-runtime#computeP2Pt4I56:L8424-L8436')
Calc '1702rt-item57-special-tax' @('frm1702RT:txtPg2Pt5I57SpecialAllowable') @('frm1702RT:txtPg2Pt4I35SpecialAllowable','frm1702RT:Pg2Pt4I40IncomeTaxRate') 'Only before 2020, Item 57 = Item 35 times income-tax rate / 100; for 2020+ this function leaves Item 57 untouched.' 'computeP2Pt5I57' @('1702rt-schedule2-total') @('official-hta-runtime#computeP2Pt5I57:L8437-L8448') 'official-bug-compatible' 'Make the post-2020 source and validation of Item 57 explicit.'
Calc '1702rt-item59-total-special-tax' @('frm1702RT:txtPg2Pt5I59TotalTax') @('frm1702RT:txtPg2Pt5I57SpecialAllowable','frm1702RT:txtPg2Pt5I58AddSpecialTax') '59 = 57 + 58.' 'computeP2Pt5I59' @('1702rt-item57-special-tax') @('official-hta-runtime#computeP2Pt5I59:L8449-L8454')
Calc '1702rt-schedule1-total' @('frm1702RT:txtPg3Sc1I18TotalOrdinaryAllowable') @('Schedule 1 Items 1 through 17i, including modal rows') 'Sum all ordinary allowable deduction rows into Item 18.' 'computeP3Sc1I18TotalOrdinaryAllowable' @() @('official-hta-runtime#computeP3Sc1I18TotalOrdinaryAllowable:L8455-L8469')
Calc '1702rt-schedule2-total' @('frm1702RT:txtPg3Sc2I5TotalSpecialAllowable','frm1702RT:txtPg2Pt4I35SpecialAllowable') @('Schedule 2 Items 1 through 4, including modal rows') 'Sum special allowable deductions into Item 5 and copy to Page 2 Item 35.' 'computeP3Sc2I5TotalSpecialAllowable' @() @('official-hta-runtime#computeP3Sc2I5TotalSpecialAllowable:L8485-L8496')
Calc '1702rt-nolco-row-balance' @('frm1702RT:txtPg4Sc3AI7.{N>=1}C6') @('frm1702RT:txtPg4Sc3AI7.{N>=1}C2','frm1702RT:txtPg4Sc3AI7.{N>=1}C3','frm1702RT:txtPg4Sc3AI7.{N>=1}C4','frm1702RT:txtPg4Sc3AI7.{N>=1}C5') 'C6 = C2 - (C3 + C4 + C5) for each retained modal row.' 'saveModalTable' @() @('official-hta-runtime#saveModalTable:L5444-L5688')
Calc '1702rt-nolco-total' @('frm1702RT:txtPg2Pt4I36Nolco','frm1702RT:txtPg4Sc4I8TotalNOLCO') @('Schedule 3A Items 4 through 7 applied-current-year values') 'Validate per-row applied/current and remaining balances, then sum NOLCO applied to Page 2 Item 36 and Schedule total.' 'computeP4Sc3AI8TotalNOLCO' @('1702rt-nolco-row-balance') @('official-hta-runtime#computeP4Sc3AI8TotalNOLCO:L8497-L8541') 'official-bug-compatible' 'Represent each row canonically and validate all columns before summing.'
Calc '1702rt-schedule3-net-loss' @('frm1702RT:txtPg4Sc3I3NetOperatingLoss') @('frm1702RT:txtPg4Sc3I1GrossIncome','frm1702RT:txtPg4Sc3I2TotalDeductions') '3 = 2 - 1.' 'computePg4Sc3I3NetOperatingLoss' @() @('official-hta-runtime#computePg4Sc3I3NetOperatingLoss:L8542-L8546')
Calc '1702rt-schedule4-row-balances' @('frm1702RT:txtPg4Sc4I1C8','frm1702RT:txtPg4Sc4I2C8','frm1702RT:txtPg4Sc4I3C8') @('Schedule 4 row columns C through F') 'For each row, C8 = C4 - (C5 + C6 + C7); negative balances alert.' 'computeP4Sc4I1C8/2C8/3C8' @() @('official-hta-runtime#computeP4Sc4I1C8:L8725-L8732','official-hta-runtime#computeP4Sc4I2C8:L8733-L8740','official-hta-runtime#computeP4Sc4I3C8:L8741-L8748')
Calc '1702rt-schedule4-total-excess' @('frm1702RT:txtPg4Sc4I4TotalExcessMCIT','frm1702RT:txtPg2Pt4I47ExcessMCIT') @('frm1702RT:txtPg4Sc4I1C8','frm1702RT:txtPg4Sc4I2C8','frm1702RT:txtPg4Sc4I3C8') 'Sum the three Schedule 4 balances and copy to Page 2 Item 47.' 'computeP4Sc4I4TotalExcessMCIT' @('1702rt-schedule4-row-balances') @('official-hta-runtime#computeP4Sc4I4TotalExcessMCIT:L8749-L8754')
Calc '1702rt-schedule5-item4' @('frm1702RT:txtPg4Sc5I4Total') @('Schedule 5 Items 1 through 3, including modal rows') '4 = sum Items 1 through 3.' 'computeP4Sc5I4Total' @() @('official-hta-runtime#computeP4Sc5I4Total:L8755-L8759')
Calc '1702rt-schedule5-item9' @('frm1702RT:txtPg4Sc5I9Total') @('Schedule 5 Items 5 through 8, including modal rows') '9 = sum Items 5 through 8.' 'computeP4Sc5I9Total' @() @('official-hta-runtime#computeP4Sc5I9Total:L8760-L8763')
Calc '1702rt-schedule5-item10' @('frm1702RT:txtPg4Sc5I10NetTaxableIncome') @('frm1702RT:txtPg4Sc5I4Total','frm1702RT:txtPg4Sc5I9Total') '10 = 4 - 9.' 'computeP4Sc5I10NetTaxableIncome' @('1702rt-schedule5-item4','1702rt-schedule5-item9') @('official-hta-runtime#computeP4Sc5I10NetTaxableIncome:L8764-L8775')
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema'='../../schema/calculations.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    evaluation_order=@($calcs.calculation_id); calculations=$calcs
})

$negativeRules = @($rules | Where-Object { $_.exact_message } | Select-Object -First 40)
$cases=@(); $caseNumber=0
foreach ($rule in $negativeRules) {
    $caseNumber++
    $cases += [pscustomobject][ordered]@{
        case_id=('case-{0:d2}-{1}' -f $caseNumber,$rule.rule_id); phase=$rule.phase
        mutations=@{synthetic_condition=$rule.condition}; expected_message=$rule.exact_message
        expected_behavior=$rule.official_behavior; rule_id=$rule.rule_id
    }
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
$calcCases=@(
    @{case_id='item21-positive';calculation_id='1702rt-item21-total-payable';inputs=@{item16=100;penalties=25};official_output='125'},
    @{case_id='item21-overpayment-with-penalty';calculation_id='1702rt-item21-total-payable';inputs=@{item16=-100;penalties=25};official_output='25'},
    @{case_id='osd-40-percent';calculation_id='1702rt-item38-osd';inputs=@{gross=1000000};official_output='400000'},
    @{case_id='pre2020-regular-tax';calculation_id='1702rt-item41-rate-tax';inputs=@{year=2019;net_taxable=1000000};official_output='300000'},
    @{case_id='pre2020-mcit';calculation_id='1702rt-item42-mcit';inputs=@{year=2019;gross=1000000;atc_ic055=$true};official_output='20000'},
    @{case_id='schedule4-negative';calculation_id='1702rt-schedule4-row-balances';inputs=@{c4=100;c5=50;c6=40;c7=20};official_output='-10';expected_validation='negative-balance alert'}
)
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=$calcCases})

$resources=@()
foreach ($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1') | ForEach-Object { $_.Groups['v'].Value } | Sort-Object -Unique)) {
    $full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if (Test-Path -LiteralPath $full) {
        $resources += [pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}
    } else {
        $resources += [pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}
    }
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$workflow=[ordered]@{
    '$schema'='../../schema/workflow.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    phases=@(
        @{phase='edit';official_behavior='Four-page January 2018 ENCS regular corporate income-tax return with seven active unbounded modal schedules.';source_refs=@('official-hta-runtime#APPLICATIONNAME','official-hta-runtime#loadModalTable:L5144-L5328');confidence='high'},
        @{phase='saved-draft';official_behavior='Save alerts only the first of five narrow errors and then serializes a flat 258-key control state.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L10969-L11012','xml-editable-v1');confidence='high'},
        @{phase='validated';official_behavior='Validate runs in source order and disables controls after the success alert.';source_refs=@('official-hta-runtime#validate:L10657-L10968');confidence='high'},
        @{phase='final-copy';official_behavior='Encrypted companion contains the identical 258-key inventory; finalization remains coupled to profile/connectivity paths.';source_refs=@('encrypted-field-audit-v796','official-hta-runtime#final-copy-functions');confidence='high'},
        @{phase='submitted';official_behavior='Online transport exists but was not exercised.';source_refs=@('official-hta-runtime#submit-functions','shared-1702rt-js');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='Five narrow preflight checks pass.';side_effects=@('Writes plaintext pseudo-XML with 258 keys.');source_refs=@('official-hta-runtime#initialValidateBeforeSave:L10969-L11012')},
        @{from='edit';action='Validate';to='validated';guard='All active source-ordered checks pass.';side_effects=@('Disables controls.','Enables Edit, upload, Final Copy and Add-more controls.');source_refs=@('official-hta-runtime#validate:L10657-L10968')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables applicable controls.');source_refs=@('official-hta-runtime#enableAllControl')},
        @{from='validated';action='Final Copy';to='final-copy';guard='Profile/confirmation/connectivity path permits progress.';side_effects=@('Creates encrypted artifact with matching field inventory.');source_refs=@('encrypted-field-audit-v796')},
        @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and credentials succeed.';side_effects=@('Attempts online submission; untested.');source_refs=@('shared-1702rt-js')}
    )
    prerequisites=@('January 2018 ENCS revision','Return period 2018 or later','Applicable corporate ATC and deduction method','Required schedules for claimed deductions/credits')
    required_attachments=@(
        @{attachment_id='financial-statements';label='Audited financial statements or other statements required for the filing context';required_when='Required by applicable law/instructions for the filer.';official_ui_enforcement='No local document-presence check was identified.';source_refs=@('official-form-pdf#instructions');confidence='medium'},
        @{attachment_id='tax-credit-certificates';label='Certificates or proof supporting claimed tax credits/payments';required_when='A corresponding credit is claimed.';official_ui_enforcement='Description/amount consistency is checked for some payment rows; document presence is not.';source_refs=@('official-form-pdf#instructions','official-hta-runtime#validate:L10737-L10773');confidence='medium'},
        @{attachment_id='nolco-mcit-schedules';label='Supporting NOLCO and excess-MCIT schedules';required_when='NOLCO or excess MCIT is claimed.';official_ui_enforcement='Schedule arithmetic/year checks exist; external attachment presence is not checked.';source_refs=@('official-hta-runtime#validateMCITYear:L7386-L7446','official-hta-runtime#computeP4Sc3AI8TotalNOLCO:L8497-L8541');confidence='medium'}
    )
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='Annual return deadline is governed by the filer fiscal/calendar year and applicable law; exact guide text for this revision remains unverified.';source_refs=@('official-form-pdf#instructions');confidence='low'},
        @{quarter='Q2';due_date_rule='Not a quarterly return; annual deadline applies.';source_refs=@('official-form-pdf#instructions');confidence='medium'},
        @{quarter='Q3';due_date_rule='Not a quarterly return; annual deadline applies.';source_refs=@('official-form-pdf#instructions');confidence='medium'},
        @{quarter='Q4';due_date_rule='Not a quarterly return; annual deadline applies.';source_refs=@('official-form-pdf#instructions');confidence='medium'}
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount=@($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$encryptedAsset=Asset 'xml-encrypted-v1' 'dummy-profile-encrypted-copy' $encryptedPath 'Reviewed target-revision encrypted companion; decrypted shape is audited without emitting values.'
$encryptedAsset.path=Join-Path $SourceDir '00000000000000-1702RTv2018C-122025#email-redacted#.xml'
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' 'C:\eBIRForms\BIRForms.exe' 'Installed Offline eBIRForms package 7.9.6.0.',
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1702RTv2018C, HTA version 4.7, printed January 2018 ENCS.',
    Asset 'shared-1702rt-js' 'official-linked-script' $sharedPath 'Loaded directly by the exact target HTA.',
    Asset 'xml-editable-v1' 'dummy-profile-editable-save' $plainPath 'Reviewed target-revision 258-key dummy save; values excluded.',
    $encryptedAsset,
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 ENCS Form 1702-RT.'
)
$manifest=[ordered]@{
    '$schema'='../../schema/form-manifest.schema.json'; schema_version='1.0.0'; form_id=$formId; form_code='1702RT'
    revision=$revision; revision_label='January 2018 (ENCS), corrected runtime variant'; package_version=$packageVersion; status='complete'
    official_assets=$assets
    counts=[ordered]@{
        concrete_fields=258; runtime_field_families=19; fields_total=$fields.Count; typed_fields=$fields.Count
        validation_rules=$rules.Count; confirmed_official_bugs=$bugCount; calculations=$calcs.Count
        negative_fixtures=$cases.Count; unverified_gaps=3
    }
    artifacts=[ordered]@{
        fields='fields.json'; validations='validations.json'; calculations='calculations.json'; workflow='workflow.json'
        evidence='evidence.md'; audit='audit.md'; gaps='gaps.md'
        runtime_control_fixture='fixtures/runtime-control-inventory-v796.json'
        encrypted_field_audit='fixtures/encrypted-field-audit-v796.json'
        validation_function_fixture='fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture='fixtures/calculation-function-inventory-v796.json'
        shared_function_fixture='fixtures/shared-1702rt-function-inventory-v796.json'
        resource_hash_fixture='fixtures/official-resource-hashes-v796.json'
        negative_fixtures='fixtures/negative-cases.json'; calculation_fixtures='fixtures/calculation-boundaries.json'
    }
    scope_notes=@(
        'Research only; no renderer/release changes.',
        'No source values or email address are copied.',
        'Plaintext and decrypted encrypted saves contain the same 258-key inventory; 19 active unbounded modal families are explicit.',
        'Legacy June 2013 1702RT assets and the unrelated legacy encrypted save are excluded.',
        'The installed Help1702RT.hta is June 2013 and is not treated as evidence for this January 2018 revision.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') @"
# BIR Form 1702-RT - January 2018 (ENCS), corrected runtime variant

Revision-specific Offline eBIRForms rule package for `1702RTv2018C`: 258 concrete serialized keys and 19 active unbounded modal field families. Source values and email-bearing filenames are excluded.
"@
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- Exact HTA SHA-256: $($expectedHashes.hta); `APPLICATIONNAME=1702RTv2018C`, HTA version 4.7, printed January 2018 (ENCS).
- Plaintext dummy save SHA-256: $($expectedHashes.plain); 258 unique keys; sorted inventory SHA-256 $($expectedHashes.inventory).
- Encrypted companion ciphertext SHA-256: $($expectedHashes.encrypted); in-memory decrypted SHA-256 $($expectedHashes.decrypted); the same 258-key inventory; no values emitted.
- Official PDF SHA-256: $($expectedHashes.pdf), with valid PDF magic.
- Linked `js/lib/1702RT.js` SHA-256: $($expectedHashes.shared).
- 351 controls, 292 static serializer candidates, seven active modal entry points, and 19 dynamic families are machine-inventoried.
- `gserializer.js`, `jxboxprinting.js`, and `js/lib/test.js` are linked but absent; each is preserved as a package defect.
- The installed `Help1702RT.hta` and `BIR-Form1702RT.hta` are June 2013 and deliberately excluded from this January 2018 evidence binding.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. Online submission was not exercised.
2. The installed help file is June 2013; exact January 2018 attachment/deadline prose is not locally available, so PDF-derived attachment/deadline statements remain medium/low confidence.
3. Post-2020 editable rate, MCIT, and Part V Item 57 behavior is faithfully recorded from source but needs law/revision reconciliation before recommended implementation.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- Revision/assets: **pass** - corrected January 2018 HTA, official PDF, target plaintext save, encrypted replay, package executable, and linked form script are pinned.
- Field inventory: **pass** - plaintext and decrypted encrypted inventories match at 258 unique keys; 19 active unbounded modal families are explicit.
- Validation/calculation/workflow: **pass** - source-ordered Save/Validate rules, exact observable messages, modal behavior, calculations, phase differences, and first-error behavior are recorded.
- Official defects: **pass** - $bugCount bug-compatible, incorrect, or obsolete rules are separated from recommendations.
- Resource integrity: **pass with recorded defects** - all present resources are hashed and three missing linked scripts are explicit.
- Privacy: **pass** - no values or email address copied.
- Online submit and exact revision-matched guide prose: **unverified** and explicit gaps.
"@
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 13: 1702rt-v2018c. Next: 1604C.`n"

$indexPath=Join-Path $RepoRoot 'rules\index.json'
$index=Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry=[pscustomobject][ordered]@{form_id=$formId;form_code='1702RT';revision=$revision;package_version=$packageVersion;priority=13;status='complete';path='forms/1702rt-v2018c/manifest.json'}
$index.forms=@(@($index.forms | Where-Object { $_.form_id -ne $formId }) + $entry | Sort-Object priority)
$index.updated=(Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index
"Generated ${formId}: fields=$($fields.Count), rules=$($rules.Count), calculations=$($calcs.Count), negative_cases=$($cases.Count), bug_classifications=$bugCount"
