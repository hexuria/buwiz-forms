param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$HtaPath = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\forms\BIR-Form2550M.hta',
    [string]$HelpPath = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\helpfile\Help2550M.hta',
    [string]$SavePath = 'C:\eBIRForms\savefile\00000000000000-2550M-072026.xml',
    [string]$AtcPath = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\xml\atcCodes.xml'
)

$ErrorActionPreference = 'Stop'
$formId = '2550m-v2007'
$revision = '2007-02-01'
$packageVersion = '7.9.6.0'
$outDir = Join-Path $RepoRoot 'rules\forms\2550m-v2007'
$fixtureDir = Join-Path $outDir 'fixtures'
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

foreach ($required in @($HtaPath, $HelpPath, $SavePath, $AtcPath)) {
    if (-not (Test-Path -LiteralPath $required)) { throw "Required source is missing: $required" }
}

function Write-Json([string]$Path, $Value) {
    $json = $Value | ConvertTo-Json -Depth 30
    [IO.File]::WriteAllText($Path, $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
}

function Get-Attr([string]$Tag, [string]$Name) {
    $pattern = '(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)
    $match = [regex]::Match($Tag, $pattern)
    if ($match.Success) { return $match.Groups[2].Value }
    return $null
}

function Get-Sha256Text([string[]]$Lines) {
    $bytes = [Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}

$hta = [IO.File]::ReadAllText($HtaPath)
$scriptRanges = @([regex]::Matches($hta, '<script\b.*?</script>', 'IgnoreCase,Singleline'))
$controls = @()
$ordinal = 0
foreach ($match in [regex]::Matches($hta, '<(input|select|textarea|button)\b[^>]*>', 'IgnoreCase,Singleline')) {
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
        source_line = 1 + [regex]::Matches($hta.Substring(0, $match.Index), "`n").Count
        value = Get-Attr $tag 'value'
        maxlength = Get-Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
}

$saveText = [IO.File]::ReadAllText($SavePath)
$saveMatches = [regex]::Matches($saveText, '(?m)^\s*<div>(?<key>[^=<>]+)=(?<value>.*?)\k<key>=</div>\s*$')
$observedKeys = @($saveMatches | ForEach-Object { $_.Groups['key'].Value })
if ($observedKeys.Count -ne 142 -or ($observedKeys | Sort-Object -Unique).Count -ne 142) {
    throw "Expected 142 unique representative-save keys; found $($observedKeys.Count) entries and $(($observedKeys | Sort-Object -Unique).Count) unique keys."
}

$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control }
}

$requiredMain = @(
    'frm2550m:txtYear','frm2550m:txtTIN1','frm2550m:txtTIN2','frm2550m:txtTIN3',
    'frm2550m:txtBranchCode','frm2550m:txtRDOCode','frm2550m:txtLineBus',
    'frm2550m:txtTaxPayerName','frm2550m:txtTelephoneNum','frm2550m:txtAddress','frm2550m:txtZipCode'
)
$computedTax = '^(frm2550m:txtTax(12[AB]|13B|16[AB]|17F|18[BDFHJLOP]|19|20[BCF]|21|22|23[ABCG]|24|25D|26)|frm2550M:txtmodaltxtTotal12[AB]|txtmodalTotal|txtTotal|txtProduct)'

function Get-FieldMetadata([string]$Key, $Control, [bool]$Family) {
    $page = $null; $item = $null; $label = $Key; $logical = 'string'; $required = 'optional'
    $constraints = [ordered]@{}; $enum = @(); $normalization = @(); $computed = $false; $calculation = $null
    $controlKind = if ($Control) { $Control.control_kind } elseif ($Family) { 'runtime-indexed-family' } else { 'serialized-runtime-control' }

    if ($Key -eq 'frm2550m:RtnYear') { $page=1; $item='1'; $label='Month'; $logical='integer'; $required='required'; $enum=@('01','02','03','04','05','06','07','08','09','10','11','12') }
    elseif ($Key -eq 'frm2550m:txtYear') { $page=1; $item='1'; $label='Year'; $logical='integer'; $constraints.minimum=2000; $required='required' }
    elseif ($Key -match '^frm2550m:OptAmendedYN') { $page=1; $item='2'; $label='Amended Return'; $logical='boolean'; $enum=@('true','false') }
    elseif ($Key -eq 'frm2550m:txtSheets') { $page=1; $item='3'; $label='Number of sheets attached'; $logical='integer' }
    elseif ($Key -match '^frm2550m:txtTIN[123]$') { $page=1; $item='4'; $label='TIN segment'; $logical='digit-string'; $required='required'; $constraints.max_length=3 }
    elseif ($Key -eq 'frm2550m:txtBranchCode') { $page=1; $item='4'; $label='TIN branch code'; $logical='digit-string'; $required='required'; $constraints.max_length=5 }
    elseif ($Key -eq 'frm2550m:txtRDOCode') { $page=1; $item='5'; $label='RDO code'; $logical='code'; $required='required' }
    elseif ($Key -eq 'frm2550m:txtLineBus') { $page=1; $item='6'; $label='Line of business'; $required='required' }
    elseif ($Key -eq 'frm2550m:txtTaxPayerName') { $page=1; $item='7'; $label='Taxpayer name'; $required='required' }
    elseif ($Key -eq 'frm2550m:txtTelephoneNum') { $page=1; $item='8'; $label='Telephone number'; $logical='phone-string'; $required='required' }
    elseif ($Key -eq 'frm2550m:txtAddress') { $page=1; $item='9'; $label='Registered address'; $required='required' }
    elseif ($Key -eq 'frm2550m:txtZipCode') { $page=1; $item='10'; $label='ZIP code'; $logical='digit-string'; $required='required' }
    elseif ($Key -match '^frm2550m:OptSpecialTax') { $page=1; $item='11'; $label='Special tax/rate'; $logical='boolean'; $enum=@('true','false') }
    elseif ($Key -eq 'frm2550m:lstSpecialTax') { $page=1; $item='11'; $label='Special tax/rate classification'; $logical='enum'; $required='conditional' }
    elseif ($Key -match '^frm2550m:txtTax(?<number>\d+[A-Z]?)$') { $page=1; $item=$Matches.number; $label="Item $($Matches.number) amount"; $logical='decimal-money'; $normalization=@('NumWithComma','formatCurrency') }
    elseif ($Key -match '^frm2550m:txt(AmountSales|OutputTax)') { $label = if ($Key -match 'AmountSales') {'Schedule 1 sales/receipts'} else {'Schedule 1 output tax'}; $logical='decimal-money'; $normalization=@('NumWithComma','formatCurrency') }
    elseif ($Key -match '^frm2550m:txtAtcCde') { $label='Schedule 1 ATC code'; $logical='code'; $enum=@('See fixtures/atc-catalog-v796.json') }
    elseif ($Key -match '^AtcCode\d+$') { $label='Schedule 1 ATC selection flag'; $logical='boolean'; $enum=@('true','false') }
    elseif ($Key -match '(DatePurchased|PeriodCovered)') { $label='Schedule date'; $logical='date-string'; $constraints.format='MM/DD/YYYY' }
    elseif ($Key -match '(Amt|Amount|Tax|Withheld|Paid|Applied|Sale|Income)') { $label='Schedule amount'; $logical='decimal-money'; $normalization=@('NumWithComma','formatCurrency') }
    elseif ($Key -match '(EstLife|RecogLife)') { $label='Schedule useful-life count'; $logical='integer' }
    elseif ($Key -match '^chxSched') { $label='Schedule row selection'; $logical='boolean'; $enum=@('true','false') }
    elseif ($Key -match '(Description|NameAgent|NameMiller|NameTaxPayer|ORNum)') { $label='Schedule text'; $logical='string' }
    elseif ($Key -match '^(txtFinalFlag|txtEnroll|ebirOnline|driveSelectTPExport|txtEmail)') { $label='Workflow metadata'; $required='hidden'; $controlKind='hidden/workflow-metadata' }

    if ($Control -and $Control.maxlength) { $constraints.max_length = [int]$Control.maxlength }
    if ($Key -match $computedTax) { $computed=$true; $required='computed'; $calculation='See calculations.json' }
    if ($Key -match '^(txtInputTax|txtAllowInputTax|txtBalInputTax)') { $computed=$true; $required='computed'; $calculation='See calculations.json' }
    if ($Family) { $constraints.index='N >= 0; no runtime maximum'; $required='conditional' }
    [pscustomobject][ordered]@{page=$page;item=$item;label=$label;logical=$logical;required=$required;constraints=[pscustomobject]$constraints;enum=$enum;normalization=$normalization;computed=$computed;calculation=$calculation;control_kind=$controlKind}
}

