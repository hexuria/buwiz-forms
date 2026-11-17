param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$SourceDir = 'C:\Mac\Home\Downloads\forms\1604Ev2018'
)

$ErrorActionPreference = 'Stop'
$formId = '1604e-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1604Ev2018.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1604Ev2018.hta'
$pdfPath = Join-Path $SourceDir '1604E Jan 2018 ENCS Final2.pdf'
$plainPath = 'C:\eBIRForms\savefile\00000000000000-1604Ev2018-2025.xml'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1604e-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'
foreach ($path in @($htaPath, $helpPath, $pdfPath, $plainPath, $packagePath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 50) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function Get-Attr([string]$Tag, [string]$Name) {
    $match = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($match.Success) { $match.Groups[2].Value } else { $null }
}
function Get-HashText([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-', '').ToLowerInvariant()
    } finally { $sha.Dispose() }
}
function Asset([string]$Id, [string]$Kind, [string]$Path, [string]$Binding) {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $Id
        kind = $Kind
        path = $Path
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        size = $item.Length
        revision_binding = $Binding
    }
}

$expected = @{
    hta = '8e100c0457c6050dfcaeeddb0b7962443071433a5cd7b6218217d7a5288a0be8'
    help = '90f35839c2525d05710a1a301288f6ad8368a91dc2869cc947d35b05dbb7a58a'
    pdf = '1db203442630c74ff4c95b509e204f542c5ba8fb1bd812440793e314ce709876'
    plain = '78a460fc5953fb66d95e8327ba9389e2113a192472d62f860872f78b0dbab50b'
    inventory = '2a96b8edd8a35a7436fce6300869811f21a825078f9de57ad9bd05aa69fd4215'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}
