param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$SourceDir = 'C:\Mac\Home\Downloads\forms\1604Fv2018'
)

$ErrorActionPreference = 'Stop'
$formId = '1604f-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1604F.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1604F.hta'
$pdfPath = Join-Path $SourceDir '1604-F Jan 2018 Final 2.pdf'
$plainPath = 'C:\eBIRForms\savefile\00000000000000-1604F-2025.xml'
$encryptedCandidates = @(Get-ChildItem -LiteralPath $SourceDir -File | Where-Object { $_.Name -like '00000000000000-1604F-2025#*#.xml' })
if ($encryptedCandidates.Count -ne 1) { throw "Expected one encrypted 1604F companion; found $($encryptedCandidates.Count)." }
$encryptedPath = $encryptedCandidates[0].FullName
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1604f-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'
foreach ($path in @($htaPath, $helpPath, $pdfPath, $plainPath, $encryptedPath, $packagePath)) {
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
        asset_id = $Id; kind = $Kind; path = $Path
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        size = $item.Length; revision_binding = $Binding
    }
}

$expected = @{
    hta = 'e6bf54d0fdffac4a459ceb4ff66f7afdaec10eec15f9b0e53501f66142758741'
    help = '9bb5163298d12a41ea26cb9d5159a9b084d51d65200091679bdf841e97a0c3c6'
    pdf = 'fc34de40dc7e6bc5f7a8cbc3feb5b170cca4bce4f0abd5b7b0dece4e9dd75c4d'
    plain = 'bcc3829950b783e95e200db6bd4bd7b470f9f923a58c7c37335fe33c0f7e3d80'
    encrypted = '44f1e38d94b923ff52136b7369cdd1d329e61efb4d2ce77894178c0a4aab6be2'
    decrypted = '32ce23c9c0a1c21058b803b7a2b2079495690b1c56d01f3498cdba1ed207add6'
    inventory = 'e4e0d1fbb36cdb0958374b76cb763a171a3a6f49269d47b775836b76b5b67308'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}
foreach ($pair in @(
    @($htaPath, 'hta'), @($helpPath, 'help'), @($pdfPath, 'pdf'), @($plainPath, 'plain'),
    @($encryptedPath, 'encrypted'), @($packagePath, 'package')
)) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') { throw 'PDF magic mismatch.' }
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
$plain = [IO.File]::ReadAllText($plainPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*"1604F"') { throw 'APPLICATIONNAME mismatch.' }
if ($hta -notmatch '(?i)January\s+2018' -or $help -notmatch '(?i)January\s+2018') { throw 'Revision mismatch.' }

function Save-Keys([string]$Text) {
    @([regex]::Matches($Text, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') | ForEach-Object { $_.Groups['key'].Value })
}
$keys = Save-Keys $plain
if ($keys.Count -ne 105 -or ($keys | Sort-Object -Unique).Count -ne 105) { throw "Expected 105 unique keys; found $($keys.Count)." }
if ((Get-HashText @($keys | Sort-Object)) -ne $expected.inventory) { throw 'Plain inventory hash changed.' }
foreach ($requiredKey in @('frm1604f:txtSched1Date1','frm1604f:txtSched2Date1','frm1604f:txtSched3Date1','frm1604f:availedTaxRelief')) {
    if ($keys -notcontains $requiredKey) { throw "Target-revision key missing: $requiredKey" }
}

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
        ordinal = $ordinal; id = Get-Attr $tag 'id'; name = Get-Attr $tag 'name'
        element = $element; control_kind = $kind.ToLowerInvariant()
        source_line = 1 + [regex]::Matches($hta.Substring(0, $offset + $match.Index), "`n").Count
        value = Get-Attr $tag 'value'; maxlength = Get-Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
}
$serial = @($controls | Where-Object { $_.control_kind -in @('text','select','select-one','textarea','radio','checkbox','hidden') })
if ($controls.Count -ne 147 -or $serial.Count -ne 125) { throw "Expected 147 controls/125 serializer candidates; found $($controls.Count)/$($serial.Count)." }
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control }
}

