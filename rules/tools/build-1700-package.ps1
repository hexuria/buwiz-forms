param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$SourceDir = 'C:\Mac\Home\Downloads\forms\1700',
    [string]$PlainPath = 'C:\eBIRForms\savefile\00000000000000-1700-25.xml'
)

$ErrorActionPreference = 'Stop'
$formId = '1700-v2013'
$revision = '2013-06-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1700.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1700.hta'
$sharedPath = Join-Path $ExtractedRoot 'js\lib\1700.js'
$exePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1700-v2013'
$fixtureDir = Join-Path $outDir 'fixtures'
$encryptedCandidates = @(Get-ChildItem -LiteralPath $SourceDir -File | Where-Object { $_.Name -like '*.xml' })
if ($encryptedCandidates.Count -ne 1) { throw "Expected one encrypted companion; found $($encryptedCandidates.Count)." }
$encryptedPath = $encryptedCandidates[0].FullName

foreach ($path in @($htaPath,$helpPath,$sharedPath,$exePath,$PlainPath,$encryptedPath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

function Write-Json([string]$Path,$Value) {
    [IO.File]::WriteAllText($Path,(($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine),[Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path,[string]$Value) {
    [IO.File]::WriteAllText($Path,$Value,[Text.UTF8Encoding]::new($false))
}
function Get-HashText([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-','').ToLowerInvariant() }
    finally { $sha.Dispose() }
}
function Get-Attr([string]$Tag,[string]$Name) {
    $m = [regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($m.Success) { $m.Groups[2].Value } else { $null }
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
    hta='d025bb0743123dc0dfdf8251da18c42cc7827c5c571f5b06ff8f39a62f8437ba'
    help='343726a2a1463905151e3de1f8025f8763c2998f6a8afee8917db53b5b4f7ca8'
    shared='49b5603bed5a87f94c6d9bfeb46da7399f1bbbe7ffd429cd81dc41bc46404fee'
    executable='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    plain='0a2c6a2f458b28814df2892e342d99b010669b7d5d25e6a00100508b1649a350'
    encrypted='6fbedb576e641f0a66a84bdf3f3bc273f3beeb2c9e5a76494cb6c10460c208ac'
    decrypted='1cbbf61b9038b03e41f81edb3976b1e4360353aad605a86589cbefc36687cc51'
    inventory='4821dd338ebd6c3a73d706db1ff73f7cc7e6115a15d65893f25d6e760a699904'
}
foreach ($pair in @(@($htaPath,'hta'),@($helpPath,'help'),@($sharedPath,'shared'),@($exePath,'executable'),@($PlainPath,'plain'),@($encryptedPath,'encrypted'))) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expectedHashes[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}

$hta = [IO.File]::ReadAllText($htaPath)
$plain = [IO.File]::ReadAllText($PlainPath)
$saveMatches = @([regex]::Matches($plain,'<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>'))
$keys = @($saveMatches | ForEach-Object { $_.Groups['key'].Value })
if ($keys.Count -ne 311 -or ($keys | Sort-Object -Unique).Count -ne 311) { throw "Expected 311 unique plaintext keys; found $($keys.Count)." }
if ((Get-HashText @($keys | Sort-Object)) -ne $expectedHashes.inventory) { throw 'Plaintext field inventory changed.' }

$formMatch = [regex]::Match($hta,'(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain not found.' }
$formBody = $formMatch.Groups['body'].Value
$formOffset = $formMatch.Groups['body'].Index
$scriptRanges = @([regex]::Matches($formBody,'(?is)<script\b.*?</script>'))
$controls = @()
$ordinal = 0
foreach ($m in [regex]::Matches($formBody,'(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $inside = $false
    foreach ($range in $scriptRanges) {
        if ($m.Index -ge $range.Index -and $m.Index -lt ($range.Index + $range.Length)) { $inside = $true; break }
    }
    if ($inside) { continue }
    $ordinal++
    $tag = $m.Value
    $element = $m.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $controls += [pscustomobject][ordered]@{
        ordinal=$ordinal; id=Get-Attr $tag 'id'; name=Get-Attr $tag 'name'; element=$element
        control_kind=$kind.ToLowerInvariant(); source_line=1 + [regex]::Matches($hta.Substring(0,$formOffset+$m.Index),"`n").Count
        value=Get-Attr $tag 'value'; maxlength=Get-Attr $tag 'maxlength'
        disabled=$tag -match '(?i)\bdisabled(?:\s*=|\s|>)'; readonly=$tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
}
$serializable = @($controls | Where-Object { $_.control_kind -in @('text','hidden','select','select-one','radio','checkbox','textarea') -and $_.id })
$uniqueStaticIds = @($serializable.id | Sort-Object -Unique)
if ($controls.Count -ne 391 -or $serializable.Count -ne 318 -or $uniqueStaticIds.Count -ne 315) {
    throw "Expected 391 controls/318 candidates/315 unique candidate IDs; found $($controls.Count)/$($serializable.Count)/$($uniqueStaticIds.Count)."
}
$controlById = @{}
foreach ($control in $controls) { if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control } }
$selectEnums = @{}
foreach ($m in [regex]::Matches($formBody,'(?is)<select\b(?<tag>[^>]*)>(?<body>.*?)</select>')) {
    $id = Get-Attr ('<select ' + $m.Groups['tag'].Value + '>') 'id'
    if ($id) {
        $values = @([regex]::Matches($m.Groups['body'].Value,'(?is)<option\b[^>]*value\s*=\s*(["''])(?<v>.*?)\1') | ForEach-Object { $_.Groups['v'].Value })
        $selectEnums[$id] = [object[]]$values
    }
}

$required = @(
    'frm1700:txtYear','frm1700:SourceOfIncome_1','frm1700:SourceOfIncome_2','frm1700:rdoPg1Pt1I3JointFilingYes','frm1700:rdoPg1Pt1I3JointFilingNo',
    'frm1700:txtPg1Pt1I5TIN1','frm1700:txtPg1Pt1I5TIN2','frm1700:txtPg1Pt1I5TIN3','frm1700:txtPg1Pt1I5BranchCode','frm1700:txtPg1Pt1I6RDOCode','frm1700:txtPg1Pt1I7Psoc',
    'frm1700:txtPg1Pt1I8TaxFilerLastName','frm1700:txtPg1Pt1I9Address1','frm1700:txtPg1Pt1I10DOB','frm1700:txtPg1Pt1I11EmailAddress','frm1700:txtPg1Pt1I12ContactNo',
    'frm1700:rdoPg1Pt1I13SingleInd','frm1700:rdoPg1Pt1I13MarriedInd','frm1700:rdoPg1Pt1I13LegalSeparatedInd','frm1700:rdoPg1Pt1I13WidowerInd',
    'frm1700:rdoPg1Pt1I14YesInd','frm1700:rdoPg1Pt1I14NoInd','frm1700:rdoPg1Pt2I32GovID','frm1700:rdoPg1Pt2I32CTC',
    'frm1700:txtPg1Pt2I32GovID','frm1700:txtPg1P23I33DateofIssue','frm1700:txtPg1Pt2I34Amount','frm1700:txtPg1Pt2I35PlaceofIssue'
)
$conditionalPattern = '(?i)(Spouse|Dependent|Pg4T|I18TC|modal|\{N>=|rdoP4T2|AddressChanged)'
$computedPattern = '(?i)(summation|Subtotal|txtPg1Pt2I2[3-9]|txtPg1Pt2I3[01]|txtPg2Pt4I(4A|4B|6C|9C|10C|11C|12C|13C|14C|15C|19C|20C|21$|23$|27$|28$)|txtPg3Pt5I21|txtPg3Pt5BI10)'

function FieldMeta([string]$Key,$Control,[bool]$Family) {
    $page = $null; $item = $null; $logical = 'string'; $status = 'optional'; $normalization = [string[]]@(); $enum = [object[]]@(); $computed = $false
    if ($Key -match '(?i)Pg(?<p>[1-4])') { $page = [int]$Matches.p }
    if ($Key -match '(?i)Pt\d+I(?<i>\d+)') { $item = $Matches.i }
    elseif ($Key -eq 'frm1700:txtYear') { $page=1; $item='1' }
    if ($Control -and $Control.control_kind -in @('radio','checkbox')) { $logical='boolean'; $enum=[object[]]@('true','false') }
    elseif ($Control -and $Control.control_kind -in @('select','select-one')) { $logical='enum'; if ($selectEnums.ContainsKey($Key)) { $enum=[object[]]$selectEnums[$Key] } }
    elseif ($Key -match '(?i)(TIN|BranchCode|RDOCode|Psoc)') { $logical='code' }
    elseif ($Key -match '(?i)(DOB|DateofIssue|DatePayment)') { $logical='date-string'; $normalization=[string[]]@('MM/DD/YYYY where the official handler runs') }
    elseif ($Key -match '(?i)Email') { $logical='email-string' }
    elseif ($Key -match '(?i)(ContactNo|Phone|Tel)') { $logical='phone-string' }
    elseif ($Key -eq 'frm1700:txtYear') { $logical='two-digit-tax-year' }
    elseif ($Key -match '(?i)(Amount|Income|Tax|Withhold|Credit|Surcharge|Interest|Compromise|Subtotal|summation|AAFM|C[12]$|BI(1|2|3|7A|7B|9A|9B|10))') { $logical='whole-peso-amount'; $normalization=[string[]]@('NumWithComma','parseInt','formatCurrencyWOC','NegativeValue') }
    if ($required -contains $Key) { $status='required' }
    if ($Key -match $conditionalPattern) { $status='conditional' }
    if ($Key -match $computedPattern) { $status='computed'; $computed=$true }
    if ($Key -match '^(driveSelectTPExport|txtFinalFlag|txtEnroll|ebirOnline|summation)') { if (-not $computed) { $status='hidden' } }
    if ($Family) { $status='conditional' }
    $constraints=[ordered]@{}
    if ($Control -and $Control.maxlength) { $constraints.max_length=[int]$Control.maxlength }
    if ($Family) { $constraints.index='Unbounded runtime index; see source family.' }
    [pscustomobject]@{page=$page;item=$item;logical=$logical;status=$status;normalization=$normalization;enum=$enum;computed=$computed;constraints=[pscustomobject]$constraints}
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    $control = if ($controlById.ContainsKey($key)) { $controlById[$key] } else { $null }
    $meta = FieldMeta $key $control $false
    $refs = @("xml-editable-v1#field:$key")
    if ($control) { $refs += "official-hta-runtime#control:L$($control.source_line)" } else { $refs += 'official-hta-runtime#saveXML:L7009-L7368' }
    $fields.Add([pscustomobject][ordered]@{
        field_key=$key; serialized_key=$key; serialized_occurrence=1; label=$key; page=$meta.page; item_number=$meta.item
        control_kind=if($control){$control.control_kind}else{'runtime-injected-control'}; storage_type='string'; logical_type=$meta.logical
        required=$meta.status; required_when=if($meta.status -eq 'conditional'){'Applicable party, option, schedule, or modal row is active.'}else{$null}
        enabled_when=$null; visible_when=$null; default_value=if($control){$control.value}else{$null}; empty_representation=''
        constraints=$meta.constraints; enum_values=$meta.enum; normalization=$meta.normalization; computed=$meta.computed
        calculation_id=if($meta.computed){'See calculations.json'}else{$null}; source_refs=$refs; confidence=if($control){'high'}else{'medium'}
        notes=@('Observed in the reviewed 311-key dummy save; source values are excluded.')
    })
}

$families = [Collections.Generic.List[object]]::new()
function Add-Family([string]$Key,[string]$Logical,[int]$Page,[string]$Item,[string]$Lines,[string]$Notes='Unbounded indexed runtime family materialized inside frmMain and serialized by saveXML.') {
    $families.Add([pscustomobject][ordered]@{key=$Key;logical=$Logical;page=$Page;item=$Item;lines=$Lines;notes=$Notes})
}
Add-Family 'frm1700:txtPg2Pt4I3NameOfEmployer{N>=4}' 'string' 2 '3' '12633-L12757'
Add-Family 'frm1700:rdoPg2Pt4I{N>=4}TaxFiler' 'boolean' 2 '3' '12681'
Add-Family 'frm1700:rdoPg2Pt4I{N>=4}Spouse' 'boolean' 2 '3' '12687'
foreach ($part in @('1','2','3','BranchCode')) { Add-Family "frm1700:txtPg2Pt4I3EMPTIN${part}{N>=4}" 'code' 2 '3' '12703-L12710' }
Add-Family 'frm1700:txtPg2Pt4I{N>=4}ComIncomeC1' 'whole-peso-amount' 2 '3' '12725-L12726'
Add-Family 'frm1700:txtPg2Pt4I{N>=4}TaxWithholdC2' 'whole-peso-amount' 2 '3' '12740-L12741'
Add-Family 'frm1700:txtPg2Pt4I12Description_{N>=1}C1' 'string' 2 '12' '5226'
Add-Family 'frm1700:txtPg2Pt4I12TaxFiler_{N>=1}C2' 'whole-peso-amount' 2 '12' '5229'
Add-Family 'frm1700:txtPg2Pt4I12Spouse_{N>=1}C3' 'whole-peso-amount' 2 '12' '5232'
Add-Family 'frm1700:txtPg3Pt5I6Description_{N>=1}C1' 'string' 3 '6' '5259'
Add-Family 'frm1700:txtPg3Pt5I6AAFM_{N>=1}C2' 'whole-peso-amount' 3 '6' '5262'
Add-Family 'frm1700:txtPg3Pt5I6FinalTaxWithheld_{N>=1}C3' 'whole-peso-amount' 3 '6' '5265'
foreach ($item in 7..10) { foreach ($col in 1..2) { Add-Family "frm1700:txtPg3Pt5I${item}_{N>=1}C${col}" $(if($item -le 9){'string'}else{'whole-peso-amount'}) 3 "$item" '5274-L5366' } }
Add-Family 'frm1700:txtPg3Pt5I11SaleEx1_{N>=1}C1' 'whole-peso-amount' 3 '11' '5353'
Add-Family 'frm1700:txtPg3Pt5I11SaleEx2_{N>=1}C2' 'whole-peso-amount' 3 '11' '5356'
Add-Family 'frm1700:drpPg3Pt5I12A1{N>=1}' 'enum' 3 '12' '5391'
Add-Family 'frm1700:drpPg3Pt5I12B2{N>=1}' 'enum' 3 '12' '5395'
foreach ($item in 12..14) { foreach ($col in 1..2) { Add-Family "frm1700:txtPg3Pt5I${item}_{N>=1}C${col}" $(if($item -le 13){'string'}else{'whole-peso-amount'}) 3 "$item" '5367-L5471' } }
foreach ($col in 1..2) { Add-Family "frm1700:txtPg3Pt5I15DateofIssue_{N>=1}C${col}" 'date-string' 3 '15' '5432-L5435' }
foreach ($col in 1..2) { Add-Family "frm1700:txtPg3Pt5I16{N>=1}C${col}" 'whole-peso-amount' 3 '16' '5445-L5448' }
Add-Family 'frm1700:txtPg3Pt5I17SaleEx1_{N>=1}C1' 'whole-peso-amount' 3 '17' '5458'
Add-Family 'frm1700:txtPg3Pt5I17SaleEx2_{N>=1}C2' 'whole-peso-amount' 3 '17' '5461'
foreach ($row in 1..2) { foreach ($col in 1..2) { $actualCol=if($col -eq 1){$row}else{$row+2}; Add-Family "frm1700:txtPg3Pt5I18R${row}_{N>=1}C${actualCol}" 'string' 3 '18' '5497-L5502' } }
foreach ($col in 1..2) { Add-Family "frm1700:txtPg3Pt5I19_{N>=1}C${col}" 'whole-peso-amount' 3 '19' '5512-L5515' }
Add-Family 'frm1700:txtPg3Pt5I20SaleEx1_{N>=1}C1' 'whole-peso-amount' 3 '20' '5525'
Add-Family 'frm1700:txtPg3Pt5I20SaleEx2_{N>=1}C2' 'whole-peso-amount' 3 '20' '5528'
foreach ($item in 4..7) { foreach ($col in 1..2) { Add-Family "frm1700:txtPg3Pt5BI${item}_{N>=1}C${col}" $(if($item -le 6){'string'}else{'whole-peso-amount'}) 3 "B$item" '5539-L5617' } }
foreach ($item in 8..9) { foreach ($col in 1..2) { Add-Family "frm1700:txtPg3Pt5BI${item}_{N>=1}C${col}" $(if($item -eq 8){'string'}else{'whole-peso-amount'}) 3 "B$item" '5618-L5669' } }
if ($families.Count -ne 59) { throw "Expected 59 dynamic families; found $($families.Count)." }
foreach ($family in $families) {
    [string[]]$normalization = if ($family.logical -eq 'whole-peso-amount') { @('whole-number key filter','formatDisplayAmount') } elseif ($family.logical -eq 'date-string') { @('MM/DD/YYYY') } else { @('capitalize where wired') }
    [object[]]$familyEnum = @()
    if ($family.logical -eq 'boolean') { $familyEnum = [object[]]@('true','false') }
    elseif ($family.logical -eq 'enum') { $familyEnum = [object[]]@('PS','CS') }
    $fields.Add([pscustomobject][ordered]@{
        field_key=$family.key; serialized_key=$null; serialized_occurrence=$null; label="Runtime family $($family.key)"; page=$family.page; item_number=$family.item
        control_kind='runtime-indexed-family'; storage_type='string'; logical_type=$family.logical; required='conditional'; required_when='The corresponding modal row N exists.'
        enabled_when='The corresponding party/section is active.'; visible_when='The modal row is materialized.'; default_value=$null; empty_representation=''
        constraints=[pscustomobject]@{index='N is unbounded; minimum shown in field key.'}; enum_values=$familyEnum
        normalization=$normalization; computed=$false; calculation_id=$null; source_refs=@("official-hta-runtime#dynamic-family:L$($family.lines)",'official-hta-runtime#saveXML:L7009-L7368')
        confidence='high'; notes=@($family.notes)
    })
}
if ($fields.Count -ne 370 -or ($fields.field_key | Sort-Object -Unique).Count -ne 370) { throw "Expected 370 unique concrete/family fields; found $($fields.Count)." }
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;field_count=$fields.Count;runtime_serializable_element_count=311;inventory_sha256=Get-HashText @($fields.field_key|Sort-Object);fields=$fields})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expectedHashes.hta;form_control_count=$controls.Count;static_serializer_candidate_count=$serializable.Count;static_serializer_unique_id_count=$uniqueStaticIds.Count;reviewed_plaintext_key_count=311;reviewed_encrypted_key_count=311;dynamic_family_count=59;serializer_set_differences=[ordered]@{runtime_injected_observed=@('summationPage3Item21','summationIterationPage3Item21');static_not_in_plain_snapshot=@('btnUpload','dummyBranchCodePage1','frm1700:ddlPg3Pt5I7C1','frm1700:ddlPg3Pt5I7C2','frm1701:txtPg5Sc1I4SubtotalC1','frm1701:txtPg5Sc1I4SubtotalC2')};controls=$controls;dynamic_families=$families})

$decryptTool = Join-Path $RepoRoot 'rules\tools\audit-1701-encrypted-fields.ps1'
$decryptAudit = (& $decryptTool -SourceDir $SourceDir -FormId $formId -FilePattern '*.xml' -RedactedFileName '00000000000000-1700-25#email-redacted#.xml' -ExpectedCiphertextSha256 $expectedHashes.encrypted -ExpectedDecryptedSha256 $expectedHashes.decrypted -ExpectedFieldCount 311 -ExpectedFieldInventorySha256 $expectedHashes.inventory -ExpectedExtraField '*' -VersionField 'frm1700:txtVersion' -ExpectedXmlVersion '*') -join [Environment]::NewLine
Write-Utf8 (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') $decryptAudit
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1700:' -NamePattern '(?i)valid|check|mandatory|save') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1700:' -NamePattern '(?i)compute|sum|processPart1|processTaxReturn') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'shared-1700-function-inventory-v796.json') ((& $functionTool -HtaPath $sharedPath -ControlPrefix 'frm1700:' -NamePattern '(?i)populate|process') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$FieldKeys,[string]$Accepted,[string]$Rejected,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The first active branch alerts and returns false.',[string]$Recommended='Retain as a structured revision-aware field error.',[string]$Confidence='high') {
    $rules.Add([pscustomobject][ordered]@{rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$FieldKeys;accepted_behavior=$Accepted;rejected_behavior=$Rejected;exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence=$Confidence;unresolved_questions=@()})
}
Rule '1700-save-001' 'save' 1 'Any taxpayer TIN segment is blank.' @('frm1700:txtPg1Pt1I5TIN1','frm1700:txtPg1Pt1I5TIN2','frm1700:txtPg1Pt1I5TIN3') 'All three segments are nonblank.' 'Save is blocked.' 'Please enter a valid TIN number on Page 1 Part 1 Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L9564-L9567') 'incorrect-official-behavior' 'Only nonblankness is checked; branch code and checksum are ignored.' 'Permit lossless drafts; require complete shape and checksum before finalization.'
Rule '1700-save-002' 'save' 2 'Taxpayer last name is blank.' @('frm1700:txtPg1Pt1I8TaxFilerLastName') 'Last name is nonblank.' 'Save is blocked.' 'Please enter a valid Taxpayer Name on Page 1 Part 1 Item 8.' @('official-hta-runtime#initialValidateBeforeSave:L9569-L9572')
Rule '1700-save-003' 'save' 3 'Registered address line 1 is blank.' @('frm1700:txtPg1Pt1I9Address1') 'Line 1 is nonblank.' 'Save is blocked even if lines 2/3 are populated.' 'Please enter a valid Registered Address on Page1 Part 1 Item 9.' @('official-hta-runtime#initialValidateBeforeSave:L9574-L9577') 'official-bug-compatible' 'Save differs from Validate, which accepts any of the three address lines.' 'Use one consistent address-presence rule.'
Rule '1700-save-004' 'save' 4 'Contact number is blank.' @('frm1700:txtPg1Pt1I12ContactNo') 'Contact number is nonblank.' 'Save is blocked.' 'Please enter a valid Contact Number on Page 1 Part 1 Item 12.' @('official-hta-runtime#initialValidateBeforeSave:L9579-L9582')
Rule '1700-save-005' 'save' 5 'Other required/invalid fields exist.' @('return-body') 'Four narrow checks pass.' 'No other rejection occurs during Save.' $null @('official-hta-runtime#initialValidateBeforeSave:L9558-L9590','official-hta-runtime#saveXML:L7009-L7368') 'official-bug-compatible' 'Save is intentionally much narrower than Validate.' 'Preserve drafts losslessly and report completeness separately.'

$order=0
function V([string]$Suffix,[string]$Condition,[string[]]$FieldKeys,$Message,[string]$Lines,[string]$Assessment='verified-correct',[string]$Official='The ordered branch stops validation.',[string]$Recommended='Retain with exact revision-aware wording.') {
    $script:order++
    Rule "1700-validate-$Suffix" 'validate' $script:order $Condition $FieldKeys 'Condition is false; validation continues.' 'Validation stops at this first failure.' $Message @("official-hta-runtime#mandatoryFields/validateAll:L$Lines") $Assessment $Official $Recommended
}
V '001' 'Two-digit tax year is blank.' @('frm1700:txtYear') 'It is a mandatory field. Please enter a valid Year End on Page 1 Item 1' '11324'
V '002' 'Tax year is nonnumeric, 00, below 13, negative, or greater than current year minus 2000 minus one.' @('frm1700:txtYear') 'Page 1 Item 1: \nInvalid input. The Year should not be greater than {previous full year} .' '11325,11726-L11742' 'official-bug-compatible' 'The control accepts a two-digit year despite maxlength 4; years below 13 use a second message.' 'Model the exact two-digit convention for compatibility and expose the full year in app UI.'
V '003' 'Tax year numeric value is below 13.' @('frm1700:txtYear') 'Page 1 Item 1: \nInvalid input. The Year should not be earlier than September 2013' '11737-L11739'
V '004' 'Neither Source of Income radio is selected.' @('frm1700:SourceOfIncome_1','frm1700:SourceOfIncome_2') 'It is a mandatory field.  Please select a Source of Income on Page 1 Item 4' '11326'
V '005' 'Neither Joint Filing radio is selected.' @('frm1700:rdoPg1Pt1I3JointFilingYes','frm1700:rdoPg1Pt1I3JointFilingNo') 'It is a mandatory field.  Please select a if it is joint filing on Page 1 Item 3' '11327' 'official-bug-compatible'
foreach ($part in @('TIN1','TIN2','TIN3','BranchCode')) { V ("006-$part") "Taxpayer $part is blank." @("frm1700:txtPg1Pt1I5$part") 'It is a mandatory field.  Please enter a valid TIN number on Page 1 Item 5' '11328-L11331' }
V '010' 'RDO code is blank.' @('frm1700:txtPg1Pt1I6RDOCode') 'It is a mandatory field.  Please enter a valid RDO Code on Page 1 Item 6' '11332'
V '011' 'PSOC is blank.' @('frm1700:txtPg1Pt1I7Psoc') 'It is a mandatory field.  Please enter a valid PSOC on Page 1 Item 7' '11333'
V '012' 'Tax filer last name is blank.' @('frm1700:txtPg1Pt1I8TaxFilerLastName') "It is a mandatory field.  Please enter a valid Tax Filer's Name on Page 1 Item 8" '11334'
V '013' 'All three registered-address lines are blank.' @('frm1700:txtPg1Pt1I9Address1','frm1700:txtPg1Pt1I9Address2','frm1700:txtPg1Pt1I9Address3') 'It is a mandatory field.  Please enter a valid Registered Address on Page 1 Item 9' '11337'
V '014' 'Tax filer DOB is blank.' @('frm1700:txtPg1Pt1I10DOB') 'It is a mandatory field.  Please enter a valid Date of Birth (MM/DD/YYYY ) on Page 1 Item 10' '11340'
V '015' 'Tax filer DOB is malformed or future.' @('frm1700:txtPg1Pt1I10DOB') 'Page 1 Item 10:\nPlease provide a valid date for Date of Birth (MM/DD/YYYY )' '11341,11856-L11868'
V '016' 'Tax filer email is blank.' @('frm1700:txtPg1Pt1I11EmailAddress') 'It is a mandatory field.  Please enter a valid Email Address on Page 1 Item 11' '11342'
V '017' 'Tax filer contact number is blank.' @('frm1700:txtPg1Pt1I12ContactNo') 'It is a mandatory field.  Please enter a valid Contact Number on Page 1 Item 12' '11343'
V '018' 'No civil-status radio is selected.' @('frm1700:rdoPg1Pt1I13SingleInd','frm1700:rdoPg1Pt1I13MarriedInd','frm1700:rdoPg1Pt1I13LegalSeparatedInd','frm1700:rdoPg1Pt1I13WidowerInd') 'It is a mandatory field. Please select a Civil Status on Page 1 Item 13' '11344-L11348'
V '019' 'Neither taxpayer additional-exemption Yes nor No is selected.' @('frm1700:rdoPg1Pt1I14YesInd','frm1700:rdoPg1Pt1I14NoInd') "It is a mandatory field.  Please select either 'Yes or No; Claiming Additional Exemptions' on Page 1 Item 14" '11349' 'official-bug-compatible'
foreach ($part in @('LastName','FirstName','MiddleName')) { V ("020-$part") "All spouse name controls are enabled and spouse $part is blank." @("frm1700:txtPg1Pt1I16TaxSpouse$part") 'Please enter a valid Name on Page 1 Item 16' '11351-L11355' }
foreach ($part in @('TIN1','TIN2','TIN3','BranchCode')) { V ("023-$part") "Joint Filing Yes is selected and spouse $part is blank." @("frm1700:txtPg1Pt1I17Spouse$part") $(if($part -eq 'BranchCode'){'Please enter a valid TIN number on Item 17'}else{'Please enter a valid TIN number on Page 1 Item 17'}) '11356-L11361' }
V '027' 'Spouse TIN checksum helper returns nonzero whenever spouse-name controls are enabled.' @('frm1700:txtPg1Pt1I17SpouseTIN1','frm1700:txtPg1Pt1I17SpouseTIN2','frm1700:txtPg1Pt1I17SpouseTIN3') '{shared checksum description} on Page 1 Item 17.' '11362-L11363'
V '028' 'Spouse name controls are enabled and spouse DOB is blank.' @('frm1700:txtPg1Pt1I19SpouseDOB') 'It is a mandatory field. Please enter a valid Date of Birth (MM/DD/YYYY ) on Page 1 Item 19' '11364'
V '029' 'Spouse DOB is malformed or future.' @('frm1700:txtPg1Pt1I19SpouseDOB') 'Page 1 Item 19:\nPlease provide a valid date for Date of Birth (MM/DD/YYYY )' '11365,11856-L11868'
V '030' 'Joint Filing Yes is selected and spouse email is blank.' @('frm1700:txtPg1Pt1I20SpouseEmailAddress') 'Please enter a valid Email Address on Page 1 Item 20' '11366-L11368'
V '031' 'Neither Gov ID nor CTC selector is checked.' @('frm1700:rdoPg1Pt2I32GovID','frm1700:rdoPg1Pt2I32CTC') 'Please select Gov ID or CTC on Page 1 Item 32' '11371,11571-L11587'
V '032' 'Gov ID/CTC number field is blank.' @('frm1700:txtPg1Pt2I32GovID') 'It is a mandatory field. Please enter a valid Gov ID on Page 1 Item 32' '11373' 'official-bug-compatible' 'The same field is required with Gov ID wording for either selection.' 'Use selection-specific labels while preserving stored key.'
V '033' 'Date of issue is blank.' @('frm1700:txtPg1P23I33DateofIssue') 'It is a mandatory field. Please enter a valid Date on Page 1 Item 33' '11374'
V '034' 'CTC/Gov ID issue date violates the official date window.' @('frm1700:txtPg1P23I33DateofIssue') 'This date cannot be a past date. / This date cannot be a future date. / CTC year should not be less than {current year - 50} for Page 1 Item 33' '11376,11881-L11922' 'ambiguous'
V '035' 'CTC/Gov ID amount is blank.' @('frm1700:txtPg1Pt2I34Amount') 'It is a mandatory field. Please enter a valid amount on Page 1 Item 34' '11378'
V '036' 'Place of issue is blank.' @('frm1700:txtPg1Pt2I35PlaceofIssue') 'It is a mandatory field. Please enter a Place of Issue on Page 1 Item 35' '11379'
V '037' 'PSOC is blank (duplicate later check).' @('frm1700:txtPg1Pt1I7Psoc') 'It is a mandatory field. Please enter a valid PSOC on Page 1 Item 7' '11381' 'official-bug-compatible' 'Unreachable after rule 011 unless the field changes during validation.' 'Keep one PSOC rule.'
V '038' 'Either party claims additional exemptions and first dependent last name is blank.' @('frm1700:txtPg4T2LastName1') 'Please enter Last Name of Dependent on Page 4 Table 2' '11383-L11384'
V '039' 'Either party claims additional exemptions and first dependent first name is blank.' @('frm1700:txtPg4T2FirstName1') 'Please enter First Name of Dependent on Page 4 Table 2' '11385'
V '040' 'Page 2 Item 18 has a nonzero party amount and description is blank.' @('frm1700:txtPg2Pt4I18TC','frm1700:txtPg2Pt4I18C1','frm1700:txtPg2Pt4I18C2') 'Please enter a valid Other Payment description on Page 2 Item 18' '11392-L11394'
foreach ($item in 1..3) { V ("041-$item") "Both Page 3 Item 6 totals equal string 0 and enabled Page 2 employer Item $item compensation equals numeric zero." @("frm1700:txtPg2Pt4I${item}ComIncomeC1") "Page 2 Item $item Compensation Income should not be zero." '11396-L11410,11415-L11429' 'official-bug-compatible' 'The same checks run twice: once in mandatoryFields and again in validateAll.' 'Evaluate once in the same relative order.' }
V '044' 'Details-of-payment amount is nonzero and differs from total amount payable.' @('frm1700:txtPg1Pt3I36Amount','frm1700:txtPg1Pt2I31') 'Sum of Amount field in Details of Payment Segment must be equal to TOTAL AMOUNT PAYABLE' '11431,13425-L13433'
V '045' 'Details amount is nonzero and payment type, drawee bank, payment date, or number is missing.' @('frm1700:rdoPg1Pt3I36DeatilsofPaymentCash','frm1700:rdoPg1Pt3I36DeatilsofPaymentCheck','frm1700:txtPg1Pt2I36DraweeBank','frm1700:txtPg1Pt3I36DatePayment','frm1700:txtPg1Pt3I36Number','frm1700:txtPg1Pt3I36Amount') 'Please fill up Details of Payment.' '11432,13435-L13450' 'official-bug-compatible' 'Drawee bank is required even for cash; the else branch assigns misspelled isvalid.' 'Require only fields applicable to the selected payment method.'
V '046' 'Neither taxpayer nor spouse is selected for employer Item 1.' @('frm1700:rdoPg2Pt4I1TaxFiler','frm1700:rdoPg2Pt4I1Spouse') 'Please select at least 1 employer on Page 2 Item 1' '11434-L11438'
foreach ($item in 1..3) {
    V ("047-name-$item") "A party is selected for employer Item $item and employer name is blank; Item 3 is exempt when name is literal OTHERS." @("frm1700:txtPg2Pt4I${item}NameOfEmployer") "Please enter Employer's Name on Page 2 Item $item" '11440-L11450'
    V ("047-tin-$item") "A party is selected for employer Item $item and any employer TIN component is blank; Item 3 is exempt when name is literal OTHERS." @("frm1700:txtPg2Pt4I${item}EMPTIN1","frm1700:txtPg2Pt4I${item}EMPTIN2","frm1700:txtPg2Pt4I${item}EMPTIN3","frm1700:txtPg2Pt4I${item}EMPTINBranchCode") "Please enter Employer's TIN on Page 2 Item $item" '11440-L11450'
}
V '053' 'Payment date is invalid or future.' @('frm1700:txtPg1Pt3I36DatePayment') 'Page 1 Item 36:\nPlease provide a valid date for ' '11453,11869-L11880'
V '054' 'Page 3 Item 15 taxpayer date of issue is invalid or future when nonblank.' @('frm1700:txtPg3Pt5I15DateofIssueC1') 'Page 3 Item 15 Column 1:\nPlease provide a valid date for Date of Issue (MM/DD/YYYY)' '11454-L11456'
V '055' 'Page 3 Item 15 spouse date of issue is invalid or future when nonblank.' @('frm1700:txtPg3Pt5I15DateofIssueC2') 'Page 3 Item 15 Column 2:\nPlease provide a valid date for Date of Issue (MM/DD/YYYY)' '11458-L11460'
V '056-address-SubdivisionVillage' 'Address Changed is checked and Page 4 Table 1 subdivision/village is blank.' @('frm1700:txtPg4T1SubdivisionVillage') 'It is a mandatory field. Please enter a valid Subdivision/Village on Page 4 Table 1' '11469-L11470' 'official-bug-compatible'
V '057-address-Barangay' 'Address Changed is checked and Page 4 Table 1 barangay is blank.' @('frm1700:txtPg4T1Barangay') 'It is a mandatory field. Please enter a valid barangay on Page 4 Table 1' '11469-L11471' 'official-bug-compatible'
V '058-address-MunicipalityCity' 'Address Changed is checked and Page 4 Table 1 municipality/city is blank.' @('frm1700:txtPg4T1MunicipalityCity') 'It is a mandatory field. Please enter a valid Municipality or City on Page 4 Table 1' '11469-L11472' 'official-bug-compatible'
V '059-address-Province' 'Address Changed is checked and Page 4 Table 1 province is blank.' @('frm1700:txtPg4T1Province') 'It is a mandatory field. Please enter a valid province on Page 4 Table 1' '11469-L11473' 'official-bug-compatible'
V '060-address-ZipCde' 'Address Changed is checked and Page 4 Table 1 zip code is blank.' @('frm1700:txtPg4T1ZipCde') 'It is a mandatory field. Please enter a valid zip code on Page 4 Table 1' '11469-L11474' 'official-bug-compatible'
foreach ($n in 1..4) { V ("061-dob-$n") "Dependent row $n has a first or last name and its DOB is malformed, future, or blank." @("frm1700:txtPg4T2LastName$n","frm1700:txtPg4T2FirstName$n","frm1700:txtPg4T2DependentDOB$n") 'It is a mandatory field. Please enter a valid date on Page 4 Table 2' '11477-L11497,11856-L11868' }
foreach ($n in 1..4) { V ("065-age-$n") "Dependent row $n DOB is present and AgeCheck rejects it." @("frm1700:txtPg4T2DependentDOB$n","frm1700:rdoP4T2MarkIfIncapacitated$n") 'Dependent should be 21 years old and below. If dependent is more than 21 years old and incapacitated, mark as incapacitated.' '11499-L11509,12919-L12990' 'official-bug-compatible' 'A dependent older than the filer returns false silently in Validate; age uses only tax-year minus birth-year.' 'Use full dates, always provide an error, and allow over-21 only when incapacitated.' }
V '069' 'All ordered validation branches pass.' @('return-body') 'Validation successful. Click on Edit if you wish to modify your entries.' '7706-L7718'

Rule '1700-input-001' 'blur/change' 1 'A nonempty TIN segment has fewer than three characters.' @('TIN-segment-controls') 'Three characters or blank.' 'An alert is shown but the helper still returns true.' 'Please provide a valid TIN (must have 3 numbers per box).' @('official-hta-runtime#validateTIN:L11589-L11617') 'incorrect-official-behavior' 'The line that sets isValid=false is commented out.' 'Reject malformed segments before finalization.'
Rule '1700-input-002' 'blur/change' 2 'Taxpayer TIN passes nonblank checks but fails checksum.' @('frm1700:txtPg1Pt1I5TIN1','frm1700:txtPg1Pt1I5TIN2','frm1700:txtPg1Pt1I5TIN3') 'No checksum is evaluated.' 'No local rejection occurs.' $null @('official-hta-runtime#mandatoryFields:L11322-L11413','official-hta-runtime#getTinChkCode-uses:L9899-L9901,L11362-L11363') 'incorrect-official-behavior' 'Only spouse TIN invokes getTinChkCode.' 'Apply shared checksum validation to both taxpayer and spouse TINs.'
Rule '1700-input-003' 'blur/change' 3 'Email does not match the official regex.' @('frm1700:txtPg1Pt1I11EmailAddress','frm1700:txtPg1Pt1I20SpouseEmailAddress') 'Regex matches or value is blank.' 'Alert is shown and value is cleared.' 'You have entered an invalid email address format!' @('official-hta-runtime#validateEmail:L13006-L13016')
Rule '1700-input-004' 'blur/change' 4 'Page 3 Item 1 AAFM is positive and final tax withheld is nonpositive.' @('frm1700:txtPg3Pt5I1AAFM','frm1700:txtPg3Pt5I1FinalTaxWithheld') 'Withheld is positive.' 'Alert is shown and focus moves.' 'Final Tax Withheld on Page 3 Item 1 Should be greater than 0' @('official-hta-runtime#page3Item1ExemptValidation:L12123-L12136')
Rule '1700-input-005' 'blur/change' 5 'Part IV Item 22 exceeds 50% of Items 14A+14B.' @('frm1700:txtPg2Pt4I22','frm1700:txtPg2Pt4I14C1','frm1700:txtPg2Pt4I14C2') 'Amount is within cap.' 'Alert is shown and Item 22 is cleared.' 'Part IV Item 22 should not more than 50% of the sum of Item 14A and 14B' @('official-hta-runtime#computeP2Pt4I22:L9281-L9295') 'official-bug-compatible'
Rule '1700-input-006' 'blur/change' 6 'Part IV Item 22 exceeds Item 21.' @('frm1700:txtPg2Pt4I22','frm1700:txtPg2Pt4I21') 'Amount does not exceed Item 21.' 'Alert is shown and Item 22 is cleared.' 'Part IV Item 22 should not be more than the Item 21' @('official-hta-runtime#computeP2Pt4I22:L9296-L9299')

Rule '1700-modal-001' 'input' 1 'Page 2 Item 12 modal description is blank on Save.' @('frm1700:txtPg2Pt4I12Description_{N>=1}C1') 'Description is nonblank.' 'The row is silently deleted.' $null @('official-hta-runtime#saveRowColI12:L5983-L5989') 'official-bug-compatible' 'The intended alert is commented out.' 'Explain the missing description instead of silently deleting data.'
Rule '1700-modal-002' 'input' 2 'Page 2 Item 12 description is nonblank and either enabled party amount equals string 0.' @('frm1700:txtPg2Pt4I12TaxFiler_{N>=1}C2','frm1700:txtPg2Pt4I12Spouse_{N>=1}C3') 'Every enabled amount is nonzero.' 'Modal save is blocked.' 'Amount should not be zero.' @('official-hta-runtime#saveRowColI12:L5990-L5999')
Rule '1700-modal-003' 'input' 3 'Page 3 Item 6 modal Save is clicked.' @('frm1700:txtPg3Pt5I6Description_{N>=1}C1','frm1700:txtPg3Pt5I6AAFM_{N>=1}C2','frm1700:txtPg3Pt5I6FinalTaxWithheld_{N>=1}C3') 'No branch executes.' 'No validation, copy, counter update, or alert occurs.' $null @('official-hta-runtime#saveRowColI6:L5900-L5963') 'incorrect-official-behavior' 'The entire function body is commented out.' 'Implement explicit row validation and persistence; preserve compatibility evidence separately.'
Rule '1700-modal-004' 'input' 4 'A generic Page 3 A2/A3/A4/B1/B2 modal is saved with blank or zero data.' @('page-3-dynamic-families') 'Any values are accepted.' 'No rejection occurs; repository HTML is replaced and Saved! is shown.' 'Saved!' @('official-hta-runtime#saveRowCol:L5869-L5898') 'official-bug-compatible' 'No row validation is performed.' 'Validate only legally/applicably required cells and preserve incomplete drafts.'
Rule '1700-modal-005' 'input' 5 'An employer modal row has neither taxpayer nor spouse checked.' @('frm1700:rdoPg2Pt4I{N>=4}TaxFiler','frm1700:rdoPg2Pt4I{N>=4}Spouse') 'At least one party is checked.' 'The row is silently deleted.' $null @('official-hta-runtime#saveChangesPg2Pt4:L12431-L12438') 'official-bug-compatible'
Rule '1700-modal-006' 'input' 6 'Employer modal row name is blank.' @('frm1700:txtPg2Pt4I3NameOfEmployer{N>=4}') 'Name is nonblank.' 'Modal save is blocked.' 'Please enter Name of Employer.' @('official-hta-runtime#saveChangesPg2Pt4:L12439-L12442')
Rule '1700-modal-007' 'input' 7 'Employer name is nonblank and any TIN component is blank.' @('frm1700:txtPg2Pt4I3EMPTIN1{N>=4}','frm1700:txtPg2Pt4I3EMPTIN2{N>=4}','frm1700:txtPg2Pt4I3EMPTIN3{N>=4}','frm1700:txtPg2Pt4I3EMPTINBranchCode{N>=4}') 'All components are nonblank.' 'Modal save is blocked.' 'Please enter Employer TIN.' @('official-hta-runtime#saveChangesPg2Pt4:L12443-L12446')
Rule '1700-modal-008' 'input' 8 'Employer name is nonblank and enabled compensation equals string 0.' @('frm1700:txtPg2Pt4I{N>=4}ComIncomeC1') 'Compensation is nonzero.' 'Modal save is blocked.' 'Compensation Income should not be zero.' @('official-hta-runtime#saveChangesPg2Pt4:L12447-L12450')
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{'$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;rules=$rules})

$calcs = [Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Outputs,[string[]]$Inputs,[string]$Formula,[string]$Trigger,[string[]]$Depends,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Use integer/decimal arithmetic and recompute from authoritative inputs.') {
    $calcs.Add([pscustomobject][ordered]@{calculation_id=$Id;outputs=$Outputs;inputs=$Inputs;condition=$null;official_formula=$Formula;rounding='Official code predominantly parseInt-truncates inputs, Math.rounds tax/summation outputs, and formats whole pesos with NegativeValue/formatCurrencyWOC.';trigger=$Trigger;depends_on=$Depends;source_refs=$Refs;assessment=$Assessment;recommended_app_behavior=$Recommended;confidence='high'})
}
Calc '1700-qualified-dependent-count' @('frm1700:txtPg1Pt1I15QualifyDependent','frm1700:txtPg1Pt1I22SpouseQualifiedDependentChildren') @('page-4-dependent-rows','claiming-party') 'Count qualifying dependent rows and assign the count to the claiming party.' 'computeQualifiedDependent' @() @('official-hta-runtime#computeQualifiedDependent:L8831-L8872')
Calc '1700-item9-additional-exemption' @('frm1700:txtPg2Pt4I9C1','frm1700:txtPg2Pt4I9C2') @('qualified-dependent-count','claiming-party') 'Item 9 = 25,000 times qualified-dependent count for the claiming party; the other party is forced to zero.' 'computePt4I9C1/C2' @('1700-qualified-dependent-count') @('official-hta-runtime#computePt4I9C1/C2:L9212-L9241')
Calc '1700-employer-sums' @('frm1700:txtPg2Pt4I4AC1','frm1700:txtPg2Pt4I4AC2','frm1700:txtPg2Pt4I4BC1','frm1700:txtPg2Pt4I4BC2') @('all-static-and-dynamic-employer-rows') 'Sum compensation and withholding separately for taxpayer-selected and spouse-selected employer rows.' 'sumTaxSpouseIterate' @() @('official-hta-runtime#sumTaxSpouseIterate:L12761-L12833')
Calc '1700-item6-net-compensation' @('frm1700:txtPg2Pt4I6C1','frm1700:txtPg2Pt4I6C2') @('items4A/4B','item5') 'Item 6 per party = gross compensation Item 4 minus non-taxable/exempt Item 5.' 'computePt4I6C1/C2' @('1700-employer-sums') @('official-hta-runtime#computePt4I6C1/C2:L9124-L9165')
Calc '1700-item10-total-exemptions' @('frm1700:txtPg2Pt4I10C1','frm1700:txtPg2Pt4I10C2') @('items7','items8','items9') 'Item 10 per party = Items 7 + 8 + 9.' 'computePt4I10C1/C2' @('1700-item9-additional-exemption') @('official-hta-runtime#computePt4I10C1/C2:L9167-L9193')
Calc '1700-item11-net-taxable-compensation' @('frm1700:txtPg2Pt4I11C1','frm1700:txtPg2Pt4I11C2') @('items6','items10') 'Item 11 per party = Item 6 - Item 10.' 'computePt4I11C1/C2' @('1700-item6-net-compensation','1700-item10-total-exemptions') @('official-hta-runtime#computePt4I11C1/C2:L9195-L9210')
Calc '1700-item12-other-taxable-income' @('frm1700:txtPg2Pt4I12C1','frm1700:txtPg2Pt4I12C2') @('frm1700:txtPg2Pt4I12TaxFiler_{N>=1}C2','frm1700:txtPg2Pt4I12Spouse_{N>=1}C3') 'Sum modal amounts per party, then divide the combined temp/repository scan by two.' 'computePage2I12' @() @('official-hta-runtime#computePage2I12:L13253-L13258') 'official-bug-compatible' 'Avoid duplicate-container summation; sum the canonical row store exactly once.'
Calc '1700-item13-total-taxable-income' @('frm1700:txtPg2Pt4I13C1','frm1700:txtPg2Pt4I13C2') @('items11','items12') 'Item 13 per party = Item 11 + Item 12.' 'computePt4I13' @('1700-item11-net-taxable-compensation','1700-item12-other-taxable-income') @('official-hta-runtime#computePt4I13:L9369-L9385')
Calc '1700-tax-table' @('frm1700:txtPg2Pt4I14C1','frm1700:txtPg2Pt4I14C2') @('frm1700:txtPg2Pt4I13C1','frm1700:txtPg2Pt4I13C2') 'Pre-TRAIN table: <=0 0; <10k 5%; <30k 500+10% over 10k; <70k 2,500+15% over 30k; <140k 8,500+20% over 70k; <250k 22,500+25% over 140k; <500k 50,000+30% over 250k; >=500k 125,000+32% over 500k.' 'computePt4I14' @('1700-item13-total-taxable-income') @('official-hta-runtime#computePt4I14:L9387-L9441','official-help#tax-table:L655-L700')
Calc '1700-item15-withholding-copy' @('frm1700:txtPg2Pt4I15C1','frm1700:txtPg2Pt4I15C2') @('frm1700:txtPg2Pt4I4AC2','frm1700:txtPg2Pt4I4BC2') 'Copy total compensation tax withheld per party from employer aggregates.' 'computePt4I15C1/C2' @('1700-employer-sums') @('official-hta-runtime#computePt4I15C1/C2:L9445-L9452')
Calc '1700-item19-total-credits' @('frm1700:txtPg2Pt4I19C1','frm1700:txtPg2Pt4I19C2') @('items15-through-18') 'Item 19 per party = Items 15 + 16 + 17 + 18.' 'computePt4I19' @('1700-item15-withholding-copy') @('official-hta-runtime#computePt4I19:L9455-L9490')
Calc '1700-item20-net-tax' @('frm1700:txtPg2Pt4I20C1','frm1700:txtPg2Pt4I20C2') @('items14','items19') 'Item 20 per party = Item 14 - Item 19.' 'computePt4I20/computeP2Pt4I20C1/C2' @('1700-tax-table','1700-item19-total-credits') @('official-hta-runtime#computeP2Pt4I20C1/C2:L9244-L9266','official-hta-runtime#computePt4I20:L9492-L9509')
Calc '1700-item21-total-net-tax' @('frm1700:txtPg2Pt4I21') @('frm1700:txtPg2Pt4I20C1','frm1700:txtPg2Pt4I20C2') 'Item 21 = taxpayer Item 20 + spouse Item 20.' 'computeP2Pt4I21' @('1700-item20-net-tax') @('official-hta-runtime#computeP2Pt4I21:L9268-L9280')
Calc '1700-item23-net-after-installment' @('frm1700:txtPg2Pt4I23') @('items21','item22') 'Item 23 = Item 21 - Item 22.' 'computeP2Pt4I23' @('1700-item21-total-net-tax') @('official-hta-runtime#computeP2Pt4I23:L9313-L9325')
Calc '1700-item27-total-penalties' @('frm1700:txtPg2Pt4I27') @('items24','25','26') 'Item 27 = surcharge + interest + compromise.' 'computeP2Pt4I27' @() @('official-hta-runtime#computeP2Pt4I27:L9327-L9341')
Calc '1700-item28-total-payable' @('frm1700:txtPg2Pt4I28') @('items23','27') 'Item 28 = Item 23 + Item 27.' 'computeP2Pt4I28' @('1700-item23-net-after-installment','1700-item27-total-penalties') @('official-hta-runtime#computeP2Pt4I28:L9342-L9353')
Calc '1700-page1-item25' @('frm1700:txtPg1Pt2I25TotalIncomeTaxDue') @('frm1700:txtPg1Pt2I23FilerTaxDue','frm1700:txtPg1Pt2I24SpouseTaxDue') 'Page 1 Item 25 = filer tax due + spouse tax due; Items 23/24 copy Part IV Item 14 columns.' 'processPart1Item23/24,ComputePart1Item25' @('1700-tax-table') @('official-hta-runtime#processPart1Item23-LComputePart1Item25:L8874-L8890')
Calc '1700-page1-item28' @('frm1700:txtPg1Pt2I28NetTaxPayable') @('page1-item25','page1-items26-27') 'Page 1 Item 28 = total income tax due - filer credit - spouse credit; credits copy Part IV Item 19.' 'processPart1Item26/27,ComputePart1Item28' @('1700-page1-item25','1700-item19-total-credits') @('official-hta-runtime#ComputePart1Item28:L8891-L8911')
Calc '1700-page1-item31' @('frm1700:txtPg1Pt2I31') @('page1-item28','page1-item29','page1-item30') 'Normally Item 31 = Item 28 - Item 29 + Item 30; if Item 28 - Item 29 is negative and Item 30 positive, output Item 30 only.' 'ComputePart1Item31' @('1700-page1-item28','1700-item27-total-penalties') @('official-hta-runtime#ComputePart1Item31:L8921-L8942') 'official-bug-compatible' 'Review the special override against filing instructions before reproducing it as tax logic.'
Calc '1700-page3-item6-modal' @('summationIterationPage3PItem6','frm1700:txtPg2Pt4I6SubtotalC1','frm1700:txtPg2Pt4I6SubtotalC2') @('frm1700:txtPg3Pt5I6AAFM_{N>=1}C2','frm1700:txtPg3Pt5I6FinalTaxWithheld_{N>=1}C3') 'Sum modal columns, divide by two to compensate for scanning duplicate temp/repository DOM, and copy withheld subtotal to hidden summation key.' 'computePage3I6' @() @('official-hta-runtime#computePage3I6:L13206-L13218') 'official-bug-compatible' 'Canonicalize rows and never rely on duplicate-DOM division.'
foreach ($item in @(11,17,20)) { Calc "1700-page3-item${item}-modal" @("summationIterationPage3PItem${item}C1","summationIterationPage3PItem${item}C2") @("page3-item${item}-dynamic-columns") "Sum Item $item modal columns and divide by two to compensate for duplicate temp/repository DOM." "computePage3I$item" @() @("official-hta-runtime#computePage3I$item:L13220-L13251") 'official-bug-compatible' 'Canonicalize rows and sum once.' }
Calc '1700-page3-item21-total-withheld' @('summationPage3Item21','summationIterationPage3Item21','frm1700:txtPg3Pt5I21TotTaxWithheldPaid') @('static-and-dynamic-final-tax-withheld') 'Sum static/dynamic withheld fields, round, and copy to Item 21.' 'computeSummationPage3Item21' @('1700-page3-item6-modal','1700-page3-item11-modal','1700-page3-item17-modal','1700-page3-item20-modal') @('official-hta-runtime#computeSummationPage3Item21:L13149-L13154')
Calc '1700-page3-b-item10-total-income' @('frm1700:txtPg3Pt5BI10TotalIncome') @('page3-B-static-and-dynamic-income-fields') 'Sum B Items 1,2,3,7,9 across static and dynamic containers, then divide by two for duplicate DOM copies.' 'summation+adjustSummation on blur/modal Save' @() @('official-hta-runtime#page3-B-onblur:L3316-L3497','official-hta-runtime#summation/adjustSummation:L13108-L13147') 'official-bug-compatible' 'Canonicalize rows and sum once.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=@($calcs.calculation_id);calculations=$calcs})

$negative = @($rules | Where-Object { $_.exact_message } | Select-Object -First 40)
$cases=@();$n=0
foreach ($rule in $negative) { $n++; $cases += [pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}' -f $n,$rule.rule_id);phase=$rule.phase;mutations=@{synthetic_condition=$rule.condition};expected_message=$rule.exact_message;expected_behavior=$rule.official_behavior;rule_id=$rule.rule_id} }
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
$calcCases=@(
    @{case_id='tax-10k';calculation_id='1700-tax-table';inputs=@{taxable=10000};official_output='500'},
    @{case_id='tax-30k';calculation_id='1700-tax-table';inputs=@{taxable=30000};official_output='2500'},
    @{case_id='tax-500k';calculation_id='1700-tax-table';inputs=@{taxable=500000};official_output='125000'},
    @{case_id='item22-cap';calculation_id='1700-item23-net-after-installment';inputs=@{item14a=100;item14b=100;item21=200;item22=101};official_behavior='Clear Item 22 and alert because it exceeds 50% of Items 14A+14B.'},
    @{case_id='page1-item31-override';calculation_id='1700-page1-item31';inputs=@{item28=50;item29=100;item30=20};official_output='20';recommended_review='Ordinary arithmetic would produce -30.'}
)
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=$calcCases})

$resources=@()
foreach ($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1') | ForEach-Object {$_.Groups['v'].Value} | Sort-Object -Unique)) {
    $full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    $resources += [pscustomobject][ordered]@{src=$src;path=$full;present=(Test-Path -LiteralPath $full);size=if(Test-Path -LiteralPath $full){(Get-Item -LiteralPath $full).Length}else{$null};sha256=if(Test-Path -LiteralPath $full){(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}else{$null}}
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$workflow=[ordered]@{
    '$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    phases=@(
        @{phase='edit';official_behavior='Four-page June 2013 ENCS annual compensation-income return with employer, other-income, supplemental-information, address-change, and dependent modal tables.';source_refs=@('official-hta-runtime#frmMain','official-help#revision:L134');confidence='high'},
        @{phase='saved-draft';official_behavior='Save checks only taxpayer TIN segment nonblankness, taxpayer last name, address line 1, and contact number, then serializes flat frmMain state.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L9558-L9590','official-hta-runtime#saveXML:L7009-L7368');confidence='high'},
        @{phase='validated';official_behavior='Validate calls mandatoryFields then validateAll; first error stops processing, and successful validation disables controls.';source_refs=@('official-hta-runtime#validate:L7706-L7724','official-hta-runtime#mandatoryFields:L11322-L11413','official-hta-runtime#validateAll:L11415-L11466');confidence='high'},
        @{phase='final-copy';official_behavior='Final-copy path encrypts the same 311-key inventory and is coupled to profile/connectivity workflow.';source_refs=@('official-hta-runtime#saveEncryptedProfile:L6807','official-hta-runtime#saveXML:L7009-L7368');confidence='high'},
        @{phase='submitted';official_behavior='Online submission transport exists but was not exercised.';source_refs=@('official-hta-runtime#processSubmit:L8468-L8762','official-hta-runtime#submitOnline:L13762');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='Four narrow Save checks pass.';side_effects=@('Writes plaintext pseudo-XML with 311 keys.');source_refs=@('official-hta-runtime#initialValidateBeforeSave:L9558-L9590')},
        @{from='edit';action='Validate';to='validated';guard='All ordered mandatoryFields and validateAll rules pass.';side_effects=@('Disables controls.','Shows success alert.');source_refs=@('official-hta-runtime#validate:L7706-L7718')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables applicable controls.');source_refs=@('official-hta-runtime#enableAllControl:L7896')},
        @{from='validated';action='Final Copy';to='final-copy';guard='Local final-copy/profile workflow permits progress.';side_effects=@('Creates encrypted companion with identical field inventory.');source_refs=@('encrypted-field-audit-v796')},
        @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and credentials succeed.';side_effects=@('Attempts online submission; untested.');source_refs=@('official-hta-runtime#submitOnline:L13762')}
    )
    prerequisites=@('June 2013 ENCS revision','Two-digit tax year 13 or later and no later than previous calendar year','Applicable spouse/employer/dependent/modal details')
    required_attachments=@(
        @{attachment_id='bir-2316';label='Certificate of Compensation Payment/Tax Withheld (BIR Form 2316)';required_when='Always applicable to compensation reported.';official_ui_enforcement='Not locally verified.';source_refs=@('official-help#required-attachments:L637-L641');confidence='high'},
        @{attachment_id='husband-waiver';label="Waiver of husband's right to claim additional exemption";required_when='Applicable.';official_ui_enforcement='Not locally enforced.';source_refs=@('official-help#required-attachments:L642-L643');confidence='high'},
        @{attachment_id='tax-debit-memo';label='Duly approved Tax Debit Memo';required_when='Applicable.';official_ui_enforcement='Not locally enforced.';source_refs=@('official-help#required-attachments:L644');confidence='high'},
        @{attachment_id='foreign-tax-credit-proof';label='Proof of Foreign Tax Credits';required_when='Foreign tax credit is claimed.';official_ui_enforcement='Not locally enforced.';source_refs=@('official-help#required-attachments:L645');confidence='high'},
        @{attachment_id='amended-return-proof';label='Proof of tax payment and return previously filed';required_when='Amended return.';official_ui_enforcement='Not locally enforced.';source_refs=@('official-help#required-attachments:L646-L647');confidence='high'},
        @{attachment_id='other-credit-proof';label='Proof of other tax payment/credit';required_when='Applicable.';official_ui_enforcement='Not locally enforced.';source_refs=@('official-help#required-attachments:L648');confidence='high'},
        @{attachment_id='authorization-letter';label='Authorization letter';required_when='Filed by an authorized representative.';official_ui_enforcement='Not locally enforced.';source_refs=@('official-help#required-attachments:L649-L650');confidence='high'}
    )
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='Annual return due on or before April 15 following the taxable year.';source_refs=@('official-help#deadline:L246-L253');confidence='high'},
        @{quarter='Q2';due_date_rule='Not a quarterly return; annual deadline applies.';source_refs=@('official-help#deadline:L246-L253');confidence='high'},
        @{quarter='Q3';due_date_rule='Not a quarterly return; annual deadline applies.';source_refs=@('official-help#deadline:L246-L253');confidence='high'},
        @{quarter='Q4';due_date_rule='Not a quarterly return; annual deadline applies.';source_refs=@('official-help#deadline:L246-L253');confidence='high'}
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugs=@($rules | Where-Object {$_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$encAsset=Asset 'xml-encrypted-v1' 'dummy-profile-encrypted-copy' $encryptedPath 'Reviewed encrypted companion; decrypted in memory to the same 311-key inventory without emitting values.'
$encAsset.path=Join-Path $SourceDir '00000000000000-1700-25#email-redacted#.xml'
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $exePath 'Installed Offline eBIRForms package 7.9.6.0.',
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1700, HTA version 4.7, printed June 2013 ENCS.',
    Asset 'official-help' 'runtime-extracted-help' $helpPath 'Guidelines and Instructions for BIR Form 1700 June 2013 ENCS.',
    Asset 'shared-1700-js' 'official-linked-script' $sharedPath 'Linked population/transport mapping for exact HTA.',
    Asset 'xml-editable-v1' 'dummy-profile-editable-save' $PlainPath 'Reviewed 311-key plaintext save; values excluded.',
    $encAsset
)
$manifest=[ordered]@{
    '$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='1700';revision=$revision;revision_label='June 2013 (ENCS)';package_version=$packageVersion;status='complete';official_assets=$assets
    counts=[ordered]@{concrete_fields=311;runtime_field_families=59;fields_total=$fields.Count;typed_fields=$fields.Count;validation_rules=$rules.Count;confirmed_official_bugs=$bugs;calculations=$calcs.Count;negative_fixtures=$cases.Count;unverified_gaps=3}
    artifacts=[ordered]@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';runtime_control_fixture='fixtures/runtime-control-inventory-v796.json';encrypted_field_audit='fixtures/encrypted-field-audit-v796.json';validation_function_fixture='fixtures/validation-function-inventory-v796.json';calculation_function_fixture='fixtures/calculation-function-inventory-v796.json';shared_function_fixture='fixtures/shared-1700-function-inventory-v796.json';resource_hash_fixture='fixtures/official-resource-hashes-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'}
    scope_notes=@('Research only; no renderer/release changes.','No source values or email address are copied.','311 concrete keys have identical plaintext/encrypted inventory hashes; 59 unbounded runtime families are explicit.','The January 2018 1700v2018 PDF/HTA is a different revision and is not mixed into this package.')
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1700 - June 2013 (ENCS)`n`nRevision-specific Offline eBIRForms rule package with 311 concrete keys and 59 unbounded runtime field families. The separate January 2018 revision is intentionally excluded.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') "# Evidence`n`n- Exact HTA SHA-256: $($expectedHashes.hta); APPLICATIONNAME 1700, version 4.7, printed June 2013 ENCS.`n- Official help SHA-256: $($expectedHashes.help); it identifies the same revision, instructions, tax table, deadline, and attachments.`n- The reviewed plaintext save has 311 unique keys and field-inventory SHA-256 $($expectedHashes.inventory).`n- The encrypted companion SHA-256 $($expectedHashes.encrypted) replays in memory to decrypted SHA-256 $($expectedHashes.decrypted), the identical 311-key inventory, and emits no values.`n- Shared js/lib/1700.js SHA-256: $($expectedHashes.shared); it provides population/transport mapping while validation/calculations are inline in the HTA.`n- Validation ordering: Save uses four checks and displays only the first collected error. Validate calls mandatoryFields and then validateAll; both stop on the first failing branch. Blur/modal handlers execute independently.`n- The separate January 2018 1700v2018 HTA/PDF is not evidence for this June 2013 package.`n"
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. Online submission was not exercised.`n2. No local June 2013 official PDF was found; the package's exact-revision Help1700.hta is the instruction source. The January 2018 PDF is excluded.`n3. External attachment presence is not locally enforced and remains a documented workflow requirement.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- Revision/assets: **pass** - June 2013 HTA, exact-revision help, linked script, plaintext save, and encrypted replay are pinned.`n- Field inventory: **pass** - 311 identical plaintext/encrypted keys plus 59 unbounded runtime families.`n- Validation/calculation/workflow: **pass** - source-ordered rules, exact messages, pre-TRAIN table, modal behavior, and phase differences.`n- Official defects: **pass** - $bugs bug-compatible/incorrect rules are separated from recommendations, including the wholly commented-out Item 6 modal Save body.`n- Privacy: **pass** - no source values or email address copied.`n- Online submit and legacy PDF: **unverified** and explicit gaps.`n"
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 12: 1700-v2013. Next: 1702RT.`n"

$indexPath=Join-Path $RepoRoot 'rules\index.json'
$index=Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry=[pscustomobject][ordered]@{form_id=$formId;form_code='1700';revision=$revision;package_version=$packageVersion;priority=12;status='complete';path='forms/1700-v2013/manifest.json'}
$index.forms=@(@($index.forms | Where-Object {$_.form_id -ne $formId}) + $entry | Sort-Object priority)
$index.updated=(Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index
"Generated ${formId}: fields=$($fields.Count), rules=$($rules.Count), calculations=$($calcs.Count), negative_cases=$($cases.Count), bug_classifications=$bugs"
