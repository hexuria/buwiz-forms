param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$SourceDir = 'C:\Mac\Home\Downloads\forms\1701v2018'
)

$ErrorActionPreference = 'Stop'
$formId = '1701-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1701v2018.hta'
$plainPath = Join-Path $SourceDir '00000000000000-1701v2018-122025.xml'
$encryptedCandidates = @(Get-ChildItem -LiteralPath $SourceDir -File | Where-Object { $_.Name -like '00000000000000-1701v2018-122025#*#.xml' })
if ($encryptedCandidates.Count -ne 1) { throw "Expected one reviewed encrypted companion; found $($encryptedCandidates.Count)." }
$encryptedPath = $encryptedCandidates[0].FullName
$officialPdf = Join-Path $SourceDir '1701 Jan 2018 final with rates.pdf'
$attachmentPdf = Join-Path $SourceDir '1701 Attachment Jan 2018 ENCSv3.pdf'
$consolidatedPdf = Join-Path $SourceDir '1701 January 2018 Consov4.pdf'
$typedModel = Join-Path $RepoRoot 'crates\bir-core\src\forms\form_1701.rs'
$typedXml = Join-Path $RepoRoot 'crates\bir-core\src\forms\form_1701_xml.rs'
$outDir = Join-Path $RepoRoot 'rules\forms\1701-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'

foreach ($path in @($htaPath, $plainPath, $encryptedPath, $officialPdf, $attachmentPdf, $consolidatedPdf, $typedModel, $typedXml)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing required source: $path" }
}
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 50) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function Get-Attr([string]$Tag, [string]$Name) {
    $m = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($m.Success) { $m.Groups[2].Value } else { $null }
}
function Get-HashText([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))
        ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
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