$fields = @()
foreach ($key in $observedKeys) {
    $control = if ($controlById.ContainsKey($key)) { $controlById[$key] } else { $null }
    $meta = Get-FieldMetadata $key $control $false
    $refs = @("xml-editable-v1#field:$key")
    if ($control) { $refs += "official-hta-runtime#control:L$($control.source_line)" } else { $refs += 'official-hta-runtime#saveXML:L3010-L3305' }
    $fields += [pscustomobject][ordered]@{
        field_key=$key; serialized_key=$key; serialized_occurrence=1; label=$meta.label; page=$meta.page; item_number=$meta.item
        control_kind=$meta.control_kind; storage_type='string'; logical_type=$meta.logical; required=$meta.required
        required_when=if($key -eq 'frm2550m:lstSpecialTax'){'Item 11 Yes is selected.'}else{$null}
        enabled_when=$null; visible_when=$null; default_value=$null; empty_representation=''
        constraints=$meta.constraints; enum_values=$meta.enum; normalization=$meta.normalization
        computed=$meta.computed; calculation_id=$meta.calculation; source_refs=$refs; confidence=if($control){'high'}else{'medium'}
        notes=@('Key is present in the representative dummy plaintext save; values are intentionally not copied into this knowledge base.')
    }
}

for ($n=1; $n -le 36; $n++) {
    foreach ($definition in @(
        @("frm2550m:txtAtcCde$n",'Schedule 1 ATC code','code',$false),
        @("frm2550m:txtAmountSales$n",'Schedule 1 amount of sales/receipts','decimal-money',$false),
        @("frm2550m:txtOutputTax$n",'Schedule 1 output tax','decimal-money',$true)
    )) {
        $slotEnumValues = [object[]]@()
        $slotNormalization = [string[]]@()
        if ($definition[2] -eq 'code') { $slotEnumValues = [object[]]@('See fixtures/atc-catalog-v796.json') }
        if ($definition[2] -eq 'decimal-money') { $slotNormalization = [string[]]@('NumWithComma','formatCurrency') }
        $fields += [pscustomobject][ordered]@{
            field_key=$definition[0]; serialized_key=$definition[0]; serialized_occurrence=1; label=$definition[1]; page=$null; item_number='Schedule 1'
            control_kind=if($definition[2]-eq'code'){'select'}else{'text'}; storage_type='string'; logical_type=$definition[2]
            required='conditional'; required_when="ATC selector slot $n is selected."; enabled_when=$null; visible_when="ATC selector slot $n is selected."
            default_value=if($definition[2]-eq'code'){''}else{'0.00'}; empty_representation=''; constraints=[pscustomobject][ordered]@{slot=$n;maximum_slots=36}
            enum_values=$slotEnumValues
            normalization=$slotNormalization
            computed=[bool]$definition[3]; calculation_id=if($definition[3]){'2550m-s1-row-output-tax'}else{$null}
            source_refs=@('official-hta-runtime#getATCCode:L3836-L3880','official-hta-runtime#saveXMLsubmit:L3403-L3481')
            confidence='high'; notes=@('Bounded concrete runtime control; absent from the zero-row representative save but generated for a selected ATC.')
        }
    }
}

$families = [ordered]@{
    Schedule2=@('chxSched2','txtDatePurchased','txtDescription','txtAmt','txtInputTax')
    Schedule3A=@('chxSched3A','txtDatePurchased3A','txtDescription3A','txtAmt3A','txtInputTax3A','txtEstLife3A','txtRecogLife3A','txtAllowInputTax3A','txtBalInputTax3A')
    Schedule3B=@('chxSched3B','txtDatePurchased3B','txtDescription3B','txtAmt3B','txtBalInputTaxPrevious3B','txtEstLife3B','txtRecogLife3B','txtAllowInputTax3B','txtBalInputTax3B')
    Schedule6=@('chxSched6','txtPeriodCovered','txtNameAgent','txtIncomePayment','txtTotalWithheld','txtAppliedCurr')
    Schedule7=@('chxSched7','txtPeriodCoveredSch7','txtNameMillerSch7','txtNameTaxPayerSch7','txtORNumSch7','txtAmountPaidSch7','txtAppliedCurrSch7')
    Schedule8=@('chxSched8','txtPeriodCoveredSch8','txtNameAgentSch8','txtIncomePaymentSch8','txtTotalWithheldSch8','txtAppliedCurrSch8')
}
foreach ($schedule in $families.Keys) {
    $sourceFunction = $schedule -replace '^Schedule','Sched'
    foreach ($prefix in $families[$schedule]) {
        $key = "$prefix{N>=0}"
        $meta = Get-FieldMetadata $key $null $true
        $fields += [pscustomobject][ordered]@{
            field_key=$key; serialized_key=$null; serialized_occurrence=$null; label="$schedule indexed field: $prefix"; page=$null; item_number=$schedule
            control_kind='runtime-indexed-family'; storage_type='string'; logical_type=$meta.logical; required='conditional'
            required_when="A $schedule row with index N exists."; enabled_when=$null; visible_when="A $schedule row with index N exists."
            default_value=$null; empty_representation=''; constraints=$meta.constraints; enum_values=$meta.enum; normalization=$meta.normalization
            computed=$meta.computed; calculation_id=$meta.calculation
            source_refs=@("official-hta-runtime#addlist$sourceFunction",'official-hta-runtime#saveXMLsubmit:L3403-L3481')
            confidence='high'; notes=@('Unbounded runtime family descriptor. Each materialized N is serialized under its concrete DOM id; the HTA has no maximum-row guard.')
        }
    }
}

if ($fields.Count -ne 292 -or ($fields.field_key | Sort-Object -Unique).Count -ne 292) { throw "Expected 292 unique inventory entries; got $($fields.Count)." }
$inventoryHash = Get-Sha256Text @($fields.field_key | Sort-Object)
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema'='../../schema/fields.schema.json'; schema_version='1.0.0'; form_id=$formId; revision=$revision
    field_count=$fields.Count; inventory_sha256=$inventoryHash; fields=$fields
})

$runtimeControls = foreach ($control in $controls) {
    [pscustomobject][ordered]@{
        ordinal=$control.ordinal; id=$control.id; name=$control.name; element=$control.element; control_kind=$control.control_kind
        source_line=$control.source_line; value=$control.value; maxlength=$control.maxlength; disabled=$control.disabled; readonly=$control.readonly
        serializable_by_save_loop=($control.id -and $observedKeys -contains $control.id)
    }
}
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0'; form_id=$formId; package_version=$packageVersion
    official_hta_sha256=(Get-FileHash -LiteralPath $HtaPath -Algorithm SHA256).Hash.ToLowerInvariant()
    static_control_count=$controls.Count; static_controls_with_id_count=@($controls|Where-Object id).Count
    static_controls_without_id_count=@($controls|Where-Object{-not $_.id}).Count
    representative_save_key_count=$observedKeys.Count; bounded_concrete_key_count=250; unbounded_family_count=42
    static_controls=$runtimeControls
    bounded_dynamic=@{schedule1_slots=36; keys_per_slot=@('frm2550m:txtAtcCdeN','frm2550m:txtAmountSalesN','frm2550m:txtOutputTaxN')}
    unbounded_dynamic_families=$families
})

$atcEntries = @()
foreach ($line in Get-Content -LiteralPath $AtcPath) {
    $match = [regex]::Match($line, '<div>atc(?<index>\d+):(?<payload>.*?)atc\k<index>:</div>')
    if (-not $match.Success) { continue }
    $parts = $match.Groups['payload'].Value -split '~', -1
    if ($parts.Count -lt 10 -or $parts[9] -notmatch '(^|\|)2550M(\||$)') { continue }
    $atcEntries += [pscustomobject][ordered]@{source_index=[int]$match.Groups['index'].Value;code=$parts[0];description=$parts[1];form_types=$parts[9];catalog_rate=$null}
}
if ($atcEntries.Count -ne 36) { throw "Expected 36 2550M ATC records; found $($atcEntries.Count)." }
Write-Json (Join-Path $fixtureDir 'atc-catalog-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;package_version=$packageVersion
    source_sha256=(Get-FileHash -LiteralPath $AtcPath -Algorithm SHA256).Hash.ToLowerInvariant()
    selection_semantics='The HTA filters the shared catalog for 2550M/2550Q and exposes 36 selector slots. Catalog entries carry no rate; output tax is hard-coded to 12%.'
    entry_count=$atcEntries.Count;entries=$atcEntries
})

$rules = [Collections.Generic.List[object]]::new()
function Add-Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Fields,[string]$Accepted,[string]$Rejected,$Message,[string[]]$Refs,[string]$Assessment,[string]$Official,[string]$Recommended,[string]$Confidence='high') {
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Fields
        accepted_behavior=$Accepted;rejected_behavior=$Rejected;exact_message=$Message;source_refs=$Refs;evidence_type=@('source')
        assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence=$Confidence;unresolved_questions=@()
    })
}