foreach ($pair in @(
    @($htaPath, 'hta'), @($helpPath, 'help'), @($pdfPath, 'pdf'),
    @($plainPath, 'plain'), @($packagePath, 'package')
)) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') { throw 'PDF magic mismatch.' }
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
$plain = [IO.File]::ReadAllText($plainPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*"1604Ev2018"') { throw 'APPLICATIONNAME mismatch.' }
if ($hta -notmatch '(?i)January\s+2018' -or $help -notmatch '(?i)January\s+2018') { throw 'Revision mismatch.' }

function Save-Keys([string]$Text) {
    @([regex]::Matches($Text, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') | ForEach-Object { $_.Groups['key'].Value })
}
$keys = Save-Keys $plain
if ($keys.Count -ne 134 -or ($keys | Sort-Object -Unique).Count -ne 134) { throw "Expected 134 unique keys; found $($keys.Count)." }
if ((Get-HashText @($keys | Sort-Object)) -ne $expected.inventory) { throw 'Plain inventory hash changed.' }
if ($keys -notcontains 'frm1604e:txtWthhldngAgntsNme' -or $keys -notcontains 'frm1604e:txtSched1RemDate1') { throw 'Target-revision keys missing.' }
if ($keys -contains 'frm1604e:txtAgentname') { throw 'Legacy 1604E save was selected.' }

$formMatch = [regex]::Match($hta, '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain missing.' }
$body = $formMatch.Groups['body'].Value
$offset = $formMatch.Groups['body'].Index
$scriptRanges = @([regex]::Matches($body, '(?is)<script\b.*?</script>'))
$controls = @()
$ordinal = 0
foreach ($match in [regex]::Matches($body, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $insideScript = $false
    foreach ($range in $scriptRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) { $insideScript = $true; break }
    }
    if ($insideScript) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $controls += [pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Get-Attr $tag 'id'
        name = Get-Attr $tag 'name'
        element = $element
        control_kind = $kind.ToLowerInvariant()
        source_line = 1 + [regex]::Matches($hta.Substring(0, $offset + $match.Index), "`n").Count
        value = Get-Attr $tag 'value'
        maxlength = Get-Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
}
$serial = @($controls | Where-Object { $_.control_kind -in @('text', 'select', 'select-one', 'textarea', 'radio', 'checkbox', 'hidden') })
if ($controls.Count -ne 160 -or $serial.Count -ne 138) { throw "Expected 160 controls/138 serializer candidates; found $($controls.Count)/$($serial.Count)." }
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control }
}

$required = @(
    'frm1604e:txtYear', 'frm1604e:txtTIN1', 'frm1604e:txtTIN2', 'frm1604e:txtTIN3',
    'frm1604e:txtBranchCode', 'frm1604e:txtRDOCode', 'frm1604e:txtWthhldngAgntsNme',
    'frm1604e:txtAddress', 'frm1604e:txtZipCode', 'frm1604e:txtTelNum',
    'frm1604e:WthldngAgntCtgry_1', 'frm1604e:WthldngAgntCtgry_2'
)
$computedPattern = '(?i)(TotRemAmt|TaxWithheldTtl|PenaltiesTtl)$'
function Field-Meta([string]$Key, $Control) {
    $page = if ($Key -match '(?i)(Pg2|Sched2)') { 2 } else { 1 }
    $item = $null
    $logical = 'string'
    $status = 'optional'
    $enum = [object[]]@()
    $normalization = [string[]]@()
    if ($Key -match '(?i)txtSched1(?<kind>RemDate|BankCode|TRANo|TaxWithheld|Penalties|TotRemAmt)(?<row>\d+)$') {
        $item = "Schedule 1 row $($Matches.row)"
    } elseif ($Key -match '(?i)txtSched2(?<kind>RemDate|BankCode|TRANo|TaxWithheld|Penalties|TotRemAmt)(?<row>\d+)$') {
        $item = "Schedule 2 month $($Matches.row)"
    }
    if ($Key -match '(?i)(AmendedRtn|WthldngAgntCtgry|TpWthldngAgnt)') {
        $logical = 'boolean'; $enum = [object[]]@('true', 'false')
    } elseif ($Key -match '(?i)(TIN\d|BranchCode|RDOCode)') {
        $logical = 'code'
    } elseif ($Key -eq 'txtEmail') {
        $logical = 'email-string'
    } elseif ($Key -match '(?i)TelNum') {
        $logical = 'phone-string'
    } elseif ($Key -match '(?i)RemDate') {
        $logical = 'date-string'; $normalization = [string[]]@('MM/DD/YYYY')
    } elseif ($Key -match '(?i)(TaxWithheld|Penalties|TotRemAmt)') {
        $logical = 'decimal-amount'; $normalization = [string[]]@('NumWithComma', 'round(this,2)', 'formatCurrency')
    } elseif ($Key -match '(?i)(txtYear|txtSheets|txtCurrentPage|txtMaxPage)') {
        $logical = 'integer'
    }
    if ($required -contains $Key) { $status = 'required' }
    $computed = $Key -match $computedPattern
    if ($computed) { $status = 'computed' }
    if ($Key -match '^(txtFinalFlag|txtEnroll|ebirOnline|driveSelectTPExport)') { $status = 'hidden' }
    $constraints = [ordered]@{}
    if ($Control -and $Control.maxlength -and $Control.maxlength -match '^\d+$') { $constraints.max_length = [int]$Control.maxlength }
    [pscustomobject]@{
        page = $page; item = $item; logical = $logical; status = $status; enum = $enum
        normalization = $normalization; computed = $computed; constraints = [pscustomobject]$constraints
    }
}
$labels = @{
    'frm1604e:txtYear' = 'Taxable year'
    'frm1604e:txtSheets' = 'Number of sheets attached'
    'frm1604e:txtWthhldngAgntsNme' = 'Withholding agent name'
    'frm1604e:txtAddress' = 'Registered address'
    'frm1604e:txtZipCode' = 'ZIP code'
    'frm1604e:txtTelNum' = 'Contact number'
    'txtEmail' = 'Email address'
}
$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    $control = if ($controlById.ContainsKey($key)) { $controlById[$key] } else { $null }
    $meta = Field-Meta $key $control
    $refs = @("xml-editable-v1#field:$key")
    if ($control) { $refs += "official-hta-runtime#control:L$($control.source_line)" } else { $refs += 'official-hta-runtime#saveXML/runtime-injection' }
    $label = if ($labels.ContainsKey($key)) { $labels[$key] } elseif ($key -match '(?i)txtSched(?<schedule>[12])(?<kind>RemDate|BankCode|TRANo|TaxWithheld|Penalties|TotRemAmt)(?<row>\d+)') { "Schedule $($Matches.schedule) row $($Matches.row) $($Matches.kind)" } else { $key }
    $fields.Add([pscustomobject][ordered]@{
        field_key = $key; serialized_key = $key; serialized_occurrence = 1; label = $label
        page = $meta.page; item_number = $meta.item
        control_kind = if ($control) { $control.control_kind } else { 'runtime-injected-control' }
        storage_type = 'string'; logical_type = $meta.logical; required = $meta.status
        required_when = if ($key -match '(?i)txtSched[12](RemDate|BankCode|TRANo|TaxWithheld)\d+$') { 'Any field in the same remittance row is populated or nonzero.' } else { $null }
        enabled_when = $null; visible_when = $null
        default_value = if ($control) { $control.value } else { $null }
        empty_representation = ''; constraints = $meta.constraints; enum_values = $meta.enum
        normalization = $meta.normalization; computed = $meta.computed
        calculation_id = if ($meta.computed) { 'See calculations.json' } else { $null }
        source_refs = $refs; confidence = if ($control) { 'high' } else { 'medium' }
        notes = @('Observed in the revision-matched 134-key dummy save; source values are excluded.')
    })
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'; schema_version = '1.0.0'; form_id = $formId
    revision = $revision; field_count = $fields.Count; runtime_serializable_element_count = 134
    inventory_sha256 = Get-HashText @($fields.field_key | Sort-Object); fields = $fields
})