$required = @(
    'frm1604f:txtYear','frm1604f:tinA','frm1604f:tinB','frm1604f:tinC','frm1604f:branchCode',
    'frm1604f:rdoCode','frm1604f:registeredName','frm1604f:RegisteredAddress','frm1604f:zipCode',
    'frm1604f:telephoneNumber','frm1604f:categoryAgent_1','frm1604f:categoryAgent_2'
)
$computedPattern = '(?i)(TaxWheldTotal|PenTotal|TotalAmt\d+|txtSched[123]Total)$'
function Field-Meta([string]$Key, $Control) {
    $page = if ($Key -match '(?i)Pg2') { 2 } else { 1 }
    $item = $null
    $logical = 'string'
    $status = 'optional'
    $enum = [object[]]@()
    $normalization = [string[]]@()
    if ($Key -match '(?i)txtSched(?<schedule>[123])(?<kind>Date|TRA|TaxWheld|Pen|TotalAmt)(?<row>\d+)$') {
        $item = "Schedule $($Matches.schedule) quarter $($Matches.row)"
    }
    if ($Key -match '(?i)(amendedRtn|categoryAgent|privateAgent|taxRelief)_') {
        $logical = 'boolean'; $enum = [object[]]@('true','false')
    } elseif ($Key -match '(?i)(tin[ABC]|BranchCode|rdoCode)') {
        $logical = 'code'
    } elseif ($Key -in @('frm1604f:email','txtEmail')) {
        $logical = 'email-string'
    } elseif ($Key -match '(?i)telephoneNumber') {
        $logical = 'phone-string'
    } elseif ($Key -match '(?i)txtSched[123]Date') {
        $logical = 'date-string'; $normalization = [string[]]@('MM/DD/YYYY')
    } elseif ($Key -match '(?i)(TaxWheld|Pen|TotalAmt|txtSched[123]Total)') {
        $logical = 'decimal-amount'; $normalization = [string[]]@('parseFloat', 'toFixed(2)', 'NumWithComma', 'formatCurrency', 'negative values reset to 0.00')
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
    'frm1604f:txtYear' = 'Taxable year'; 'frm1604f:txtSheets' = 'Number of sheets attached'
    'frm1604f:registeredName' = 'Withholding agent name'; 'frm1604f:RegisteredAddress' = 'Registered address'
    'frm1604f:zipCode' = 'ZIP code'; 'frm1604f:telephoneNumber' = 'Contact number'
    'frm1604f:email' = 'Taxpayer email address'; 'txtEmail' = 'Profile/online email address'
    'frm1604f:availedTaxRelief' = 'Special Law or International Tax Treaty relief availed'
}
$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    $controlKey = if ($key -eq 'frm1604f:RegisteredAddress') { 'frm1604f:registeredAddress' } else { $key }
    $control = if ($controlById.ContainsKey($controlKey)) { $controlById[$controlKey] } else { $null }
    $meta = Field-Meta $key $control
    $refs = @("xml-editable-v1#field:$key")
    if ($control) { $refs += "official-hta-runtime#control:L$($control.source_line)" } else { $refs += 'official-hta-runtime#saveXML/runtime-injection' }
    $label = if ($labels.ContainsKey($key)) {
        $labels[$key]
    } elseif ($key -match '(?i)txtSched(?<schedule>[123])(?<kind>Date|TRA|TaxWheld|Pen|TotalAmt)(?<row>\d+)') {
        "Schedule $($Matches.schedule) quarter $($Matches.row) $($Matches.kind)"
    } else { $key }
    $requiredWhen = $null
    if ($key -eq 'frm1604f:availedTaxRelief') { $requiredWhen = 'frm1604f:taxRelief_1 is checked.' }
    elseif ($key -match '(?i)txtSched[123](Date|TRA|TaxWheld)\d+$') { $requiredWhen = 'Any amount or identifying field in the same schedule row is populated.' }
    $fields.Add([pscustomobject][ordered]@{
        field_key = $key; serialized_key = $key; serialized_occurrence = 1; label = $label
        page = $meta.page; item_number = $meta.item
        control_kind = if ($control) { $control.control_kind } else { 'runtime-injected-control' }
        storage_type = 'string'; logical_type = $meta.logical; required = $meta.status
        required_when = $requiredWhen
        enabled_when = if ($key -eq 'frm1604f:availedTaxRelief') { 'frm1604f:taxRelief_1 is checked.' } else { $null }
        visible_when = $null; default_value = if ($control) { $control.value } else { $null }
        empty_representation = ''; constraints = $meta.constraints; enum_values = $meta.enum
        normalization = $meta.normalization; computed = $meta.computed
        calculation_id = if ($meta.computed) { 'See calculations.json' } else { $null }
        source_refs = $refs; confidence = if ($control) { 'high' } else { 'medium' }
        notes = @('Observed in matching 105-key plaintext and decrypted encrypted dummy-save inventories; source values are excluded.')
    })
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'; schema_version = '1.0.0'; form_id = $formId
    revision = $revision; field_count = $fields.Count; runtime_serializable_element_count = 105
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
$decryptTool = Join-Path $RepoRoot 'rules\tools\audit-1701-encrypted-fields.ps1'
Write-Utf8 (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') ((& $decryptTool `
    -SourceDir $SourceDir -FormId $formId -FilePattern '00000000000000-1604F-2025#*#.xml' `
    -RedactedFileName '00000000000000-1604F-2025#email-redacted#.xml' `
    -ExpectedCiphertextSha256 $expected.encrypted -ExpectedDecryptedSha256 $expected.decrypted `
    -ExpectedFieldCount 105 -ExpectedFieldInventorySha256 $expected.inventory `
    -ExpectedExtraField '*' -ExpectedXmlVersion '*') -join [Environment]::NewLine)
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1604f:' -NamePattern '(?i)valid|check|enable|disable|save|submit|final|year') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1604f:' -NamePattern '(?i)compute|total|clearpart') -join [Environment]::NewLine)

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
        rejected_behavior = 'The active operation stops unless official_behavior states otherwise.'
        exact_message = $Message; source_refs = $Refs; evidence_type = @('source')
        assessment = $Assessment; official_behavior = $Official; recommended_app_behavior = $Recommended
        confidence = $Confidence; unresolved_questions = @()
    })
}