Add-Rule '2550m-input-001' 'blur/change' $null 'A decimal input loses focus.' @('money-fields') 'A parseable value is converted to two decimals.' 'NaN becomes 0.00; JavaScript binary floating-point is used.' $null @('official-hta-runtime#blockletter:L3953-L3963') 'official-bug-compatible' 'parseFloat/toFixed(2) normalization.' 'Use bounded decimal parsing and explicit rounding.'
Add-Rule '2550m-input-002' 'blur/change' $null 'An integer-like input loses focus.' @('integer-fields') 'A parseable value is rounded to zero decimals.' 'NaN becomes blank.' $null @('official-hta-runtime#blockletterWithout2Decimal:L3965-L3975') 'official-bug-compatible' 'parseFloat/toFixed(0) normalization.' 'Validate integer syntax before conversion.'
Add-Rule '2550m-modal-001' 'page navigation' $null 'Schedule 2, 6, 7, or 8 is opened with year <= 2000.' @('frm2550m:txtYear') 'Year greater than 2000 opens the schedule.' 'The schedule is blocked.' 'Please input a valid Return period year. Please enter 2000 above.' @('official-hta-runtime#showSched2:L4051-L4085','official-hta-runtime#showSched6','official-hta-runtime#showSched7','official-hta-runtime#showSched8') 'incorrect-official-behavior' 'The duplicated conjunction <=2000 && <=2020 reduces to <=2000, so year 2000 is rejected although Validate accepts it.' 'Use the same inclusive >=2000 rule in every phase.'
Add-Rule '2550m-s1-001' 'page navigation' $null 'Schedule 1 OK is clicked with zero/blank sales or output tax.' @('frm2550m:txtAmountSalesN','frm2550m:txtOutputTaxN') 'The modal closes despite the invalid row.' 'No rejection occurs; the intended alert is commented out.' $null @('official-hta-runtime#checkifFieldEmptySched1:L3991-L4005') 'incorrect-official-behavior' 'Schedule 1 row validation is an intentional no-op after a QAD change.' 'Require selected ATC rows to have valid sales and a recomputed output tax.'
Add-Rule '2550m-s2-001' 'page navigation' 1 'A Schedule 2 row has blank purchase date or description.' @('txtDatePurchased{N>=0}','txtDescription{N>=0}') 'Both are nonblank.' 'First incomplete row is rejected.' 'Please enter valid row {row} data for Schedule 2.\nEmpty fields are not allowed.' @('official-hta-runtime#checkifEmptyFieldSched2:L4144-L4152') 'verified-correct' 'Blank date/description is rejected.' 'Retain with row-specific structured errors.'
Add-Rule '2550m-s2-002' 'page navigation' 2 'A Schedule 2 row amount exceeds 1,000,000.' @('txtAmt{N>=0}') 'Amount <= 1,000,000.' 'Row is rejected.' 'Please enter valid data.\n Aggregate amount should not exceed P1 Million.' @('official-hta-runtime#checkifEmptyFieldSched2:L4154-L4156') 'verified-correct' 'Per-row cap is enforced.' 'Use decimal comparison and clarify whether the statutory cap is per row or aggregate.'
Add-Rule '2550m-s2-003' 'page navigation' 3 'Schedule 2 date does not split into three numeric components.' @('txtDatePurchased{N>=0}') 'Three numeric components proceed.' 'Row is rejected.' 'Please enter a valid date for Date of Purchase on row {row}.\nPlease enter a date in the MM/DD/YYYY format.' @('official-hta-runtime#checkifEmptyFieldSched2:L4158-L4217') 'official-bug-compatible' 'Ad-hoc split/isNaN validation is used.' 'Use strict calendar-date parsing.'
Add-Rule '2550m-s2-004' 'page navigation' 4 'Schedule 2 date has an impossible day or, except February, is outside the return month/year.' @('txtDatePurchased{N>=0}','frm2550m:RtnYear','frm2550m:txtYear') 'Date passes the branch checks.' 'Row is rejected.' 'Invalid entry on row {row}. Date of Purchase must be within the Return Period only.' @('official-hta-runtime#checkifEmptyFieldSched2:L4170-L4211') 'incorrect-official-behavior' 'Month 02 skips the return-period comparison entirely; month/day lower bounds and unknown months are also not checked.' 'Use a real date and require exact return month/year.'
Add-Rule '2550m-s2-005' 'page navigation' 5 'Schedule 2 net-of-VAT amount is <= 0.' @('txtAmt{N>=0}') 'Amount > 0.' 'Row is rejected.' 'Please enter amount for Net of VAT on row {row}.\nValue must be greater than 0.' @('official-hta-runtime#checkifEmptyFieldSched2:L4223-L4226') 'verified-correct' 'Positive amount required.' 'Retain using decimal comparison.'
Add-Rule '2550m-s2-006' 'page navigation' 6 'Schedule 2 total amount exceeds 1,000,000 when OK is clicked.' @('txtmodalTotalAmt') 'Total <= 1,000,000.' 'Modal remains open.' 'The total aggregate amount should not exceed 1 Million.\n Please re-enter the values of Schedule 2.' @('official-hta-runtime#getSched2Modal:L4087-L4111') 'verified-correct' 'Aggregate cap is enforced after row validation.' 'Retain with exact decimal sum.'
Add-Rule '2550m-s3a-001' 'page navigation' 1 'Schedule 3A date/description is blank or allowable input tax compares equal to zero.' @('txtDatePurchased3A{N>=0}','txtDescription3A{N>=0}','txtAllowInputTax3A{N>=0}') 'All checks pass.' 'Row is rejected.' 'Please enter valid row {row} data for Schedule 3A.\nEmpty fields are not allowed.' @('official-hta-runtime#checkifEmptyFieldSched3:L4419-L4428') 'official-bug-compatible' 'Computed allowable tax is treated as required.' 'Validate source fields, then compute allowable tax deterministically.'
Add-Rule '2550m-s3a-002' 'page navigation' 2 'Schedule 3A date is malformed or outside the return period.' @('txtDatePurchased3A{N>=0}','frm2550m:RtnYear','frm2550m:txtYear') 'Legacy branches pass.' 'Row is rejected.' 'Invalid entry on row {row}. Date of Purchase must be within the Return Period only.' @('official-hta-runtime#checkifEmptyFieldSched3:L4434-L4494') 'incorrect-official-behavior' 'The same February/lower-bound date defects as Schedule 2 apply.' 'Use strict calendar and period comparison.'
Add-Rule '2550m-s3a-003' 'page navigation' 3 'Schedule 3A amount is <= 0.' @('txtAmt3A{N>=0}') 'Amount > 0.' 'Row is rejected.' 'Please enter amount for Net of VAT on row {row}.\nValue must be greater than 0.' @('official-hta-runtime#checkifEmptyFieldSched3:L4500-L4503') 'verified-correct' 'Positive amount required.' 'Retain.'
Add-Rule '2550m-s3a-004' 'page navigation' 4 'Schedule 3A estimated life is outside 1..999.' @('txtEstLife3A{N>=0}') 'Value 1..999.' 'Row is rejected.' 'Please enter a value 1 to 999 for Estimated Life on row {row} Schedule 3Part A.' @('official-hta-runtime#checkifEmptyFieldSched3:L4504-L4507') 'official-bug-compatible' 'Bounds are correct; message lacks spacing.' 'Retain bounds and fix presentation.'
Add-Rule '2550m-s3a-005' 'page navigation' 5 'Schedule 3A recognized life is outside 1..60.' @('txtRecogLife3A{N>=0}') 'Value 1..60.' 'Row is rejected.' 'Please enter a value 1 to 60 for Recognized Life on row {row}Part A.' @('official-hta-runtime#checkifEmptyFieldSched3:L4508-L4511') 'official-bug-compatible' 'Bounds are enforced; message lacks spacing.' 'Retain bounds and fix presentation.'
Add-Rule '2550m-s3a-006' 'page navigation' 6 'Schedule 3A is committed in AandB mode with rows and total <= 1,000,000.' @('txtmodalTotalAmountSched3') 'Total > 1,000,000.' 'Schedule is rejected.' 'The total aggregate amount does not exceed 1 Million.\n Please re-enter the values of Schedule 3.' @('official-hta-runtime#checkifEmptyFieldSched3:L4514-L4522') 'verified-correct' 'Schedule 3A is reserved for aggregate acquisitions above one million.' 'Retain with exact decimal comparison.'
Add-Rule '2550m-s3b-001' 'page navigation' 1 'Schedule 3B row has blank date or description.' @('txtDatePurchased3B{N>=0}','txtDescription3B{N>=0}') 'Both are nonblank.' 'Row is rejected.' "If you don't have any entries for Schedule 3b,\nplease delete the row on this schedule if not applicable." @('official-hta-runtime#checkifEmptyFieldSched3:L4524-L4531') 'verified-correct' 'Blank placeholder row must be deleted.' 'Allow an explicit empty schedule or require complete rows.'
Add-Rule '2550m-s3b-002' 'page navigation' 2 'Schedule 3B date is malformed, future, or in the current return month.' @('txtDatePurchased3B{N>=0}','frm2550m:RtnYear','frm2550m:txtYear') 'Prior-period date passes.' 'Row is rejected.' 'Invalid entry on row {row}. Date of Purchase must be within the Return Period only.' @('official-hta-runtime#checkifEmptyFieldSched3:L4532-L4607') 'incorrect-official-behavior' 'The comparison rejects current-month dates but the message says within return period; February and lower-bound defects remain.' 'Require the legally intended prior-period acquisition range with a correct message.'
Add-Rule '2550m-s3b-003' 'page navigation' 3 'Schedule 3B amount is <= 0.' @('txtAmt3B{N>=0}') 'Amount > 0.' 'Row is rejected.' 'Please enter amount for Net of VAT on row {row}.\nValue must be greater than 0 in Part B.' @('official-hta-runtime#checkifEmptyFieldSched3:L4610-L4613') 'verified-correct' 'Positive amount required.' 'Retain.'
Add-Rule '2550m-s3b-004' 'page navigation' 4 'Schedule 3B estimated life is outside 1..999.' @('txtEstLife3B{N>=0}') 'Value 1..999.' 'Row is rejected.' 'Please enter a value 1 to 999 for Estimated Life on row {row} Schedule 3Part B.' @('official-hta-runtime#checkifEmptyFieldSched3:L4614-L4617') 'official-bug-compatible' 'Bounds enforced; message lacks spacing.' 'Retain bounds and fix message.'
Add-Rule '2550m-s3b-005' 'page navigation' 5 'Schedule 3B recognized life is outside 1..60.' @('txtRecogLife3B{N>=0}') 'Value 1..60.' 'Row is rejected.' 'Please enter a value 1 to 60 for Recognized Life on row {row}Part B.' @('official-hta-runtime#checkifEmptyFieldSched3:L4618-L4621') 'official-bug-compatible' 'Bounds enforced; message lacks spacing.' 'Retain bounds and fix message.'
Add-Rule '2550m-s4-001' 'page navigation' $null 'Schedule 4 is opened when Item 13A <= 0.' @('frm2550m:txtTax13A') 'Positive Item 13A opens it.' 'Schedule is blocked.' 'Please enter a valid value on Item 13A to be able to load the Schedule 4.' @('official-hta-runtime#showSched4:L5158-L5196') 'obsolete' 'Legacy Schedule 4 remains active in this revision.' 'For periods from January 2021 follow RMC 36-2021: deactivate Item 20B/Schedule 4 and use Item 23C/Schedule 8.'
Add-Rule '2550m-s4-002' 'page navigation' $null 'Schedule 4 OK is clicked with incomplete/invalid values.' @('Schedule4') 'Always accepted.' 'No rejection occurs.' $null @('official-hta-runtime#checkifEmptyFieldSched4:L5212-L5214') 'incorrect-official-behavior' 'Validation function unconditionally returns true.' 'Validate denominators and nonnegative inputs when the legacy schedule is applicable.'
Add-Rule '2550m-s5-001' 'page navigation' $null 'Schedule 5 is opened when Item 15 <= 0.' @('frm2550m:txtTax15') 'Positive Item 15 opens it.' 'Schedule is blocked.' 'Please enter a valid value on Item 15 to be able to load the Schedule 5.' @('official-hta-runtime#showSched5:L5272-L5305') 'verified-correct' 'Exempt-sales schedule requires exempt sales.' 'Retain dependency.'
Add-Rule '2550m-s5-002' 'page navigation' $null 'Schedule 5 OK is clicked with incomplete/invalid values.' @('Schedule5') 'Always accepted.' 'No rejection occurs.' $null @('official-hta-runtime#checkifEmptyFieldSched5:L5321-L5323') 'incorrect-official-behavior' 'Validation function unconditionally returns true.' 'Validate denominator and nonnegative inputs.'