$hta = [IO.File]::ReadAllText($htaPath)
$htaLines = Get-Content -LiteralPath $htaPath
$plainText = [IO.File]::ReadAllText($plainPath)
$saveMatches = @([regex]::Matches($plainText, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>'))
$observedKeys = @($saveMatches | ForEach-Object { $_.Groups['key'].Value })
if ($observedKeys.Count -ne 837 -or ($observedKeys | Sort-Object -Unique).Count -ne 837) {
    throw "Expected 837 unique reviewed plaintext keys; found $($observedKeys.Count)."
}
if ((Get-FileHash -LiteralPath $plainPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne 'b168c7b3273d30a10f28f4653847519b876d5a88e77ed82911718a80f65c7827') { throw 'Plain save hash changed.' }
if ((Get-FileHash -LiteralPath $encryptedPath -Algorithm SHA256).Hash.ToLowerInvariant() -ne '3771c99c191ef5e84b1b5e4c51499911bfbec6002febc3c53dca3f08730e92e3') { throw 'Encrypted save hash changed.' }
if ($plainText -notmatch '<div>frm1701:txtVersion=051414frm1701:txtVersion=</div>') { throw 'Reviewed save version marker changed.' }

$formMatch = [regex]::Match($hta, '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain form not found.' }
$formBody = $formMatch.Groups['body'].Value
$formOffset = $formMatch.Groups['body'].Index
$scriptRanges = @([regex]::Matches($formBody, '(?is)<script\b.*?</script>'))
$controls = @()
$ordinal = 0
foreach ($m in [regex]::Matches($formBody, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $insideScript = $false
    foreach ($range in $scriptRanges) {
        if ($m.Index -ge $range.Index -and $m.Index -lt ($range.Index + $range.Length)) { $insideScript = $true; break }
    }
    if ($insideScript) { continue }
    $ordinal++
    $tag = $m.Value
    $element = $m.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $controls += [pscustomobject][ordered]@{
        ordinal = $ordinal; id = Get-Attr $tag 'id'; name = Get-Attr $tag 'name'
        element = $element; control_kind = $kind.ToLowerInvariant()
        source_line = 1 + [regex]::Matches($hta.Substring(0, $formOffset + $m.Index), "`n").Count
        value = Get-Attr $tag 'value'; maxlength = Get-Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
}
$serializableControls = @($controls | Where-Object { $_.control_kind -in @('text', 'select', 'select-one', 'radio', 'checkbox') })
if ($controls.Count -ne 887 -or $serializableControls.Count -ne 837) {
    throw "Expected 887 form controls and 837 static serializer candidates; found $($controls.Count)/$($serializableControls.Count)."
}
$controlById = @{}
foreach ($control in $controls) { if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control } }

$requiredKeys = @(
    'frm1701:txtPg1I1Year','frm1701:txtPg1I4TIN1','frm1701:txtPg1I4TIN2','frm1701:txtPg1I4TIN3','frm1701:txtPg1I4BranchCode',
    'frm1701:txtPg1I5RDOCode','frm1701:txtPg1I8TaxpayerName','frm1701:txtPg1I10BirthDate','frm1701:txtPg1I12Citizenship'
)
$computedPattern = '(?i)(TaxDue|TaxPayable|Total|Aggregate|NetIncome|TaxableIncome|GrossIncome|GrossSales|NetSales|NetTaxable|IncomeTax|AllowableDeduc|AmountPayable|txtPg1I2[246]|txtPg1I3[012]|txtPg2I(6|10|12|16|17|22|23|24|25)|txtPg[234]IShed.*_(3|6|7|8|9|10|11|12|13|18|23|24|25|28|30|31|32)[AB]?$)'

function Field-Meta([string]$Key, $Control, [bool]$Family) {
    $page = $null; $item = $null; $logical = 'string'; $required = 'optional'; $enum = [object[]]@(); $normalization = [string[]]@(); $computed = $false; $calc = $null
    if ($Key -match 'Pg(?<p>[1-9])(?<m>m)?') { $page = [int]$Matches.p }
    if ($Key -match '(?:^|[:_])I(?<i>\d+[A-Za-z]?)') { $item = $Matches.i }
    elseif ($Key -match '_(?<i>\d+[A-Za-z]?)') { $item = $Matches.i }
    if ($Key -match '(?i)(rdo|checkbox|CheckBox|Overpayment)') { $logical = 'boolean'; $enum = [object[]]@('true','false') }
    elseif ($Key -match '(?i)(TIN|BranchCode|RDOCode|ATC_)') { $logical = 'code' }
    elseif ($Key -match '(?i)(Email)') { $logical = 'email-string' }
    elseif ($Key -match '(?i)(TelNum|Contact)') { $logical = 'phone-string' }
    elseif ($Key -match '(?i)(BirthDate|DateOfTaxRelief|DateIssued|Date$)') { $logical = 'date-string'; $normalization = [string[]]@('MM/DD/YYYY where validateDate is attached') }
    elseif ($Key -match '(?i)(Month$|I1Month)') { $logical = 'month' }
    elseif ($Key -match '(?i)(Year|YearIncurred)') { $logical = 'integer' }
    elseif ($Key -match '(?i)(Amt|Amount|Income|Sales|Revenue|Receipts|Deduct|Tax|Loss|Cost|Payments|Credits|Penalt|Surcharge|Interest|Compromise|Expense|Relief|Capital|Inventory|Purchases|Depreciation|Salary|Rent|Contribution|Advertising|Insurance|Utilities|Supplies|Representation|Research|Royal|Management|Professional|Miscellaneous|Rate)') {
        $logical = 'decimal-amount'; $normalization = [string[]]@('NumWithComma parsing', 'formatCurrency formatting where invoked')
    }
    if ($requiredKeys -contains $Key) { $required = 'required' }
    if ($Key -match $computedPattern) { $computed = $true; $required = 'computed'; $calc = 'See calculations.json' }
    if ($Key -match '^(txtFinalFlag|txtEnroll|ebirOnline|driveSelectTPExport|attachmentCurrent|attachmentTotal)$') { $required = 'hidden' }
    if ($Family) { $required = 'conditional' }
    $constraints = [ordered]@{}
    if ($Control -and $Control.maxlength) { $constraints.max_length = [int]$Control.maxlength }
    if ($logical -eq 'date-string') { $constraints.format = 'MM/DD/YYYY in official date validators; some attachment dates are only length-tested' }
    if ($Family) { $constraints.index = 'Attachment type EX or SP followed by N >= 1; no maximum is enforced in addAttachment().' }
    [pscustomobject]@{ page=$page; item=$item; logical=$logical; required=$required; enum=$enum; normalization=$normalization; computed=$computed; calc=$calc; constraints=[pscustomobject]$constraints }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $observedKeys) {
    $control = if ($controlById.ContainsKey($key)) { $controlById[$key] } else { $null }
    $meta = Field-Meta $key $control $false
    $refs = @("xml-editable-v1#field:$key")
    if ($control) { $refs += "official-hta-runtime#control:L$($control.source_line)" }
    elseif ($key -match 'RDOCode') { $refs += 'official-hta-runtime#loadRdo:L8497-L8528' }
    else { $refs += 'official-hta-runtime#saveXML:L8095-L8221' }
    $fields.Add([pscustomobject][ordered]@{
        field_key=$key; serialized_key=$key; serialized_occurrence=1; label=$key; page=$meta.page; item_number=$meta.item
        control_kind=if($control){$control.control_kind}else{'runtime-injected-control'}; storage_type='string'; logical_type=$meta.logical
        required=$meta.required; required_when=$null; enabled_when=$null; visible_when=$null; default_value=if($control){$control.value}else{$null}
        empty_representation=''; constraints=$meta.constraints; enum_values=$meta.enum; normalization=$meta.normalization
        computed=$meta.computed; calculation_id=$meta.calc; source_refs=$refs; confidence=if($control){'high'}else{'medium'}
        notes=@('Observed in the reviewed 837-key plaintext save; the source value is intentionally excluded.')
    })
}
$extraKey = 'frm1701:txtPg1I9Address2'
$extraControl = $controlById[$extraKey]
$extraMeta = Field-Meta $extraKey $extraControl $false
$fields.Add([pscustomobject][ordered]@{
    field_key=$extraKey; serialized_key=$extraKey; serialized_occurrence=1; label='Registered address line 2'; page=1; item_number='9'
    control_kind='text'; storage_type='string'; logical_type='string'; required='optional'; required_when=$null; enabled_when=$null; visible_when=$null
    default_value=$extraControl.value; empty_representation=''; constraints=$extraMeta.constraints; enum_values=[object[]]@(); normalization=[string[]]@('escape() in encrypted read-only save branch')
    computed=$false; calculation_id=$null
    source_refs=@('xml-encrypted-v1#decrypted-field:frm1701:txtPg1I9Address2',"official-hta-runtime#control:L$($extraControl.source_line)",'typed-form-1701#REVIEWED_ENCRYPTED_XML_EXTRA_FIELD')
    confidence='high'; notes=@('The encrypted reviewed companion has 838 fields and adds this key to the 837-field plaintext snapshot. The value is intentionally excluded.')
})

$templateKeys = @($observedKeys | Where-Object { $_ -match 'TYPE$' })
if ($templateKeys.Count -ne 115) { throw "Expected 115 attachment template keys; found $($templateKeys.Count)." }
foreach ($templateKey in $templateKeys) {
    $familyKey = $templateKey.Substring(0, $templateKey.Length - 4) + '{EX|SP}{N>=1}'
    $control = if ($controlById.ContainsKey($templateKey)) { $controlById[$templateKey] } else { $null }
    $meta = Field-Meta $familyKey $control $true
    $fields.Add([pscustomobject][ordered]@{
        field_key=$familyKey; serialized_key=$null; serialized_occurrence=$null; label="Runtime attachment family cloned from $templateKey"
        page=$meta.page; item_number=$meta.item; control_kind='runtime-indexed-family'; storage_type='string'; logical_type=$meta.logical
        required='conditional'; required_when='An exempt (EXn) or special-rate (SPn) attachment instance exists.'
        enabled_when='The attachment section enables the corresponding taxpayer/spouse cell.'; visible_when='The matching attachment is selected.'
        default_value=$null; empty_representation=''; constraints=$meta.constraints; enum_values=$meta.enum; normalization=$meta.normalization
        computed=$meta.computed; calculation_id=$meta.calc
        source_refs=@('official-hta-runtime#addAttachment:L13594-L13668',"official-hta-runtime#template-control:L$($control.source_line)")
        confidence='high'; notes=@('addAttachment replaces every literal TYPE token with EXn or SPn and appends the clone to frmMain, so saveXML serializes the resulting field.')
    })
}
if ($fields.Count -ne 953 -or ($fields.field_key | Sort-Object -Unique).Count -ne 953) { throw "Expected 953 unique concrete/family fields; found $($fields.Count)." }

Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema'='../../schema/fields.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    field_count=$fields.Count; runtime_serializable_element_count=837; inventory_sha256=Get-HashText @($fields.field_key | Sort-Object); fields=$fields
})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0'; form_id=$formId; application_name='1701v2018'; hta_application_version='1.0'
    official_hta_sha256=(Get-FileHash -LiteralPath $htaPath -Algorithm SHA256).Hash.ToLowerInvariant()
    form_control_count=$controls.Count; static_serializer_candidate_count=$serializableControls.Count
    reviewed_plaintext_key_count=$observedKeys.Count; reviewed_encrypted_key_count=838; attachment_template_family_count=$templateKeys.Count
    serializer_set_differences=[ordered]@{ runtime_injected_observed=@('frm1701:txtPg1I5RDOCode','frm1701:txtPg2I2SpouseRDOCode'); static_not_in_plain_snapshot=@('frm1701:txtPg1I9Address2','frm1701:txtTelNum') }
    controls=$controls; attachment_template_keys=$templateKeys
})
$encryptedAuditTool = Join-Path $RepoRoot 'rules\tools\audit-1701-encrypted-fields.ps1'
$encryptedAudit = (& $encryptedAuditTool -SourceDir $SourceDir) -join [Environment]::NewLine
Write-Utf8 (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') $encryptedAudit

$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
$validationInventory = (& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1701:' -NamePattern '(?i)valid|check|compare|rate|filer|overpayment|attachment') -join [Environment]::NewLine
$calculationInventory = (& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1701:' -NamePattern '(?i)compute|calc|calculate|getSum|getProduct|getDifference|differenceOfElements|populateTotal|allFormat') -join [Environment]::NewLine
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') $validationInventory
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') $calculationInventory

$rules = [Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$FieldKeys,[string]$Accepted,[string]$Rejected,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The first active failing branch alerts and returns.',[string]$Recommended='Retain as a structured field error.',[string]$Confidence='high',[string[]]$Questions=@()) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id; form_id=$formId; revision=$revision; phase=$Phase; order=$Order; condition=$Condition; fields=$FieldKeys
        accepted_behavior=$Accepted; rejected_behavior=$Rejected; exact_message=$Message; source_refs=$Refs; evidence_type=@('source')
        assessment=$Assessment; official_behavior=$Official; recommended_app_behavior=$Recommended; confidence=$Confidence; unresolved_questions=$Questions
    })
}

Rule '1701-save-001' 'save' 1 'Any taxpayer TIN segment or branch code is blank.' @('frm1701:txtPg1I4TIN1','frm1701:txtPg1I4TIN2','frm1701:txtPg1I4TIN3','frm1701:txtPg1I4BranchCode') 'All four strings are nonblank.' 'Save is blocked.' 'Please enter a valid TIN number on Item 4.' @('official-hta-runtime#initialValidateBeforeSave:L7918-L7922') 'incorrect-official-behavior' 'Only nonblankness is checked during Save.' 'Allow lossless draft save, but run shape/checksum validation before finalization.'
Rule '1701-save-002' 'save' 2 'Taxpayer RDO select value equals literal 000.' @('frm1701:txtPg1I5RDOCode') 'Any value other than 000, including blank, passes this branch.' 'Save is blocked only for 000.' 'Please enter a valid RDO Code on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L7923-L7926') 'official-bug-compatible' 'Blank RDO can pass because the check is equality to 000 rather than required/non-placeholder validation.' 'Require a valid catalog RDO code before finalization.'
Rule '1701-save-003' 'save' 3 'Taxpayer name is blank.' @('frm1701:txtPg1I8TaxpayerName') 'Nonblank name passes.' 'Save is blocked.' 'Please enter a valid Taxpayer Name on Item 8.' @('official-hta-runtime#initialValidateBeforeSave:L7927-L7931')
Rule '1701-save-004' 'save' 4 'Any other return field is missing, malformed, contradictory, or incomplete.' @('return-body') 'Save proceeds after the three narrow preflight checks.' 'No rejection occurs.' $null @('official-hta-runtime#initialValidateBeforeSave:L7918-L7932','official-hta-runtime#saveXML:L8095-L8221') 'official-bug-compatible' 'Save is deliberately much weaker than Validate.' 'Preserve incomplete drafts losslessly and report completeness separately.'