Rule '1604f-save-001' 'save' 1 'Any TIN segment or branch code is blank.' @('frm1604f:tinA','frm1604f:tinB','frm1604f:tinC','frm1604f:branchCode') 'Please enter a valid TIN number on Item 4.' @('official-hta-runtime#initialValidateBeforeSave:L4711-L4716') 'incorrect-official-behavior' 'Only nonblankness is checked.' 'Permit lossless drafts, but require exact shape and checksum before finalization.'
Rule '1604f-save-002' 'save' 2 'RDO code equals literal 000.' @('frm1604f:rdoCode') 'Please enter a valid RDO Code on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L4717-L4720')
Rule '1604f-save-003' 'save' 3 'Withholding agent name is blank.' @('frm1604f:registeredName') 'Please enter a valid Taxpayer Name on Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L4721-L4725') 'incorrect-official-behavior' 'The message cites Item 7, while the printed name is Item 6.' 'Report Item 6.'
Rule '1604f-save-004' 'save' 4 'Any other field, including taxable year, is invalid or incomplete.' @('return-body') $null @('official-hta-runtime#initialValidateBeforeSave:L4711-L4727') 'official-bug-compatible' 'Save omits year, address, ZIP, contact, category, tax relief, schedule, and format checks.' 'Save drafts losslessly and report completeness separately.'

$validateOrder = 0
function Validate-Rule([string]$Suffix, [string]$Condition, [string[]]$FieldKeys, $Message, [string[]]$Refs, [string]$Assessment = 'verified-correct', [string]$Official = 'The branch alerts and returns.', [string]$Recommended = 'Retain with revision-aware wording.') {
    $script:validateOrder++
    Rule "1604f-validate-$Suffix" 'validate' $script:validateOrder $Condition $FieldKeys $Message $Refs $Assessment $Official $Recommended
}
$yearMessage = 'Invalid data entry on item no. 1. Entry should be current or prior year but not be earlier than the effectivity date of January 2018.'
Validate-Rule '001-year-alert-only' 'Year is 2017 or earlier, or later than the current full year.' @('frm1604f:txtYear') $yearMessage @('official-hta-runtime#validateForm:L3789-L3794','official-hta-runtime#checkYear:L4406-L4413') 'incorrect-official-behavior' 'checkYear alerts but returns no failure signal; Validate continues after dismissal.' 'Return a structured blocking error for years outside 2018 through the current year.'
Validate-Rule '002-year-blank' 'Taxable year is blank after the non-blocking checkYear alert.' @('frm1604f:txtYear') 'Please enter a valid year on Item 1.' @('official-hta-runtime#validateForm:L3795-L3799') 'official-bug-compatible' 'Blank year produces two consecutive alerts: effectivity first, then blank.' 'Emit one precise required-field error.'
Validate-Rule '003-tax-relief-detail' 'Item 11 Yes is selected and Item 11A relief detail is blank.' @('frm1604f:taxRelief_1','frm1604f:availedTaxRelief') 'Please specify the Special Treaty or International Law the payee is availing in item 11A.' @('official-hta-runtime#validateForm:L3801-L3805')
Validate-Rule '004-year-below1900' 'Taxable year coerces below 1900.' @('frm1604f:txtYear') 'Invalid date entry on Item no.1. Entry should not be lower than 1900.' @('official-hta-runtime#validateForm:L3812-L3816') 'incorrect-official-behavior' 'The earlier effectivity alert did not stop; this second alert finally stops only values below 1900.' 'Use the single 2018-through-current-year boundary.'
Validate-Rule '005-tin' 'Any TIN segment or branch code is blank.' @('frm1604f:tinA','frm1604f:tinB','frm1604f:tinC','frm1604f:branchCode') 'Please enter a valid TIN number on Item 4.' @('official-hta-runtime#validateForm:L3818-L3822') 'incorrect-official-behavior' 'No segment length, digit, or checksum check follows.' 'Require exact shape and checksum before finalization.'
Validate-Rule '006-rdo' 'RDO selectedIndex is zero.' @('frm1604f:rdoCode') 'Please enter a valid RDO Code on Item 5.' @('official-hta-runtime#validateForm:L3823-L3828')
Validate-Rule '007-name' 'Withholding agent name is blank.' @('frm1604f:registeredName') "Please enter a valid Taxpayer's Name on Item 7." @('official-hta-runtime#validateForm:L3834-L3838') 'incorrect-official-behavior' 'The message cites Item 7; the printed name is Item 6.' 'Report Item 6.'
Validate-Rule '008-phone' 'Contact number is blank.' @('frm1604f:telephoneNumber') 'Please enter a valid Telephone Number on Item 8.' @('official-hta-runtime#validateForm:L3839-L3843') 'incorrect-official-behavior' 'The message cites Item 8; contact number is Item 9.' 'Report Item 9.'
Validate-Rule '009-address' 'Registered address is blank.' @('frm1604f:RegisteredAddress') "Please enter Taxpayer's Registered Address on Item 9." @('official-hta-runtime#validateForm:L3844-L3848') 'incorrect-official-behavior' 'The message cites Item 9; registered address is Item 7.' 'Report Item 7.'
Validate-Rule '010-zip' 'ZIP code is blank.' @('frm1604f:zipCode') "Please enter Taxpayer's Zip Code on Item 10." @('official-hta-runtime#validateForm:L3849-L3853') 'incorrect-official-behavior' 'The message cites Item 10; ZIP code is Item 7A.' 'Report Item 7A.'
Validate-Rule '011-category' 'Neither private nor government category is selected.' @('frm1604f:categoryAgent_1','frm1604f:categoryAgent_2') 'Please select an option for Item 8.' @('official-hta-runtime#validateForm:L3877-L3881')

