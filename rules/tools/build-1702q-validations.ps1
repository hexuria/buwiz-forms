param([Parameter(Mandatory = $true)][string]$OutputPath)

$ErrorActionPreference = 'Stop'
$form = '1702q-v2018c'
$revision = '2018-01-01'
$rules = @()

function Add-Rule {
    param(
        [string]$Id,[string]$Phase,[Nullable[int]]$Order,[string]$Condition,[string[]]$Fields,
        [AllowNull()][string]$Message,[string[]]$Sources,[string[]]$Evidence,[string]$Assessment,
        [string]$Official,[string]$Recommended,[string]$Confidence='high',[string[]]$Questions=@(),
        [string]$Accepted='Condition is false and processing continues.',
        [string]$Rejected='Official processing applies the documented rejection or mutation.'
    )
    $script:rules += [ordered]@{
        rule_id=$Id; form_id=$form; revision=$revision; phase=$Phase; order=$Order; condition=$Condition; fields=@($Fields);
        accepted_behavior=$Accepted; rejected_behavior=$Rejected; exact_message=$Message; source_refs=@($Sources);
        evidence_type=@($Evidence); assessment=$Assessment; official_behavior=$Official; recommended_app_behavior=$Recommended;
        confidence=$Confidence; unresolved_questions=@($Questions)
    }
}

Add-Rule '1702q-input-001' 'blur/change' $null 'A monetary field handled by blockletter is blank, nonnumeric, or negative on blur.' @('money-fields') $null @('official-hta-runtime#blockletter:L5986-L6000') @('source') 'official-bug-compatible' 'Nonnumeric and negative values are silently replaced with 0.00; numeric values are rounded with binary parseFloat().toFixed(2).' 'Reject malformed or negative values explicitly and use decimal arithmetic.'
Add-Rule '1702q-input-002' 'blur/change' $null 'A whole-number field handled by blockletterWithout2Decimal loses focus.' @('whole-number-fields') $null @('official-hta-runtime#blockletterWithout2Decimal:L6002-L6012') @('source') 'official-bug-compatible' 'Nonnumeric input is cleared; numeric input is rounded with toFixed(0).' 'Reject invalid values explicitly and document rounding.'
Add-Rule '1702q-input-003' 'blur/change' $null 'A payment date is malformed or later than the current date.' @('payment-date-fields') 'Please provide a valid date. (MM/DD/YYYY format)' @('official-hta-runtime#validateDate:L6014-L6080') @('source') 'verified-correct' 'Malformed dates are cleared with the format alert; future dates are cleared with `This date cannot be a future date.`' 'Apply calendar-valid MM/DD/YYYY parsing and the same future-date boundary.'
Add-Rule '1702q-input-004' 'input' $null 'A numeric key filter receives a second decimal point or malformed pasted value.' @('money-fields') $null @('shared-string-util#numbersonly:L148-L174','official-hta-runtime#blockletter:L5986-L6000') @('source') 'official-bug-compatible' 'The package permits multiple dots during key entry; blur later coerces malformed text to 0.00.' 'Permit at most one decimal separator and validate pasted/programmatic values.'