$order = 0
function V([string]$Suffix,[string]$Condition,[string[]]$FieldKeys,$Message,[string]$Lines,[string]$Assessment='verified-correct',[string]$Official='The branch alerts, navigates where coded, and stops validation.',[string]$Recommended='Retain the rule with revision-aware wording.') {
    $script:order++
    Rule "1701-validate-$Suffix" 'validate' $script:order $Condition $FieldKeys 'The condition is false and ordered validation continues.' 'Validation stops at this branch.' $Message @("official-hta-runtime#validate:L$Lines") $Assessment $Official $Recommended
}
V '001' 'Short-period return is Yes and month value loosely equals 00.' @('frm1701:rdoPg1I3ShortPeriodYes','frm1701:txtPg1I1Month') 'Please choose a month on page 1 Item 1.' '9135-L9142'
V '002' 'Return year is blank.' @('frm1701:txtPg1I1Year') 'Please enter a valid year on page 1 Item 1.' '9143-L9149'
V '003' 'Return year is greater than the current system year.' @('frm1701:txtPg1I1Year') 'Invalid date entry on page 1 Item 1. Entry should not be later than Current Date.' '9150-L9155'
V '004' 'Return year is below 1900.' @('frm1701:txtPg1I1Year') 'Invalid date entry on page 1 Item 1. Entry should not be lower than 1900.' '9156-L9161' 'obsolete' 'This branch is shadowed for years below 2018 by the later revision check only when reached; it still emits its own message first.' 'Validate the revision boundary directly.'
V '005' 'Return year is below 2018.' @('frm1701:txtPg1I1Year') 'Please file using the old version of the form.' '9162-L9167'
V '006' 'No taxpayer type radio is selected.' @('frm1701:rdoPg1I6TaxpayerTypeS','frm1701:rdoPg1I6TaxpayerTypeP','frm1701:rdoPg1I6TaxpayerTypeE','frm1701:rdoPg1I6TaxpayerTypeT','frm1701:rdoPg1I6TaxpayerTypeC') 'Please select an option for page 1 Item 6.' '9168-L9173'
V '007' 'No taxpayer ATC II011 through II017 is selected.' @('frm1701:rdoPg1I7ATC_II011','frm1701:rdoPg1I7ATC_II012','frm1701:rdoPg1I7ATC_II013','frm1701:rdoPg1I7ATC_II014','frm1701:rdoPg1I7ATC_II015','frm1701:rdoPg1I7ATC_II016','frm1701:rdoPg1I7ATC_II017') 'Please select an option for page 1 Item 7.' '9174-L9179'
V '008' 'Birth date is nonblank and validateMonthDayYearDate returns true.' @('frm1701:txtPg1I10BirthDate') 'Invalid birth date on page 1 item 10.  Please check date format.' '9180-L9186' 'official-bug-compatible' 'The helper permits month 00 and uses a defective isNaN expression.' 'Use strict calendar-date parsing and retain the exact official message only for compatibility.'
V '009' 'Birth date is blank.' @('frm1701:txtPg1I10BirthDate') 'Please indicate birth date on page 1 item 10.' '9187-L9192'
V '010' 'Parsed birth year is later than current system year.' @('frm1701:txtPg1I10BirthDate') 'Birth year on page 1 Item 10 should not be later than current year.' '9193-L9196' 'official-bug-compatible' 'Only the year component is compared; future dates within the current year pass.' 'Reject any birth date later than today.'
V '011' 'Citizenship is blank.' @('frm1701:txtPg1I12Citizenship') 'Please fill up citizenship on page 1 Item 12.' '9197-L9202'
V '012' 'Foreign-tax-credit Yes is selected and foreign tax number is blank.' @('frm1701:rdoPg1I13ForeignTaxCreditsYes','frm1701:txtPg1I14ForeignTaxNumber') 'Please fill up foreign tax number on page 1 Item 14.' '9203-L9210'
V '013' 'No civil status radio is selected.' @('frm1701:rdoPg1I16CivilStatusS','frm1701:rdoPg1I16CivilStatusM','frm1701:rdoPg1I16CivilStatusLS','frm1701:rdoPg1I16CivilStatusW') 'Please select an option for page 1 Item 16.' '9211-L9216'
V '014' 'Civil status is Married and neither spouse-income choice is selected.' @('frm1701:rdoPg1I16CivilStatusM','frm1701:rdoPg1I17SpouseIncomeYes','frm1701:rdoPg1I17SpouseIncomeNo') 'Please select an option for page 1 Item 17.' '9217-L9224'
V '015' 'Spouse-income Yes is selected and neither filing-status choice is selected.' @('frm1701:rdoPg1I17SpouseIncomeYes','frm1701:rdoPg1I18FilingStatusJ','frm1701:rdoPg1I18FilingStatusS') 'Please select an option for page 1 Item 18.' '9225-L9232'
V '016' 'Neither taxpayer exempt-income choice is selected.' @('frm1701:rdoPg1I19IncomeExemptYes','frm1701:rdoPg1I19IncomeExemptNo') 'Please select an option for page 1 Item 19.' '9233-L9238'
V '017' 'Neither taxpayer special-rate-income choice is selected.' @('frm1701:rdoPg1I20IncomeSpecialYes','frm1701:rdoPg1I20IncomeSpecialNo') 'Please select an option for page 1 Item 20.' '9239-L9244'
V '018' 'No taxpayer tax-rate choice is selected and taxpayer is not compensation-income-only.' @('frm1701:rdoPg1I21TaxRateG','frm1701:rdoPg1I21TaxRateP') 'Please select an option for page 1 Item 21.' '9245-L9251'
V '019' 'Taxpayer graduated rate is selected, taxpayer is not compensation-only, and neither deduction method is selected.' @('frm1701:rdoPg1I21TaxRateG','frm1701:rdoPg1I21AMethodDeductionI','frm1701:rdoPg1I21AMethodDeductionO') 'Please select an option for page 1 Item 21A.' '9252-L9259'
V '020' 'Taxpayer Item 25A exceeds 50% of taxpayer Item 22 tax due.' @('frm1701:txtPg1I25A','frm1701:txtPg1I22ATaxDue') 'Amount in page 1 Item 25A cannot be more than 50% of Item 22.' '9260-L9265'
V '021' 'Spouse Item 25B exceeds 50% of spouse Item 22 tax due.' @('frm1701:txtPg1I25B','frm1701:txtPg1I22BTaxDue') 'Amount in page 1 Item 25B cannot be more than 50% of Item 22.' '9266-L9271'
V '022' 'Any Schedule 5 item 1, 2, 4, or 5 has amount without description/legal basis, or description/legal basis without a nonzero amount.' @('frm1701:txtPg3IShed5_1Amt','frm1701:txtPg3IShed5_1Desc','frm1701:txtPg3IShed5_1Legal','frm1701:txtPg3IShed5_2Amt','frm1701:txtPg3IShed5_2Desc','frm1701:txtPg3IShed5_2Legal','frm1701:txtPg3IShed5_4Amt','frm1701:txtPg3IShed5_4Desc','frm1701:txtPg3IShed5_4Legal','frm1701:txtPg3IShed5_5Amt','frm1701:txtPg3IShed5_5Desc','frm1701:txtPg3IShed5_5Legal') 'Please complete details in Page 3 Schedule 5 Item #<i>.' '9273-L9278' 'official-bug-compatible' 'The helper compares formatted amount strings directly to numeric zero.' 'Parse amounts numerically and require description/legal basis together.'
V '023' 'Aggregate Item 32 is negative and no overpayment disposition is selected.' @('frm1701:txtPg1I32AggregateAmtPyble','frm1701:rdoPg1OverpaymentRefund','frm1701:rdoPg1OverpaymentTCC','frm1701:rdoPg1OverpaymentCarryOver') 'Please select an Overpayment option on Page 1 Part II.' '9280-L9288'
V '024' 'Cross-schedule taxable income comparison fails for taxpayer or spouse.' @('frm1701:txtPg4IPart9_11A','frm1701:txtPg4IPart9_11B','frm1701:txtPg2IShed3_23A','frm1701:txtPg2IShed3_23B','frm1701:txtPg3IShed3_30A','frm1701:txtPg3IShed3_30B') $null '9290-L9292' 'verified-correct' 'A helper emits one of four exact cross-schedule messages and returns true.' 'Retain four distinct field errors; see helper rules.'
V '025' 'Applicable taxpayer Part X Schedule A completeness helper returns true.' @('frm1701:rdoPg1I19IncomeExemptYes','frm1701:rdoPg1I20IncomeSpecialYes','frm1701:rdoPg1mOption1') $null '9294-L9303'
V '026' 'Applicable spouse Part X Schedule A completeness helper returns true.' @('frm1701:rdoPg2I10IncomeExemptYes','frm1701:rdoPg2I11IncomeSpecialYes','frm1701:rdoPg1mOption1') $null '9305-L9313'
V '027' 'Joint filing is selected and validateSpouseFields returns true.' @('frm1701:rdoPg1I18FilingStatusJ') $null '9315-L9321'
Rule '1701-validate-success' 'validate' 28 'All active ordered validation branches pass.' @('return-body') 'All controls are disabled and a success alert is shown.' 'No rejection occurs.' 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validate:L9323-L9325','official-hta-runtime#disableAllControl:L9330-L9351')