$staticIds = @($serial.id | Where-Object { $_ } | Sort-Object -Unique)
$plainOnly = @(Compare-Object $staticIds @($keys | Sort-Object -Unique) -PassThru | Where-Object SideIndicator -eq '=>')
$staticOnly = @(Compare-Object $staticIds @($keys | Sort-Object -Unique) -PassThru | Where-Object SideIndicator -eq '<=')
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; official_hta_sha256 = $expected.hta
    form_control_count = $controls.Count; static_serializer_candidate_count = $serial.Count
    static_serializer_unique_id_count = $staticIds.Count; reviewed_plaintext_key_count = $keys.Count
    runtime_modal_family_count = 0
    serializer_set_differences = [ordered]@{ runtime_injected_observed = $plainOnly; static_not_in_plain_snapshot = $staticOnly }
    controls = $controls; dynamic_families = @()
})
Write-Json (Join-Path $fixtureDir 'plaintext-field-audit-v796.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; file_name = '1604Ev2018-dummy-save.xml'
    sha256 = $expected.plain; field_count = $keys.Count; unique_field_count = ($keys | Sort-Object -Unique).Count
    field_inventory_sha256 = $expected.inventory; contains_target_revision_keys = $true
    contains_legacy_agent_key = $false; values_emitted = $false
})
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1604e:' -NamePattern '(?i)valid|check|enable|disable|save|submit|final') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1604e:' -NamePattern '(?i)compute|sum|total|returnperiod|clearcomputed') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule(
    [string]$Id, [string]$Phase, $Order, [string]$Condition, [string[]]$FieldKeys, $Message,
    [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Official = 'The branch alerts and stops the active operation.',
    [string]$Recommended = 'Retain as a structured revision-aware error.',
    [string]$Confidence = 'high'
) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id = $Id; form_id = $formId; revision = $revision; phase = $Phase; order = $Order
        condition = $Condition; fields = $FieldKeys; accepted_behavior = 'Condition is false; processing continues.'
        rejected_behavior = 'The active operation stops.'; exact_message = $Message; source_refs = $Refs
        evidence_type = @('source'); assessment = $Assessment; official_behavior = $Official
        recommended_app_behavior = $Recommended; confidence = $Confidence; unresolved_questions = @()
    })
}

Rule '1604e-save-001' 'save' 1 'Taxable year is blank.' @('frm1604e:txtYear') 'Please enter a valid year on Item 1.' @('official-hta-runtime#initialValidateBeforeSave:L5464-L5471')
Rule '1604e-save-002' 'save' 2 'Any TIN segment or branch code is blank.' @('frm1604e:txtTIN1','frm1604e:txtTIN2','frm1604e:txtTIN3','frm1604e:txtBranchCode') 'Please enter a valid TIN number on Item 4.' @('official-hta-runtime#initialValidateBeforeSave:L5472-L5483') 'incorrect-official-behavior' 'Only nonblankness is checked.' 'Permit drafts, but require exact shape and checksum before finalization.'
Rule '1604e-save-003' 'save' 3 'RDO code is blank.' @('frm1604e:txtRDOCode') 'Please enter a valid RDO Code on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L5484-L5491')
Rule '1604e-save-004' 'save' 4 'Withholding agent name is blank.' @('frm1604e:txtWthhldngAgntsNme') "Please enter a valid Withholding Agent's Name on Item 6." @('official-hta-runtime#initialValidateBeforeSave:L5492-L5500')
Rule '1604e-save-005' 'save' 5 'Other Validate requirements are absent.' @('return-body') $null @('official-hta-runtime#initialValidateBeforeSave:L5464-L5522') 'official-bug-compatible' 'Save omits address, ZIP, contact, category, schedules, and format checks.' 'Save drafts losslessly and report completeness separately.'

