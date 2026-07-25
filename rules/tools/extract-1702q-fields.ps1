param(
    [Parameter(Mandatory = $true)][string]$ControlInventoryPath,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$inventory = Get-Content -Raw -LiteralPath $ControlInventoryPath | ConvertFrom-Json

function Get-Page([string]$id) {
    if ($id -match ':Sched4:') { return 3 }
    if ($id -match ':Sched[123]:' -or $id -match ':txtPg2') { return 2 }
    if ($id -match '^(resultOtherSpecify|txtFinalFlag|txtEnroll|ebirOnline|frm1702q:txtLOB|driveSelectTPExport)$') { return $null }
    return 1
}

function Get-ItemNumber([string]$id) {
    $map = @{
        'frm1702q:rbForClndrFscl_1'='1'; 'frm1702q:rbForClndrFscl_2'='1';
        'frm1702q:rbYrEndMonth'='2'; 'frm1702q:txtYrEndYear'='2';
        'frm1702q:rbQuarter_1'='3'; 'frm1702q:rbQuarter_2'='3'; 'frm1702q:rbQuarter_3'='3';
        'frm1702q:rbAmendedRtn_1'='4'; 'frm1702q:rbAmendedRtn_2'='4';
        'frm1702q:txtATC_1'='5'; 'frm1702q:rbATC_1'='5'; 'frm1702q:cbATC_2'='5'; 'frm1702q:rbATC_2'='5';
        'frm1702q:txtTIN1'='6'; 'frm1702q:txtTIN2'='6'; 'frm1702q:txtTIN3'='6'; 'frm1702q:txtBranchCode'='6';
        'frm1702q:txtRDOCode'='7'; 'frm1702q:txtTaxpayerName1'='8'; 'frm1702q:txtAddress'='9';
        'frm1702q:txtZipCode'='9A'; 'frm1702q:txtTelNum'='10'; 'txtEmail'='11';
        'frm1702q:rbMthdOfDdctns_1'='12'; 'frm1702q:rbMthdOfDdctns_2'='12';
        'frm1702q:rbTxRlf_1'='13'; 'frm1702q:rbTxRlf_2'='13'; 'frm1702q:txtTxRlfSpcfy'='13A';
        'frm1702q:txtSheets'='26'
    }
    if ($map.ContainsKey($id)) { return $map[$id] }
    if ($id -match '^frm1702q:txtTax(1[4-9]|2[0-5])$') { return $Matches[1] }
    if ($id -match '^frm1702q:Sched([1-4]):txtTax(\d+[AB]?)$') { return "Schedule $($Matches[1]) Item $($Matches[2])" }
    if ($id -match '^frm1702q:Sched4:(?:chkOthrTxCrdts|txtOthrTxCrdts|txtOthrTxCrdtAmnt)(\d+)$') { return "Schedule 4 Item 6 row $([int]$Matches[1] + 1)" }
    return $null
}

function Get-Label([string]$id, [int]$occurrence) {
    $map = @{
        'frm1702q:rbForClndrFscl_1'='Calendar year'; 'frm1702q:rbForClndrFscl_2'='Fiscal year';
        'frm1702q:rbYrEndMonth'='Taxable year-end month'; 'frm1702q:txtYrEndYear'='Taxable year-end two-digit year';
        'frm1702q:rbQuarter_1'='First quarter'; 'frm1702q:rbQuarter_2'='Second quarter'; 'frm1702q:rbQuarter_3'='Third quarter';
        'frm1702q:rbAmendedRtn_1'='Amended return Yes'; 'frm1702q:rbAmendedRtn_2'='Amended return No';
        'frm1702q:txtATC_1'='IC055 Minimum Corporate Income Tax label'; 'frm1702q:rbATC_1'='Select IC055 MCIT';
        'frm1702q:cbATC_2'='Other corporate income-tax ATC'; 'frm1702q:rbATC_2'='Select other ATC';
        'frm1702q:txtTIN1'='TIN first segment'; 'frm1702q:txtTIN2'='TIN second segment'; 'frm1702q:txtTIN3'='TIN third segment';
        'frm1702q:txtBranchCode'='TIN branch code'; 'frm1702q:txtRDOCode'='RDO code';
        'frm1702q:txtTaxpayerName1'='Taxpayer registered name'; 'frm1702q:txtAddress'='Registered address';
        'frm1702q:txtZipCode'='ZIP code'; 'txtEmail'='Email address';
        'frm1702q:rbMthdOfDdctns_1'='Itemized deductions'; 'frm1702q:rbMthdOfDdctns_2'='Optional Standard Deduction';
        'frm1702q:rbTxRlf_1'='Tax relief Yes'; 'frm1702q:rbTxRlf_2'='Tax relief No'; 'frm1702q:txtTxRlfSpcfy'='Special law or tax treaty';
        'frm1702q:txtSheets'='Number of attachments';
        'frm1702q:txtPg2TIN1'='Page 2 TIN first segment'; 'frm1702q:txtPg2TIN2'='Page 2 TIN second segment';
        'frm1702q:txtPg2TIN3'='Page 2 TIN third segment'; 'frm1702q:txtPg2BranchCode'='Page 2 TIN branch code';
        'frm1702q:txtPg2TaxpayerName'='Page 2 taxpayer name'; 'frm1702q:txtCurrentPage'='Current page';
        'frm1702q:txtMaxPage'='Maximum page'; 'resultOtherSpecify'='Schedule 4 various-details total';
        'txtFinalFlag'='Final-copy state flag'; 'txtEnroll'='Online enrollment state';
        'ebirOnlineConfirmUsername'='Online confirmation username'; 'ebirOnlineUsername'='Online username';
        'ebirOnlineSecret'='Online encrypted password'; 'frm1702q:txtLOB'='Hidden profile line of business';
        'driveSelectTPExport'='Final-copy export drive'
    }
    if ($id -eq 'frm1702q:txtTelNum') {
        if ($occurrence -eq 1) { return 'Contact number' }
        return 'Hidden duplicate profile contact number'
    }
    if ($map.ContainsKey($id)) { return $map[$id] }
    if ($id -match '^frm1702q:txtTax(\d+)$') { return "Part II Item $($Matches[1]) amount" }
    if ($id -match '^frm1702q:Sched(\d):txtTax(\d+[AB]?)$') { return "Schedule $($Matches[1]) Item $($Matches[2])" }
    if ($id -match '^frm1702q:Sched4:chkOthrTxCrdts(\d+)$') { return "Schedule 4 other-credit row $([int]$Matches[1] + 1) selection" }
    if ($id -match '^frm1702q:Sched4:txtOthrTxCrdts(\d+)$') { return "Schedule 4 other-credit row $([int]$Matches[1] + 1) description" }
    if ($id -match '^frm1702q:Sched4:txtOthrTxCrdtAmnt(\d+)$') { return "Schedule 4 other-credit row $([int]$Matches[1] + 1) amount" }
    return "Runtime field $id"
}

function Test-Computed([string]$id) {
    if ($id -match '^frm1702q:txtTax(14|16|17|18|19|20|24|25)$') { return $true }
    if ($id -match '^frm1702q:Sched1:txtTax(3A|3B|5A|5B|7A|7B|9A|9B|11A|11B|13B)$') { return $true }
    if ($id -match '^frm1702q:Sched2:txtTax(3|5|7|9|11|12|13)$') { return $true }
    if ($id -match '^frm1702q:Sched3:txtTax(4|6)$') { return $true }
    if ($id -match '^frm1702q:Sched4:txtTax7$' -or $id -eq 'resultOtherSpecify') { return $true }
    return $false
}

function Get-Required([string]$id, [bool]$computed, [int]$occurrence) {
    if ($computed) { return 'computed' }
    if ($id -in @('frm1702q:rbYrEndMonth','frm1702q:txtYrEndYear','frm1702q:txtTIN1','frm1702q:txtTIN2','frm1702q:txtTIN3','frm1702q:txtBranchCode','frm1702q:txtRDOCode','frm1702q:txtTaxpayerName1','frm1702q:txtAddress','frm1702q:txtZipCode','frm1702q:txtTelNum') -and -not ($id -eq 'frm1702q:txtTelNum' -and $occurrence -eq 2)) { return 'required' }
    if ($id -match 'rbQuarter_|rbATC_|rbMthdOfDdctns_') { return 'required' }
    if ($id -eq 'frm1702q:cbATC_2' -or $id -eq 'frm1702q:txtTxRlfSpcfy' -or $id -match 'Sched4:txtOthr') { return 'conditional' }
    if ($id -match '^(txtFinalFlag|txtEnroll|ebirOnline|resultOtherSpecify|frm1702q:txtLOB|driveSelectTPExport)' -or ($id -eq 'frm1702q:txtTelNum' -and $occurrence -eq 2)) { return 'hidden' }
    return 'optional'
}

$fields = @()
for ($i = 0; $i -lt $inventory.controls.Count; $i++) {
    $control = $inventory.controls[$i]
    $saveEntry = $inventory.representative_save[$i]
    if ($control.serialized_key -cne $saveEntry.serialized_key) { throw "Control/save sequence mismatch at ordinal $($i + 1)" }
    $id = $control.serialized_key
    $computed = Test-Computed $id
    $required = Get-Required $id $computed $control.occurrence
    $logicalType = if ($id -match 'Sched1:txtTax10[AB]$|Sched2:txtTax10$|Sched3:txtTax5$') { 'percentage' } elseif ($id -match 'txtTax|CrdtAmnt|resultOtherSpecify') { 'money' } elseif ($control.control_type -in @('radio','checkbox')) { 'boolean' } else { 'text' }
    $enumValues = @()
    if ($control.control_type -in @('radio','checkbox')) {
        $enumValues += [ordered]@{ html_value = $control.value_attribute; serialized_selected = 'true'; serialized_unselected = 'false' }
    }
    $normalization = @()
    if ($control.control_type -in @('radio','checkbox')) { $normalization += 'Serialize checked state as lowercase true/false.' }
    else { $normalization += 'Serialize with JavaScript escape().' }
    $fields += [ordered]@{
        field_key = $control.field_key
        serialized_key = $id
        serialized_occurrence = [int]$control.occurrence
        label = Get-Label $id $control.occurrence
        page = Get-Page $id
        item_number = Get-ItemNumber $id
        control_kind = $control.control_type
        storage_type = if ($control.control_type -in @('radio','checkbox')) { 'boolean-string' } else { 'string' }
        logical_type = $logicalType
        required = $required
        required_when = if ($required -eq 'conditional') { 'See validations.json and workflow.json for the exact official dependency.' } else { $null }
        enabled_when = if ($control.disabled_attribute) { 'Enabled only by the official dependency/state transition recorded in validations.json or workflow.json.' } else { $null }
        visible_when = if ($required -eq 'hidden') { 'Internal workflow/profile/export control; not a printed filing field.' } else { $null }
        default_value = $saveEntry.serialized_value
        empty_representation = ''
        constraints = [ordered]@{ maxlength = $control.maxlength; duplicate_serialized_key_occurrences = [int]$control.occurrence_count }
        enum_values = $enumValues
        normalization = $normalization
        computed = $computed
        calculation_id = $null
        source_refs = @("official-hta-runtime#L$($control.source_line)", "representative-save#element:$($i + 1)")
        confidence = 'high'
        notes = @($(if ($control.occurrence_count -gt 1) { "The official DOM and XML contain $($control.occurrence_count) elements with serialized key $id; field_key disambiguates this occurrence." }))
    }
}

$output = [ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = '1702q-v2018c'
    revision = '2018-01-01'
    field_count = $fields.Count
    runtime_serializable_element_count = $inventory.runtime_serializable_element_count
    inventory_sha256 = $inventory.ordered_serialized_keys_sha256
    fields = $fields
}
$json = $output | ConvertTo-Json -Depth 14
[IO.File]::WriteAllText($OutputPath, $json + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
[pscustomobject]@{ form_id = $output.form_id; fields = $fields.Count; inventory_sha256 = $output.inventory_sha256 } | ConvertTo-Json