foreach ($s in @('6','8')) {
    $prefix = if($s-eq'6'){''}else{'Sch8'}
    $base = if($s-eq'6'){5384}else{6139}
    Add-Rule "2550m-s$s-001" 'page navigation' 1 "A Schedule $s row has blank period covered or agent name." @("txtPeriodCovered$prefix{N>=0}","txtNameAgent$prefix{N>=0}") 'Both are nonblank.' 'Row is rejected.' "Please enter valid row {row} data for Schedule $s.\nEmpty fields are not allowed." @("official-hta-runtime#checkifEmptyFieldSched$s:L$base") 'verified-correct' 'Required row text is checked.' 'Retain.'
    Add-Rule "2550m-s$s-002" 'page navigation' 2 "A Schedule $s period date is malformed or outside the return period." @("txtPeriodCovered$prefix{N>=0}") 'Legacy date branches pass.' 'Row is rejected using a copied purchase-date message.' 'Please enter a valid date for Date of Purchase on row {row}.\nPlease enter a date in the MM/DD/YYYY format.' @("official-hta-runtime#checkifEmptyFieldSched$s") 'incorrect-official-behavior' 'Copied message mislabels Period Covered as Date of Purchase and shares the February date bug.' 'Use strict date parsing and the correct field label.'
    foreach ($amount in @(@('IncomePayment','Income Payment'),@('TotalWithheld','Total Tax Withheld'),@('AppliedCurr','Applied Current Month'))) {
        $name = "$($amount[0])$prefix{N>=0}"
        Add-Rule "2550m-s$s-00$([array]::IndexOf(@('IncomePayment','TotalWithheld','AppliedCurr'),$amount[0])+3)" 'page navigation' $null "$($amount[1]) is <= 0." @($name) 'Amount > 0.' 'Row is rejected.' "Please enter a valid amount for $($amount[1]) on row {row}.\nValue must be greater than 0." @("official-hta-runtime#checkifEmptyFieldSched$s") 'verified-correct' 'Positive amount required.' 'Retain.'
    }
}
Add-Rule '2550m-s6-006' 'page navigation' 6 'Schedule 6 applied amount exceeds total withheld.' @('txtAppliedCurr{N>=0}','txtTotalWithheld{N>=0}') 'Applied <= withheld.' 'Row is rejected.' 'The Applied Current Month on row {row} should not be greater than the Total Tax Withheld.' @('official-hta-runtime#checkifEmptyFieldSched6:L5469-L5471') 'verified-correct' 'Comparison is active.' 'Retain with decimal arithmetic.'
Add-Rule '2550m-s7-001' 'page navigation' 1 'Schedule 7 row has a blank period, miller, taxpayer, or OR number.' @('txtPeriodCoveredSch7{N>=0}','txtNameMillerSch7{N>=0}','txtNameTaxPayerSch7{N>=0}','txtORNumSch7{N>=0}') 'All are nonblank.' 'Row is rejected.' 'Please enter valid row {row} data for Schedule 7.\nEmpty fields are not allowed.' @('official-hta-runtime#checkifEmptyFieldSched7:L5764-L5773') 'verified-correct' 'Required row text is checked.' 'Retain.'
Add-Rule '2550m-s7-002' 'page navigation' 2 'Schedule 7 period date is malformed or outside return period.' @('txtPeriodCoveredSch7{N>=0}') 'Legacy branches pass.' 'Row is rejected using a copied purchase-date message.' 'Please enter a valid date for Date of Purchase on row {row}.\nPlease enter a date in the MM/DD/YYYY format.' @('official-hta-runtime#checkifEmptyFieldSched7:L5773-L5833') 'incorrect-official-behavior' 'Copied label and February bug.' 'Use strict parsing and correct label.'
Add-Rule '2550m-s7-003' 'page navigation' 3 'Schedule 7 amount paid is <= 0.' @('txtAmountPaidSch7{N>=0}') 'Amount > 0.' 'Row rejected.' 'Please enter a valid amount for Amount Paid on row {row}.\nValue must be greater than 0.' @('official-hta-runtime#checkifEmptyFieldSched7:L5839-L5842') 'verified-correct' 'Positive amount required.' 'Retain.'
Add-Rule '2550m-s7-004' 'page navigation' 4 'Schedule 7 applied amount is <= 0.' @('txtAppliedCurrSch7{N>=0}') 'Amount > 0.' 'Row rejected.' 'Please enter a valid amount for Applied Current Month on row {row}.\nValue must be greater than 0.' @('official-hta-runtime#checkifEmptyFieldSched7:L5843-L5846') 'verified-correct' 'Positive amount required.' 'Retain.'
Add-Rule '2550m-s7-005' 'page navigation' 5 'Schedule 7 applied amount exceeds amount paid.' @('txtAppliedCurrSch7{N>=0}','txtAmountPaidSch7{N>=0}') 'Any positive applied amount passes.' 'No comparison occurs because the block is commented out; its message also says Total Tax Withheld.' $null @('official-hta-runtime#checkifEmptyFieldSched7:L5847-L5849') 'incorrect-official-behavior' 'Over-application is accepted.' 'Require applied <= amount paid and use the correct label.'
Add-Rule '2550m-s8-006' 'page navigation' 6 'Schedule 8 applied amount exceeds total withheld.' @('txtAppliedCurrSch8{N>=0}','txtTotalWithheldSch8{N>=0}') 'Any positive applied amount passes.' 'No comparison occurs because the block is commented out.' $null @('official-hta-runtime#checkifEmptyFieldSched8:L6225-L6227') 'incorrect-official-behavior' 'Over-application is accepted.' 'Require applied <= total withheld.'
Add-Rule '2550m-s8-007' 'page navigation' $null 'Schedule 8 is opened when Item 13A <= 0.' @('frm2550m:txtTax13A') 'Positive Item 13A opens it.' 'Schedule is blocked.' 'Please enter a valid value on Item 13A to be able to load the Schedule 8.' @('official-hta-runtime#showSched8') 'ambiguous' 'The withholding schedule is coupled to government sales.' 'Retain only if confirmed by the applicable instructions.' 'medium'