Add-Rule '1702q-change-001' 'blur/change' $null 'Calendar-year mode is selected.' @('frm1702q:rbForClndrFscl_1','frm1702q:rbYrEndMonth','frm1702q:txtYrEndYear') $null @('official-hta-runtime#checkFilingYear:L8040-L8075') @('source') 'verified-correct' 'Month is forced to 12 and disabled; the two-digit year is cleared.' 'Apply the same dependency transactionally.'
Add-Rule '1702q-change-002' 'blur/change' $null 'Fiscal-year mode is selected.' @('frm1702q:rbForClndrFscl_2','frm1702q:rbYrEndMonth','frm1702q:txtYrEndYear') $null @('official-hta-runtime#checkFilingYear:L8040-L8075') @('source') 'verified-correct' 'Month is cleared and enabled; the two-digit year is cleared.' 'Apply the same dependency transactionally.'
Add-Rule '1702q-change-003' 'blur/change' $null 'validateYear runs while no quarter is selected.' @('frm1702q:rbQuarter_1','frm1702q:rbQuarter_2','frm1702q:rbQuarter_3') $null @('official-hta-runtime#validateYear:L7907-L7938') @('source') 'incorrect-official-behavior' 'The function returns silently before applying year-range checks.' 'Report the missing quarter and validate year independently of quarter selection.'
Add-Rule '1702q-change-004' 'blur/change' $null 'validateYear detects a future fiscal quarter.' @('frm1702q:rbForClndrFscl_2','frm1702q:rbYrEndMonth','frm1702q:txtYrEndYear','quarter-selection') $null @('official-hta-runtime#validateYear:L7907-L8032') @('source') 'incorrect-official-behavior' 'The change handler returns silently, whereas final Validate shows `Future filing is not allowed.` for the same boundary.' 'Use one shared predicate and one explicit message.'
Add-Rule '1702q-change-005' 'blur/change' $null 'validateFiscalMonth compares the four-digit input year with Date.getYear().' @('frm1702q:rbYrEndMonth','frm1702q:txtYrEndYear') 'Date (Page 1 Item 2) cannot be greater than current date when filing for Fiscal Year.' @('official-hta-runtime#validateFiscalMonth:L8129-L8142') @('source') 'incorrect-official-behavior' 'Date.getYear() returns years since 1900, so ordinary four-digit input years always satisfy inputYear > currentYear; current/future-month logic is applied under the wrong year comparison.' 'Use getFullYear() and compare a complete year-month value.'
Add-Rule '1702q-change-006' 'blur/change' $null 'Other-ATC checkbox is cleared.' @('frm1702q:rbATC_2','frm1702q:cbATC_2') $null @('official-hta-runtime#rbATC2_Selected:L8148-L8155') @('source') 'verified-correct' 'The ATC select is cleared and disabled.' 'Apply the same dependency.'
Add-Rule '1702q-change-007' 'blur/change' $null 'An ATC selection changes.' @('frm1702q:cbATC_2','schedule-fields') $null @('official-hta-runtime#ATCEnableDisableSchedules:L8157-L8235') @('source') 'verified-correct' 'Schedule 1A-only, Schedule 1B-only, or combined Schedules 1-4 are enabled according to the exact ATC switch.' 'Represent the switch as revision-scoped ATC metadata.'
Add-Rule '1702q-change-008' 'blur/change' $null 'A rate-bearing ATC is selected for any August, or for July 2020.' @('frm1702q:cbATC_2','frm1702q:Sched2:txtTax10') $null @('official-hta-runtime#Sched2Item10TaxRate:L9021-L9080') @('source') 'incorrect-official-behavior' 'Because `CreateYear == 2020 && CreateMonth == 07 || CreateMonth == 08` lacks parentheses, every August in every year enables a manual 0.00 rate.' 'Parenthesize the 2020 month condition and bind rates to effective periods.'
Add-Rule '1702q-change-009' 'blur/change' $null 'Amended Return No is selected for a combined regular-rate ATC.' @('frm1702q:rbAmendedRtn_2','frm1702q:Sched4:txtTax6') $null @('official-hta-runtime#ProcessAmended:L6629-L6681') @('source') 'verified-correct' 'Schedule 4 Item 6 is reset to 0.00 and disabled.' 'Apply the same dependency.'

$validateOrder = 0
function Add-Validate {
    param([string]$Id,[string]$Condition,[string[]]$Fields,[AllowNull()][string]$Message,[string[]]$Sources,[string]$Assessment='verified-correct',[string]$Official='The ordered Validate branch rejects the condition.',[string]$Recommended='Apply the same ordered validation.',[string[]]$Questions=@())
    $script:validateOrder++
    Add-Rule $Id 'validate' $script:validateOrder $Condition $Fields $Message $Sources @('source') $Assessment $Official $Recommended 'high' $Questions
}

