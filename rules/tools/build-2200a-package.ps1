param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\2200Av2020',
    [string]$SampleDir = 'C:\Mac\Home\Downloads\forms\2200A'
)
$ErrorActionPreference = 'Stop'
$formId = '2200a-v2020'
$revision = '2020-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form2200Av2020.hta'
$legacyPath = Join-Path $ExtractedRoot 'forms\BIR-Form2200A.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help2200Av2020.hta'
$legacyHelpPath = Join-Path $ExtractedRoot 'helpfile\Help2200A.hta'
$pdfPath = Join-Path $OfficialDir '2200-A Jan 2020 ENCS Final version2.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\2200a-v2020'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = '1df302eeb1352eccb88f6aa7a23fdcc185b6fbb4d15435996250f985a0198e2c'
    legacy = '192f730cc713179f5a1d8233cb5133cdd62bd3e5896b54546da14b82fa226a3f'
    help = '344b6a6f92d3854ab8b4bceb6aad2b33518d4f1116469a2a2f2572d85573ecde'
    legacy_help = '7f62c7979beafb26328557b35171dcb9cc5c3c9a272b88b4130130498023c146'
    pdf = 'c294bd45da56aa641f40ed5ed22b6c7c782860e84c2da6431c3340bd73194879'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher = 'fdb9f1939c0350b45e11d5aac5b68df6ec72a5439d6aaa8c394476fd9505ffe6'
    plain = '38079e62cd13498384fc271735ffadd5c99375af41d253fb4839bff03615901d'
    inventory = '21824af426b951b769b3b0b9a9518e099bbbe982750ce5063182a84c843edccb'
}
function Write-Json([string]$Path, $Value) { [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false)) }
function Write-Utf8([string]$Path, [string]$Value) { [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false)) }
function Hash-Lines([string[]]$Lines) { $s = [Security.Cryptography.SHA256]::Create(); try { ([BitConverter]::ToString($s.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-', '').ToLowerInvariant() } finally { $s.Dispose() } }
function Attr([string]$Tag, [string]$Name) { $m = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name))); if ($m.Success) { $m.Groups[2].Value } else { $null } }
function Asset([string]$Id, [string]$Kind, [string]$Path, [string]$Binding, [string]$Display = '') {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{ asset_id = $Id; kind = $Kind; path = if ($Display) { $Display } else { $Path }; sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant(); size = $item.Length; revision_binding = $Binding }
}
foreach ($pair in @(@($htaPath,'hta'),@($legacyPath,'legacy'),@($helpPath,'help'),@($legacyHelpPath,'legacy_help'),@($pdfPath,'pdf'),@($packagePath,'package'))) {
    if (-not (Test-Path -LiteralPath $pair[0] -PathType Leaf)) { throw "Missing $($pair[0])" }
    if ((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
$samples = @(Get-ChildItem -LiteralPath $SampleDir -File -Filter '*.xml')
if ($samples.Count -ne 1) { throw "Expected one sample; found $($samples.Count)." }
if ((Get-FileHash -LiteralPath $samples[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected.cipher) { throw 'Sample hash changed.' }
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') { throw 'PDF magic mismatch.' }
$hta = [IO.File]::ReadAllText($htaPath)
$legacy = [IO.File]::ReadAllText($legacyPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)January\s+2020\s+\(ENCS\)' -or $hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']2200Av2020["'']') { throw 'January 2020 binding changed.' }
if ($help -notmatch '(?i)before removal of the alcohol products' -or $help -notmatch '(?i)For each place of production') { throw 'Current help binding changed.' }
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

$keyTool = Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$redactedSample = Join-Path $SampleDir '2200A-final-copy-#email-redacted#.xml'
$keyJson = & $keyTool -SourcePath $samples[0].FullName -RedactedSourcePath $redactedSample -FormId '2200a-legacy-excluded' -ExpectedCiphertextSha256 $expected.cipher -ExpectedDecryptedSha256 $expected.plain -ExpectedFieldCount 134 -ExpectedFieldInventorySha256 $expected.inventory
$keyAudit = $keyJson | ConvertFrom-Json
$legacyKeys = @($keyAudit.keys)
Write-Utf8 (Join-Path $fixtureDir 'excluded-legacy-encrypted-field-keys-v796.json') ($keyJson -join [Environment]::NewLine)
$currentAllIds = @([regex]::Matches($hta, '(?i)\bid\s*=\s*(["''])(?<id>.*?)\1') | ForEach-Object { $_.Groups['id'].Value } | Where-Object { $_ } | Sort-Object -Unique)
$legacyAllIds = @([regex]::Matches($legacy, '(?i)\bid\s*=\s*(["''])(?<id>.*?)\1') | ForEach-Object { $_.Groups['id'].Value } | Where-Object { $_ } | Sort-Object -Unique)
$currentOverlap = @($legacyKeys | Where-Object { $currentAllIds -contains $_ })
$legacyOverlap = @($legacyKeys | Where-Object { $legacyAllIds -contains $_ })
if ($legacyOverlap.Count -ne 134 -or $currentOverlap.Count -ne 6) { throw "Sample discrimination changed: legacy/current $($legacyOverlap.Count)/$($currentOverlap.Count)." }

$formMatch = [regex]::Match($hta, '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain missing.' }
$body = $formMatch.Groups['body'].Value
$bodyOffset = $formMatch.Groups['body'].Index
$excluded = @(@([regex]::Matches($body, '(?is)<script\b.*?</script>')) + @([regex]::Matches($body, '(?is)<!--.*?-->')))
$controls = [Collections.Generic.List[object]]::new()
$ordinal = 0
foreach ($match in [regex]::Matches($body, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $skip = $false
    foreach ($range in $excluded) { if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) { $skip = $true; break } }
    if ($skip) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $kind = $kind.ToLowerInvariant()
    $default = Attr $tag 'value'
    if ($kind -in @('radio','checkbox')) { $default = if ($tag -match '(?i)\bchecked(?:\s*=|\s|>)') { 'true' } else { 'false' } }
    $controls.Add([pscustomobject][ordered]@{
        ordinal = $ordinal; id = Attr $tag 'id'; name = Attr $tag 'name'; element = $element; control_kind = $kind
        source_line = 1 + [regex]::Matches($hta.Substring(0, $bodyOffset + $match.Index), "`n").Count
        default_value = $default; maxlength = Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'; readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}
$serial = @($controls | Where-Object { $_.control_kind -in @('text','select','select-one','textarea','radio','checkbox') -and $_.id })
$staticIds = @($serial.id | Sort-Object -Unique)
$runtimeRdo = 'frm2200Av2020:rdoCode'
if ($controls.Count -ne 308 -or $serial.Count -ne 283 -or $staticIds.Count -ne 273) { throw "Control inventory changed: $($controls.Count)/$($serial.Count)/$($staticIds.Count)." }
if ($staticIds -contains $runtimeRdo -or $hta -notmatch [regex]::Escape("<select class='iceSelOneMnu' id='frm2200Av2020:rdoCode'")) { throw 'Runtime RDO derivation changed.' }
$duplicateCounts = @($serial | Group-Object id | Where-Object { $_.Count -gt 1 } | Sort-Object Name)
if ($duplicateCounts.Count -ne 5 -or @($duplicateCounts | Where-Object { $_.Count -ne 3 }).Count) { throw 'Duplicate serialized occurrence inventory changed.' }
$familySuffixes = @('Check','Atc','Desc','Bracket','Rate','Export','Taxable','Due')
$families = @($familySuffixes | ForEach-Object { [pscustomobject]@{ field_pattern = "frm2200Av2020:txtSched1_xa_others{N>=4}_$_"; suffix = $_; source_line = 7582 + [array]::IndexOf($familySuffixes, $_) } })
if ($families.Count -ne 8 -or $hta -notmatch 'var\s+sched1Index\s*=\s*4') { throw 'Dynamic family derivation changed.' }

$required = @('frm2200Av2020:txtDateMonth','frm2200Av2020:txtDateDay','frm2200Av2020:txtDateYear','frm2200Av2020:tinA','frm2200Av2020:tinB','frm2200Av2020:tinC','frm2200Av2020:branchCode',$runtimeRdo,'frm2200Av2020:registeredName','frm2200Av2020:registeredAddress','frm2200Av2020:zipCode','frm2200Av2020:phoneNumber','frm2200Av2020:prodCity','frm2200Av2020:remCity')
$computedPattern = '(?i)(?:_Due|TotalDue|txtExciseDue|txtLess_Tot|txtNetTaxDue|txtStillDue|txtPen_Tot|txtAmtPayable|txtPay_Penalties|txtPay_Tot|txtBalance)$'
function Item-For([string]$Key) {
    $map = @{
        'frm2200Av2020:txtDateMonth'='1';'frm2200Av2020:txtDateDay'='1';'frm2200Av2020:txtDateYear'='1'
        'frm2200Av2020:amendedRtn_1'='2';'frm2200Av2020:amendedRtn_2'='2';'frm2200Av2020:txtSheets'='3'
        'frm2200Av2020:tinA'='4';'frm2200Av2020:tinB'='4';'frm2200Av2020:tinC'='4';'frm2200Av2020:branchCode'='4'
        'frm2200Av2020:rdoCode'='5';'frm2200Av2020:registeredName'='6';'frm2200Av2020:registeredAddress'='7'
        'frm2200Av2020:zipCode'='7A';'frm2200Av2020:phoneNumber'='8';'frm2200Av2020:txtEmail'='9'
        'frm2200Av2020:prodCity'='10';'frm2200Av2020:remCity'='11';'frm2200Av2020:optTreaty_1'='12';'frm2200Av2020:optTreaty_2'='12';'frm2200Av2020:treatyY'='12A'
        'frm2200Av2020:optPayment_1'='13';'frm2200Av2020:optPayment_2'='14';'frm2200Av2020:optPayment_3'='15';'frm2200Av2020:paymentOther'='15'
        'frm2200Av2020:txtExciseDue'='16';'frm2200Av2020:txtLess_Balance'='17A';'frm2200Av2020:txtLess_Excise'='17B';'frm2200Av2020:txtLess_Tot'='17C'
        'frm2200Av2020:txtNetTaxDue'='18';'frm2200Av2020:txtPrevReturn'='19';'frm2200Av2020:txtStillDue'='20'
        'frm2200Av2020:txtPen_Surcharge'='21A';'frm2200Av2020:txtPen_Interest'='21B';'frm2200Av2020:txtPen_Compromise'='21C';'frm2200Av2020:txtPen_Tot'='21D'
        'frm2200Av2020:txtAmtPayable'='22';'frm2200Av2020:txtPay_TaxPayment'='23A';'frm2200Av2020:txtPay_Penalties'='23B';'frm2200Av2020:txtPay_Tot'='23C';'frm2200Av2020:txtBalance'='24'
    }
    if ($map.ContainsKey($Key)) { $map[$Key] } elseif ($Key -like 'frm2200Av2020:txtSched1*') { 'Schedule 1' } else { $null }
}
function Make-Field($Control, [string]$Key, $SerializedKey, $Occurrence, [bool]$Family = $false) {
    $logical = 'string'; $normalization = [string[]]@(); $enum = [object[]]@()
    $kind = if ($Family) { 'runtime-indexed-family' } elseif ($Control) { $Control.control_kind } else { 'runtime-generated-select' }
    if ($kind -in @('radio','checkbox') -or $Key -match '(?i)(?:amendedRtn|optTreaty|optPayment|_Check)') { $logical = 'boolean'; $enum = [object[]]@('true','false') }
    elseif ($Key -match '(?i)(?:DateMonth|DateDay|DateYear|tin[A-C]|branchCode|rdoCode|Atc|zipCode)') { $logical = 'code' }
    elseif ($Key -match '(?i)(?:Date)$') { $logical = 'date-string-mm-dd-yyyy'; $normalization = [string[]]@('MM/DD/YYYY') }
    elseif ($Key -match '(?i)(?:ATR|Rate)$') { $logical = 'decimal-rate' }
    elseif ($Key -match '(?i)(?:Export|Taxable|Due|Balance|Excise|Tot|Payable|Payment|Pen_|PrevReturn)') { $logical = 'decimal-amount'; $normalization = [string[]]@('NumWithComma','amtFormat','round(2)') }
    $isComputed = $Key -match $computedPattern
    $status = if ($Family) { 'conditional' } elseif ($isComputed) { 'computed' } elseif ($required -contains $SerializedKey) { 'required' } else { 'optional' }
    $requiredWhen = if ($Family) { 'The corresponding runtime-added Others row exists.' } elseif ($SerializedKey -eq 'frm2200Av2020:treatyY') { 'Item 12 Yes is selected.' } elseif ($SerializedKey -eq 'frm2200Av2020:paymentOther') { 'Item 15 Other Similar Scheme is selected.' } else { $null }
    if ($requiredWhen) { $status = 'conditional' }
    $constraints = [ordered]@{}
    if ($Control -and $Control.maxlength -match '^\d+$') { $constraints.max_length = [int]$Control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision = 2; $constraints.sign = 'nonnegative for source-wired editable schedule amounts; otherwise source-dependent' }
    [string[]]$notes = if ($Family) { @('Source-derived unbounded family beginning at index 4.') } elseif ($Occurrence -gt 1) { @('Duplicate serialized key occurrence preserved losslessly in DOM order.','No January 2020 final copy is available.') } else { @('Source-derived January 2020 serialized control; no revision-matched final copy is available.') }
    [pscustomobject][ordered]@{
        field_key = $Key; serialized_key = $SerializedKey; serialized_occurrence = $Occurrence
        label = if ($Family) { "Runtime Others Schedule 1 $($Key.Split('_')[-1])" } else { $SerializedKey }
        page = if ($Key -like '*txtSched1*') { 2 } elseif ($SerializedKey -like 'frm2200Av2020:*') { 1 } else { $null }
        item_number = Item-For $SerializedKey; control_kind = $kind; storage_type = 'string'; logical_type = $logical
        required = $status; required_when = $requiredWhen
        enabled_when = if ($SerializedKey -eq 'frm2200Av2020:treatyY') { 'Item 12 Yes is selected.' } elseif ($SerializedKey -eq 'frm2200Av2020:txtPrevReturn') { 'Amended Return Yes is selected.' } else { $null }
        visible_when = $null; default_value = if ($Family) { $null } elseif ($Control) { $Control.default_value } else { '000' }
        empty_representation = ''; constraints = [pscustomobject]$constraints; enum_values = $enum; normalization = $normalization
        computed = $isComputed; calculation_id = if ($isComputed) { 'See calculations.json' } else { $null }
        source_refs = if ($Family) { @("official-hta-runtime#sched1Fields:L$($Control.source_line)",'official-hta-runtime#saveXML') } else { @('official-hta-runtime#saveXML',"official-hta-runtime#control:L$($Control.source_line)") }
        confidence = 'high'
        notes = $notes
    }
}
$fields = [Collections.Generic.List[object]]::new()
$seen = @{}
foreach ($control in $serial) {
    $rawKey = $control.id
    if (-not $seen.ContainsKey($rawKey)) { $seen[$rawKey] = 0 }
    $seen[$rawKey]++
    $occurrence = $seen[$rawKey]
    $fieldKey = if ($occurrence -eq 1) { $rawKey } else { "$rawKey#occurrence-$occurrence" }
    $fields.Add((Make-Field $control $fieldKey $rawKey $occurrence))
}
$runtimeControl = [pscustomobject]@{ control_kind='runtime-generated-select'; source_line=5926; default_value='000'; maxlength=$null }
$fields.Add((Make-Field $runtimeControl $runtimeRdo $runtimeRdo 1))
foreach ($family in $families) {
    $familyControl = [pscustomobject]@{ control_kind='runtime-indexed-family'; source_line=$family.source_line; default_value=$null; maxlength=if($family.suffix -eq 'Atc'){'3'}else{$null} }
    $fields.Add((Make-Field $familyControl $family.field_pattern $null $null $true))
}
if ($fields.Count -ne 292) { throw "Expected 292 typed fields; found $($fields.Count)." }
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;field_count=$fields.Count;runtime_serializable_element_count=284;inventory_sha256=Hash-Lines @($fields.field_key|Sort-Object);fields=$fields})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta;live_control_count=$controls.Count;static_serialized_occurrence_count=$serial.Count;static_unique_serialized_id_count=$staticIds.Count;duplicate_serialized_keys=@($duplicateCounts|ForEach-Object{[pscustomobject]@{key=$_.Name;occurrences=$_.Count}});runtime_generated_scalar_count=1;runtime_generated_scalars=@($runtimeRdo);revision_matched_final_copy_key_count=0;excluded_legacy_sample_key_count=$legacyKeys.Count;excluded_legacy_sample_overlap_with_legacy_runtime=$legacyOverlap.Count;excluded_legacy_sample_overlap_with_current_runtime=$currentOverlap.Count;active_runtime_family_count=$families.Count;controls=$controls;dynamic_families=$families})
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm2200Av2020:' -NamePattern '(?i)valid|check|save|enable|date|submit|final|payment|treaty')-join[Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm2200Av2020:' -NamePattern '(?i)compute|calculate|amount|tax|penalt|balance|format')-join[Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Keys,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',[string]$Recommended='Retain as a structured revision-aware error.') {
    $rules.Add([pscustomobject][ordered]@{rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Keys;accepted_behavior='Condition is false; processing continues.';rejected_behavior='The active operation stops unless official_behavior states otherwise.';exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()})
}
Rule '2200a-validate-001-month' validate 1 'Month is blank, below 1, above 12, or parses falsy.' @('frm2200Av2020:txtDateMonth') 'Please enter a valid month on Item 1.' @('official-hta-runtime#validateForm:L5631-L5634')
Rule '2200a-validate-002-nonleap-february' validate 2 'February day exceeds 28 in a non-leap year.' @('frm2200Av2020:txtDateMonth','frm2200Av2020:txtDateDay','frm2200Av2020:txtDateYear') 'Please enter a valid date on Item 1. Filing year is not a leap year.' @('official-hta-runtime#validateForm:L5636-L5639')
Rule '2200a-validate-003-day' validate 3 'Day is blank, below 1, above the computed month maximum, or parses falsy.' @('frm2200Av2020:txtDateDay') 'Please enter a valid day on Item 1.' @('official-hta-runtime#validateForm:L5641-L5644')
Rule '2200a-validate-004-year' validate 4 'Year is blank.' @('frm2200Av2020:txtDateYear') 'Please enter a valid year on Item 1.' @('official-hta-runtime#validateForm:L5646-L5649')
Rule '2200a-validate-005-effectivity' validate 5 'Year is earlier than 2020.' @('frm2200Av2020:txtDateYear') 'Returning old forms is restricted to the last version up to the effectivity of the new form version.' @('official-hta-runtime#validateForm:L5658-L5663')
Rule '2200a-validate-006-future' validate 6 'Return date is after today.' @('frm2200Av2020:txtDateMonth','frm2200Av2020:txtDateDay','frm2200Av2020:txtDateYear') 'Invalid date entry on Item 1. Date cannot be after the current date.' @('official-hta-runtime#validateForm:L5665-L5668')
Rule '2200a-validate-007-tin' validate 7 'Any TIN segment or branch code is blank.' @('frm2200Av2020:tinA','frm2200Av2020:tinB','frm2200Av2020:tinC','frm2200Av2020:branchCode') 'Please enter a valid TIN number on Item 4.' @('official-hta-runtime#validateForm:L5670-L5673')
Rule '2200a-validate-008-rdo' validate 8 'Runtime RDO selectedIndex is 0.' @($runtimeRdo) 'Please enter a valid RDO Code on Item 5.' @('official-hta-runtime#validateForm:L5675-L5678')
Rule '2200a-validate-009-name' validate 9 'Registered name is blank.' @('frm2200Av2020:registeredName') "Please enter a valid Taxpayer's Name on Item 6." @('official-hta-runtime#validateForm:L5680-L5683')
Rule '2200a-validate-010-address' validate 10 'Registered address is blank.' @('frm2200Av2020:registeredAddress') "Please enter Taxpayer's Registered Address on Item 7." @('official-hta-runtime#validateForm:L5685-L5688')
Rule '2200a-validate-011-zip' validate 11 'ZIP code is blank.' @('frm2200Av2020:zipCode') "Please enter Taxpayer's Zip Code on Item 7A." @('official-hta-runtime#validateForm:L5690-L5693')
Rule '2200a-validate-012-phone' validate 12 'Telephone number is blank.' @('frm2200Av2020:phoneNumber') 'Please enter a valid Telephone Number on Item 8.' @('official-hta-runtime#validateForm:L5695-L5698')
Rule '2200a-validate-013-production-place' validate 13 'Production city selectedIndex is 0.' @('frm2200Av2020:prodRegion','frm2200Av2020:prodProvince','frm2200Av2020:prodCity') 'Please enter a valid Place of Production on Item 10.' @('official-hta-runtime#validateForm:L5700-L5703')
Rule '2200a-validate-014-removal-place' validate 14 'Removal city selectedIndex is 0.' @('frm2200Av2020:remRegion','frm2200Av2020:remProvince','frm2200Av2020:remCity') 'Please enter a valid Place of Removal on Item 11.' @('official-hta-runtime#validateForm:L5705-L5708')
Rule '2200a-validate-015-tax-relief' validate 15 'Tax relief Yes is selected and Item 12A is blank.' @('frm2200Av2020:optTreaty_1','frm2200Av2020:treatyY') 'Please specify a Tax Relief on Item 12A.' @('official-hta-runtime#validateForm:L5710-L5713')
Rule '2200a-validate-016-payment-kind' validate 16 'No Part II payment option is selected.' @('frm2200Av2020:optPayment_1','frm2200Av2020:optPayment_2','frm2200Av2020:optPayment_3') 'Please enter a Manner of Payment on Part II.' @('official-hta-runtime#validateForm:L5715-L5721')
Rule '2200a-validate-017-other-scheme' validate 17 'Other Similar Scheme is selected and Item 15 specification is blank.' @('frm2200Av2020:optPayment_3','frm2200Av2020:paymentOther') 'Please specify a Scheme on Item 15.' @('official-hta-runtime#validateForm:L5723-L5726')
Rule '2200a-validate-018-insufficient-fund' validate 18 'Item 22 amount payable minus Item 23C payment is positive.' @('frm2200Av2020:txtAmtPayable','frm2200Av2020:txtPay_Tot') 'YOU HAVE INSUFFICIENT FUND. PLEASE APPLY DEPOSIT TO PROCEED' @('official-hta-runtime#validateForm:L5728-L5735')
Rule '2200a-date-019-format' 'blur/change' 1 'Payment date is not valid MM/DD/YYYY.' @('frm2200Av2020:cashDate','frm2200Av2020:checkDate','frm2200Av2020:taxDate') 'Please provide a valid date. (MM/DD/YYYY format)' @('official-hta-runtime#validateDate:L7111-L7169')
Rule '2200a-date-020-future' 'blur/change' 2 'Payment date is after the current moment.' @('frm2200Av2020:cashDate','frm2200Av2020:checkDate','frm2200Av2020:taxDate') 'This date cannot be a future date.' @('official-hta-runtime#validateDate:L7170-L7174')
Rule '2200a-date-021-prior-2018' 'blur/change' 3 'Payment date year is before 2018.' @('frm2200Av2020:cashDate','frm2200Av2020:checkDate','frm2200Av2020:taxDate') 'This date cannot be prior to 2018.' @('official-hta-runtime#validateDate:L7175-L7179') 'official-bug-compatible' 'The January 2020 form accepts payment dates from 2018 onward.' 'Bind the minimum to the actual form and payment legal rule.'
Rule '2200a-schedule-022-atc-required' input 1 'An existing Others row has blank ATC when Add is pressed.' @('frm2200Av2020:txtSched1_xa_others{N>=1}_Atc') 'ATC is required in row #{row}' @('official-hta-runtime#isValidDataOnSched1:L7426-L7460')
Rule '2200a-schedule-023-atc-length' input 2 'Others ATC is nonblank but length is not exactly 3.' @('frm2200Av2020:txtSched1_xa_others{N>=1}_Atc') 'Please supply valid ATC Code for row #{row}' @('official-hta-runtime#isValidDataOnSched1:L7432-L7436')
Rule '2200a-schedule-024-description' input 3 'Others description is blank.' @('frm2200Av2020:txtSched1_xa_others{N>=1}_Desc') 'Description is required in row #{row}' @('official-hta-runtime#isValidDataOnSched1:L7438-L7454')
Rule '2200a-schedule-025-bracket' input 4 'Others tax bracket or unit of measure is blank.' @('frm2200Av2020:txtSched1_xa_others{N>=1}_Bracket') 'Tax Bracket/Unit of Measure is required in row #{row}' @('official-hta-runtime#isValidDataOnSched1:L7439-L7450')
Rule '2200a-schedule-026-rate' input 5 'Others applicable rate is blank.' @('frm2200Av2020:txtSched1_xa_others{N>=1}_Rate') 'Applicable Tax Rate is required in row #{row}' @('official-hta-runtime#isValidDataOnSched1:L7440-L7446')
Rule '2200a-schedule-027-first-three-delete' input 6 'Delete targets one of the first three Others rows.' @('frm2200Av2020:txtSched1_xa_others{N=1..3}_Check') 'First 3 (three) rows cannot be deleted.' @('official-hta-runtime#deleteFieldForSched1:L7501-L7517')
Rule '2200a-navigation-028-schedule' 'page navigation' 1 'A new form initializes and Schedule 1 has not yet been visited.' @('frm2200Av2020:txtCurrentPage') 'Fill-up Schedule 1.' @('official-hta-runtime#navigateToSched1:L7233-L7238') 'official-bug-compatible' 'The alert navigates but does not prove Schedule 1 completeness.' 'Use a visible required-schedule state and validate its contents.'
Rule '2200a-save-029-return-date' save 1 'All three return-date components are blank.' @('frm2200Av2020:txtDateMonth','frm2200Av2020:txtDateDay','frm2200Av2020:txtDateYear') 'Please enter a valid Return Date' @('official-hta-runtime#initialValidateBeforeSave:L5938-L5942')
Rule '2200a-save-030-tin' save 2 'Any TIN segment or branch code is blank.' @('frm2200Av2020:tinA','frm2200Av2020:tinB','frm2200Av2020:tinC','frm2200Av2020:branchCode') 'Please enter a valid TIN number on Item 4.' @('official-hta-runtime#initialValidateBeforeSave:L5944-L5948')
Rule '2200a-save-031-rdo' save 3 'RDO value is 000.' @($runtimeRdo) 'Please enter a valid RDO Code on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L5949-L5952')
Rule '2200a-save-032-name' save 4 'Registered name is blank.' @('frm2200Av2020:registeredName') 'Please enter a valid Taxpayer Name on Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L5953-L5957') 'official-bug-compatible' 'The printed field is Item 6, but Save reports Item 7.' 'Use Item 6 consistently.'
Rule '2200a-defect-033-partial-save-date' save 5 'Only one or two return-date components are populated.' @('frm2200Av2020:txtDateMonth','frm2200Av2020:txtDateDay','frm2200Av2020:txtDateYear') $null @('official-hta-runtime#initialValidateBeforeSave:L5938-L5942') 'incorrect-official-behavior' 'The Save condition uses AND, so any partially populated date bypasses the date check.' 'Require all three components together and validate the composed date.'
Rule '2200a-defect-034-nonnumeric-year' validate 19 'Year contains nonnumeric text introduced without keypress filtering.' @('frm2200Av2020:txtDateYear') $null @('official-hta-runtime#validateForm:L5611-L5668') 'incorrect-official-behavior' 'Blank, range, and date comparisons do not reject NaN text; paste/programmatic input can pass.' 'Parse a four-digit integer and reject nonnumeric input.'
Rule '2200a-defect-035-date-return' 'blur/change' 4 'Payment date is future or before 2018.' @('frm2200Av2020:cashDate','frm2200Av2020:checkDate','frm2200Av2020:taxDate') $null @('official-hta-runtime#validateDate:L7165-L7181') 'incorrect-official-behavior' 'The field is cleared, but isValid remains true for future and pre-2018 branches.' 'Return false for every rejected date.'
Rule '2200a-defect-036-others-due' 'blur/change' 5 'Others row rate, export, taxable, or due changes.' @('frm2200Av2020:txtSched1_xa_others{N>=1}_Rate','frm2200Av2020:txtSched1_xa_others{N>=1}_Taxable','frm2200Av2020:txtSched1_xa_others{N>=1}_Due') $null @('official-hta-runtime#computeOthers:L7351-L7388','official-hta-runtime#sched1Fields:L7591-L7602') 'incorrect-official-behavior' 'computeOthers reads values but never computes Due; dynamically added Due is editable and included in totals.' 'Compute due from the legally applicable rate and taxable base; keep output read-only.'
Rule '2200a-defect-037-schedule-main-validation' validate 20 'Any Others row is incomplete or no Schedule 1 taxable row exists.' @('Schedule-1-fields') $null @('official-hta-runtime#validateForm:L5607-L5746','official-hta-runtime#isValidDataOnSched1:L7426-L7464') 'incorrect-official-behavior' 'Main Validate never calls isValidDataOnSched1 and does not require any tax row.' 'Validate Schedule 1 completeness and the legal zero-return case explicitly.'
Rule '2200a-defect-038-edit-treaty' input 7 'Edit is clicked while Tax Relief Yes remains selected.' @('frm2200Av2020:optTreaty_1','frm2200Av2020:treatyY') $null @('official-hta-runtime#enabledDisabledControls:L5852-L5861') 'incorrect-official-behavior' 'The Yes branch always disables Item 12A even when param is false, so Edit does not restore the specification field.' 'Enable Item 12A in edit mode when Yes is selected.'
Rule '2200a-defect-039-save-sparse' save 6 'Any Validate-only required field is missing.' @('frm2200Av2020:registeredAddress','frm2200Av2020:zipCode','frm2200Av2020:phoneNumber','frm2200Av2020:prodCity','frm2200Av2020:remCity') $null @('official-hta-runtime#initialValidateBeforeSave:L5938-L5959','official-hta-runtime#validateForm:L5607-L5746') 'incorrect-official-behavior' 'Save checks only return date, TIN, RDO, and name.' 'Use a shared phase-aware validation graph.'
Rule '2200a-defect-040-email-unvalidated' validate 21 'Email is malformed or blank.' @('frm2200Av2020:txtEmail') $null @('official-hta-runtime#validateForm:L5607-L5746','official-hta-runtime#control:L542') 'ambiguous' 'The current form serializes email but validateForm never checks it.' 'Apply the shared email rule if the official/legal workflow requires email.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{'$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;first_error_behavior='Validate and Save stop at the first source-ordered failure; Add validates existing dynamic Others rows separately.';rules=$rules})