$spouseEntries = @(
    @('001','Any spouse TIN segment or branch code length is at most 2.',@('frm1701:txtPg2I1TIN1','frm1701:txtPg2I1TIN2','frm1701:txtPg2I1TIN3','frm1701:txtPg2I1BranchCode'),'You have entered an invalid TIN format for Spouse.','9041-L9045','verified-correct'),
    @('002','Spouse TIN checksum helper returns nonzero.',@('frm1701:txtPg2I1TIN1','frm1701:txtPg2I1TIN2','frm1701:txtPg2I1TIN3'),'dynamic: getChkTinErrDesc(tinChkCode) + " on Page 2 Part IV Item 1."','9047-L9051','verified-correct'),
    @('003','Spouse RDO select has selectedIndex 0.',@('frm1701:txtPg2I2SpouseRDOCode'),'Please enter a valid RDO Code on Page 2 Part IV Item 2.','9053-L9056','verified-correct'),
    @('004','No spouse filer type is selected.',@('frm1701:rdoPg2I3SpouseTypeS','frm1701:rdoPg2I3SpouseTypeP','frm1701:rdoPg2I3SpouseTypeC'),'Please select a Filer type for Spouse on Page 2 Part IV Item 3.','9058-L9063','verified-correct'),
    @('005','No spouse ATC II011 through II017 is selected.',@('frm1701:rdoPg2I4ATC_II011','frm1701:rdoPg2I4ATC_II012','frm1701:rdoPg2I4ATC_II013','frm1701:rdoPg2I4ATC_II014','frm1701:rdoPg2I4ATC_II015','frm1701:rdoPg2I4ATC_II016','frm1701:rdoPg2I4ATC_II017'),'Please select an ATC code for Spouse on Page 2 Part IV Item 4.','9065-L9074','verified-correct'),
    @('006','Spouse name is blank.',@('frm1701:txtPg2I5SpouseName'),'Please enter a name for Spouse on Page 2 Part IV Item 5.','9076-L9079','verified-correct'),
    @('007','Spouse contact number is blank.',@('frm1701:txtPg2I6TelNum'),'Please enter a contact number for Spouse on Page 2 Part IV Item 6.','9081-L9084','verified-correct'),
    @('008','Spouse contact number length is 1 through 5.',@('frm1701:txtPg2I6TelNum'),'Please enter a valid contact number for Spouse on Page 2 Part IV Item 6.','9085-L9088','incorrect-official-behavior'),
    @('009','Spouse citizenship is blank.',@('frm1701:txtPg2I7Citizenship'),'Please enter a citizenship for Spouse on Page 2 Part IV Item 7.','9090-L9093','verified-correct'),
    @('010','Neither spouse foreign-tax-credit choice is selected.',@('frm1701:rdoPg2I8ForeignTaxCreditsYes','frm1701:rdoPg2I8ForeignTaxCreditsNo'),'Please select a Foreign Tax Credit for Spouse on Page 2 Part IV Item 8.','9095-L9099','verified-correct'),
    @('011','Spouse foreign-tax-credit Yes is selected and foreign tax number is blank.',@('frm1701:rdoPg2I8ForeignTaxCreditsYes','frm1701:txtPg2I9ForeignTaxNumber'),'Please select a Foreign Tax Number for Spouse on Page 2 Part IV Item 9.','9101-L9105','verified-correct'),
    @('012','Neither spouse exempt-income choice is selected.',@('frm1701:rdoPg2I10IncomeExemptYes','frm1701:rdoPg2I10IncomeExemptNo'),'Please select an option for Spouse on Page 2 Part IV Item 10.','9107-L9111','verified-correct'),
    @('013','Neither spouse special-rate-income choice is selected.',@('frm1701:rdoPg2I11IncomeSpecialYes','frm1701:rdoPg2I11IncomeSpecialNo'),'Please select an option for Spouse on Page 2 Part IV Item 11.','9113-L9117','verified-correct'),
    @('014','No spouse tax rate is selected and spouse is not compensation-income-only.',@('frm1701:rdoPg2I12TaxRateG','frm1701:rdoPg2I12TaxRateP'),'Please select a Tax Rate option for Spouse on Page 2 Part IV Item 12.','9119-L9124','verified-correct'),
    @('015','Spouse graduated rate is selected, spouse is not compensation-only, and neither deduction method is selected.',@('frm1701:rdoPg2I12AMethodDeductionI','frm1701:rdoPg2I12AMethodDeductionO'),'Please select a Method of Deduction for Spouse on Page 2 Part IV Item 12A.','9126-L9132','verified-correct')
)
$spouseOrder = 0
foreach ($entry in $spouseEntries) {
    $spouseOrder++
    Rule "1701-spouse-$($entry[0])" 'validate' $spouseOrder $entry[1] $entry[2] 'Condition is false.' 'Joint-return validation stops.' $entry[3] @("official-hta-runtime#validateSpouseFields:L$($entry[4])") $entry[5] 'This helper runs only when joint filing is selected.' 'Validate the spouse field semantically and preserve official ordering.'
}

Rule '1701-cross-001' 'validate' $null 'Taxpayer uses graduated/itemized and Part IX Item 11A differs from Schedule 3.A Item 23A.' @('frm1701:txtPg4IPart9_11A','frm1701:txtPg2IShed3_23A') 'Values compare equal after NumWithComma.' 'Validation stops.' 'Page 4 Part IX Item 11 must be equal to Item 23 on Page 2 Part V Schedule 3.A (Taxpayer is under Graduated Rates)' @('official-hta-runtime#compareTaxableIncForTP:L8922-L8931')
Rule '1701-cross-002' 'validate' $null 'Taxpayer uses 8% rate and Part IX Item 11A differs from Schedule 3.B Item 30A.' @('frm1701:txtPg4IPart9_11A','frm1701:txtPg3IShed3_30A') 'Values compare equal after NumWithComma.' 'Validation stops.' 'Page 4 Part IX Item 11 must be equal to Item 30 on Page 3 Part V Schedule 3.B (Taxpayer is under 8% IT Rate)' @('official-hta-runtime#compareTaxableIncForTP:L8933-L8941')
Rule '1701-cross-003' 'validate' $null 'Spouse uses graduated/itemized and Part IX Item 11B differs from Schedule 3.A Item 23B.' @('frm1701:txtPg4IPart9_11B','frm1701:txtPg2IShed3_23B') 'Values compare equal after NumWithComma.' 'Validation stops.' 'Page 4 Part IX Item 11 must be equal to Item 23 on Page 2 Part V Schedule 3.A (Spouse is under Graduated Rates)' @('official-hta-runtime#compareTaxableIncForSP:L8947-L8956')
Rule '1701-cross-004' 'validate' $null 'Spouse uses 8% rate and Part IX Item 11B differs from Schedule 3.B Item 30B.' @('frm1701:txtPg4IPart9_11B','frm1701:txtPg3IShed3_30B') 'Values compare equal after NumWithComma.' 'Validation stops.' 'Page 4 Part IX Item 11 must be equal to Item 30 on Page 3 Part V Schedule 3.B (Spouse is under 8% IT Rate)' @('official-hta-runtime#compareTaxableIncForSP:L8958-L8966')
Rule '1701-part10-001' 'validate' $null 'Taxpayer exempt attachment option 1 applies and any required Schedule A item 1,2,3,5,6 is blank.' @('frm1701:rdoPg1I19IncomeExemptYes','frm1701:rdoPg1mOption1') 'All required cells are nonblank.' 'First missing numbered cell stops validation.' "dynamic: Please populate Taxpayer's Part 10 Schedule A Item <x> Exempt." @('official-hta-runtime#checkPart10TPSchedA:L8972-L8983')
Rule '1701-part10-002' 'validate' $null 'Taxpayer special attachment option 1 applies and any Schedule A item 1 through 6 is blank.' @('frm1701:rdoPg1I20IncomeSpecialYes','frm1701:rdoPg1mOption1') 'All six cells are nonblank.' 'First missing numbered cell stops validation.' "dynamic: Please populate Taxpayer's Part 10 Schedule A Item <x> Special." @('official-hta-runtime#checkPart10TPSchedA:L8985-L8993')
Rule '1701-part10-003' 'validate' $null 'Taxpayer regular Schedule A items 1,2,3 are all nonblank and either date string length is at most 9.' @('frm1701:txtPg1mI1CSchdA','frm1701:txtPg1mI2CSchdA','frm1701:txtPg1mI3CSchdA','frm1701:txtPg1mI5CSchdA','frm1701:txtPg1mI6CSchdA') 'Both date strings have at least 10 characters.' 'Validation stops.' "Please complete Date of Tax Relief in Taxpayer's Part 10 Schedule A Regular Fields since you have an input in IPA/Legal Basis/Registered Activity." @('official-hta-runtime#checkPart10TPSchedA:L8995-L9002') 'official-bug-compatible' 'Only length is checked; invalid dates of length 10 pass.' 'Parse both dates strictly.'
Rule '1701-part10-004' 'validate' $null 'Spouse exempt-income Yes applies and any required Schedule A item 1,2,3,5,6 is blank.' @('frm1701:rdoPg2I10IncomeExemptYes') 'All required cells are nonblank.' 'First missing numbered cell stops validation.' 'dynamic: Please populate Spouse Part 10 Schedule A Item <x> Exempt.' @('official-hta-runtime#checkPart10SPSchedA:L9005-L9015')
Rule '1701-part10-005' 'validate' $null 'Spouse special-rate-income Yes applies and any Schedule A item 1 through 6 is blank.' @('frm1701:rdoPg2I11IncomeSpecialYes') 'All six cells are nonblank.' 'First missing numbered cell stops validation.' 'dynamic: Please populate Spouse Part 10 Schedule A Item <x> Special.' @('official-hta-runtime#checkPart10SPSchedA:L9017-L9024')
Rule '1701-part10-006' 'validate' $null 'Spouse regular Schedule A items 1,2,3 are all nonblank and either date string length is at most 9.' @('frm1701:txtPg1mI1FSchdA','frm1701:txtPg1mI2FSchdA','frm1701:txtPg1mI3FSchdA','frm1701:txtPg1mI5FSchdA','frm1701:txtPg1mI6FSchdA') 'Both date strings have at least 10 characters.' 'Validation stops.' "Please complete Date of Tax Relief in Spouse Part 10 Schedule A Regular Fields since you have an input in IPA/Legal Basis/Registered Activity." @('official-hta-runtime#checkPart10SPSchedA:L9026-L9033') 'official-bug-compatible' 'Only length is checked.' 'Parse both dates strictly.'