$order = 0
function Validate-Rule([string]$Suffix, [string]$Condition, [string[]]$FieldKeys, $Message, [string]$Lines, [string]$Assessment = 'verified-correct', [string]$Official = 'The branch alerts and returns.', [string]$Recommended = 'Retain with revision-aware wording.') {
    $script:order++
    Rule "1604e-validate-$Suffix" 'validate' $script:order $Condition $FieldKeys $Message @("official-hta-runtime#validate:L$Lines") $Assessment $Official $Recommended
}
Validate-Rule '001' 'Taxable year is blank.' @('frm1604e:txtYear') 'Please enter a valid year on Item 1.' '4487-L4492'
Validate-Rule '002' 'Any TIN segment or branch code is blank.' @('frm1604e:txtTIN1','frm1604e:txtTIN2','frm1604e:txtTIN3','frm1604e:txtBranchCode') 'Please enter a valid TIN number on Item 4.' '4493-L4500' 'incorrect-official-behavior' 'No length, digit, or checksum validation follows.' 'Require exact shape and checksum before finalization.'
Validate-Rule '003' 'RDO code is blank.' @('frm1604e:txtRDOCode') 'Please enter a valid RDO Code on Item 5.' '4501-L4506'
Validate-Rule '004' 'Withholding agent name is blank.' @('frm1604e:txtWthhldngAgntsNme') 'Please enter a valid Taxpayer Name on Item 6.' '4507-L4512'
Validate-Rule '005' 'Contact number is blank.' @('frm1604e:txtTelNum') 'Please enter a valid Telephone Number on Item 9.' '4513-L4518'
Validate-Rule '006' 'Registered address is blank.' @('frm1604e:txtAddress') "Please enter Taxpayer's Registered Address on Item 7." '4519-L4524'
Validate-Rule '007' 'ZIP code is blank.' @('frm1604e:txtZipCode') "Please enter Taxpayer's Zip Code on Item 7A." '4525-L4530'
Validate-Rule '008' 'Neither withholding-agent category is selected.' @('frm1604e:WthldngAgntCtgry_1','frm1604e:WthldngAgntCtgry_2') 'Please select an option in Item 8.' '4531-L4536'

$quarterLabels = @('1st Quarter', '2nd Quarter', '3rd Quarter', '4th Quarter')
$monthLabels = @('January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December')
function Add-ScheduleRules([int]$Schedule, [string[]]$Periods, [string]$SourceForm, [string]$Lines) {
    for ($index = 1; $index -le $Periods.Count; $index++) {
        $period = $Periods[$index - 1]
        $prefix = "frm1604e:txtSched$Schedule"
        $periodWord = if ($Schedule -eq 1) { 'Quarter' } else { 'month' }
        $base = "1604e-sched$Schedule-{0:d2}" -f $index
        Rule "$base-date" 'validate' (100 + ($Schedule * 100) + ($index * 5) + 1) "A partially populated $period row has no date of remittance." @("$prefix`RemDate$index") "Please enter Date of Remittance for the $period. You may refer to your $SourceForm for the said $periodWord." @("official-hta-runtime#$Lines",'official-hta-runtime#validateScheduleFields:L5042-L5158')
        Rule "$base-bank" 'validate' (100 + ($Schedule * 100) + ($index * 5) + 2) "A partially populated $period row has no drawee bank/bank code/agency." @("$prefix`BankCode$index") "Please enter any of the following details Drawee Bank / Bank Code / Agency for the $period. You may refer to your $SourceForm for the said $periodWord." @("official-hta-runtime#$Lines",'official-hta-runtime#validateScheduleFields:L5042-L5158')
        Rule "$base-tra" 'validate' (100 + ($Schedule * 100) + ($index * 5) + 3) "A partially populated $period row has no TRA/eROR/eAR number." @("$prefix`TRANo$index") "Please enter any of the following details TRA / eROR / eAR Number for the $period. You may refer to your $SourceForm for the said $periodWord." @("official-hta-runtime#$Lines",'official-hta-runtime#validateScheduleFields:L5042-L5158')
        Rule "$base-tax" 'validate' (100 + ($Schedule * 100) + ($index * 5) + 4) "A partially populated $period row has Taxes Withheld equal to 0.00." @("$prefix`TaxWithheld$index") "Please enter the Taxes Withheld for the $period. You may refer to your $SourceForm for the said $periodWord." @("official-hta-runtime#$Lines",'official-hta-runtime#validateScheduleFields:L5042-L5158')
        Rule "$base-penalty-unreachable" 'validate' (100 + ($Schedule * 100) + ($index * 5) + 5) "Penalties equal 0.00 in a populated $period row." @("$prefix`Penalties$index") "Please enter the Penalties for the $period. You may refer to your $SourceForm for the said $periodWord." @("official-hta-runtime#$Lines",'official-hta-runtime#validateScheduleFields:L5125-L5158') 'obsolete' 'A duplicated penalties != 0.00 condition makes this message branch unreachable; zero penalties pass.' 'Treat penalties as optional unless the legal facts require them.'
    }
}
Add-ScheduleRules 1 $quarterLabels '1601EQ' 'validateSchedule1:L4554-L4677'
Add-ScheduleRules 2 $monthLabels '1606' 'validateSchedule2:L4678-L5041'