$quarters = @('1st Quarter','2nd Quarter','3rd Quarter','4th Quarter')
$sourceForms = @('1601-FQ','1602-Q','1603-Q')
$lineRanges = @('L3888-L4019','L4020-L4130','L4131-L4241')
for ($schedule = 1; $schedule -le 3; $schedule++) {
    $sourceForm = $sourceForms[$schedule - 1]
    $lineRange = $lineRanges[$schedule - 1]
    for ($row = 1; $row -le 4; $row++) {
        $quarter = $quarters[$row - 1]
        $prefix = "frm1604f:txtSched$schedule"
        Validate-Rule "sched$schedule-row$row-date" "Schedule $schedule $quarter is started but Date of Remittance is blank." @("$prefix`Date$row","$prefix`TRA$row","$prefix`TaxWheld$row","$prefix`Pen$row") "Please enter the Date of Remittance for the $quarter. You may refer to your $sourceForm for the said quarter." @("official-hta-runtime#validateForm:$lineRange")
        if ($schedule -eq 1) {
            Validate-Rule "sched1-row$row-tra-placeholder" "Schedule 1 $quarter has a positive tax/penalty amount and TRA/eROR/eAR is blank." @("$prefix`TRA$row","$prefix`TaxWheld$row","$prefix`Pen$row") 'Please enter any of the following details TRA/eROR/eAR Number for the <month>. You may refer to your 1601-FQ for the said Quarter.' @('official-hta-runtime#validateForm:L3891-L3941') 'incorrect-official-behavior' 'The active message contains a literal <month> placeholder even though rows are quarters.' "Report the actual $quarter."
            Validate-Rule "sched1-row$row-tra-specific" "Schedule 1 $quarter has Date or TRA populated, no positive amount branch has already returned, and TRA is blank." @("$prefix`Date$row","$prefix`TRA$row") "Please enter the following details TRA/eROR/eAR Number for the $quarter. You may refer to your 1601-FQ for the said quarter." @('official-hta-runtime#validateForm:L3943-L3974')
        } else {
            Validate-Rule "sched$schedule-row$row-tra" "Schedule $schedule $quarter is started but TRA/eROR/eAR is blank." @("$prefix`Date$row","$prefix`TRA$row","$prefix`TaxWheld$row","$prefix`Pen$row") "Please enter the following details TRA/eROR/eAR Number for the $quarter. You may refer to your $sourceForm for the said quarter." @("official-hta-runtime#validateForm:$lineRange")
        }
        Validate-Rule "sched$schedule-row$row-tax" "Schedule $schedule $quarter has a date and Taxes Withheld compares equal to 0.00." @("$prefix`Date$row","$prefix`TaxWheld$row") "Please enter the Taxes Withheld for the $quarter. You may refer to your $sourceForm for the said quarter." @("official-hta-runtime#validateForm:$lineRange")
        Validate-Rule "sched$schedule-row$row-date-format" "Schedule $schedule $quarter Date of Remittance fails validateMonthDayYearDate." @("$prefix`Date$row") "Please enter the Date of Remittance for the $quarter. You may refer to your $sourceForm for the said quarter." @("official-hta-runtime#validateForm:$lineRange",'official-hta-runtime#validateMonthDayYearDate:L4340-L4403') 'official-bug-compatible' 'Malformed dates reuse the missing-date message.' 'Report the expected MM/DD/YYYY shape and invalid component.'
    }
}