Rule '1701-change-001' 'blur/change' $null 'A date is malformed or year is below 1800.' @('date-field') 'Strict-looking component tests pass.' 'The field is cleared and focused.' 'Please provide a valid date. (MM/DD/YYYY format)' @('official-hta-runtime#validateDate:L8822-L8886') 'official-bug-compatible' 'The future-date rejection branch is commented out and month 00 can pass.' 'Use strict date parsing and enforce field-specific date ranges.'
Rule '1701-change-002' 'blur/change' $null 'Taxpayer or spouse Item 25 exceeds 50% of Item 22 during computation.' @('frm1701:txtPg1I25A','frm1701:txtPg1I25B','frm1701:txtPg1I22ATaxDue','frm1701:txtPg1I22BTaxDue') 'Amount is within the cap.' 'Item 25 is reset to 0.00.' 'dynamic: Amount in page 1 Item 25A/25B cannot be more than 50% of Item 22.' @('official-hta-runtime#computeTxtPg1I26:L11604-L11619')
Rule '1701-change-003' 'blur/change' $null '8% deduction Item 29 exceeds 250,000.' @('frm1701:txtPg3IShed3_29A','frm1701:txtPg3IShed3_29B') 'Amount is at most 250,000.' 'The field is reset to 0.00.' 'Amount cannot be more than 250,000.' @('official-hta-runtime#computeTxtPg3Sc3I30:L11683-L11700')
Rule '1701-change-004' 'blur/change' $null 'Schedule 6 deduction amount is less than gross income when field marker is item2.' @('frm1701:txtPg3IShed6_1A','frm1701:txtPg3IShed6_2A','frm1701:txtPg3IShed6_1B','frm1701:txtPg3IShed6_2B') 'Deduction is not less than gross income.' 'Deduction is reset to 0.00.' 'Deduction amount should be greater than the gross income amount.' @('official-hta-runtime#computePg3Sc6I3:L11766-L11795') 'incorrect-official-behavior' 'The comparison and message contradict each other: it rejects deduction < gross while saying deduction should be greater.' 'Re-derive the intended NOLCO calculation from the official instructions.'
Rule '1701-change-005' 'blur/change' $null 'NOLCO year is earlier than return year minus 3, later than return year, or equal to return year.' @('nolco-year','frm1701:txtPg1I1Year') 'Year falls in the prior three-year window.' 'Year is cleared.' 'dynamic: Year incurred cannot be more than 3 years of current year. / Year incurred in this field cannot be a future year. / Year incurred in this field cannot be the same as current year.' @('official-hta-runtime#checkValidNOLCOYear:L11797-L11813') 'official-bug-compatible' 'String coercion and the phrase current year actually refer to the return year.' 'Parse integer years and describe the return-year window precisely.'
Rule '1701-change-006' 'blur/change' $null 'Sum of NOLCO columns B, C, and D exceeds column A.' @('nolco-row') 'Sum does not exceed source amount.' 'Columns B, C, and D are all reset to zero.' 'Amount is invalid. Sum of Column B, C and D shall not be greater than the amount in Column E (Net Operating Loss)' @('official-hta-runtime#checkNOLCOoperLoss:L11841-L11857') 'incorrect-official-behavior' 'The message calls the source amount Column E although the code compares against suffix A.' 'Use printed-column labels from the January 2018 form and identify the reset fields accurately.'
Rule '1701-change-007' 'blur/change' $null 'Schedule 5 description or legal basis is blank when its amount handler runs.' @('schedule-5-row') 'Both strings are nonblank.' 'Amount is reset to 0.00 and description focused.' 'dynamic: Description and Legal Basis should both have values for item #<item>.' @('official-hta-runtime#checkSched5Fields:L11859-L11865')
Rule '1701-change-008' 'blur/change' $null '8% gross sales/receipts plus other non-operating income exceeds 3,000,000.' @('frm1701:txtPg3IShed3_26A','frm1701:txtPg3IShed3_26B') 'Amount is at most 3,000,000.' 'The triggering field is reset to 0.00.' 'Your Gross Sales/Receipts and Other Non-Operating Income exceeds VAT Threshold (P3M), thus, not qualified to 8% tax rate and shall be subjected to graduated rates. Please choose a graduated rate ATC and fill in Page 2 Schedule 3.' @('official-hta-runtime#check8percGross:L11867-L11880') 'obsolete' 'The threshold is hard-coded at P3M for this revision.' 'Apply the threshold legally effective for the filing period while retaining revision compatibility.'
Rule '1701-change-009' 'blur/change' $null 'A percentage is negative or greater than or equal to 100.' @('percentage-field') '0 <= value < 100.' 'Value is reset to 0.0 and blur is re-entered.' 'Percentage cannot be greater than or equal to 100%' @('official-hta-runtime#setPercentage:L12795-L12817') 'official-bug-compatible' 'The message omits the negative-value rejection and recursively invokes onblur.' 'Reject negative and >=100 with a non-recursive structured error.'
Rule '1701-change-010' 'blur/change' $null 'Part X or XI rate exceeds 100%.' @('attachment-rate-field') 'Rate is at most 100%.' 'Handler alerts; downstream reset depends on caller.' 'Rate cannot exceed 100%' @('official-hta-runtime#p10rateChk:L11179-L11186','official-hta-runtime#p11rateChk:L11187-L11194')
Rule '1701-final-001' 'final-copy' 1 'Final Copy is requested after validation.' @('txtFinalFlag','return-body') 'Confirmation/encryption/connectivity path proceeds.' 'Offline or connectivity branches can stop the coupled workflow.' $null @('official-hta-runtime#openAlertEmail:L13116-L13181','official-hta-runtime#saveEncryptedProfile:L13256-L13351') 'official-bug-compatible' 'Final copy is coupled to connectivity and encrypted transport preparation.' 'Create deterministic offline finalization independently of submission.'
Rule '1701-submit-001' 'submit' 1 'Online send path is invoked.' @('return-body') 'Encrypted payload is prepared and transport is attempted.' 'No online submission was exercised in this research.' $null @('official-hta-runtime#sendEmail:L13182-L13255','official-hta-runtime#saveXMLsubmit:L8249-L8473') 'unverified' 'Source-derived only; no live submission was performed.' 'Keep online transport outside local validation tests.' 'medium'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    first_error_behavior='validate() runs in source order and stops at the first active main/helper failure; blur/change handlers may alert and mutate fields before Validate. Save runs only three narrow preflight branches.'
    rules=$rules
})