Validate-Rule '009' 'All active checks pass.' @('return-body') "Validation successful. Click on 'Edit' if you wish to modify your entries." '4537-L4553'
Rule '1604e-input-year-pre2018' 'blur/change' $null 'Edited year is before 2018.' @('frm1604e:txtYear') "Invalid data entry on item no. 1. `nEntry should be current or prior year but not be earlier than the effectivity date of January 2018." @('official-hta-runtime#validateYear:L6485-L6492')
Rule '1604e-input-year-getyear-bug' 'blur/change' $null 'Any normal four-digit year is compared with Date.getYear().' @('frm1604e:txtYear') "Invalid data entry on item no. 1. `nEntry should be current or prior year but not be earlier than the effectivity date of January 2018." @('official-hta-runtime#validateYear:L6485-L6499') 'incorrect-official-behavior' 'Date.getYear() returns current year minus 1900, so a normal year is replaced with a value such as 126 in 2026.' 'Use getFullYear(), enforce 2018 through the current full year, and preserve the entered year on a failed edit.'
Rule '1604e-input-date-shape' 'blur/change' $null 'Date is not a valid MM/DD/YYYY calendar date.' @('schedule-remittance-date-fields') 'Please provide a valid date. (MM/DD/YYYY format)' @('official-hta-runtime#validateDate:L6412-L6484')
Rule '1604e-input-date-future' 'blur/change' $null 'Date is later than the current date.' @('schedule-remittance-date-fields') 'This date cannot be a future date.' @('official-hta-runtime#validateDate:L6412-L6484')
Rule '1604e-input-date-pre2018' 'blur/change' $null 'Date is before 2018.' @('schedule-remittance-date-fields') 'This date cannot be prior to 2018.' @('official-hta-runtime#validateDate:L6412-L6484')
Rule '1604e-schedule-empty-row' 'validate' $null 'Every field in a schedule row is blank or zero.' @('schedule-row-fields') $null @('official-hta-runtime#validateScheduleFields:L5042-L5158') 'verified-correct' 'A fully blank/zero row passes.' 'Keep empty rows optional.'
Rule '1604e-schedule-penalty-fallthrough' 'validate' $null 'A populated row has nonzero tax withheld and zero penalties.' @('schedule-row-fields') $null @('official-hta-runtime#validateScheduleFields:L5125-L5158') 'official-bug-compatible' 'The helper can fall through undefined, and callers ultimately accept the row.' 'Represent penalties as optional and avoid undefined control flow.'
Rule '1604e-email-unvalidated' 'validate' $null 'A nonblank malformed email is present.' @('txtEmail') $null @('official-hta-runtime#validate:L4487-L4553','official-help#item10') 'incorrect-official-behavior' 'Email is optional and never format-checked.' 'Allow blank; validate format when nonblank.'
Rule '1604e-top-agent-omitted' 'validate' $null 'Private category is selected but Item 8A is unanswered.' @('frm1604e:WthldngAgntCtgry_1','frm1604e:TpWthldngAgnt_1','frm1604e:TpWthldngAgnt_2') $null @('official-hta-runtime#validate:L4487-L4553','official-help#item8a') 'incorrect-official-behavior' 'Validate ignores Item 8A.' 'Require Item 8A when private category makes it applicable.'
Rule '1604e-schedule3-commented' 'validate' $null 'Commented Schedule 3 controls and computations are inspected.' @('commented-schedule3-fields') $null @('official-hta-runtime#commented-schedule3') 'obsolete' 'Schedule 3 validation and computation are wholly commented out.' 'Do not model commented controls as active behavior.'
Rule '1604e-final-001' 'final-copy' 1 'Final Copy is requested after successful validation.' @('txtFinalFlag','return-body') $null @('official-hta-runtime#saveEncryptedProfile:L2414-L2589','official-hta-runtime#saveXML:L2590-L2932') 'unverified' 'The source couples encrypted output to profile/connectivity behavior; no target-revision encrypted artifact was produced.' 'Keep deterministic local finalization separate from transport.' 'medium'
Rule '1604e-submit-001' 'submit' 1 'Online transport is invoked.' @('return-body') $null @('official-hta-runtime#saveXMLsubmit:L2933-L3182','official-hta-runtime#sendEmail:L6114-L6216') 'unverified' 'Transport exists but was not exercised.' 'Keep local validation independently testable.' 'medium'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'; schema_version = '1.0.0'; form_id = $formId; revision = $revision
    first_error_behavior = 'Save returns at its first narrow failure. Validate checks background fields in source order, then Schedule 1 and Schedule 2 in period/field order, and stops at the first active failure.'
    rules = $rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Calc([string]$Id, [string[]]$Outputs, [string[]]$Inputs, [string]$Formula, [string]$Trigger, [string[]]$Depends, [string[]]$Refs, [string]$Assessment = 'verified-correct', [string]$Recommended = 'Use decimal arithmetic and recompute from authoritative inputs.') {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id = $Id; outputs = $Outputs; inputs = $Inputs; condition = $null
        official_formula = $Formula; rounding = 'Inputs are parsed after comma removal; outputs use formatCurrency.'
        trigger = $Trigger; depends_on = $Depends; source_refs = $Refs; assessment = $Assessment
        recommended_app_behavior = $Recommended; confidence = 'high'
    })
}
Calc '1604e-schedule1-row-total' @('frm1604e:txtSched1TotRemAmt1..4') @('corresponding Schedule 1 TaxesWithheld','corresponding Schedule 1 Penalties') 'Total Amount Remitted = Taxes Withheld + Penalties.' 'computeSched1(row)' @() @('official-hta-runtime#computeSched1:L6500-L6511')
Calc '1604e-schedule1-column-totals' @('frm1604e:txtSched1TaxWithheldTtl','frm1604e:txtSched1PenaltiesTtl','frm1604e:txtSched1TotRemAmtTtl') @('four Schedule 1 rows') 'Independently sum the four quarterly Taxes Withheld, Penalties, and Total Amount Remitted columns.' 'computeSched1Total' @('1604e-schedule1-row-total') @('official-hta-runtime#computeSched1Total:L6512-L6534')
Calc '1604e-schedule2-row-total' @('frm1604e:txtSched2TotRemAmt1..12') @('corresponding Schedule 2 TaxesWithheld','corresponding Schedule 2 Penalties') 'Total Amount Remitted = Taxes Withheld + Penalties.' 'computeSched2(row)' @() @('official-hta-runtime#computeSched2:L6535-L6546')
Calc '1604e-schedule2-column-totals' @('frm1604e:txtSched2TaxWithheldTtl','frm1604e:txtSched2PenaltiesTtl','frm1604e:txtSched2TotRemAmtTtl') @('twelve Schedule 2 rows') 'Independently sum the twelve monthly Taxes Withheld, Penalties, and Total Amount Remitted columns.' 'computeSched2Total' @('1604e-schedule2-row-total') @('official-hta-runtime#computeSched2Total:L6547-L6619')
Calc '1604e-schedule3-obsolete' @('commented Schedule 3 outputs') @('commented Schedule 3 inputs') 'Commented-out computation only; it does not execute.' 'none' @() @('official-hta-runtime#commented-computeSched3') 'obsolete' 'Do not implement as active behavior.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema' = '../../schema/calculations.schema.json'; schema_version = '1.0.0'; form_id = $formId; revision = $revision
    evaluation_order = @($calculations.calculation_id); calculations = $calculations
})