Add-Validate '1702q-validate-001' 'Two-digit year is blank or nonnumeric.' @('frm1702q:txtYrEndYear') 'Invalid year entered. Please provide a valid year.' @('official-hta-runtime#validate:L6089-L6096')
Add-Validate '1702q-validate-002' 'Fiscal/calendar year-end month is blank or nonnumeric.' @('frm1702q:rbYrEndMonth') 'Please provide a valid fiscal year-end month.' @('official-hta-runtime#validate:L6100-L6107')
Add-Validate '1702q-validate-003' 'No quarter is selected during the preliminary selectedQuarter branch.' @('frm1702q:rbQuarter_1','frm1702q:rbQuarter_2','frm1702q:rbQuarter_3') $null @('official-hta-runtime#validate:L6111-L6122') 'incorrect-official-behavior' 'Validate returns silently before reaching its later explicit missing-quarter alert.' 'Show `Please select Quarter in Item 3.` and do not silently return.'
Add-Validate '1702q-validate-004' 'Calendar-year input is earlier than 2018.' @('frm1702q:rbForClndrFscl_1','frm1702q:txtYrEndYear') 'Year (Page 1 Item 2) should not be earlier than January 2018.' @('official-hta-runtime#validate:L6126-L6130')
Add-Validate '1702q-validate-005' 'Calendar-year input is later than the current year.' @('frm1702q:rbForClndrFscl_1','frm1702q:txtYrEndYear') 'Year (Page 1 Item 2) cannot be greater than the current year for Calendar Year.' @('official-hta-runtime#validate:L6131-L6135')
Add-Validate '1702q-validate-006' 'Fiscal-year input is earlier than 2018.' @('frm1702q:rbForClndrFscl_2','frm1702q:txtYrEndYear') 'Year (Page 1 Item 2) should not be earlier than January 2018.' @('official-hta-runtime#validate:L6139-L6143')
Add-Validate '1702q-validate-007' 'Selected fiscal quarter end is later than the current date.' @('frm1702q:rbForClndrFscl_2','frm1702q:rbYrEndMonth','frm1702q:txtYrEndYear','quarter-selection') 'Future filing is not allowed.' @('official-hta-runtime#validate:L6144-L6210')
Add-Validate '1702q-validate-008' 'Month or year is blank after the preliminary checks.' @('frm1702q:rbYrEndMonth','frm1702q:txtYrEndYear') 'Please enter a valid Date on Item 2.' @('official-hta-runtime#validate:L6217-L6220') 'obsolete' 'This branch is unreachable for blank/nonnumeric values because earlier checks already return.' 'Consolidate date validation into one reachable rule.'
Add-Validate '1702q-validate-009' 'No quarter is selected at the explicit Item 3 check.' @('quarter-selection') 'Please select Quarter in Item 3.' @('official-hta-runtime#validate:L6222-L6225') 'obsolete' 'This message is unreachable because the preliminary selectedQuarter branch returns silently first.' 'Move this message to the preliminary branch.'
Add-Validate '1702q-validate-010' 'Neither MCIT nor other ATC checkbox is selected.' @('frm1702q:rbATC_1','frm1702q:rbATC_2') 'Please select ATC on Item 5.' @('official-hta-runtime#validate:L6228-L6231')
Add-Validate '1702q-validate-011' 'Other ATC is checked but the ATC select is blank.' @('frm1702q:rbATC_2','frm1702q:cbATC_2') 'Please select ATC on Item 5' @('official-hta-runtime#validate:L6233-L6238')
Add-Validate '1702q-validate-012' 'Any TIN core segment or branch code is blank.' @('frm1702q:txtTIN1','frm1702q:txtTIN2','frm1702q:txtTIN3','frm1702q:txtBranchCode') 'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#validate:L6241-L6245')
Add-Validate '1702q-validate-013' 'A TIN core segment is not exactly three characters or branch code is shorter than three.' @('frm1702q:txtTIN1','frm1702q:txtTIN2','frm1702q:txtTIN3','frm1702q:txtBranchCode') 'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#validate:L6247-L6250') 'official-bug-compatible' 'Length is checked but no TIN checksum is invoked; branch length 3-5 is accepted.' 'Validate the official TIN checksum and exact revision-appropriate branch shape.'
Add-Validate '1702q-validate-014' 'RDO value loosely equals numeric zero.' @('frm1702q:txtRDOCode') 'Please enter a valid RDO Code on Item 7.' @('official-hta-runtime#validate:L6252-L6255')
Add-Validate '1702q-validate-015' 'Taxpayer name is blank.' @('frm1702q:txtTaxpayerName1') 'Please enter a valid Taxpayer Name on Item 8.' @('official-hta-runtime#validate:L6257-L6260')
Add-Validate '1702q-validate-016' 'Registered address is blank.' @('frm1702q:txtAddress') "Please enter Taxpayer's Registered Address on Item 9." @('official-hta-runtime#validate:L6262-L6265')
Add-Validate '1702q-validate-017' 'ZIP code is blank.' @('frm1702q:txtZipCode') 'Please enter Zip Code on Item 9A.' @('official-hta-runtime#validate:L6267-L6270')
Add-Validate '1702q-validate-018' 'First DOM occurrence of contact number is blank.' @('frm1702q:txtTelNum#occurrence-1') 'Please enter Contact Number on Item 10.' @('official-hta-runtime#validate:L6272-L6275')
Add-Validate '1702q-validate-019' 'Neither deduction method is selected.' @('frm1702q:rbMthdOfDdctns_1','frm1702q:rbMthdOfDdctns_2') 'Please select Method of Deductions on Item 12.' @('official-hta-runtime#validate:L6277-L6280')
Add-Validate '1702q-validate-020' 'Tax relief Yes is selected but Item 13A is blank.' @('frm1702q:rbTxRlf_1','frm1702q:txtTxRlfSpcfy') 'Please specify Special Law/International Tax Treaty on Item 13A.' @('official-hta-runtime#validate:L6282-L6288')
Add-Validate '1702q-validate-021' 'A Schedule 4 Item 6 description is nonblank but its amount equals exactly 0.00.' @('frm1702q:Sched4:txtOthrTxCrdts0','frm1702q:Sched4:txtOthrTxCrdtAmnt0','frm1702q:Sched4:txtOthrTxCrdts1','frm1702q:Sched4:txtOthrTxCrdtAmnt1') 'Please input Other Tax Credits/Payments amount on Schedule 4 Item 6' @('official-hta-runtime#validate:L6290-L6295') 'official-bug-compatible' 'Only exact string 0.00 is rejected; equivalent zero spellings can bypass the rule.' 'Compare parsed decimal value to zero.'
Add-Validate '1702q-validate-022' 'A Schedule 4 Item 6 description is blank but its amount is not exactly 0.00.' @('frm1702q:Sched4:txtOthrTxCrdts0','frm1702q:Sched4:txtOthrTxCrdtAmnt0','frm1702q:Sched4:txtOthrTxCrdts1','frm1702q:Sched4:txtOthrTxCrdtAmnt1') 'Please specify Other Tax Credits/Payments on Schedule 4 Item 6' @('official-hta-runtime#validate:L6298-L6303') 'official-bug-compatible' 'The check is string-based rather than decimal-value-based.' 'Compare parsed decimal value and normalized description.'