$calcs = [Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Outputs,[string[]]$Inputs,[string]$Formula,[string]$Trigger,[string[]]$Depends,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Use decimal arithmetic and recompute from authoritative inputs.',[string]$Rounding='NumWithComma parses formatted strings and formatCurrency emits two-decimal display strings unless the called helper specifies otherwise.') {
    $calcs.Add([pscustomobject][ordered]@{
        calculation_id=$Id; outputs=$Outputs; inputs=$Inputs; condition=$null; official_formula=$Formula; rounding=$Rounding
        trigger=$Trigger; depends_on=$Depends; source_refs=$Refs; assessment=$Assessment; recommended_app_behavior=$Recommended; confidence='high'
    })
}
Calc '1701-tax-table-2018' @('graduated-income-tax') @('taxable-income') 'Up to 250,000: 0; 250,000-400,000: 20% of excess over 250,000; 400,000-800,000: 30,000 + 25% over 400,000; 800,000-2,000,000: 130,000 + 30% over 800,000; 2,000,000-8,000,000: 490,000 + 32% over 2,000,000; over 8,000,000: 2,410,000 + 35% over 8,000,000.' 'calcTaxAmt' @() @('official-hta-runtime#calcTaxAmt:L11427-L11485','official-form-pdf#page-2-tax-table')
Calc '1701-part2-item22' @('frm1701:txtPg1I22ATaxDue','frm1701:txtPg1I22BTaxDue') @('frm1701:txtPg2I25A','frm1701:txtPg2I25B','frm1701:txtPg3IShed3_32A','frm1701:txtPg3IShed3_32B','tax-rate') 'Copies the applicable total income tax due from graduated or 8% schedules into Part II Item 22 by taxpayer/spouse.' 'computeTxtPg2I22 and rate handlers' @('1701-tax-table-2018') @('official-hta-runtime#computeTxtPg2I22:L11530-L11540','official-hta-runtime#taxRateEnableDisableFields:L11195-L11223')
Calc '1701-part2-item24' @('frm1701:txtPg1I24ATaxPayable','frm1701:txtPg1I24BTaxPayable') @('item22','item23') '24 = 22 - 23.' 'computeTxtPg1I24' @('1701-part2-item22') @('official-hta-runtime#computeTxtPg1I24:L11578-L11587')
Calc '1701-part2-item26' @('frm1701:txtPg1I26A','frm1701:txtPg1I26B') @('item24','item25') '26 = 24 - 25 after enforcing Item 25 <= 50% of Item 22.' 'computeTxtPg1I26' @('1701-part2-item24') @('official-hta-runtime#computeTxtPg1I26:L11604-L11619')
Calc '1701-part2-item30' @('frm1701:txtPg1I30A','frm1701:txtPg1I30B') @('item27','item28','item29') '30 = surcharge + interest + compromise.' 'computeTxtPg1I30' @() @('official-hta-runtime#computeTxtPg1I30:L11621-L11628')
Calc '1701-part2-item31' @('frm1701:txtPg1I31ATotalAmtPyble','frm1701:txtPg1I31BTotalAmtPyble') @('item26','item30') 'Normally 31 = 26 + 30; if Item 26 is negative and penalties are positive, Item 31 is penalties only.' 'computeTxtPg1I31' @('1701-part2-item26','1701-part2-item30') @('official-hta-runtime#computeTxtPg1I31:L11630-L11649') 'official-bug-compatible' 'Verify whether overpayment should offset penalties; preserve official behavior separately.'
Calc '1701-part2-item32' @('frm1701:txtPg1I32AggregateAmtPyble') @('item31A','item31B') '32 = Item 31A + Item 31B.' 'computeTxtPg1I32' @('1701-part2-item31') @('official-hta-runtime#computeTxtPg1I32:L11651-L11655')
Calc '1701-schedule2-item4' @('frm1701:txtPg2IShed2_4A','frm1701:txtPg2IShed2_4B') @('schedule2-items-1-through-3') 'Item 4 is the sum of taxable compensation components in Schedule 2.' 'computeTxtPg2Sc2I4CI' @() @('official-hta-runtime#computeTxtPg2Sc2I4CI:L11380-L11391')
Calc '1701-schedule2-item6' @('frm1701:txtPg2IShed2_6A','frm1701:txtPg2IShed2_6B') @('schedule2-item4','schedule2-item5') '6 = 4 - 5.' 'computeTxtPg2I6' @('1701-schedule2-item4') @('official-hta-runtime#computeTxtPg2I6:L11392-L11403')
Calc '1701-schedule2-item7' @('frm1701:txtPg2IShed2_7A','frm1701:txtPg2IShed2_7B') @('schedule2-item6') '7 = graduated tax table(Item 6).' 'computeTxtPg1I7' @('1701-tax-table-2018','1701-schedule2-item6') @('official-hta-runtime#computeTxtPg1I7:L11404-L11426')
Calc '1701-schedule3a-item10' @('frm1701:txtPg2IShed3_10A','frm1701:txtPg2IShed3_10B') @('schedule3a-items-8-and-9') '10 = 8 - 9.' 'computeTxtPg2I10' @() @('official-hta-runtime#computeTxtPg2I10:L11486-L11496')
Calc '1701-schedule3a-item12' @('frm1701:txtPg2IShed3_12A','frm1701:txtPg2IShed3_12B') @('schedule3a-items-10-and-11') '12 = 10 - 11.' 'computeTxtPg2I12' @('1701-schedule3a-item10') @('official-hta-runtime#computeTxtPg2I12:L11497-L11507')
Calc '1701-schedule3a-item16' @('frm1701:txtPg2IShed3_16A','frm1701:txtPg2IShed3_16B') @('schedule3a-item12','schedule3a-items-13-through-15') '16 = Item 12 less the applicable deduction aggregate.' 'computeTxtPg2I16' @('1701-schedule3a-item12') @('official-hta-runtime#computeTxtPg2I16:L11508-L11520')
Calc '1701-schedule3a-item17' @('frm1701:txtPg2IShed3_17A','frm1701:txtPg2IShed3_17B') @('schedule3a-item16') '17 = graduated tax table(Item 16).' 'computeTxtPg2I17' @('1701-tax-table-2018','1701-schedule3a-item16') @('official-hta-runtime#computeTxtPg2I17:L11521-L11529')
Calc '1701-schedule3a-item23' @('frm1701:txtPg2IShed3_23A','frm1701:txtPg2IShed3_23B') @('schedule3a-items-17-through-22') '23 is the computed taxable income total used by the Part IX cross-check.' 'computeTxtPg2I23' @('1701-schedule3a-item17') @('official-hta-runtime#computeTxtPg2I23:L11541-L11551')
Calc '1701-schedule3a-item24' @('frm1701:txtPg2IShed3_24A','frm1701:txtPg2IShed3_24B') @('schedule3a-item23') '24 = graduated tax table(Item 23).' 'computeTxtPg2I24' @('1701-tax-table-2018','1701-schedule3a-item23') @('official-hta-runtime#computeTxtPg2I24:L11552-L11562')
Calc '1701-schedule3a-item25' @('frm1701:txtPg2IShed3_25A','frm1701:txtPg2IShed3_25B') @('schedule2-item7','schedule3a-item24') '25 combines compensation tax and business/profession tax according to filer type.' 'computeTxtPg2I25' @('1701-schedule2-item7','1701-schedule3a-item24') @('official-hta-runtime#computeTxtPg2I25:L11563-L11577')
Calc '1701-schedule3b-item28' @('frm1701:txtPg3IShed3_28A','frm1701:txtPg3IShed3_28B') @('schedule3b-items-26-and-27') '28 = 26 + 27.' 'computeTxtPg3Sc3I28' @() @('official-hta-runtime#computeTxtPg3Sc3I28:L11672-L11681')
Calc '1701-schedule3b-item30' @('frm1701:txtPg3IShed3_30A','frm1701:txtPg3IShed3_30B') @('schedule3b-item28','schedule3b-item29') '30 = 28 - 29; Item 29 is capped at 250,000.' 'computeTxtPg3Sc3I30' @('1701-schedule3b-item28') @('official-hta-runtime#computeTxtPg3Sc3I30:L11683-L11700')
Calc '1701-schedule3b-item31' @('frm1701:txtPg3IShed3_31A','frm1701:txtPg3IShed3_31B') @('schedule3b-item30') 'If Item 30 > 0, Item 31 = Item 30 * 8%; if Item 30 < 0, Item 31 = 0.00.' 'computeTxtPg3Sc3I31' @('1701-schedule3b-item30') @('official-hta-runtime#computeTxtPg3Sc3I31:L11702-L11723')
Calc '1701-schedule3b-item32' @('frm1701:txtPg3IShed3_32A','frm1701:txtPg3IShed3_32B') @('schedule2-item7','schedule3b-item31') '32 = compensation tax from Schedule 2 Item 7 + 8% business tax Item 31.' 'computeTxtPg3Sc3I32' @('1701-schedule2-item7','1701-schedule3b-item31') @('official-hta-runtime#computeTxtPg3Sc3I32:L11725-L11736')
Calc '1701-schedule4-item18' @('frm1701:txtPg3IShed4_18A','frm1701:txtPg3IShed4_18B') @('schedule4-items-1-through-17d') '18 = sum of Schedule 4 Items 1 through 17d by taxpayer/spouse.' 'computePg3Sc4I18' @() @('official-hta-runtime#computePg3Sc4I18:L11738-L11749')
Calc '1701-schedule5-totals' @('frm1701:txtPg3IShed5_3','frm1701:txtPg3IShed5_6') @('schedule5-items-1,2,4,5') 'Item 3 = Items 1 + 2; Item 6 = Items 4 + 5.' 'computePg3Sc5I3/computePg3Sc5I6' @() @('official-hta-runtime#computePg3Sc5I3:L11751-L11757','official-hta-runtime#computePg3Sc5I6:L11759-L11764')
Calc '1701-schedule6-item3' @('frm1701:txtPg3IShed6_3A','frm1701:txtPg3IShed6_3B') @('schedule6-items-1-and-2') '3 = 1 - 2; a contradictory guard resets Item 2 when Item 2 < Item 1.' 'computePg3Sc6I3' @() @('official-hta-runtime#computePg3Sc6I3:L11766-L11795') 'incorrect-official-behavior' 'Re-derive the intended gross-income/deduction relationship.'
Calc '1701-nolco-row-balances' @('schedule6-row-E') @('schedule6-row-A','schedule6-row-B','schedule6-row-C','schedule6-row-D') 'E = A - (B + C + D), with a guard that zeroes B/C/D when their sum exceeds A.' 'computePg3Sc6I4E through computePg4Sc6I12E' @() @('official-hta-runtime#computePg3Sc6I4E:L11815-L11838','official-hta-runtime#computePg3Sc6I8D:L11921-L11928','official-hta-runtime#computePg4Sc6I9E:L11929-L11953')
Calc '1701-total-allowable-deductions' @('schedule3a-allowable-deduction-total') @('schedule4-item18','schedule5-total','schedule6-total','deduction-method') 'Populates the applicable itemized/OSD deduction total and propagates it into Schedule 3.A.' 'populateTotalAllowDeduc/computePg2TotAllowDeduc' @('1701-schedule4-item18','1701-schedule5-totals','1701-nolco-row-balances') @('official-hta-runtime#populateTotalAllowDeduc:L11962-L11976','official-hta-runtime#computePg2TotAllowDeduc:L11977-L11985')
Calc '1701-schedule6-summary' @('frm1701:txtPg4ISc6_4A','frm1701:txtPg4ISc6_4B','frm1701:txtPg4ISc6_5A','frm1701:txtPg4ISc6_5B') @('schedule6-prior-year-rows') 'Aggregates available NOLCO by party and copies the result to the total-allowable-deduction path.' 'computePg4Sc6I4/computeTxtPg4Sc6I5' @('1701-nolco-row-balances') @('official-hta-runtime#computePg4Sc6I4:L11986-L11996','official-hta-runtime#computeTxtPg4Sc6I5:L11997-L12007')
Calc '1701-part7-item10' @('frm1701:txtPg4IPart7_10A','frm1701:txtPg4IPart7_10B') @('part7-items-1-through-9') '10 = sum of Part VII tax credits/payments by taxpayer/spouse.' 'computeTxtPg4Pt7I10' @() @('official-hta-runtime#computeTxtPg4Pt7I10:L12008-L12020')
Calc '1701-part8-running-totals' @('part8-items-3,5,7,10') @('part8-preceding-items') 'Computes Part VIII running sums/differences for tax relief and creditable tax.' 'computeTxtPg4Pt8I3/I5/I7/I10' @() @('official-hta-runtime#computeTxtPg4Pt8I3:L12068-L12077','official-hta-runtime#computeTxtPg4Pt8I5:L12078-L12087','official-hta-runtime#computeTxtPg4Pt8I7:L12088-L12097','official-hta-runtime#computeTxtPg4Pt8I10:L12098-L12106')
Calc '1701-part9-totals' @('part9-items-5,10,11') @('part9-preceding-items') 'Computes Part IX income and taxable-income totals used by the Schedule 3 cross-check.' 'computeTxtPg4Pt9I5/I10/I11' @() @('official-hta-runtime#computeTxtPg4Pt9I5:L12107-L12117','official-hta-runtime#computeTxtPg4Pt9I10:L12118-L12127','official-hta-runtime#computeTxtPg4Pt9I11:L12128-L12137')
Calc '1701-part10-attachment-schedules' @('Part X Schedules B,C,D totals') @('Part X attachment Schedule A-D fields') 'Computes per-attachment EXn/SPn Schedule B, C, and D amounts, then consolidates them into main Part X totals.' 'computePage1mScheduleB through computeConsolSchedD' @() @('official-hta-runtime#computePage1mScheduleB:L12217-L12414','official-hta-runtime#computePage2mScheduleC:L12415-L12431','official-hta-runtime#computePage2mScheduleD:L12432-L12442','official-hta-runtime#computePage3mScheduleB:L12443-L12509','official-hta-runtime#computePage4mScheduleC:L12510-L12565','official-hta-runtime#computePage4mScheduleD:L12566-L12590','official-hta-runtime#computeConsolSchedBAmt:L12595-L12622','official-hta-runtime#computeConsolSchedC:L12623-L12650','official-hta-runtime#computeConsolSchedD:L12651-L12682')
Calc '1701-attachment-collected-value' @('consolidated-attachment-cell') @('all matching EXn/SPn attachment cells') 'Sums the selected field across materialized attachment instances.' 'computeCollectedValue' @('1701-part10-attachment-schedules') @('official-hta-runtime#computeCollectedValue:L12683-L12699')
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema'='../../schema/calculations.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    evaluation_order=@($calcs.calculation_id); calculations=$calcs
})

