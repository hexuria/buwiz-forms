param(
    [Parameter(Mandatory = $true)][string]$HtaPath,
    [Parameter(Mandatory = $true)][string]$SavePath,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$hta = [IO.File]::ReadAllText($HtaPath)
$save = [IO.File]::ReadAllText($SavePath)

function Get-Attribute([string]$tag, [string]$name) {
    $pattern = '(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($name)
    $match = [regex]::Match($tag, $pattern)
    if ($match.Success) { return $match.Groups[2].Value }
    if ([regex]::IsMatch($tag, "(?i)\b$([regex]::Escape($name))\b")) { return 'true' }
    return $null
}

function Get-Item([string]$key) {
    $map = @{
        'frm1701q:txtYear'='1'; 'frm1701q:DateQuarter_1'='2'; 'frm1701q:DateQuarter_2'='2'; 'frm1701q:DateQuarter_3'='2';
        'frm1701q:AmendedRtn_1'='3'; 'frm1701q:AmendedRtn_2'='3'; 'frm1701q:txtSheets'='4';
        'frm1701q:txtTIN1'='5'; 'frm1701q:txtTIN2'='5'; 'frm1701q:txtTIN3'='5'; 'frm1701q:txtBranchCode'='5';
        'frm1701q:txtRDOCode'='6'; 'frm1701q:txtTaxpayerName'='9'; 'frm1701q:txtAddress'='10'; 'frm1701q:txtZipCode'='10A';
        'frm1701q:txtBirthMonth'='11'; 'frm1701q:txtBirthDay'='11'; 'frm1701q:txtBirthYear'='11'; 'txtEmail'='12';
        'frm1701q:txtCitizenship'='13'; 'frm1701q:txtForeignTaxNumber'='14';
        'frm1701q:txtSpouseTIN1'='17'; 'frm1701q:txtSpouseTIN2'='17'; 'frm1701q:txtSpouseTIN3'='17'; 'frm1701q:txtSpouseBranchCode'='17';
        'frm1701q:txtSpouseRDOCode'='18'; 'frm1701q:txtSpouseName'='21'; 'frm1701q:txtSpouseCitizenship'='22'; 'frm1701q:txtSpouseForeignTaxNum'='23';
        
    }
    if ($map.ContainsKey($key)) { return $map[$key] }
    if ($key -match ':optForeignTaxCredits_') { return '15' }
    if ($key -match ':optTaxRate_') { return '16' }
    if ($key -match ':optMethodOfDeduction:') { return '16A' }
    if ($key -match ':optType_') { return '7' }
    if ($key -match ':optATC_') { return '8' }
    if ($key -match ':optSpouseForeignTaxCred_') { return '24' }
    if ($key -match ':optSpouseTaxRate_') { return '25' }
    if ($key -match ':optSpouseMethod:') { return '25A' }
    if ($key -match ':optSpouseType_') { return '19' }
    if ($key -match ':optSpouseATC_') { return '20' }
    if ($key -match ':txt(?:Agency|Number|Date|Amount|Particular)(3[2-5])$') { return $Matches[1] }
    if ($key -match ':txt(2[6-9]|3[0-1]|3[6-9]|[4-6][0-9])(?:[AB]|Desc)?$') { return $Matches[1] }
    return $null
}

function Get-Label([string]$key, [string]$item) {
    $labels = @{
        '1'='Taxable Year'; '2'='Quarter'; '3'='Amended Return'; '4'='Number of Attached Sheets'; '5'='Taxpayer Identification Number';
        '6'='RDO Code'; '7'='Taxpayer Type'; '8'='ATC'; '9'='Taxpayer Name'; '10'='Registered Address'; '10A'='ZIP Code';
        '11'='Date of Birth'; '12'='Email Address'; '13'='Citizenship'; '14'='Foreign Tax Identification Number';
        '15'='Claiming Foreign Tax Credits'; '16'='Tax Rate'; '16A'='Method of Deduction'; '17'='Spouse TIN'; '18'='Spouse RDO Code';
        '19'='Spouse Type'; '20'='Spouse ATC'; '21'='Spouse Name'; '22'='Spouse Citizenship';
        '23'='Spouse Foreign Tax Number'; '24'='Spouse Claiming Foreign Tax Credits'; '25'='Spouse Tax Rate'; '25A'='Spouse Method of Deduction';
        '26'='Tax Due'; '27'='Less: Tax Credits/Payments'; '28'='Tax Payable/(Overpayment)'; '29'='Add: Total Penalties';
        '30'='Total Amount Payable/(Overpayment)'; '31'='Aggregate Amount Payable/(Overpayment)';
        '32'='Cash/Bank Debit Memo'; '33'='Check'; '34'='Tax Debit Memo'; '35'='Other payment';
        '36'='Sales/Revenues/Receipts/Fees (net of returns, allowances and discounts)'; '37'='Less: Cost of Sales/Services'; '38'='Gross Income/(Loss) from Operation';
        '39'='Total Allowable Itemized Deductions'; '40'='Optional Standard Deduction (40% of Item 36)'; '41'='Net Income/(Loss) This Quarter'; '42'='Taxable Income/(Loss) Previous Quarter/s';
        '43'='Non-Operating Income'; '44'='Amount Received/Share in Income by a Partner from General Professional Partnership'; '45'='Total Taxable Income/(Loss) To Date'; '46'='Tax Due';
        '47'='Sales/Revenues/Receipts/Fees (net of returns, allowances and discounts)'; '48'='Add: Non-Operating Income'; '49'='Total Income for the Quarter'; '50'='Add: Total Taxable Income/(Loss) Previous Quarter';
        '51'='Cumulative Taxable Income'; '52'='Less: Allowable Reduction'; '53'='Taxable Income To Date'; '54'='Tax Due';
        '55'='Prior Year''s Excess Credits'; '56'='Tax Payment/s for the Previous Quarter/s'; '57'='Creditable Tax Withheld for the Previous Quarter/s';
        '58'='Creditable Tax Withheld per BIR Form 2307 for this Quarter'; '59'='Tax Paid in Return Previously Filed, if Amended'; '60'='Foreign Tax Credits'; '61'='Other Tax Credits/Payments';
        '62'='Total Tax Credits/Payments'; '63'='Tax Payable/(Overpayment)'; '64'='Surcharge'; '65'='Interest'; '66'='Compromise'; '67'='Total Penalties'; '68'='Total Amount Payable/(Overpayment)'
    }
    if ($item -and $key -match ':txt(Agency|Number|Date|Amount|Particular)(3[2-5])$') {
        $columns = @{ 'Agency'='Drawee Bank/Agency'; 'Number'='Number'; 'Date'='Date (MM/DD/YYYY)'; 'Amount'='Amount'; 'Particular'='Particulars' }
        return $labels[$item] + ' - ' + $columns[$Matches[1]]
    }
    if ($item -and $labels.ContainsKey($item)) { return $labels[$item] }
    $special = @{
        'frm1701q:txtCurrentPage'='Current UI page'; 'txtFinalFlag'='Final/submission state flag'; 'txtEnroll'='Online enrollment flag';
        'ebirOnlineUsername'='Online username'; 'ebirOnlineSecret'='Online credential secret'; 'ebirOnlineConfirmUsername'='Online username confirmation';
        'driveSelectTPExport'='Export destination drive selection'; 'frm1701q:txtLOB'='Line of Business'; 'frm1701q:txtTelno'='Contact Number';
        'frm1701q:txtPg2TIN1'='Page 2 repeated TIN segment 1'; 'frm1701q:txtPg2TIN2'='Page 2 repeated TIN segment 2';
        'frm1701q:txtPg2TIN3'='Page 2 repeated TIN segment 3'; 'frm1701q:txtPg2BranchCode'='Page 2 repeated TIN branch code';
        'frm1701q:txtPg2TaxpayerName'='Page 2 repeated taxpayer/filer name'; 'frm1701q:txtMaxPage'='Maximum UI page count'
    }
    if ($special.ContainsKey($key)) { return $special[$key] }
    return $null
}

function Get-ChoiceMeaning([string]$key, [string]$value) {
    if ($key -match 'DateQuarter') { return @{'1'='1st Quarter';'2'='2nd Quarter';'3'='3rd Quarter'}[$value] }
    if ($key -match 'AmendedRtn|ForeignTaxCred') { return @{'Y'='Yes';'N'='No'}[$value] }
    if ($key -match 'opt(Type|SpouseType)_') { return @{'Single'='Single Proprietor';'Professional'='Professional';'Estate'='Estate';'Trust'='Trust';'Compensation'='Compensation Earner'}[$value] }
    if ($key -match 'opt(Spouse)?ATC_') { return @{'II012'='Business Income - Graduated IT Rates';'II014'='Income from Profession - Graduated IT Rates';'II013'='Mixed Income - Graduated IT Rates';'II011'='Compensation Income';'II015'='Business Income - 8% IT Rate';'II017'='Income from Profession - 8% IT Rate';'II016'='Mixed Income - 8% IT Rate'}[$value] }
    if ($key -match 'opt(Spouse)?TaxRate_') { return @{'Graduated'='Graduated Rates per Tax Table';'Percentage'='8% on gross sales/receipts and other non-operating income'}[$value] }
    if ($key -match 'opt(MethodOfDeduction:|SpouseMethod:)_') { return @{'I'='Itemized Deduction';'O'='Optional Standard Deduction (OSD)'}[$value] }
    return $null
}

$controls = @{}
$tagMatches = [regex]::Matches($hta, '<(input|select|textarea)\b[^>]*>', 'IgnoreCase,Singleline')
foreach ($match in $tagMatches) {
    $tag = $match.Value
    $id = Get-Attribute $tag 'id'
    if (-not $id) { continue }
    $line = 1 + ([regex]::Matches($hta.Substring(0, $match.Index), "`n")).Count
    $kind = $match.Groups[1].Value.ToLowerInvariant()
    if ($kind -eq 'input') {
        $type = Get-Attribute $tag 'type'
        if ($type) { $kind = $type.ToLowerInvariant() } else { $kind = 'text' }
    }
    $controls[$id] = [ordered]@{
        kind=$kind; line=$line; maxlength=(Get-Attribute $tag 'maxlength'); value=(Get-Attribute $tag 'value');
        checked=(Get-Attribute $tag 'checked'); name=(Get-Attribute $tag 'name');
        disabled=(Get-Attribute $tag 'disabled'); readonly=(Get-Attribute $tag 'readonly');
        onkeypress=(Get-Attribute $tag 'onkeypress'); onblur=(Get-Attribute $tag 'onblur')
    }
}

$computedItems = @('26','27','28','29','30','31','38','40','41','45','46','49','51','53','54','62','63','67','68')
$requiredKeys = @('frm1701q:txtYear','frm1701q:txtTIN1','frm1701q:txtTIN2','frm1701q:txtTIN3','frm1701q:txtBranchCode','frm1701q:txtRDOCode','frm1701q:txtTaxpayerName','frm1701q:txtAddress','frm1701q:txtZipCode','frm1701q:txtBirthMonth','frm1701q:txtBirthDay','frm1701q:txtBirthYear')
$fields = @()
$fieldMatches = [regex]::Matches($save, '<div>(?<key>[^=<>]+)=(?<value>.*?)\k<key>=</div>', 'Singleline')
foreach ($match in $fieldMatches) {
    $key = $match.Groups['key'].Value
    $savedValue = $match.Groups['value'].Value
    $item = Get-Item $key
    $control = $controls[$key]
    $choiceMeaning = if ($control -and $control.kind -in @('radio','checkbox')) { Get-ChoiceMeaning $key $control.value } else { $null }
    $kind = if ($control) { $control.kind } elseif ($key -match '^(txtFinalFlag|txtEnroll|ebirOnline)') { 'hidden-metadata' } else { 'serialized-field' }
    $logical = 'string'
    if ($savedValue -in @('true','false') -or $kind -in @('radio','checkbox')) { $logical = 'boolean' }
    elseif ($key -match ':txt(?:2[6-9]|3[0-1]|3[6-9]|[4-6][0-9])[AB]$' -or $key -match ':txtAmount3[2-5]$') { $logical = 'money' }
    elseif ($key -match ':txtDate3[2-5]$') { $logical = 'date' }
    elseif ($key -match 'Birth(Month|Day|Year)$|:txtYear$|:txtSheets$') { $logical = 'integer' }
    elseif ($key -match 'TIN[123]$|BranchCode$|ZipCode$|Telno$') { $logical = 'digit-string' }
    elseif ($key -match 'RDOCode$') { $logical = 'code' }
    elseif ($key -match 'Email$' -or $key -eq 'txtEmail') { $logical = 'email' }

    $computed = $false
    if ($item -and $computedItems -contains $item) { $computed = $true }
    if ($key -match ':txtPg2(TIN|BranchCode|TaxpayerName)') { $computed = $true }
    $required = 'optional'
    if ($requiredKeys -contains $key -or $item -in @('2','7','8','16','16A')) { $required = 'required' }
    if ($key -match ':txtSpouse|:optSpouse') { $required = 'conditional' }
    if ($computed) { $required = 'computed' }
    if ($kind -eq 'hidden-metadata') { $required = 'hidden' }

    $constraints = [ordered]@{}
    if ($control -and $control.maxlength -match '^\d+$') { $constraints.max_length = [int]$control.maxlength }
    if ($control -and $control.onkeypress) { $constraints.official_keypress_handler = $control.onkeypress }
    if ($control -and $control.value) { $constraints.official_control_value = $control.value }
    if ($control -and $control.name) { $constraints.official_control_group = $control.name }
    if ($choiceMeaning) { $constraints.official_control_caption = $choiceMeaning }
    if ($logical -eq 'money') { $constraints.precision = 2; $constraints.storage_format = 'comma-grouped decimal string' }
    if ($logical -eq 'boolean') { $constraints.allowed_values = @('true','false') }

    $normalization = @()
    if ($control -and $control.onblur -match 'capital') { $normalization += 'uppercase on blur' }
    if ($control -and $control.onblur -match 'round\(this,2\)') { $normalization += 'round/format to two decimals on blur' }
    $sourceRefs = @('xml-editable-v1#field:' + $key)
    if ($control) { $sourceRefs += ('official-hta-runtime#L' + $control.line) }
    else { $sourceRefs += 'official-hta-runtime#saveXML' }

    $page = $null
    if ($item) { if ([int]($item -replace '[^0-9]','') -le 35) { $page = 1 } else { $page = 2 } }
    elseif ($key -match ':txtPg2') { $page = 2 }
    if ($item) { $sourceRefs += ('official-pdf#p' + $page + ':item-' + $item) }
    $calculationId = if ($computed -and $item) { 'calc-item-' + $item } else { $null }
    $requiredWhen = if ($required -eq 'conditional') { 'Spouse data is present or spouse filing branch is enabled; exact per-field condition remains subject to audit.' } else { $null }
    if ($key -match ':txtSpouseTIN[123]$') { $requiredWhen = 'If any spouse TIN core segment is nonempty, all three core segments must contain exactly 3 characters.'; $sourceRefs += 'official-hta-runtime#L3393-L3403' }
    elseif ($key -eq 'frm1701q:txtSpouseBranchCode') { $requiredWhen = 'Checked only when any spouse TIN core segment is nonempty; official source rejects length greater than 5 but imposes no minimum or nonblank requirement.'; $sourceRefs += 'official-hta-runtime#L3393-L3398' }
    elseif ($key -in @('frm1701q:txtSpouseRDOCode','frm1701q:txtSpouseName')) { $requiredWhen = 'Required when any spouse TIN core segment is nonempty.'; $sourceRefs += 'official-hta-runtime#L3393-L3411' }
    elseif ($key -match ':optSpouseType_') { $requiredWhen = 'At least one spouse type is required when spouse name is nonempty.'; $sourceRefs += 'official-hta-runtime#L3431-L3434' }
    elseif ($key -match ':optSpouseATC_') { $requiredWhen = 'At least one spouse ATC is required when any spouse TIN core segment is present or spouse name is nonempty.'; $sourceRefs += @('official-hta-runtime#L3393-L3414','official-hta-runtime#L3431-L3437') }
    elseif ($key -match ':optSpouseTaxRate_') { $requiredWhen = 'A spouse tax rate is required when spouse name is nonempty unless Compensation Earner is selected.'; $sourceRefs += 'official-hta-runtime#L3431-L3441' }
    elseif ($key -match ':optSpouseMethod:') { $required = 'optional'; $requiredWhen = 'Official Validate never requires spouse Item 25A; when blank under graduated rate, Item 41B falls through to OSD contrary to the guide Itemized default.'; $sourceRefs += @('official-hta-runtime#L3431-L3446','official-hta-runtime#L4320-L4328','official-guide#p1:Deductions') }
    elseif ($key -match ':txtSpouse(Citizenship|ForeignTaxNum)$|:optSpouseForeignTaxCred_') { $required = 'optional'; $requiredWhen = $null }
    $enabledWhen = if ($control -and $control.disabled) { 'Enabled by official UI state logic; inspect processTaxType/processATC/schedule functions.' } else { $null }
    if ($key -match ':txtSpouse|:optSpouse') {
        $enabledWhen = 'Base spouse branch is enabled for taxpayer type Single Proprietor or Professional and disabled/reset for Estate or Trust.'
        $sourceRefs += @('official-hta-runtime#L3732-L3800','official-hta-runtime#L3841-L3867')
    }
    if ($key -eq 'frm1701q:txtSpouseForeignTaxNum') {
        $enabledWhen = 'General Edit enables Item 23. After Estate/Trust disables spouse and taxpayer type returns to Single Proprietor/Professional, enableSpouse mistakenly enables taxpayer Item 14, leaving spouse Item 23 disabled.'
        $sourceRefs += @('official-hta-runtime#L3540-L3583','official-hta-runtime#L3732-L3742','official-hta-runtime#L3760-L3780')
    }
    elseif ($key -match ':optSpouseATC_') {
        $enabledWhen = 'Within the spouse branch, spouse-type changes reset ATCs. Compensation Earner selects II011 and disables other ATCs; if multiple spouse types are selected, only II013 and II016 are re-enabled.'
        $sourceRefs += 'official-hta-runtime#L3869-L3950'
    }
    elseif ($key -match ':optSpouseTaxRate_') {
        $enabledWhen = 'Within the spouse branch, ATC selects and locks Graduated or 8%; Compensation Income disables both; multiple spouse types re-enable both.'
        $sourceRefs += @('official-hta-runtime#L3909-L3950','official-hta-runtime#L3990-L4047')
    }
    elseif ($key -match ':optSpouseMethod:') {
        $enabledWhen = 'Enabled only for spouse graduated-rate ATCs; disabled for 8% or Compensation Income. General Edit initially enables it before schedule state is restored.'
        $sourceRefs += @('official-hta-runtime#L3990-L4047','official-hta-runtime#L4060-L4098','official-hta-runtime#L4124-L4166')
    }
    if ($computed) {
        $enabledWhen = 'Never enabled for direct entry; populated by official copy/calculation logic and locked again by Validate.'
        $sourceRefs += @('official-hta-runtime#L3452-L3538','official-hta-runtime#L4244-L4586')
    }
    elseif ($key -in @('frm1701q:txtTIN1','frm1701q:txtTIN2','frm1701q:txtTIN3','frm1701q:txtBranchCode','frm1701q:txtRDOCode','frm1701q:txtTaxpayerName','frm1701q:txtAddress','frm1701q:txtZipCode','txtEmail')) {
        $enabledWhen = 'Profile/identity-owned field; the official Edit path does not re-enable it, and taxpayer TIN is explicitly locked.'
        $sourceRefs += @('official-hta-runtime#L3452-L3540','official-hta-runtime#L3540-L3705')
    }
    elseif ($item -in @('32','33','34','35')) {
        $enabledWhen = 'Statically disabled payment-detail field; no form state transition enables this control for direct entry.'
    }
    elseif ($key -match ':txt(3[6-9]|4[0-6])([AB])$' -and -not $computed) {
        $enabledWhen = 'Schedule I input: enabled for the matching party under graduated rate; Items 37 and 39 additionally require Itemized deduction. On Edit, spouse B-state is incorrectly derived from taxpayer rate/method.'
        $sourceRefs += @('official-hta-runtime#L3597-L3636','official-hta-runtime#L4060-L4122','official-hta-runtime#L4194-L4242')
    }
    elseif ($key -match ':txt(4[7-9]|5[0-4])([AB])$' -and -not $computed) {
        $enabledWhen = 'Schedule II input: enabled for the matching party under 8% rate. On Edit, spouse B-state is incorrectly derived from taxpayer rate; Item 52 has additional ATC-specific behavior.'
        $sourceRefs += @('official-hta-runtime#L3612-L3645','official-hta-runtime#L4124-L4192')
    }
    elseif ($key -match ':txt(5[5-9]|6[0-1]|6[4-6])A$') {
        $enabledWhen = if ($item -eq '59') { 'Enabled only when Amended Return Yes is selected.' } else { 'Enabled while editing; locked after successful Validate.' }
        $sourceRefs += @('official-hta-runtime#L3452-L3538','official-hta-runtime#L3648-L3685')
    }
    elseif ($key -match ':txt(5[5-9]|6[0-1]|6[4-6])B$') {
        $enabledWhen = if ($item -eq '59') { 'Enabled only for Amended Return Yes with a nonempty spouse name.' } else { 'Normal ATC transitions enable this for a non-compensation spouse; Edit incorrectly enables it even with no spouse.' }
        $sourceRefs += @('official-hta-runtime#L3898-L3907','official-hta-runtime#L4002-L4034','official-hta-runtime#L4049-L4052','official-hta-runtime#L3648-L3685')
    }
    elseif ($key -eq 'frm1701q:txt43Desc') {
        $enabledWhen = 'Shared description enabled by Schedule I and disabled when Schedule I is disabled; because it is shared, either party transition can overwrite the other party state.'
        $sourceRefs += @('official-hta-runtime#L4060-L4122')
    }
    elseif ($key -eq 'frm1701q:txt48Desc') {
        $enabledWhen = 'Shared description enabled by Schedule II and disabled when Schedule II is disabled; because it is shared, either party transition can overwrite the other party state.'
        $sourceRefs += @('official-hta-runtime#L4124-L4192')
    }
    elseif ($key -match ':AmendedRtn_[12]$') {
        $enabledWhen = 'Enabled while editing; disabled after successful Validate. Item 59 inputs follow the Yes selection.'
        $sourceRefs += @('official-hta-runtime#L3452-L3553','official-hta-runtime#L3675-L3685','official-hta-runtime#L3803-L3815')
    }
    elseif ($key -eq 'frm1701q:txtMaxPage') {
        $enabledWhen = 'Internal maximum-page metadata; never enabled for user entry.'
    }
    $notes = @('Representative saved value: ' + ($savedValue | ConvertTo-Json -Compress))
    if (-not $control) { $notes += 'No static HTA control tag matched this editable-save key; it may be metadata, runtime-injected, or serializer-derived.' }
    if ($item -eq '16A') { $sourceRefs += 'official-guide#p1:Deductions'; $notes += 'Guide contradiction: an unmarked deduction choice is deemed Itemized, while eBIRForms Validate requires an explicit Item 16A selection.' }
    if ($key -eq 'frm1701q:txt52A') { $enabledWhen = 'Enabled for 8% Schedule II except taxpayer ATC II016, which forces 0.00 and disables the field.'; $sourceRefs += @('official-hta-runtime#L4133-L4144','official-guide#p1:Tax Rate') }
    if ($key -eq 'frm1701q:txt52B') { $enabledWhen = 'Official source disables Item 52B for spouse ATC II016 only when spouse type Compensation Earner is false; it remains enabled in the ordinary compensation-earner mixed-income case, contrary to the guide.'; $sourceRefs += @('official-hta-runtime#L4152-L4163','official-guide#p1:Tax Rate') }

    if ($kind -in @('radio','checkbox')) {
        $enumValues = ,([object[]]@(
            [ordered]@{ stored_value='true'; control_value=$control.value; choice_meaning=$choiceMeaning; meaning='selected' },
            [ordered]@{ stored_value='false'; control_value=$control.value; choice_meaning=$choiceMeaning; meaning='not selected' }
        ))
        $defaultValue = if ($control.checked) { 'true' } else { $null }
        if ($control.value) { $notes += ('HTML choice value: ' + $control.value) }
        if ($choiceMeaning) { $notes += ('Printed choice: ' + $choiceMeaning) }
    }
    elseif ($logical -eq 'boolean') {
        $enumValues = ,([object[]]@('true', 'false'))
        $defaultValue = $null
    }
    else {
        $enumValues = ,([object[]]@())
        $defaultValue = if ($control) { $control.value } else { $null }
    }
    $fieldConfidence = if ($control) { 'high' } else { 'medium' }

    $fields += [ordered]@{
        field_key=$key; label=(Get-Label $key $item); page=$page; item_number=$item; control_kind=$kind;
        storage_type='string'; logical_type=$logical; required=$required; required_when=$requiredWhen; enabled_when=$enabledWhen;
        visible_when=$null; default_value=$defaultValue; empty_representation=''; constraints=$constraints;
        enum_values=$enumValues; normalization=$normalization; computed=$computed;
        calculation_id=$calculationId; source_refs=$sourceRefs; confidence=$fieldConfidence; notes=$notes
    }
}

$inventory = (($fields | ForEach-Object { $_.field_key }) -join "`n") + "`n"
$sha = [Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($inventory))
$shaHex = -join ($sha | ForEach-Object { $_.ToString('x2') })
$document = [ordered]@{
    '$schema'='../../schema/fields.schema.json'; schema_version='1.0.0'; form_id='1701q-v2018'; revision='2018-01-01';
    field_count=$fields.Count; runtime_serializable_element_count=173; inventory_sha256=$shaHex; fields=$fields
}
$json = $document | ConvertTo-Json -Depth 20
$directory = Split-Path -Parent $OutputPath
[IO.Directory]::CreateDirectory($directory) | Out-Null
[IO.File]::WriteAllText($OutputPath, $json + "`n", [Text.UTF8Encoding]::new($false))
Write-Output ([ordered]@{ field_count=$fields.Count; inventory_sha256=$shaHex; output=$OutputPath } | ConvertTo-Json)