$order=0
foreach ($v in @(
    @('2550m-validate-001','Item 11 Yes is selected and classification is 0.',@('frm2550m:OptSpecialTax1','frm2550m:lstSpecialTax'),'Please select an option on item no. 11. Entry must not be empty.','official-hta-runtime#validate:L6734-L6742','verified-correct'),
    @('2550m-validate-002','Year is blank.',@('frm2550m:txtYear'),'Please indicate a valid Year.','official-hta-runtime#validate:L6743-L6747','verified-correct'),
    @('2550m-validate-003','Year is in the future, or current-year month is in the future.',@('frm2550m:txtYear','frm2550m:RtnYear'),$null,'official-hta-runtime#validate:L6748-L6757','incorrect-official-behavior'),
    @('2550m-validate-004','Year is below 2000.',@('frm2550m:txtYear'),'Invalid date entry on Item no.1. Entry should not be lower than 2000.','official-hta-runtime#validate:L6758-L6762','verified-correct'),
    @('2550m-validate-005','Any TIN segment or branch code is blank.',@('frm2550m:txtTIN1','frm2550m:txtTIN2','frm2550m:txtTIN3','frm2550m:txtBranchCode'),'Please enter a valid TIN number on Item 4.','official-hta-runtime#validate:L6763-L6767','incorrect-official-behavior'),
    @('2550m-validate-006','RDO selectedIndex is zero.',@('frm2550m:txtRDOCode'),'Please enter a valid RDO Code on Item 5.','official-hta-runtime#validate:L6768-L6772','verified-correct'),
    @('2550m-validate-007','Line of business is blank.',@('frm2550m:txtLineBus'),'Please enter a valid Line of Business on Item 6.','official-hta-runtime#validate:L6773-L6777','verified-correct'),
    @('2550m-validate-008','Taxpayer name is blank.',@('frm2550m:txtTaxPayerName'),'Please enter a valid Taxpayer Name on Item 7.','official-hta-runtime#validate:L6778-L6782','verified-correct'),
    @('2550m-validate-009','Telephone number is blank.',@('frm2550m:txtTelephoneNum'),"Please enter Taxpayer's telephone number on Item 8.",'official-hta-runtime#validate:L6783-L6787','verified-correct'),
    @('2550m-validate-010','Registered address is blank.',@('frm2550m:txtAddress'),"Please enter Taxpayer's Registered Address on Item 9.",'official-hta-runtime#validate:L6788-L6792','verified-correct'),
    @('2550m-validate-011','ZIP code is blank.',@('frm2550m:txtZipCode'),"Please enter Taxpayer's Zip code on Item 10.",'official-hta-runtime#validate:L6793-L6797','verified-correct')
)) {
    $order++
    $official = if($v[0]-eq'2550m-validate-003'){'The entire future-period branch is commented out; the invalid period passes.'}elseif($v[0]-eq'2550m-validate-005'){'Only nonblankness is checked; length, characters, and checksum are not.'}else{'The first matching condition alerts and returns.'}
    $recommended = if($v[0]-eq'2550m-validate-003'){'Reject future periods using an explicit filing-period policy.'}elseif($v[0]-eq'2550m-validate-005'){'Validate exact segment lengths, digits, branch code, and TIN checksum.'}else{'Retain with structured field errors.'}
    Add-Rule $v[0] 'validate' $order $v[1] $v[2] 'Condition is false and validation continues.' $official $v[3] @($v[4]) $v[5] $official $recommended
}
Add-Rule '2550m-validate-012' 'validate' 12 'Month, amended choice, tax amounts, schedule consistency, arithmetic, or email is missing/malformed.' @('return-body') 'Validate still succeeds when the eleven active checks pass.' 'No rejection occurs.' 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validate:L6734-L6800') 'incorrect-official-behavior' 'Validate omits these material fields and calculations.' 'Validate all applicable inputs and recompute authoritative amounts before locking.'
Add-Rule '2550m-save-001' 'save' 1 'Any TIN segment or branch is blank.' @('frm2550m:txtTIN1','frm2550m:txtTIN2','frm2550m:txtTIN3','frm2550m:txtBranchCode') 'All are nonblank.' 'Save is blocked.' 'Please enter a valid TIN number on Item 4.' @('official-hta-runtime#initialValidateBeforeSave:L6974-L6979') 'incorrect-official-behavior' 'Only nonblankness is checked.' 'Use lossless draft saving; separately report invalid TIN shape.'
Add-Rule '2550m-save-002' 'save' 2 'RDO value is 000.' @('frm2550m:txtRDOCode') 'Value differs from 000.' 'Save is blocked.' 'Please enter a valid RDO Code on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L6980-L6983') 'verified-correct' 'Placeholder RDO is rejected.' 'Permit draft preservation while preventing finalization.'
Add-Rule '2550m-save-003' 'save' 3 'Taxpayer name is blank.' @('frm2550m:txtTaxPayerName') 'Name is nonblank.' 'Save is blocked.' 'Please enter a valid Taxpayer Name on Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L6984-L6988') 'official-bug-compatible' 'Draft save requires name.' 'Preserve drafts even when incomplete.'
Add-Rule '2550m-save-004' 'save' 4 'Year/month, address, phone, ZIP, line of business, special tax, return values, or schedules are invalid.' @('return-body') 'Save proceeds if only TIN/RDO/name preflight passes.' 'No rejection occurs.' $null @('official-hta-runtime#initialValidateBeforeSave:L6974-L6990','official-hta-runtime#saveXML:L3010-L3305') 'incorrect-official-behavior' 'Save preflight is substantially narrower than Validate.' 'Save losslessly, surface completeness separately, and never discard unknown fields.'
Add-Rule '2550m-final-001' 'final-copy' 1 'Final Copy is requested.' @('txtFinalFlag') 'Confirmation/network path proceeds.' 'Offline fallback creates an encrypted copy and changes workflow state.' $null @('official-hta-runtime#finalcopy:L7515-L7581') 'official-bug-compatible' 'Final Copy is coupled to submission confirmation and connectivity.' 'Offer a deterministic offline final copy independent of online transport.'
Add-Rule '2550m-submit-001' 'submit' 1 'Submission email/body is prepared.' @('return-body') 'Payload is prepared.' 'VAT form is mislabeled as Income Tax and Income Tax Return.' $null @('official-hta-runtime#sendEmail:L7587-L7673') 'incorrect-official-behavior' 'Copied transport labels identify the wrong tax type.' 'Identify Form 2550M as a VAT return.'

Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    first_error_behavior='Main Validate and each schedule validator alert and return on the first active failing branch; schedule validators run only when their modal OK action is used.'
    rules=$rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Add-Calc([string]$Id,[string[]]$Outputs,[string[]]$Inputs,[string]$Formula,[string]$Trigger,[string[]]$Depends,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Use decimal arithmetic and recompute from authoritative inputs.') {
    $calculations.Add([pscustomobject][ordered]@{calculation_id=$Id;outputs=$Outputs;inputs=$Inputs;condition=$null;official_formula=$Formula;rounding='formatCurrency/JavaScript number semantics unless noted.';trigger=$Trigger;depends_on=$Depends;source_refs=$Refs;assessment=$Assessment;recommended_app_behavior=$Recommended;confidence='high'})
}
Add-Calc '2550m-s1-row-output-tax' @('frm2550m:txtOutputTaxN') @('frm2550m:txtAmountSalesN') 'output tax = sales/receipts * 0.12' 'getRequiredWithheld' @() @('official-hta-runtime#getRequiredWithheld:L3947-L3950')
Add-Calc '2550m-s1-totals' @('frm2550M:txtmodaltxtTotal12A','frm2550M:txtmodaltxtTotal12B') @('all Schedule 1 rows') 'sum sales and sum output tax independently' 'totalAmountandOutputTax' @('2550m-s1-row-output-tax') @('official-hta-runtime#totalAmountandOutputTax:L3978-L3989')
Add-Calc '2550m-item13b' @('frm2550m:txtTax13B') @('frm2550m:txtTax13A') '13B = 13A * 0.12' 'compute13B' @() @('official-hta-runtime#compute13B:L6528-L6531')
Add-Calc '2550m-item16a' @('frm2550m:txtTax16A') @('frm2550m:txtTax12A','frm2550m:txtTax13A','frm2550m:txtTax14','frm2550m:txtTax15') '16A = 12A + 13A + 14 + 15' 'compute16A' @() @('official-hta-runtime#compute16A:L6533-L6537')
Add-Calc '2550m-item16b' @('frm2550m:txtTax16B') @('frm2550m:txtTax12B','frm2550m:txtTax13B') '16B = 12B + 13B' 'compute16B' @('2550m-item13b') @('official-hta-runtime#compute16B:L6539-L6543')
Add-Calc '2550m-item17f' @('frm2550m:txtTax17F') @('frm2550m:txtTax17A','frm2550m:txtTax17B','frm2550m:txtTax17C','frm2550m:txtTax17D','frm2550m:txtTax17E') '17F = 17A + 17B + 17C + 17D + 17E' 'compute17F' @() @('official-hta-runtime#compute17F:L6545-L6549')
Add-Calc '2550m-s2-row' @('txtInputTax{N>=0}') @('txtAmt{N>=0}') 'input tax = net-of-VAT amount * 0.12' 'getInputTaxCompute' @() @('official-hta-runtime#getInputTaxCompute:L6552-L6557')
Add-Calc '2550m-s2-totals' @('txtmodalTotalAmt','txtmodalTotalInputTax') @('Schedule 2 rows') 'sum net amounts and input tax' 'getInputTaxCompute' @('2550m-s2-row') @('official-hta-runtime#getInputTaxCompute:L6552-L6568')
Add-Calc '2550m-s3a-row' @('txtInputTax3A{N>=0}','txtAllowInputTax3A{N>=0}','txtBalInputTax3A{N>=0}') @('txtAmt3A{N>=0}','txtRecogLife3A{N>=0}') 'input = amount * 0.12; allowable = input / recognized life; balance = input - allowable' 'getInputTaxCompute3A' @() @('official-hta-runtime#getInputTaxCompute3A:L6570-L6583')
Add-Calc '2550m-s3a-totals' @('txtmodalTotalAmountSched3','txtmodalTotalInputTaxSched3','txtmodalTotalBalanceSched3A') @('Schedule 3A rows') 'sum amount, allowable input tax, and balance columns' 'getInputTaxCompute3A' @('2550m-s3a-row') @('official-hta-runtime#getInputTaxCompute3A:L6570-L6594')
Add-Calc '2550m-s3b-row' @('txtAllowInputTax3B{N>=0}','txtBalInputTax3B{N>=0}') @('txtBalInputTaxPrevious3B{N>=0}','txtRecogLife3B{N>=0}') 'allowable = prior balance / recognized life; balance = prior balance - allowable' 'getInputTaxCompute3B' @() @('official-hta-runtime#getInputTaxCompute3B:L6596-L6607')
Add-Calc '2550m-s3b-totals' @('txtmodalTotalBalanceSched3B') @('Schedule 3B rows') 'sum remaining balance from all Schedule 3B rows' 'getInputTaxCompute3B' @('2550m-s3b-row') @('official-hta-runtime#getInputTaxCompute3B:L6596-L6607')
Add-Calc '2550m-s3-total' @('txtmodalTotalInputTax20ASched3') @('txtmodalTotalBalanceSched3A','txtmodalTotalBalanceSched3B') 'Schedule 3 total = Part A balance + Part B balance' 'computeSumTax3B' @('2550m-s3a-totals','2550m-s3b-totals') @('official-hta-runtime#computeSumTax3B:L6645-L6647')
Add-Calc '2550m-s6-totals' @('txtmodalTotal23A','txtmodalTotalSched6AppliedCurrent') @('txtTotalWithheld{N>=0}','txtAppliedCurr{N>=0}') 'sum total withheld and applied-current amount independently' 'Schedule 6 row change handlers' @() @('official-hta-runtime#schedule6-totals:L6609-L6619')
Add-Calc '2550m-s7-totals' @('txtmodalTotal23B','txtmodalTotalSched7AppliedCurrent') @('txtAmountPaidSch7{N>=0}','txtAppliedCurrSch7{N>=0}') 'sum amount paid and applied-current amount independently' 'Schedule 7 row change handlers' @() @('official-hta-runtime#schedule7-totals:L6621-L6631')
Add-Calc '2550m-s8-totals' @('txtmodalTotal23C','txtmodalTotalSched8AppliedCurrent') @('txtTotalWithheldSch8{N>=0}','txtAppliedCurrSch8{N>=0}') 'sum total withheld and applied-current amount independently' 'Schedule 8 row change handlers' @() @('official-hta-runtime#schedule8-totals:L6633-L6643')
Add-Calc '2550m-item18-paired-output-tax' @('frm2550m:txtTax18F','frm2550m:txtTax18H','frm2550m:txtTax18J','frm2550m:txtTax18L','frm2550m:txtTax18O') @('frm2550m:txtTax18E','frm2550m:txtTax18G','frm2550m:txtTax18I','frm2550m:txtTax18K','frm2550m:txtTax18N') 'each paired output-tax field = its base * 0.12' 'change handlers' @() @('official-hta-runtime#item18-pairs:L6649-L6664')
Add-Calc '2550m-item18p' @('frm2550m:txtTax18P') @('frm2550m:txtTax18A','frm2550m:txtTax18C','frm2550m:txtTax18E','frm2550m:txtTax18G','frm2550m:txtTax18I','frm2550m:txtTax18K','frm2550m:txtTax18M','frm2550m:txtTax18N') '18P = 18A + 18C + 18E + 18G + 18I + 18K + 18M + 18N' 'compute18P' @() @('official-hta-runtime#compute18P:L6666-L6672') 'ambiguous' 'Preserve the official formula but verify Item 18 column semantics against the PDF before implementation.'
Add-Calc '2550m-item19' @('frm2550m:txtTax19') @('frm2550m:txtTax17F','frm2550m:txtTax18B','frm2550m:txtTax18D','frm2550m:txtTax18F','frm2550m:txtTax18H','frm2550m:txtTax18J','frm2550m:txtTax18L','frm2550m:txtTax18O') '19 = 17F + 18B + 18D + 18F + 18H + 18J + 18L + 18O' 'compute19' @('2550m-item17f','2550m-item18-paired-output-tax') @('official-hta-runtime#compute19:L6674-L6679')
Add-Calc '2550m-s4-allocation' @('txtTotalInputTaxnotDirectSched4','txtTotalInputSaletoGovernmentSched4','txtTotal20BSched4') @('txtTaxableSaleSched4','txtTotalSaleSched4','txtInputTaxnotDirectSched4','txtinputtaxSched4','txtlessStdTaxSched4') 'allocated = taxable-government-sales / total-sales * non-direct input tax; total = direct + allocated; result = total - standard input tax' 'Schedule 4 change handlers' @('2550m-item16a') @('official-hta-runtime#schedule4:L6681-L6690') 'obsolete' 'Deactivate for periods from January 2021 under RMC 36-2021; retain only for historical compatibility.'
Add-Calc '2550m-s4-standard-input' @('txtlessStdTaxSched4') @('frm2550m:txtTax13A') 'standard input tax = 13A * 0.07 (despite static default 5.00)' 'changedTxtTax13A' @() @('official-hta-runtime#changedTxtTax13A:L6688-L6690') 'incorrect-official-behavior' 'Do not infer a 5% or 7% rule outside the exact legacy revision; Schedule 4 is deactivated for periods from January 2021.'
Add-Calc '2550m-s5-allocation' @('txtProductnotDirectSched5','txtTotal20CSched5') @('txtTotSaleSched5','txtSumTotalSaleSched5','txtAmountInputnotDirectSched5','txtinputtaxSched5') 'allocated = exempt sales / total sales * non-direct input tax; total = direct + allocated' 'Schedule 5 change handlers' @('2550m-item16a') @('official-hta-runtime#schedule5:L6692-L6695')
Add-Calc '2550m-item20f' @('frm2550m:txtTax20F') @('frm2550m:txtTax20A','frm2550m:txtTax20B','frm2550m:txtTax20C','frm2550m:txtTax20D','frm2550m:txtTax20E') '20F = 20A + 20B + 20C + 20D + 20E' 'compute20F' @() @('official-hta-runtime#compute20F:L6697-L6701')
Add-Calc '2550m-item21' @('frm2550m:txtTax21') @('frm2550m:txtTax19','frm2550m:txtTax20F') '21 = 19 - 20F' 'compute21' @('2550m-item19','2550m-item20f') @('official-hta-runtime#compute21:L6703-L6706')
Add-Calc '2550m-item22' @('frm2550m:txtTax22') @('frm2550m:txtTax16B','frm2550m:txtTax21') '22 = 16B - 21' 'compute22' @('2550m-item16b','2550m-item21') @('official-hta-runtime#compute22:L6708-L6711')
Add-Calc '2550m-item23g' @('frm2550m:txtTax23G') @('frm2550m:txtTax23A','frm2550m:txtTax23B','frm2550m:txtTax23C','frm2550m:txtTax23D','frm2550m:txtTax23E','frm2550m:txtTax23F') '23G = 23A + 23B + 23C + 23D + 23E + 23F' 'compute23G' @() @('official-hta-runtime#compute23G:L6713-L6717') 'official-bug-compatible' 'Use the six-term formula but correct the stale printed label that says Sum A-E.'
Add-Calc '2550m-item25d' @('frm2550m:txtTax25D') @('frm2550m:txtTax25A','frm2550m:txtTax25B','frm2550m:txtTax25C') '25D = 25A + 25B + 25C' 'compute25D' @() @('official-hta-runtime#compute25D:L6720-L6723')
Add-Calc '2550m-item24' @('frm2550m:txtTax24') @('frm2550m:txtTax22','frm2550m:txtTax23G') '24 = 22 - 23G' 'compute24' @('2550m-item22','2550m-item23g') @('official-hta-runtime#compute24:L6725-L6728') 'official-bug-compatible' 'Use 23G as the source formula does; correct the printed Item 24 label that says Item 23F.'
Add-Calc '2550m-item26' @('frm2550m:txtTax26') @('frm2550m:txtTax24','frm2550m:txtTax25D') '26 = 24 + 25D' 'compute26' @('2550m-item24','2550m-item25d') @('official-hta-runtime#compute26:L6729-L6731')