$negativeRules = @($rules | Where-Object { $_.exact_message } | Select-Object -First 40)
$negativeCases = @(); $caseNumber = 0
foreach ($rule in $negativeRules) {
    $caseNumber++
    $negativeCases += [pscustomobject][ordered]@{
        case_id=('case-{0:d2}-{1}' -f $caseNumber,$rule.rule_id); phase=$rule.phase
        mutations=@{ synthetic_condition=$rule.condition }; expected_message=$rule.exact_message
        expected_behavior=$rule.official_behavior; rule_id=$rule.rule_id
    }
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{ schema_version='1.0.0'; form_id=$formId; synthetic_only=$true; cases=$negativeCases })
$calcCases = @(
    @{case_id='tax-table-250000';calculation_id='1701-tax-table-2018';inputs=@{taxable_income=250000};official_output='0.00'},
    @{case_id='tax-table-400000';calculation_id='1701-tax-table-2018';inputs=@{taxable_income=400000};official_output='30000.00'},
    @{case_id='tax-table-800000';calculation_id='1701-tax-table-2018';inputs=@{taxable_income=800000};official_output='130000.00'},
    @{case_id='tax-table-2000000';calculation_id='1701-tax-table-2018';inputs=@{taxable_income=2000000};official_output='490000.00'},
    @{case_id='tax-table-8000000';calculation_id='1701-tax-table-2018';inputs=@{taxable_income=8000000};official_output='2410000.00'},
    @{case_id='item31-negative-with-penalty';calculation_id='1701-part2-item31';inputs=@{item26=-100;item30=200};official_output='200.00';recommended_review='Determine whether 100.00 is legally intended.'},
    @{case_id='eight-percent-cap';calculation_id='1701-schedule3b-item30';inputs=@{item28=500000;item29=250000};official_output='250000.00'}
)
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{ schema_version='1.0.0'; form_id=$formId; cases=$calcCases })

$linkedAssets = @()
foreach ($src in @([regex]::Matches($hta, '(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1') | ForEach-Object { $_.Groups['v'].Value } | Sort-Object -Unique)) {
    $full = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    $linkedAssets += Asset ("linked-" + ([IO.Path]::GetFileNameWithoutExtension($full) -replace '[^A-Za-z0-9]+','-').ToLowerInvariant()) 'official-linked-script' $full "Loaded by the exact January 2018 HTA as $src."
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{ schema_version='1.0.0'; form_id=$formId; resources=$linkedAssets })