Validate-Rule '060-success' 'All active checks return no failure.' @('return-body') 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validateForm:L4289-L4300')
Rule '1604f-schedule-empty-row' 'validate' $null 'All identifying and amount fields in a quarter row are blank or zero.' @('schedule-row-fields') $null @('official-hta-runtime#validateForm:L3883-L4287') 'verified-correct' 'A wholly empty row passes.' 'Keep unused schedule rows optional.'
Rule '1604f-schedule-penalty-optional' 'validate' $null 'A populated row has Penalties equal to 0.00.' @('schedule-penalty-fields') $null @('official-hta-runtime#validateForm:L3883-L4287') 'verified-correct' 'No rule requires a positive penalty; it is included only when present.' 'Keep penalties optional and typed as nonnegative decimal.'
Rule '1604f-date-month-zero' 'blur/change' $null 'A date uses month 00 with a two-digit valid day and four-digit year.' @('schedule-date-fields') $null @('official-hta-runtime#validateMonthDayYearDate:L4340-L4403') 'incorrect-official-behavior' 'The lower-bound comparison is month < 0, so 00 bypasses all month-day branches and is accepted.' 'Parse strictly and require month 01 through 12.'
Rule '1604f-date-numeric-check' 'blur/change' $null 'A date component is malformed but the first truthy component is numeric.' @('schedule-date-fields') $null @('official-hta-runtime#validateMonthDayYearDate:L4352-L4373') 'official-bug-compatible' 'isNaN(result[0] || result[1] || result[2]) tests only the first truthy component; later coercive comparisons catch some but not all malformed forms.' 'Parse each component independently without coercion.'
Rule '1604f-year-change-clear' 'blur/change' $null 'A valid changed year is confirmed.' @('frm1604f:txtYear','all-schedule-fields') 'Changing the return period will clear all entered computed data. Do you want to continue?' @('official-hta-runtime#yearMessage:L4415-L4430','official-hta-runtime#clearPartII:L4432-L4520') 'official-bug-compatible' 'Confirm clears all three schedules; cancel does not restore the previous year.' 'Use a reversible update and restore the prior period on cancel.'
Rule '1604f-tax-relief-toggle' 'blur/change' $null 'Item 11 is switched to No.' @('frm1604f:taxRelief_2','frm1604f:availedTaxRelief') $null @('official-hta-runtime#TaxReliefEnable:L3599-L3610') 'verified-correct' 'Item 11A is disabled and cleared.' 'Retain explicit conditional clearing.'
Rule '1604f-tax-relief-answer-omitted' 'validate' $null 'Neither Item 11 Yes nor No is selected.' @('frm1604f:taxRelief_1','frm1604f:taxRelief_2') $null @('official-hta-runtime#validateForm:L3789-L4300','official-help#item11:L142-L146') 'incorrect-official-behavior' 'Validate never requires an Item 11 choice; the static UI defaults No.' 'Require an explicit typed answer when no trusted default is established.'
Rule '1604f-top-agent-omitted' 'validate' $null 'Private category is selected but neither Item 8A option is selected.' @('frm1604f:categoryAgent_1','frm1604f:privateAgent_1','frm1604f:privateAgent_2') $null @('official-hta-runtime#validateForm:L3789-L4300','official-help#item8a:L134-L136') 'incorrect-official-behavior' 'Validate ignores Item 8A; the static UI defaults No.' 'Require Item 8A when private category makes it applicable.'
Rule '1604f-email-unvalidated' 'validate' $null 'A nonblank malformed taxpayer email is present.' @('frm1604f:email') $null @('official-hta-runtime#validateForm:L3789-L4300','official-help#item10:L139-L141') 'incorrect-official-behavior' 'Email is optional and never format-checked.' 'Allow blank; validate format when nonblank.'
Rule '1604f-line-business-commented' 'validate' $null 'Line of business is blank.' @('frm1604f:description') 'Please enter a valid Line of Business on Item 6.' @('official-hta-runtime#validateForm:L3829-L3833') 'obsolete' 'The entire branch is commented out and the field is not serialized in the reviewed save.' 'Do not present it as an active rule.'
Rule '1604f-refund-fields-commented' 'validate' $null 'Legacy refund fields are inspected.' @('frm1604f:txtRefMonth','frm1604f:txtRefDate','frm1604f:txtRefYear','frm1604f:select13') $null @('official-hta-runtime#validateForm:L3854-L3876','official-hta-runtime#changeRefund:L3574-L3597') 'obsolete' 'The validation and enable/disable code are commented out; these fields are absent from the reviewed save.' 'Do not model them as active revision behavior.'
Rule '1604f-unreachable-schedule-fallback' 'validate' $null 'Schedule loop reaches an x value other than 1, 2, or 3.' @('schedule-row-fields') 'Please enter the Date of Remittance for <month>. You may refer to your 1603-Q for the said quarter.' @('official-hta-runtime#validateForm:L4242-L4280') 'obsolete' 'The outer loop only uses x=1..3, so the final else and its literal placeholder messages are unreachable.' 'Exclude unreachable fallback text from user-facing validation.'
Rule '1604f-final-001' 'final-copy' 1 'Final Copy is requested after validation.' @('txtFinalFlag','return-body') $null @('official-hta-runtime#saveEncryptedProfile:L2760-L2850','encrypted-field-audit-v796') 'verified-correct' 'The reviewed encrypted artifact decrypts to exactly the same 105-key inventory as the plaintext save.' 'Preserve all 105 fields losslessly and keep finalization distinct from transport.'
Rule '1604f-submit-001' 'submit' 1 'Online transport is invoked.' @('return-body') $null @('official-hta-runtime#saveXMLsubmit:L3269-L3452','official-hta-runtime#sendEmail:L5332-L5446') 'unverified' 'Transport exists but was not exercised.' 'Keep local validation and finalization independently testable.' 'medium'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'; schema_version = '1.0.0'; form_id = $formId; revision = $revision
    first_error_behavior = 'Validate calls checkYear first, but that helper only alerts and does not stop. Later branches return on the first active failure. Schedule order is 1 then 2 then 3, quarter 1 through 4, with date/TRA/tax/date-format checks. Save checks only TIN, RDO, and name.'
    rules = $rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Calc([string]$Id, [string[]]$Outputs, [string[]]$Inputs, [string]$Formula, [string]$Trigger, [string[]]$Depends, [string[]]$Refs, [string]$Assessment = 'verified-correct', [string]$Recommended = 'Use decimal arithmetic and recompute from authoritative inputs.') {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id = $Id; outputs = $Outputs; inputs = $Inputs; condition = $null
        official_formula = $Formula; rounding = 'Entry handlers use parseFloat(...).toFixed(2); totals use NumWithComma and formatCurrency.'
        trigger = $Trigger; depends_on = $Depends; source_refs = $Refs; assessment = $Assessment
        recommended_app_behavior = $Recommended; confidence = 'high'
    })
}
Calc '1604f-row-total-amount' @('frm1604f:txtSched1TotalAmt1..4','frm1604f:txtSched2TotalAmt1..4','frm1604f:txtSched3TotalAmt1..4') @('corresponding Taxes Withheld','corresponding Penalties') 'For each schedule/quarter, Total Amount Remitted = Taxes Withheld + Penalties.' 'computeTotalAmount(schedule,row)' @() @('official-hta-runtime#computeTotalAmount:L3735-L3746')
Calc '1604f-total-withheld' @('frm1604f:txtSched1TaxWheldTotal','frm1604f:txtSched2TaxWheldTotal','frm1604f:txtSched3TaxWheldTotal') @('four Taxes Withheld rows per schedule') 'Sum the four quarterly Taxes Withheld values for the selected schedule.' 'computeTotalWithheld(schedule)' @() @('official-hta-runtime#computeTotalWithheld:L3638-L3665')
Calc '1604f-total-penalties' @('frm1604f:txtSched1PenTotal','frm1604f:txtSched2PenTotal','frm1604f:txtSched3PenTotal') @('four Penalties rows per schedule') 'Sum the four quarterly Penalties values for the selected schedule.' 'computeTotalPenalties(schedule)' @() @('official-hta-runtime#computeTotalPenalties:L3667-L3697')
Calc '1604f-schedule-total' @('frm1604f:txtSched1Total','frm1604f:txtSched2Total','frm1604f:txtSched3Total') @('corresponding TaxWheldTotal','corresponding PenTotal') 'Schedule Total = total Taxes Withheld + total Penalties.' 'computeTotal(schedule)' @('1604f-total-withheld','1604f-total-penalties') @('official-hta-runtime#computeTotal:L3699-L3719')
Calc '1604f-adjustment-obsolete' @('commented adjustment total') @('commented adjustment rows') 'The entire adjustment computation is commented out.' 'none' @() @('official-hta-runtime#computeTotalAdjustment:L3721-L3733') 'obsolete' 'Do not implement as active January 2018 behavior.'
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
        @{ case_id = 'row-basic'; calculation_id = '1604f-row-total-amount'; inputs = @{ withheld = 100; penalties = 5 }; official_output = '105.00' },
        @{ case_id = 'four-quarter-withheld'; calculation_id = '1604f-total-withheld'; inputs = @{ rows = @(10,20,30,40) }; official_output = '100.00' },
        @{ case_id = 'schedule-total'; calculation_id = '1604f-schedule-total'; inputs = @{ withheld_total = 100; penalty_total = 5 }; official_output = '105.00' },
        @{ case_id = 'negative-normalized'; calculation_id = '1604f-row-total-amount'; inputs = @{ withheld_entry = -1 }; official_entry_after_blur = '0.00' }
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
        @{ phase = 'edit'; official_behavior = 'January 2018 annual final-withholding return with three fixed four-quarter schedules for 1601-FQ, 1602Q, and 1603Q remittances.'; source_refs = @('official-hta-runtime#frmMain','official-help#revision:L84'); confidence = 'high' },
        @{ phase = 'saved-draft'; official_behavior = 'Save checks only TIN components, RDO not 000, and withholding-agent name, then writes 105 flat keys.'; source_refs = @('official-hta-runtime#initialValidateBeforeSave:L4711-L4727','xml-editable-v1'); confidence = 'high' },
        @{ phase = 'validated'; official_behavior = 'Validate alerts for invalid year without stopping, then runs source-ordered background, tax-relief, and three-schedule checks; success disables controls.'; source_refs = @('official-hta-runtime#validateForm:L3789-L4300'); confidence = 'high' },
        @{ phase = 'final-copy'; official_behavior = 'The reviewed encrypted companion has the identical 105-key inventory as the plaintext save.'; source_refs = @('encrypted-field-audit-v796'); confidence = 'high' },
        @{ phase = 'submitted'; official_behavior = 'Online transport exists but was not exercised.'; source_refs = @('official-hta-runtime#sendEmail:L5332-L5446'); confidence = 'medium' }
    )
    transitions = @(
        @{ from = 'edit'; action = 'Save'; to = 'saved-draft'; guard = 'Three narrow checks pass.'; side_effects = @('Writes plaintext pseudo-XML.'); source_refs = @('official-hta-runtime#initialValidateBeforeSave:L4711-L4727') },
        @{ from = 'edit'; action = 'Validate'; to = 'validated'; guard = 'All blocking checks return no failure; year check itself is non-blocking.'; side_effects = @('Disables applicable controls.','Enables Print, Edit, Upload, and Final Copy.'); source_refs = @('official-hta-runtime#validateForm:L3789-L4300') },
        @{ from = 'validated'; action = 'Edit'; to = 'edit'; guard = $null; side_effects = @('Re-enables applicable controls.'); source_refs = @('official-hta-runtime#editForm:L4568-L4590') },
        @{ from = 'validated'; action = 'Final Copy'; to = 'final-copy'; guard = 'Official profile/finalization flow permits progress.'; side_effects = @('Creates encrypted artifact with the same 105-key inventory.'); source_refs = @('encrypted-field-audit-v796') },
        @{ from = 'final-copy'; action = 'Transport'; to = 'submitted'; guard = 'Connectivity and send succeed.'; side_effects = @('Attempts online submission; untested.'); source_refs = @('official-hta-runtime#sendEmail:L5332-L5446') }
    )
    prerequisites = @('January 2018 revision','Taxable year 2018 or later','Withholding-agent identity','Applicable quarterly remittance information')
    required_attachments = @(
        @{ attachment_id = 'alphalist-final-withholding'; label = 'Alphalist of Payees Subjected to Final Withholding Tax with electronic-submission acknowledgement/validation proof.'; required_when = 'Applicable.'; official_ui_enforcement = 'External attachment presence is not checked locally.'; source_refs = @('official-help#attachments:L174-L186'); confidence = 'high' },
        @{ attachment_id = 'alphalist-fringe-benefits'; label = 'Alphalist of Employees Other than Rank & File Who Were Given Fringe Benefits During the year with electronic-submission acknowledgement/validation proof.'; required_when = 'Applicable.'; official_ui_enforcement = 'External attachment presence is not checked locally.'; source_refs = @('official-help#attachments:L174-L186'); confidence = 'high' },
        @{ attachment_id = 'alphalist-exempt'; label = 'Alphalist of Other Payees Whose Income are Exempt from Withholding Tax but Subject to Income Tax with electronic-submission acknowledgement/validation proof.'; required_when = 'Applicable.'; official_ui_enforcement = 'External attachment presence is not checked locally.'; source_refs = @('official-help#attachments:L174-L186'); confidence = 'high' },
        @{ attachment_id = 'authorization-letter'; label = 'Authorization letter.'; required_when = 'Filed by an authorized representative.'; official_ui_enforcement = 'Not locally checked.'; source_refs = @('official-help#attachments:L174-L186'); confidence = 'high' }
    )
    filing_deadlines = @(
        @{ quarter = 'Q1'; due_date_rule = 'Annual return due on or before January 31 of the year following the calendar year in which final-withholding income payments were paid or accrued.'; source_refs = @('official-help#deadline:L103-L105'); confidence = 'high' },
        @{ quarter = 'Q2'; due_date_rule = 'Not quarterly; the annual January 31 deadline applies.'; source_refs = @('official-help#deadline:L103-L105'); confidence = 'high' },
        @{ quarter = 'Q3'; due_date_rule = 'Not quarterly; the annual January 31 deadline applies.'; source_refs = @('official-help#deadline:L103-L105'); confidence = 'high' },
        @{ quarter = 'Q4'; due_date_rule = 'Not quarterly; the annual January 31 deadline applies.'; source_refs = @('official-help#deadline:L103-L105'); confidence = 'high' }
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$encryptedAsset = Asset 'xml-encrypted-v1' 'dummy-profile-encrypted-copy' $encryptedPath 'Reviewed encrypted companion; decrypted 105-key shape matches plaintext; values excluded.'
$encryptedAsset.path = Join-Path $SourceDir '00000000000000-1604F-2025#email-redacted#.xml'
$assets = @(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1604F and printed January 2018.'
    Asset 'official-help' 'official-runtime-help' $helpPath 'Revision-matched January 2018 instructions.'
    Asset 'xml-editable-v1' 'dummy-profile-editable-save' $plainPath 'Reviewed 105-key target-revision save; values excluded.'
    $encryptedAsset
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 Form 1604-F.'
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'; schema_version = '1.0.0'; form_id = $formId
    form_code = '1604F'; revision = $revision; revision_label = 'January 2018'; package_version = $packageVersion; status = 'complete'
    official_assets = $assets
    counts = [ordered]@{
        concrete_fields = 105; runtime_field_families = 0; fields_total = $fields.Count; typed_fields = $fields.Count
        validation_rules = $rules.Count; confirmed_official_bugs = $bugCount; calculations = $calculations.Count
        negative_fixtures = $cases.Count; unverified_gaps = 2
    }
    artifacts = [ordered]@{
        fields = 'fields.json'; validations = 'validations.json'; calculations = 'calculations.json'; workflow = 'workflow.json'
        evidence = 'evidence.md'; audit = 'audit.md'; gaps = 'gaps.md'
        runtime_control_fixture = 'fixtures/runtime-control-inventory-v796.json'
        encrypted_field_audit = 'fixtures/encrypted-field-audit-v796.json'
        validation_function_fixture = 'fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture = 'fixtures/calculation-function-inventory-v796.json'
        resource_hash_fixture = 'fixtures/official-resource-hashes-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'; calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer or release changes.',
        'No source values or email-bearing filenames are copied.',
        'Plaintext and decrypted encrypted saves contain the same 105-key inventory; there are no active runtime field families.',
        'Seven static controls absent from the reviewed save are retained in the control fixture and classified where relevant.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1604-F - January 2018`n`nRevision-specific Offline eBIRForms rule package with 105 concrete serialized keys and no active dynamic field families. Source values and email-bearing filenames are excluded.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') "# Evidence`n`n- Exact HTA SHA-256: $($expected.hta); APPLICATIONNAME 1604F, January 2018.`n- Revision-matched help SHA-256: $($expected.help), including scope, January 31 deadline, item instructions, and required attachments.`n- Plaintext save SHA-256: $($expected.plain); 105 unique keys; inventory SHA-256 $($expected.inventory).`n- Encrypted ciphertext SHA-256: $($expected.encrypted); in-memory decrypted SHA-256 $($expected.decrypted); same 105-key inventory; no values emitted.`n- Official PDF SHA-256: $($expected.pdf), valid PDF magic.`n- Runtime inventory: 147 controls, 125 serializer candidates, 112 unique static IDs, 74 inline functions, and no active Add-more/modal field families.`n"
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. Online submission was not exercised.`n2. External attachment presence and transport were not exercised; the local UI does not verify them.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- Revision/assets: **pass** - January 2018 HTA, help, PDF, plaintext save, encrypted replay, and package executable are pinned.`n- Fields: **pass** - matching 105-key plaintext/encrypted inventories; all seven static-only controls remain visible in the runtime fixture; no active dynamic families.`n- Rules/calculations/workflow: **pass** - source order, exact schedule messages, phase differences, five calculation records, January 31 deadline, and attachments captured.`n- Official defects: **pass** - $bugCount bug-compatible/incorrect/obsolete rules separated from recommendations, including non-blocking year validation, wrong item references, literal placeholders, month 00 acceptance, and commented legacy fields.`n- Privacy: **pass** - no values or email-bearing filenames copied.`n- Online submission and attachment transport: **unverified** and explicit gaps.`n"
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 16: 1604f-v2018. Next: 1702MX.`n"

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -LiteralPath $indexPath -Raw | ConvertFrom-Json
$entry = [pscustomobject][ordered]@{
    form_id = $formId; form_code = '1604F'; revision = $revision; package_version = $packageVersion
    priority = 16; status = 'complete'; path = 'forms/1604f-v2018/manifest.json'
}
$index.forms = @(@($index.forms | Where-Object { $_.form_id -ne $formId }) + $entry | Sort-Object priority)
$index.updated = (Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index

"Generated ${formId}: fields=$($fields.Count), rules=$($rules.Count), calculations=$($calculations.Count), negative_cases=$($cases.Count), bug_classifications=$bugCount, static_unique_ids=$($staticIds.Count), plain_only=$($plainOnly.Count), static_only=$($staticOnly.Count)"