Add-Rule '1702q-save-001' 'save' 1 'No quarter is selected.' @('quarter-selection') 'Please select quarter in Item 3.' @('official-hta-runtime#initialValidateBeforeSave:L6731-L6735') @('source') 'verified-correct' 'Save stops before serialization.' 'Apply the same preflight and preserve the exact distinction from full Validate.'
Add-Rule '1702q-save-002' 'save' 2 'A TIN core segment or branch code is blank.' @('frm1702q:txtTIN1','frm1702q:txtTIN2','frm1702q:txtTIN3','frm1702q:txtBranchCode') 'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L6736-L6740') @('source') 'official-bug-compatible' 'Save checks only nonblank TIN parts, not length or checksum.' 'Reuse full identity validation before saving.'
Add-Rule '1702q-save-003' 'save' 3 'RDO code is blank.' @('frm1702q:txtRDOCode') 'Please enter a valid RDO Code on Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L6741-L6745') @('source') 'official-bug-compatible' 'Save rejects blank but not placeholder numeric zero, unlike Validate.' 'Use the same RDO predicate for Save and Validate.'
Add-Rule '1702q-save-004' 'save' 4 'Taxpayer name is blank.' @('frm1702q:txtTaxpayerName1') 'Please enter a valid Taxpayer Name on Item 8.' @('official-hta-runtime#initialValidateBeforeSave:L6746-L6750') @('source') 'verified-correct' 'Save stops before serialization.' 'Apply the same preflight.'

$output = [ordered]@{
    '$schema'='../../schema/validations.schema.json'; schema_version='1.0.0'; form_id=$form; revision=$revision;
    first_error_behavior='Validate and Save stop at the first ordered rejection. Validate contains an earlier silent no-quarter return that makes its later Item 3 alert unreachable.';
    rules=$rules
}
$json=$output|ConvertTo-Json -Depth 12
[IO.File]::WriteAllText($OutputPath,$json+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
[pscustomobject]@{form_id=$form;rules=$rules.Count;validate_rules=@($rules|Where-Object phase -eq 'validate').Count}|ConvertTo-Json