$evaluationOrder = @($calculations | ForEach-Object calculation_id)
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=$evaluationOrder;calculations=$calculations})

$negativeRules = @('2550m-modal-001','2550m-s1-001','2550m-s2-001','2550m-s2-002','2550m-s2-003','2550m-s2-004','2550m-s2-005','2550m-s2-006','2550m-s3a-001','2550m-s3a-004','2550m-s3a-005','2550m-s3a-006','2550m-s3b-001','2550m-s3b-002','2550m-s4-002','2550m-s5-002','2550m-s6-006','2550m-s7-005','2550m-s8-006','2550m-validate-001','2550m-validate-002','2550m-validate-003','2550m-validate-004','2550m-validate-005','2550m-validate-006','2550m-validate-008','2550m-save-004','2550m-submit-001')
$negativeCases = @()
$caseNumber=0
foreach ($ruleId in $negativeRules) {
    $caseNumber++
    $rule = $rules | Where-Object rule_id -eq $ruleId
    $negativeCases += [pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}' -f $caseNumber,$ruleId);phase=$rule.phase;mutations=@{synthetic_condition=$rule.condition};expected_message=$rule.exact_message;expected_behavior=$rule.official_behavior;rule_id=$ruleId}
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$negativeCases})

$calcCases = foreach ($calc in $calculations) { [pscustomobject][ordered]@{case_id="$($calc.calculation_id)-source-boundary";calculation_id=$calc.calculation_id;inputs=@{source_formula=$calc.official_formula};official_output='Derived by the pinned formula; executable numeric vectors remain a documented gap.'} }
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=$calcCases})