$calcs = [Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Out,[string[]]$In,[string]$Formula,[string]$Trigger,[string[]]$Deps,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Implement with typed decimals and two-decimal display formatting.') {
    $calcs.Add([pscustomobject][ordered]@{calculation_id=$Id;outputs=$Out;inputs=$In;condition=$null;official_formula=$Formula;rounding='amtFormat/toFixed(2) after source arithmetic.';trigger=$Trigger;depends_on=$Deps;source_refs=$Refs;assessment=$Assessment;recommended_app_behavior=$Recommended;confidence='high'})
}
Calc '2200a-schedule-fixed-row-due' @('frm2200Av2020:txtSched1_*_Due') @('frm2200Av2020:txtSched1_*_ATR','frm2200Av2020:txtSched1_*_Taxable') 'Due = ATR x taxable amount; export is read but not part of the formula.' computeField @() @('official-hta-runtime#computeField:L7327-L7348')
Calc '2200a-schedule-others-due' @('frm2200Av2020:txtSched1_xa_others{N>=1}_Due') @('frm2200Av2020:txtSched1_xa_others{N>=1}_Rate','frm2200Av2020:txtSched1_xa_others{N>=1}_Taxable') 'No assignment occurs; the existing Due value is retained.' computeOthers @() @('official-hta-runtime#computeOthers:L7351-L7388') 'incorrect-official-behavior' 'Compute rate x taxable amount and make Due read-only.'
Calc '2200a-schedule-total' @('frm2200Av2020:txtSched1_TotalDue','frm2200Av2020:txtExciseDue') @('frm2200Av2020:txtSched1_*_Due') 'Sum every Schedule 1 input whose id ends in _Due; copy total to Item 16.' calculate_Sched1_TotalDue @('2200a-schedule-fixed-row-due','2200a-schedule-others-due') @('official-hta-runtime#calculate_Sched1_TotalDue:L7391-L7400')
Calc '2200a-item17c' @('frm2200Av2020:txtLess_Tot') @('frm2200Av2020:txtLess_Balance','frm2200Av2020:txtLess_Excise') 'Item 17C = Item 17A + Item 17B.' calculate_Part3 @() @('official-hta-runtime#calculate_Part3:L7275-L7279')
Calc '2200a-item18' @('frm2200Av2020:txtNetTaxDue') @('frm2200Av2020:txtExciseDue','frm2200Av2020:txtLess_Tot') 'Item 18 = Item 16 - Item 17C.' calculate_Part3 @('2200a-schedule-total','2200a-item17c') @('official-hta-runtime#calculate_Part3:L7280-L7283')
Calc '2200a-item20' @('frm2200Av2020:txtStillDue') @('frm2200Av2020:txtNetTaxDue','frm2200Av2020:txtPrevReturn') 'Item 20 = Item 18 - Item 19.' calculate_Part3 @('2200a-item18') @('official-hta-runtime#calculate_Part3:L7285-L7288')
Calc '2200a-item21d' @('frm2200Av2020:txtPen_Tot') @('frm2200Av2020:txtPen_Surcharge','frm2200Av2020:txtPen_Interest','frm2200Av2020:txtPen_Compromise') 'Item 21D = Item 21A + Item 21B + Item 21C.' calculate_Part3 @() @('official-hta-runtime#calculate_Part3:L7290-L7294')
Calc '2200a-item22' @('frm2200Av2020:txtAmtPayable') @('frm2200Av2020:txtStillDue','frm2200Av2020:txtPen_Tot') 'Item 22 = Item 20 + Item 21D.' calculate_Part3 @('2200a-item20','2200a-item21d') @('official-hta-runtime#calculate_Part3:L7296-L7299')
Calc '2200a-item23b' @('frm2200Av2020:txtPay_Penalties') @('frm2200Av2020:txtPen_Tot') 'Item 23B copies Item 21D.' calculate_Part3 @('2200a-item21d') @('official-hta-runtime#calculate_Part3:L7301-L7302')
Calc '2200a-item23c' @('frm2200Av2020:txtPay_Tot') @('frm2200Av2020:txtPay_TaxPayment','frm2200Av2020:txtPay_Penalties') 'Item 23C = Item 23A + Item 23B.' calculate_Part3 @('2200a-item23b') @('official-hta-runtime#calculate_Part3:L7304-L7307')
Calc '2200a-item24' @('frm2200Av2020:txtBalance') @('frm2200Av2020:txtAmtPayable','frm2200Av2020:txtPay_Tot') 'Item 24 = Item 22 - Item 23C.' calculate_Part3 @('2200a-item22','2200a-item23c') @('official-hta-runtime#calculate_Part3:L7309-L7312')
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=@($calcs.calculation_id);calculations=$calcs})
$cases=@();$n=0;foreach($rule in @($rules|Where-Object{$_.exact_message})){$n++;$cases+=[pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}'-f$n,$rule.rule_id);phase=$rule.phase;mutations=@{synthetic_condition=$rule.condition};expected_message=$rule.exact_message;expected_behavior=$rule.official_behavior;rule_id=$rule.rule_id}}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=@(@{case_id='fixed-row';calculation_id='2200a-schedule-fixed-row-due';atr=0.42;taxable=1000;official_output=420},@{case_id='part3';calculation_id='2200a-item24';amount_payable=1000;payment=1000;official_output=0},@{case_id='others-defect';calculation_id='2200a-schedule-others-due';rate=0.1;taxable=1000;existing_due=0;official_output=0;recommended_output=100})})
$resources=@();foreach($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1')|ForEach-Object{$_.Groups['v'].Value}|Sort-Object -Unique)){$full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src));if(Test-Path -LiteralPath $full){$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}}else{$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}}}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$deadline='For each place of production, file a separate return and pay before removal of the alcohol products.'
$workflow=[ordered]@{'$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;phases=@(
    @{phase='edit';official_behavior='January 2020 excise-tax return for removals of alcohol products, with Schedule 1.';source_refs=@('official-hta-runtime','official-form-pdf','packaged-help');confidence='high'},
    @{phase='saved-draft';official_behavior='Save checks only a non-fully-blank return date, TIN, RDO, and name before flat serialization.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L5938-L5959','official-hta-runtime#saveXML');confidence='high'},
    @{phase='validated';official_behavior='Validate checks period, identity, production/removal place, relief, payment scheme, and fund sufficiency, but omits Schedule 1 completeness.';source_refs=@('official-hta-runtime#validateForm:L5607-L5746');confidence='high'},
    @{phase='final-copy';official_behavior='Encryption exists, but the available 134-key final copy is proven legacy and excluded.';source_refs=@('official-hta-runtime#saveEncryptedProfile','excluded-legacy-encrypted-keys');confidence='medium'},
    @{phase='submitted';official_behavior='Online transport code exists but was not exercised.';source_refs=@('official-hta-runtime#sendEmail','official-hta-runtime#uploadXMLFile');confidence='medium'}
);transitions=@(
    @{from='edit';action='Save';to='saved-draft';guard='Sparse Save checks pass.';side_effects=@('Writes flat pseudo-XML.','Preserves duplicate key occurrences and runtime rows.');source_refs=@('official-hta-runtime#saveXML')},
    @{from='edit';action='Validate';to='validated';guard='Main Validate checks pass, including sufficient deposit.';side_effects=@('Disables controls.','Enables final copy.');source_refs=@('official-hta-runtime#validateForm','official-hta-runtime#enabledDisabledControls')},
    @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables most controls; Item 12A has a source defect.');source_refs=@('official-hta-runtime#editForm')},
    @{from='validated';action='Final Copy';to='final-copy';guard='Finalization succeeds.';side_effects=@('Encrypts and compresses the copy.');source_refs=@('official-hta-runtime#saveEncryptedProfile')},
    @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and acceptance succeed.';side_effects=@('Untested online attempt.');source_refs=@('official-hta-runtime#sendEmail')}
);prerequisites=@('Return period','TIN/RDO and taxpayer identity','Place of production and removal','Tax relief specification when applicable','Manner of payment','Sufficient deposit/payment','Schedule 1 should be complete although main Validate omits it');required_attachments=@();filing_deadlines=@(
    @{quarter='Q1';due_date_rule=$deadline;source_refs=@('packaged-help#L203-L218');confidence='high'},@{quarter='Q2';due_date_rule=$deadline;source_refs=@('packaged-help#L203-L218');confidence='high'},@{quarter='Q3';due_date_rule=$deadline;source_refs=@('packaged-help#L203-L218');confidence='high'},@{quarter='Q4';due_date_rule=$deadline;source_refs=@('packaged-help#L203-L218');confidence='high'}
)}
Write-Json (Join-Path $outDir 'workflow.json') $workflow
$bugCount=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'January 2020 ENCS runtime.'
    Asset 'legacy-runtime-excluded' 'runtime-extracted-hta' $legacyPath 'Legacy predecessor, used only to exclude the sample.'
    Asset 'packaged-help' 'official-runtime-help' $helpPath 'January 2020 packaged instructions.'
    Asset 'legacy-help-excluded' 'official-runtime-help' $legacyHelpPath 'Legacy instructions, excluded from current legal facts.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2020 ENCS official form.'
    Asset 'legacy-encrypted-sample-excluded' 'dummy-profile-encrypted-final-copy' $samples[0].FullName 'Excluded: 134/134 keys match legacy; only 6/134 match January 2020.' $redactedSample
)
$manifest=[ordered]@{'$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='2200A';revision=$revision;package_version=$packageVersion;status='complete';official_assets=$assets;counts=[ordered]@{concrete_fields=284;runtime_field_families=8;fields_total=$fields.Count;typed_fields=$fields.Count;validation_rules=$rules.Count;confirmed_official_bugs=$bugCount;calculations=$calcs.Count;negative_fixtures=$cases.Count;unverified_gaps=2};artifacts=[ordered]@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';excluded_legacy_encrypted_keys='fixtures/excluded-legacy-encrypted-field-keys-v796.json';runtime_controls='fixtures/runtime-control-inventory-v796.json';validation_functions='fixtures/validation-function-inventory-v796.json';calculation_functions='fixtures/calculation-function-inventory-v796.json';resources='fixtures/official-resource-hashes-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'};scope_notes=@('Research only; no renderer/release metadata changed.','No decrypted values or email-bearing filenames emitted.','Legacy 134-key sample excluded.','283 static serialized occurrences + runtime RDO selector + 8 unbounded Others families preserved.','Five repeated identity keys occur three times each and are preserved with occurrence suffixes.')}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 2200A - January 2020`n`nRevision-specific package with 284 concrete serialized occurrences and 8 unbounded Schedule 1 Others families.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') "# Evidence`n`n- January 2020 runtime: $($expected.hta).`n- Legacy runtime: $($expected.legacy), excluded.`n- Current help: $($expected.help); official PDF: $($expected.pdf).`n- Sample: ciphertext $($expected.cipher), decrypted $($expected.plain), 134 keys, inventory $($expected.inventory); values never emitted.`n- All 134 keys overlap the legacy runtime; only 6 overlap January 2020. Sample excluded.`n- Inventory: 283 static occurrences, one runtime RDO selector, 8 unbounded families.`n- Duplicate occurrences: tinA, tinB, tinC, branchCode, and registeredName each occur three times.`n`nAll email-bearing filenames use `#email-redacted#`.`n"
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. No revision-matched January 2020 encrypted final copy is available; the supplied sample is proven legacy and excluded.`n2. Online submission was not exercised.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- January 2020 revision separation: pass.`n- Legacy sample excluded: 134/134 legacy versus 6/134 current overlap.`n- Typed inventory: 284 concrete + 8 families = $($fields.Count).`n- Validations: $($rules.Count); calculations: $($calcs.Count); negative fixtures: $($cases.Count); official defects: $bugCount.`n- Full structural/schema audit must run after generation.`n- No renderer/release/capability/commit/push changes.`n"
$indexPath=Join-Path $RepoRoot 'rules\index.json';$index=Get-Content -Raw -LiteralPath $indexPath|ConvertFrom-Json;$entry=$index.forms|Where-Object{$_.form_id-eq$formId};if($entry){$entry.form_code='2200A';$entry.revision=$revision;$entry.package_version=$packageVersion;$entry.priority=31;$entry.status='complete';$entry.path='forms/2200a-v2020/manifest.json'}else{$index.forms+=[pscustomobject][ordered]@{form_id=$formId;form_code='2200A';revision=$revision;package_version=$packageVersion;priority=31;status='complete';path='forms/2200a-v2020/manifest.json'}};$index.forms=@($index.forms|Sort-Object priority);$index.updated='2026-07-23';Write-Json $indexPath $index
[pscustomobject]@{form_id=$formId;concrete_fields=284;families=8;typed_fields=$fields.Count;validations=$rules.Count;calculations=$calcs.Count;negative_fixtures=$cases.Count;confirmed_official_bugs=$bugCount;next_form='2200AN'}|ConvertTo-Json