$negativeRules = @($rules | Where-Object { $_.exact_message } | Select-Object -First 40)
$cases = @()
$caseNumber = 0
foreach ($rule in $negativeRules) {
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
        @{ case_id = 'schedule1-row-basic'; calculation_id = '1604e-schedule1-row-total'; inputs = @{ withheld = 100; penalties = 5 }; official_output = '105.00' },
        @{ case_id = 'schedule2-row-zero-penalty'; calculation_id = '1604e-schedule2-row-total'; inputs = @{ withheld = 100; penalties = 0 }; official_output = '100.00' },
        @{ case_id = 'schedule1-four-row-total'; calculation_id = '1604e-schedule1-column-totals'; inputs = @{ rows = @(10,20,30,40) }; official_output = '100.00' }
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
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{ schema_version = '1.0.0'; form_id = $formId; resources = $resources })

$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'; schema_version = '1.0.0'; form_id = $formId; revision = $revision
    phases = @(
        @{ phase = 'edit'; official_behavior = 'January 2018 two-page annual return with four quarterly 1601-EQ rows and twelve monthly 1606 rows.'; source_refs = @('official-hta-runtime#frmMain','official-help#revision'); confidence = 'high' },
        @{ phase = 'saved-draft'; official_behavior = 'Save checks only year, TIN components, RDO, and withholding-agent name, then writes a 134-key plaintext pseudo-XML save.'; source_refs = @('official-hta-runtime#initialValidateBeforeSave:L5464-L5522','xml-editable-v1'); confidence = 'high' },
        @{ phase = 'validated'; official_behavior = 'Validate uses source-ordered background and schedule checks, then disables controls on success.'; source_refs = @('official-hta-runtime#validate:L4487-L4553'); confidence = 'high' },
        @{ phase = 'final-copy'; official_behavior = 'Final-copy code exists, but a revision-matched encrypted artifact was not produced or accepted as evidence.'; source_refs = @('official-hta-runtime#saveEncryptedProfile:L2414-L2589'); confidence = 'medium' },
        @{ phase = 'submitted'; official_behavior = 'Online transport exists but was not exercised.'; source_refs = @('official-hta-runtime#sendEmail:L6114-L6216'); confidence = 'medium' }
    )
    transitions = @(
        @{ from = 'edit'; action = 'Save'; to = 'saved-draft'; guard = 'Four narrow checks pass.'; side_effects = @('Writes plaintext pseudo-XML.'); source_refs = @('official-hta-runtime#initialValidateBeforeSave:L5464-L5522') },
        @{ from = 'edit'; action = 'Validate'; to = 'validated'; guard = 'All active ordered checks pass.'; side_effects = @('Disables controls.','Enables Print, Edit, and Final Copy.'); source_refs = @('official-hta-runtime#validate:L4487-L4553') },
        @{ from = 'validated'; action = 'Edit'; to = 'edit'; guard = $null; side_effects = @('Re-enables applicable controls.'); source_refs = @('official-hta-runtime#enableAllControl:L5285-L5400') },
        @{ from = 'validated'; action = 'Final Copy'; to = 'final-copy'; guard = 'Profile/connectivity-dependent official flow permits progress.'; side_effects = @('Attempts encrypted output.'); source_refs = @('official-hta-runtime#saveEncryptedProfile:L2414-L2589') },
        @{ from = 'final-copy'; action = 'Transport'; to = 'submitted'; guard = 'Connectivity and send succeed.'; side_effects = @('Attempts online submission; untested.'); source_refs = @('official-hta-runtime#sendEmail:L6114-L6216') }
    )
    prerequisites = @('January 2018 revision','Taxable year 2018 or later','Withholding-agent identity','Applicable quarterly and monthly remittance information')
    required_attachments = @(
        @{ attachment_id = 'alphalist-expanded'; label = 'Alphalist of Payees Subjected to Expanded Withholding Tax with electronic-submission acknowledgement/validation proof.'; required_when = 'Applicable to the annual return.'; official_ui_enforcement = 'External attachment presence is not checked locally.'; source_refs = @('official-help#attachments:L163-L176'); confidence = 'high' },
        @{ attachment_id = 'alphalist-exempt'; label = 'Alphalist of Other Payees Whose Income Payments Are Exempt from Withholding Tax but Subject to Income Tax with electronic-submission acknowledgement/validation proof.'; required_when = 'Applicable to the annual return.'; official_ui_enforcement = 'External attachment presence is not checked locally.'; source_refs = @('official-help#attachments:L163-L176'); confidence = 'high' },
        @{ attachment_id = 'authorization-letter'; label = 'Authorization letter.'; required_when = 'Filed by an authorized representative.'; official_ui_enforcement = 'Not locally checked.'; source_refs = @('official-help#attachments:L163-L176'); confidence = 'high' }
    )
    filing_deadlines = @(
        @{ quarter = 'Q1'; due_date_rule = 'Help text says the return shall be filed on or before March following the calendar year, but omits the day; exact deadline remains ambiguous in this revision artifact.'; source_refs = @('official-help#deadline:L98-L104'); confidence = 'medium' },
        @{ quarter = 'Q2'; due_date_rule = 'Annual return; same malformed March deadline text applies.'; source_refs = @('official-help#deadline:L98-L104'); confidence = 'medium' },
        @{ quarter = 'Q3'; due_date_rule = 'Annual return; same malformed March deadline text applies.'; source_refs = @('official-help#deadline:L98-L104'); confidence = 'medium' },
        @{ quarter = 'Q4'; due_date_rule = 'Annual return; same malformed March deadline text applies.'; source_refs = @('official-help#deadline:L98-L104'); confidence = 'medium' }
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$assets = @(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1604Ev2018 and printed January 2018 (ENCS).'
    Asset 'official-help' 'official-runtime-help' $helpPath 'Revision-matched January 2018 instructions.'
    Asset 'xml-editable-v1' 'dummy-profile-editable-save' $plainPath 'UI-created target-revision save with 134 keys; values excluded.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 Form 1604-E.'
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'; schema_version = '1.0.0'; form_id = $formId
    form_code = '1604E'; revision = $revision; revision_label = 'January 2018'; package_version = $packageVersion; status = 'complete'
    official_assets = $assets
    counts = [ordered]@{
        concrete_fields = 134; runtime_field_families = 0; fields_total = $fields.Count; typed_fields = $fields.Count
        validation_rules = $rules.Count; confirmed_official_bugs = $bugCount; calculations = $calculations.Count
        negative_fixtures = $cases.Count; unverified_gaps = 3
    }
    artifacts = [ordered]@{
        fields = 'fields.json'; validations = 'validations.json'; calculations = 'calculations.json'; workflow = 'workflow.json'
        evidence = 'evidence.md'; audit = 'audit.md'; gaps = 'gaps.md'
        runtime_control_fixture = 'fixtures/runtime-control-inventory-v796.json'
        plaintext_field_audit = 'fixtures/plaintext-field-audit-v796.json'
        validation_function_fixture = 'fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture = 'fixtures/calculation-function-inventory-v796.json'
        resource_hash_fixture = 'fixtures/official-resource-hashes-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'; calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer or release changes.',
        'No save values or email-bearing filenames are copied.',
        'The target-revision plaintext save has 134 unique keys and no active runtime field families.',
        'Available encrypted copies use the legacy 149-key field family and are explicitly excluded.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1604-E - January 2018`n`nRevision-specific Offline eBIRForms rule package with 134 concrete serialized keys and no active dynamic field families. Source values are excluded.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') "# Evidence`n`n- Exact HTA SHA-256: $($expected.hta); APPLICATIONNAME 1604Ev2018, January 2018 (ENCS).`n- Revision-matched help SHA-256: $($expected.help), including filer scope, item instructions, attachment list, and malformed deadline wording.`n- UI-created plaintext save SHA-256: $($expected.plain); 134 unique target-revision keys; inventory SHA-256 $($expected.inventory); values excluded.`n- Official PDF SHA-256: $($expected.pdf), valid PDF magic.`n- Runtime inventory: 160 controls, 138 serializer candidates, 75 inline functions, and no active Add-more/modal field families.`n- Available 149-key legacy encrypted/plain saves were rejected because their field family does not match 1604Ev2018.`n"
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. No revision-matched encrypted Final Copy was produced; available encrypted copies use the legacy 149-key field family and are excluded.`n2. The revision-matched help says filing is due in March but omits the day; the exact deadline is not inferred.`n3. Online submission and external attachment transport were not exercised.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- Revision/assets: **pass** - January 2018 HTA, help, PDF, package executable, and genuine target-revision plaintext save are pinned.`n- Fields: **pass** - 134 unique target-revision keys; legacy save mismatch rejected; no active dynamic families.`n- Rules/calculations/workflow: **pass** - source order, exact active and unreachable schedule messages, phase differences, five calculation records, attachment requirements, and deadline ambiguity captured.`n- Official defects: **pass** - $bugCount bug-compatible/incorrect/obsolete rules separated from recommendations, including the Date.getYear defect and unreachable penalties branches.`n- Privacy: **pass** - no values or email-bearing filenames copied.`n- Encrypted Final Copy, exact March deadline day, and online transport: **unverified** and explicit gaps.`n"
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 15: 1604e-v2018. Next: 1604F.`n"

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -LiteralPath $indexPath -Raw | ConvertFrom-Json
$entry = [pscustomobject][ordered]@{
    form_id = $formId; form_code = '1604E'; revision = $revision; package_version = $packageVersion
    priority = 15; status = 'complete'; path = 'forms/1604e-v2018/manifest.json'
}
$index.forms = @(@($index.forms | Where-Object { $_.form_id -ne $formId }) + $entry | Sort-Object priority)
$index.updated = (Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index

"Generated ${formId}: fields=$($fields.Count), rules=$($rules.Count), calculations=$($calculations.Count), negative_cases=$($cases.Count), bug_classifications=$bugCount, static_unique_ids=$($staticIds.Count), plain_only=$($plainOnly.Count), static_only=$($staticOnly.Count)"