$workflow = [ordered]@{
    '$schema'='../../schema/workflow.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    phases=@(
        @{phase='edit';official_behavior='Four-page annual return plus cloneable exempt/special attachment worksheets; change handlers enable, reset, calculate, and validate dependent controls.';source_refs=@('official-hta-runtime#checkFieldsAfterXMLLoad:L8799-L8820','official-hta-runtime#addAttachment:L13594-L13668');confidence='high'},
        @{phase='saved-draft';official_behavior='Save runs only TIN nonblank, RDO != 000, and taxpayer-name checks, then serializes every non-hidden text/select/radio/checkbox in frmMain.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L7918-L7932','official-hta-runtime#saveXML:L8095-L8221');confidence='high'},
        @{phase='validated';official_behavior='Ordered main validation and conditional helpers run; success disables controls and shows the success alert.';source_refs=@('official-hta-runtime#validate:L9135-L9325','official-hta-runtime#disableAllControl:L9330-L9351');confidence='high'},
        @{phase='final-copy';official_behavior='Final Copy is coupled to confirmation, local encryption, and connectivity checks.';source_refs=@('official-hta-runtime#openAlertEmail:L13116-L13181','official-hta-runtime#saveEncryptedProfile:L13256-L13351');confidence='high'},
        @{phase='submitted';official_behavior='The HTA has an online send path, but this research did not submit.';source_refs=@('official-hta-runtime#sendEmail:L13182-L13255');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='TIN components nonblank, RDO value not 000, and taxpayer name nonblank.';side_effects=@('Writes plaintext pseudo-XML save.','Persists a background profile.');source_refs=@('official-hta-runtime#initialValidateBeforeSave:L7918-L7932','official-hta-runtime#saveXML:L7934-L8247')},
        @{from='edit';action='Validate';to='validated';guard='All active ordered rules and helper checks pass.';side_effects=@('Disables controls.','Shows validation-success alert.');source_refs=@('official-hta-runtime#validate:L9135-L9325')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables applicable controls and recomputes conditional state.');source_refs=@('official-hta-runtime#enableAllControl:L9352-L9419')},
        @{from='validated';action='Final Copy';to='final-copy';guard='User confirms the coupled workflow and connectivity/encryption path permits progress.';side_effects=@('Creates encrypted copy.','Updates final-copy/read-only state.');source_refs=@('official-hta-runtime#openAlertEmail:L13116-L13181')},
        @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and transport succeed.';side_effects=@('Attempts online send; not exercised.');source_refs=@('official-hta-runtime#sendEmail:L13182-L13255')}
    )
    prerequisites=@('January 2018 ENCS revision','Return year 2018 or later for this HTA','Applicable taxpayer and spouse identity/ATC selections','Applicable income schedules and tax-rate election')
    required_attachments=@(
        @{attachment_id='exempt-income-worksheet';label='Form 1701 exempt tax regime attachment';required_when='Taxpayer or spouse reports exempt income and the corresponding attachment option is selected.';official_ui_enforcement='The HTA creates EXn worksheets and checks selected Schedule A cells, but does not prove documentary attachments.';source_refs=@('official-attachment-pdf#all-pages','official-hta-runtime#addAttachment:L13594-L13668','official-hta-runtime#checkPart10TPSchedA:L8972-L9003');confidence='high'},
        @{attachment_id='special-rate-worksheet';label='Form 1701 special/preferential rate attachment';required_when='Taxpayer or spouse reports special-rate income and the corresponding attachment option is selected.';official_ui_enforcement='The HTA creates SPn worksheets and validates selected Schedule A cells.';source_refs=@('official-attachment-pdf#all-pages','official-hta-runtime#addAttachment:L13594-L13668','official-hta-runtime#checkPart10SPSchedA:L9005-L9034');confidence='high'},
        @{attachment_id='foreign-tax-credit-proof';label='Proof supporting foreign tax credit';required_when='Taxpayer or spouse claims foreign tax credits.';official_ui_enforcement='Local Validate requires a foreign tax number but does not enforce an external document.';source_refs=@('official-form-pdf#instructions','official-hta-runtime#validate:L9203-L9210','official-hta-runtime#validateSpouseFields:L9095-L9105');confidence='medium'}
    )
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='Annual return; file on or before April 15 following the taxable year, subject to applicable law and calendar adjustments.';source_refs=@('official-form-pdf#filing-instructions');confidence='medium'},
        @{quarter='Q2';due_date_rule='Not a quarterly return; same annual-return deadline applies.';source_refs=@('official-form-pdf#filing-instructions');confidence='medium'},
        @{quarter='Q3';due_date_rule='Not a quarterly return; same annual-return deadline applies.';source_refs=@('official-form-pdf#filing-instructions');confidence='medium'},
        @{quarter='Q4';due_date_rule='Not a quarterly return; same annual-return deadline applies.';source_refs=@('official-form-pdf#filing-instructions');confidence='medium'}
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$gapCount = 4
$encryptedAsset = Asset 'xml-encrypted-v1' 'dummy-profile-encrypted-copy' $encryptedPath 'Reviewed 838-key encrypted companion; values excluded from artifacts.'
$encryptedAsset.path = Join-Path $SourceDir '00000000000000-1701v2018-122025#<email-redacted>#.xml'
$officialAssets = @(
    Asset 'package-7.9.6' 'official-package-executable' 'C:\eBIRForms\BIRForms.exe' 'Installed Offline eBIRForms package 7.9.6.0.',
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1701v2018 and printed header January 2018 (ENCS).',
    Asset 'xml-editable-v1' 'dummy-profile-editable-save' $plainPath 'Reviewed 837-key plaintext save; values excluded from artifacts.',
    $encryptedAsset,
    Asset 'official-form-pdf' 'official-form-pdf' $officialPdf 'January 2018 ENCS main form.',
    Asset 'official-attachment-pdf' 'official-attachment-pdf' $attachmentPdf 'January 2018 ENCS attachment worksheets.',
    Asset 'official-consolidated-pdf' 'official-consolidated-pdf' $consolidatedPdf 'January 2018 consolidated worksheet.',
    Asset 'typed-form-1701' 'repository-model' $typedModel 'Existing reviewed typed model independently pins the form revision and external hashes.',
    Asset 'typed-form-1701-xml' 'repository-xml-model' $typedXml 'Existing exact 837/838-field lossless import/export mapping and external-source test.'
)
$manifest = [ordered]@{
    '$schema'='../../schema/form-manifest.schema.json'; schema_version='1.0.0'; form_id=$formId; form_code='1701'; revision=$revision
    revision_label='January 2018 (ENCS)'; package_version=$packageVersion; status='complete'; official_assets=$officialAssets
    counts=[ordered]@{ concrete_fields=838; runtime_field_families=115; fields_total=$fields.Count; typed_fields=$fields.Count; validation_rules=$rules.Count; confirmed_official_bugs=$bugCount; calculations=$calcs.Count; negative_fixtures=$negativeCases.Count; unverified_gaps=$gapCount }
    artifacts=[ordered]@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';runtime_control_fixture='fixtures/runtime-control-inventory-v796.json';encrypted_field_audit='fixtures/encrypted-field-audit-v796.json';validation_function_fixture='fixtures/validation-function-inventory-v796.json';calculation_function_fixture='fixtures/calculation-function-inventory-v796.json';resource_hash_fixture='fixtures/official-resource-hashes-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'}
    scope_notes=@('Research artifacts only; no renderer, migration, release, or capability changes.','No source values, email address, or online submission data are copied.','The exact reviewed plaintext/encrypted snapshots contain 837/838 keys; 115 additional descriptors preserve the unbounded EXn/SPn attachment families.','The June 2013 BIR-Form1701.hta and its 452-key save are a separate revision and are excluded.')
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest

$readme = @"
# BIR Form 1701 — January 2018 (ENCS)

Revision-specific Offline eBIRForms validation, calculation, field, and workflow evidence for `1701v2018` in package 7.9.6.0.

The reviewed plaintext save proves 837 concrete keys; its encrypted companion proves 838 by adding the second registered-address line. The HTA additionally clones 115 template keys into unbounded `EXn`/`SPn` attachment families. Values and the email-bearing source filename are intentionally excluded from all artifacts.
"@
Write-Utf8 (Join-Path $outDir 'README.md') $readme
$evidence = @"
# Evidence

- `BIR-Form1701v2018.hta`: SHA-256 `$((Get-FileHash -LiteralPath $htaPath -Algorithm SHA256).Hash.ToLowerInvariant())`; application name `1701v2018`, HTA version 1.0, printed header **January 2018 (ENCS)**.
- Reviewed plaintext save: SHA-256 `b168c7b3273d30a10f28f4653847519b876d5a88e77ed82911718a80f65c7827`; exactly 837 unique keys and `txtVersion=051414`. Values are not copied.
- Reviewed encrypted companion: SHA-256 `3771c99c191ef5e84b1b5e4c51499911bfbec6002febc3c53dca3f08730e92e3`; in-memory DCPcrypt/zlib replay proves decrypted SHA-256 `95ee42ed78f104335f50168a40e207f8af71ddf8eced9ddd0db1ad42d6366800`, 838 unique keys, and the extra `frm1701:txtPg1I9Address2`. No decrypted values are written or copied.
- Main/attachment/consolidated January 2018 PDFs are pinned in `manifest.json` and independently locked by `form_1701.rs`.
- The 837 plaintext keys exactly match the count of static serializer candidates. Two RDO fields are runtime-injected; two static candidates differ from the plaintext snapshot. The differences are retained in the runtime-control fixture.
- `addAttachment()` replaces the literal template token `TYPE` with `EXn` or `SPn`, appends the controls into `frmMain`, and therefore makes 115 unbounded families serializable.
"@
Write-Utf8 (Join-Path $outDir 'evidence.md') $evidence
$gaps = @"
# Gaps

1. No online submission was performed; submit behavior is source-derived only.
2. The encrypted companion is replayed only in memory by the values-redacted audit fixture; no decrypted payload or values are written. The Rust replay test could not be rerun in this Windows session because `cargo` is not installed on `PATH`.
3. The HTA enforces worksheet cells but does not prove every legally required external documentary attachment.
4. Several Part X/XI calculations are grouped by worksheet because their 115 cloned field families are unbounded; exact function bodies and control references remain in the calculation-function fixture.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') $gaps
$audit = @"
# Audit

- Exact revision pinned: **pass** — January 2018 (ENCS), `1701v2018`.
- Official source hashes pinned: **pass** — HTA, reviewed 837/838 saves, three PDFs, linked scripts, and existing repository model.
- XML inventory: **pass** — 838 concrete union fields plus 115 unbounded attachment-family descriptors; no source values copied.
- Validation inventory: **pass** — ordered Save/Validate/helper/change/final/submit behavior with exact observable messages.
- Calculation inventory: **pass** — tax table, main return, Schedules 2–6, Parts VII–X, and attachment consolidation.
- Official defects separated from recommendations: **pass** — `$bugCount` rules classified bug-compatible, incorrect, or obsolete.
- Online submission: **not exercised**, explicitly unverified.
- Renderer/release metadata: **untouched**.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') $audit
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 10: 1701-v2018. Next: 1701A. Run rules/validate.ps1 -RequireJsonSchema before advancing.`n"

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry = [pscustomobject][ordered]@{form_id=$formId;form_code='1701';revision=$revision;package_version=$packageVersion;priority=10;status='complete';path='forms/1701-v2018/manifest.json'}
$kept = @($index.forms | Where-Object { $_.form_id -ne $formId })
$index.forms = @($kept + $entry | Sort-Object priority)
$index.updated = (Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index

"Generated ${formId}: fields=$($fields.Count), rules=$($rules.Count), calculations=$($calcs.Count), negative_cases=$($negativeCases.Count), bug_classifications=$bugCount"