$workflow = [ordered]@{
    '$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    phases=@(
        @{phase='edit';official_behavior='One main return plus eight schedule modals; Schedules 2, 3A, 3B, 6, 7, and 8 have unbounded add-row arrays.';source_refs=@('official-hta-runtime#addlistSched2/addlistSched3A/addlistSched3B/addlistSched6/addlistSched7/addlistSched8');confidence='high'},
        @{phase='saved-draft';official_behavior='Save runs only TIN-nonblank, RDO-not-000, and taxpayer-name checks, then writes flat HTML-control state.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L6974-L6990','official-hta-runtime#saveXML:L3010-L3305');confidence='high'},
        @{phase='validated';official_behavior='Eleven ordered checks run; on success all controls are disabled and an alert announces success.';source_refs=@('official-hta-runtime#validate:L6734-L6800','official-hta-runtime#disabledAllControl:L6805-L6872');confidence='high'},
        @{phase='final-copy';official_behavior='Final Copy is coupled to confirmation/connectivity and creates encrypted workflow artifacts.';source_refs=@('official-hta-runtime#finalcopy:L7515-L7581');confidence='high'},
        @{phase='submitted';official_behavior='Transport prepares a submission whose copied labels incorrectly call the VAT return an income-tax return.';source_refs=@('official-hta-runtime#sendEmail:L7587-L7673');confidence='high'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='TIN segments/branch nonblank, RDO != 000, taxpayer name nonblank.';side_effects=@('Writes a plaintext local save.');source_refs=@('official-hta-runtime#initialValidateBeforeSave:L6974-L6990')},
        @{from='edit';action='Validate';to='validated';guard='Eleven active ordered checks pass.';side_effects=@('Disables controls.','Enables post-validation actions.');source_refs=@('official-hta-runtime#validate:L6734-L6800')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables editable controls.');source_refs=@('official-hta-runtime#enableAllControl:L6873-L6946')},
        @{from='validated';action='Final Copy';to='final-copy';guard='User confirms the coupled workflow.';side_effects=@('Creates encrypted copy and changes txtFinalFlag according to transport state.');source_refs=@('official-hta-runtime#finalcopy:L7515-L7581')},
        @{from='final-copy';action='Online transport';to='submitted';guard='Connectivity and transport proceed.';side_effects=@('Prepares/sends encrypted artifact; no online submission was exercised for this research.');source_refs=@('official-hta-runtime#sendEmail:L7587-L7673')}
    )
    prerequisites=@('Exact February 2007 ENCS revision','Complete applicable background fields','Complete applicable schedules','For post-2020 periods apply later legal changes instead of blindly reproducing obsolete Schedule 4 behavior')
    required_attachments=@(
        @{attachment_id='vat-withheld-certificate';label='Certificate of Creditable VAT Withheld at Source';required_when='Creditable VAT was withheld.';official_ui_enforcement='Not enforced by local Validate.';source_refs=@('official-installed-help#attachments:L205-L213');confidence='high'},
        @{attachment_id='sawt';label='Summary Alphalist of Withholding Taxes (SAWT)';required_when='Creditable VAT withheld at source is claimed.';official_ui_enforcement='Not enforced by local Validate.';source_refs=@('official-installed-help#attachments:L205-L213');confidence='high'},
        @{attachment_id='tax-debit-memo';label='Tax Debit Memo';required_when='Tax debit is claimed.';official_ui_enforcement='Not enforced by local Validate.';source_refs=@('official-installed-help#attachments:L205-L213');confidence='high'},
        @{attachment_id='tax-compliance-certificate';label='Tax Compliance Certificate';required_when='Applicable to the claim/filing.';official_ui_enforcement='Not enforced by local Validate.';source_refs=@('official-installed-help#attachments:L205-L213');confidence='medium'},
        @{attachment_id='authorization';label='Authorization letter';required_when='Filed by an authorized representative.';official_ui_enforcement='Not enforced by local Validate.';source_refs=@('official-installed-help#attachments:L205-L213');confidence='high'}
    )
    filing_deadlines=@()
}
foreach ($quarter in @('Q1','Q2','Q3','Q4')) { $workflow.filing_deadlines += @{quarter=$quarter;due_date_rule='Legacy February 2007 help says not later than the 20th day following the month. For periods beginning January 2023, RMC 5-2023 removed mandatory monthly filing and RMC 52-2023 permits optional monthly filing with no prescribed deadline.';source_refs=@('official-installed-help#filing-deadline','official-rmc-5-2023','official-rmc-52-2023');confidence='high'} }
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @($rules | Where-Object assessment -in @('incorrect-official-behavior','official-bug-compatible','obsolete')).Count
$manifest = [ordered]@{
    '$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='2550M';revision=$revision;revision_label='February 2007 ENCS';package_version=$packageVersion;status='complete'
    official_assets=@(
        @{asset_id='package-7.9.6';kind='official-package-executable';path='C:\eBIRForms\BIRForms.exe';sha256='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca';size=57506304},
        @{asset_id='official-hta-runtime';kind='runtime-extracted-hta';path=$HtaPath;sha256='72f1422dab2f8523d140aa51fe5f54f7d9025acc2cf877b37c8d92b60c7668b5';size=439121;revision_binding='Printed header and application identifiers bind February 2007 ENCS Form 2550M.'},
        @{asset_id='official-installed-help';kind='runtime-help';path=$HelpPath;sha256='9c4102f6377023e4dd302538f2d365c868db7a2a8f2d54ae6b4de45350f2fdcc';size=19070;revision_binding='Installed legacy 2550M help; later circulars are recorded separately.'},
        @{asset_id='official-pdf-2007';kind='official-form-pdf';path='C:\Mac\Home\Downloads\forms\2550M\bir2550m.pdf';sha256='9fb4101ace8c781436dac85df138a8fb9790775291affe2dada030c490d0d2b6';size=147791;revision_binding='February 2007 ENCS printed form.'},
        @{asset_id='xml-editable-v1';kind='dummy-profile-editable-save';path=$SavePath;sha256='ed563de9e6ffbf2226d87c54e09205ccb04221b9b333a307bf52e2a86b9c61c5';size=9477;revision_binding='Dummy 2550M serialization; values excluded from artifacts.'},
        @{asset_id='xml-amended-v1';kind='dummy-profile-editable-save';path='C:\eBIRForms\savefile\00000000000000-2550M-072026V1.xml';sha256='679710010803f73781340e0708367d66ea4f05203528c8f681a7534cd7b4a116';size=9485;revision_binding='Dummy amended/version save with the same 142-key set.'},
        @{asset_id='atc-catalog-runtime';kind='official-package-xml';path=$AtcPath;sha256='16e4db6ce456a6fb0a97f085cf8ef19349c2f6fc183971e97d4e253f63cdd22b';size=153753;revision_binding='Catalog loaded by the exact HTA.'},
        @{asset_id='official-rmc-36-2021';kind='official-circular-pdf';path='C:\Users\uriah\.codex\visualizations\2026\07\22\019f8b9e-3f1f-72c2-bf81-4294cc208f76\RMC-36-2021.pdf';sha256='644d9f85a0d978154ed0ac7ca70b43457cb34c19fe90568df4715200df4625a6';size=120740;revision_binding='Later legal change: deactivate Item 20B/Schedule 4 from January 2021 and use Item 23C/Schedule 8.'},
        @{asset_id='official-rmc-5-2023';kind='official-circular-pdf';path='C:\Users\uriah\.codex\visualizations\2026\07\22\019f8b9e-3f1f-72c2-bf81-4294cc208f76\RMC-5-2023.pdf';sha256='0f154fd3ade2c592254353fa84b0959787235ac00946a8dc6cdf778c8f1a5581';size=12323;revision_binding='Later filing change beginning January 2023.'},
        @{asset_id='official-rmc-52-2023';kind='official-circular-pdf';path='C:\Users\uriah\.codex\visualizations\2026\07\22\019f8b9e-3f1f-72c2-bf81-4294cc208f76\RMC-52-2023.pdf';sha256='7199bc39cdc3a9f0d37d5140b1257c66c2af8f863a8bfd4db46764c5b2a50398';size=5904;revision_binding='Clarifies optional monthly filing has no prescribed deadline.'}
    )
    counts=@{typed_fields=$fields.Count;validation_rules=$rules.Count;calculations=$calculations.Count;negative_fixtures=$negativeCases.Count;atc_records_for_form=$atcEntries.Count;confirmed_official_bugs=$bugCount;unverified_gaps=2}
    artifacts=@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';runtime_control_fixture='fixtures/runtime-control-inventory-v796.json';atc_catalog_fixture='fixtures/atc-catalog-v796.json';calculation_fixtures='fixtures/calculation-boundaries.json'}
    scope_notes=@('Research artifacts only; no renderer, migration, release, or capability changes.','Representative XML values and email address are not copied.','No online submission or mutation of official save/encrypted artifacts was performed.','The runtime field universe is unbounded; 292 entries comprise 250 bounded concrete keys and 42 explicit unbounded family descriptors.')
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest

$evidence = @"
# Evidence — 2550M February 2007 ENCS

The exact runtime HTA, installed help, official PDF, two dummy plaintext saves, shared ATC catalog, and later official BIR circulars are pinned in `manifest.json`. The representative save has 142 unique keys; both save variants have the same key set. Static source inspection found 186 controls (182 with IDs). The bounded concrete serialization universe is 250 keys after adding 36 Schedule 1 ATC-code/sales/output-tax triplets. Six add-row schedules have no maximum guard, so `fields.json` adds 42 explicit `{N>=0}` family descriptors instead of inventing a finite count.

The source line references bind to HTA SHA-256 `72f1422dab2f8523d140aa51fe5f54f7d9025acc2cf877b37c8d92b60c7668b5`. ATC evidence binds to catalog SHA-256 `16e4db6ce456a6fb0a97f085cf8ef19349c2f6fc183971e97d4e253f63cdd22b`; 36 records identify 2550M/2550Q and carry no rate.

Later-law evidence is intentionally separate from the 2007 implementation: RMC 36-2021 deactivates Item 20B/Schedule 4 for periods from January 2021; RMC 5-2023 removes mandatory monthly filing; RMC 52-2023 permits optional monthly filing without a prescribed deadline. All three downloaded files have `%PDF-` magic and pinned SHA-256 hashes.

Authoritative URLs (retrieved 2026-07-23):

- https://efps.bir.gov.ph/efps-war/EFPSWeb_war/forms/2550M/2550m_v3.xhtml
- https://efps.bir.gov.ph/efps-war/EFPSWeb_war/help/proc2550m2006.html
- https://efps.bir.gov.ph/efps-war/EFPSWeb_war/help/help2550m2006.html
- https://bir-cdn.bir.gov.ph/local/pdf/RMC%20No.%2036-2021.pdf
- https://bir-cdn.bir.gov.ph/local/pdf/RMC%20No.%205-2023.pdf
- https://bir-cdn.bir.gov.ph/local/pdf/RMC%20No.%2052-2023.pdf
"@
[IO.File]::WriteAllText((Join-Path $outDir 'evidence.md'),$evidence,[Text.UTF8Encoding]::new($false))

$gaps = @"
# Gaps — 2550M February 2007 ENCS

1. No destructive black-box mutation of the representative save and no online submission was performed. Exact source branches/messages are high confidence; transport-only behavior remains source-derived.
2. The six add-row schedules are genuinely unbounded. The 42 family descriptors preserve this fact, but a finite exhaustive list of concrete indexed keys is impossible by construction. Printed page coordinates for modal-only controls remain null where the source does not bind them unambiguously.
"@
[IO.File]::WriteAllText((Join-Path $outDir 'gaps.md'),$gaps,[Text.UTF8Encoding]::new($false))

$audit = @"
# Audit — 2550M February 2007 ENCS

- Revision pinned to February 2007 ENCS through runtime header/application identity and the official PDF.
- HTA, help, PDF, plaintext saves, ATC catalog, package executable, and three later BIR circulars are hashed.
- Inventory: 142 observed save keys; 250 bounded concrete keys; 42 unbounded indexed families; 292 unique entries total.
- Runtime controls: 186 static controls, 182 with IDs, four without IDs.
- ATC catalog: 36 revision-applicable records; hard-coded 12% output-tax computation recorded separately.
- Main Validate, Save preflight, all eight schedule modals, calculations, final-copy, and transport code inspected.
- First-error ordering is recorded for main Validate and row validators.
- Dummy-only negative cases bind to rule IDs; no real taxpayer data is stored.
- Confirmed defects include the commented future-period validation, nonblank-only TIN validation, February date bypasses, no-op Schedule 1/4/5 validation, disabled Schedule 7/8 over-application checks, stale Item 23/24 labels, obsolete Schedule 4 behavior, and wrong Income Tax transport labels.
- Later legal behavior is not merged into the legacy revision; compatibility and recommended current behavior are both retained.

Run `rtk powershell -NoProfile -ExecutionPolicy Bypass -File '\\mac\goldcoders\reverse-engineer-ebir-forms\bir-print-parity\rules\validate.ps1' -RequireJsonSchema` after updating the index.
"@
[IO.File]::WriteAllText((Join-Path $outDir 'audit.md'),$audit,[Text.UTF8Encoding]::new($false))

$readme = @"
# BIR Form 2550M — February 2007 ENCS

Revision-specific validation knowledge for the legacy Offline eBIRForms 2550M runtime. The package distinguishes exact 2007 application behavior from later RMC 36-2021, RMC 5-2023, and RMC 52-2023 changes. It contains 292 inventory entries, including explicit unbounded indexed families; it does not claim a finite runtime field count.

Research only. No renderer or release metadata is changed, no online submission was performed, and representative values are excluded.
"@
[IO.File]::WriteAllText((Join-Path $outDir 'README.md'),$readme,[Text.UTF8Encoding]::new($false))

$handoff = @"
# Handoff

- Completed: 2550M February 2007 ENCS (`2550m-v2007`)
- Inventory: 292 entries (250 bounded concrete + 42 unbounded families)
- Rules: $($rules.Count)
- Calculations: $($calculations.Count)
- Negative fixtures: $($negativeCases.Count)
- ATC records: $($atcEntries.Count)
- Next priority: 1701-MS
"@
[IO.File]::WriteAllText((Join-Path $outDir 'HANDOFF.md'),$handoff,[Text.UTF8Encoding]::new($false))

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -LiteralPath $indexPath -Raw | ConvertFrom-Json
if (-not ($index.forms | Where-Object form_id -eq $formId)) {
    $index.forms += [pscustomobject][ordered]@{form_id=$formId;form_code='2550M';revision=$revision;package_version=$packageVersion;priority=8;status='complete';path='forms/2550m-v2007/manifest.json'}
}
$index.updated = '2026-07-23'
Write-Json $indexPath $index

[pscustomobject]@{form_id=$formId;fields=$fields.Count;validations=$rules.Count;calculations=$calculations.Count;negative_fixtures=$negativeCases.Count;atc_records=$atcEntries.Count;official_defect_classifications=$bugCount;output=$outDir} | ConvertTo-Json
