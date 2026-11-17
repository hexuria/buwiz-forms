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
        'frm1601EQ:txtYear'='1'; 'frm1601EQ:txtNoSheets'='5';
        'frm1601EQ:txtTIN1'='6'; 'frm1601EQ:txtTIN2'='6'; 'frm1601EQ:txtTIN3'='6'; 'frm1601EQ:txtBranchCode'='6';
        'frm1601EQ:txtRDOCode'='7'; 'frm1601EQ:txtTaxpayerName'='8'; 'frm1601EQ:txtAddress'='9';
        'frm1601EQ:txtZipCode'='9A'; 'frm1601EQ:txtTelNum'='10'; 'txtEmail'='12';
        'frm1601EQ:ifRefund'='30'; 'frm1601EQ:ifIssueCert'='30'; 'frm1601EQ:ifCarriedOver'='30'
    }
    if ($map.ContainsKey($key)) { return $map[$key] }
    if ($key -match ':optQuarter:') { return '2' }
    if ($key -match ':optAmend:') { return '3' }
    if ($key -match ':optWithheld:') { return '4' }
    if ($key -match ':optCategory:') { return '11' }
    if ($key -match ':txt(?:AtcCd|TaxBase|TaxRate|TaxbeWithHeld)(\d+)$') {
        $row = [int]$Matches[1]
        if ($row -le 6) { return ($row + 12).ToString() }
        return $null
    }
    if ($key -match ':txtTax(1[9]|2[0-9]|30)$') { return $Matches[1] }
    if ($key -match ':txt(?:Particular|Agency|Number|Date|Amount)(3[3-6])$') { return ([int]$Matches[1] - 2).ToString() }
    return $null
}

function Get-Label([string]$key, [string]$item) {
    $labels = @{
        '1'='Taxable Year'; '2'='Quarter'; '3'='Amended Return'; '4'='Any Taxes Withheld'; '5'='Number of Sheets Attached';
        '6'='Taxpayer Identification Number'; '7'='RDO Code'; '8'='Withholding Agent Name'; '9'='Registered Address';
        '9A'='ZIP Code'; '10'='Contact Number'; '11'='Category of Withholding Agent'; '12'='Email Address';
        '19'='Total Taxes Withheld for the Quarter'; '20'='First-Month Remittance'; '21'='Second-Month Remittance';
        '22'='Tax Remitted in Previously Filed Return'; '23'='Over-remittance from Previous Quarter';
        '24'='Total Remittances Made'; '25'='Tax Still Due/(Over-remittance)'; '26'='Surcharge'; '27'='Interest';
        '28'='Compromise'; '29'='Total Penalties'; '30'='Total Amount Still Due/(Over-remittance)';
        '31'='Cash/Bank Debit Memo'; '32'='Check'; '33'='Tax Debit Memo'; '34'='Other Payment'
    }
    if ($key -match ':txtAtcCd(\d+)$') { return $(if ([int]$Matches[1] -le 6) { 'ATC - Item ' + $item } else { 'Additional ATC - runtime row ' + $Matches[1] }) }
    if ($key -match ':txtTaxBase(\d+)$') { return $(if ([int]$Matches[1] -le 6) { 'Tax Base - Item ' + $item } else { 'Additional ATC Tax Base - runtime row ' + $Matches[1] }) }
    if ($key -match ':txtTaxRate(\d+)$') { return $(if ([int]$Matches[1] -le 6) { 'Tax Rate - Item ' + $item } else { 'Additional ATC Tax Rate - runtime row ' + $Matches[1] }) }
    if ($key -match ':txtTaxbeWithHeld(\d+)$') { return $(if ([int]$Matches[1] -le 6) { 'Tax Withheld - Item ' + $item } else { 'Additional ATC Tax Withheld - runtime row ' + $Matches[1] }) }
    if ($item -and $key -match ':txt(Particular|Agency|Number|Date|Amount)3[3-6]$') {
        $columns = @{ Particular='Particulars'; Agency='Drawee Bank/Agency'; Number='Number'; Date='Date (MM/DD/YYYY)'; Amount='Amount' }
        return $labels[$item] + ' - ' + $columns[$Matches[1]]
    }
    if ($item -and $labels.ContainsKey($item)) { return $labels[$item] }
    if ($key -match '^AtcCode(\d+)$') { return 'Runtime ATC selection slot ' + $Matches[1] }
    $special = @{
        'frm1601EQ:txtLineBus'='Line of Business'; 'frm1601EQ:txtTotalOtherTax'='Total Tax Withheld for Additional ATCs';
        'txtTaxAgentNo'='Tax Agent Accreditation/Attorney Roll Number'; 'txtDateIssue'='Tax Agent Accreditation Date of Issue';
        'txtDateExpiry'='Tax Agent Accreditation Date of Expiry'; 'hPartIITableSize'='Serialized Part II table row count';
        'txtFinalFlag'='Final/submission state flag'; 'txtEnroll'='Online enrollment flag';
        'ebirOnlineConfirmUsername'='Online username confirmation'; 'ebirOnlineUsername'='Online username';
        'ebirOnlineSecret'='Online credential secret'; 'driveSelectTPExport'='Export destination drive selection'
    }
    if ($special.ContainsKey($key)) { return $special[$key] }
    return $null
}

function Get-ChoiceMeaning([string]$key, [string]$value) {
    if ($key -match ':optQuarter:') { return @{'1'='1st Quarter';'2'='2nd Quarter';'3'='3rd Quarter';'4'='4th Quarter'}[$value] }
    if ($key -match ':optAmend:|:optWithheld:') { return @{'Y'='Yes';'N'='No'}[$value] }
    if ($key -match ':optCategory:') { return @{'P'='Private';'G'='Government'}[$value] }
    if ($key -eq 'frm1601EQ:ifRefund') { return 'To be refunded' }
    if ($key -eq 'frm1601EQ:ifIssueCert') { return 'To be issued Tax Credit Certificate' }
    if ($key -eq 'frm1601EQ:ifCarriedOver') { return 'To be carried over to next quarter within same calendar year' }
    return $null
}

$controls = @{}
foreach ($match in [regex]::Matches($hta, '<(input|select|textarea)\b[^>]*>', 'IgnoreCase,Singleline')) {
    $tag = $match.Value
    $id = Get-Attribute $tag 'id'
    if (-not $id) { continue }
    $line = 1 + ([regex]::Matches($hta.Substring(0, $match.Index), "`n")).Count
    $kind = $match.Groups[1].Value.ToLowerInvariant()
    if ($kind -eq 'input') {
        $inputType = Get-Attribute $tag 'type'
        if ($inputType) { $kind = $inputType.ToLowerInvariant() } else { $kind = 'text' }
    }
    $controls[$id] = [ordered]@{
        kind=$kind; line=$line; maxlength=(Get-Attribute $tag 'maxlength'); value=(Get-Attribute $tag 'value');
        checked=(Get-Attribute $tag 'checked'); name=(Get-Attribute $tag 'name'); disabled=(Get-Attribute $tag 'disabled');
        readonly=(Get-Attribute $tag 'readonly'); onkeypress=(Get-Attribute $tag 'onkeypress'); onblur=(Get-Attribute $tag 'onblur');
        onchange=(Get-Attribute $tag 'onchange')
    }
}

$requiredKeys = @(
    'frm1601EQ:txtYear','frm1601EQ:txtTIN1','frm1601EQ:txtTIN2','frm1601EQ:txtTIN3','frm1601EQ:txtBranchCode',
    'frm1601EQ:txtRDOCode','frm1601EQ:txtTaxpayerName','frm1601EQ:txtAddress','frm1601EQ:txtZipCode','frm1601EQ:txtTelNum','txtEmail'
)
$computedItems = @('19','24','25','29','30')
$fields = @()
$sampleMatches = [regex]::Matches($save, '<div>(?<key>[^=<>]+)=(?<value>.*?)\k<key>=</div>', 'Singleline')
$sampleKeys = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
foreach ($sampleMatch in $sampleMatches) { $null = $sampleKeys.Add($sampleMatch.Groups['key'].Value) }
$inventorySave = $save
foreach ($row in 7..111) {
    foreach ($prefix in @('txtAtcCd','txtTaxBase','txtTaxRate','txtTaxbeWithHeld')) {
        $dynamicKey = 'frm1601EQ:' + $prefix + $row
        $inventorySave += '<div>' + $dynamicKey + '=' + $dynamicKey + '=</div>'
    }
}
$matches = [regex]::Matches($inventorySave, '<div>(?<key>[^=<>]+)=(?<value>.*?)\k<key>=</div>', 'Singleline')
foreach ($match in $matches) {
    $key = $match.Groups['key'].Value
    $savedValue = $match.Groups['value'].Value
    $samplePresent = $sampleKeys.Contains($key)
    $item = Get-Item $key
    $control = $controls[$key]
    $dynamicRow = $key -match ':txt(AtcCd|TaxBase|TaxRate|TaxbeWithHeld)(\d+)$'
    $atcSlot = $key -match '^AtcCode(\d+)$'
    if ($dynamicRow) {
        $column = $Matches[1]; $row = [int]$Matches[2]
        $kind = 'runtime-text'
    } elseif ($atcSlot) {
        $kind = 'runtime-checkbox'
    } elseif ($control) {
        $kind = $control.kind
    } elseif ($key -match '^(txtFinalFlag|txtEnroll|ebirOnline|driveSelect)') {
        $kind = 'hidden-metadata'
    } else {
        $kind = 'serialized-field'
    }

    $logical = 'string'
    if ($savedValue -in @('true','false') -or $kind -in @('radio','checkbox','runtime-checkbox')) { $logical = 'boolean' }
    elseif ($key -match ':txt(?:TaxBase|TaxbeWithHeld)\d+$|:txtTax(?:1[9]|2[0-9]|30)$|:txtAmount3[3-6]$|:txtTotalOtherTax$') { $logical = 'money' }
    elseif ($key -match ':txtTaxRate\d+$') { $logical = 'percentage' }
    elseif ($key -match ':txtDate3[3-6]$|^txtDate(?:Issue|Expiry)$') { $logical = 'date' }
    elseif ($key -match ':txtYear$|:txtNoSheets$|^hPartIITableSize$') { $logical = 'integer' }
    elseif ($key -match ':txtTIN[123]$|:txtBranchCode$|:txtZipCode$|:txtTelNum$') { $logical = 'digit-string' }
    elseif ($key -match ':txtRDOCode$|:txtAtcCd\d+$') { $logical = 'code' }
    elseif ($key -eq 'txtEmail') { $logical = 'email' }

    $computed = $false
    if ($item -and $computedItems -contains $item) { $computed = $true }
    if ($key -match ':txt(?:TaxRate|TaxbeWithHeld)\d+$|:txtTotalOtherTax$|^hPartIITableSize$') { $computed = $true }
    $required = 'optional'
    if ($requiredKeys -contains $key -or $item -in @('2','3','4','11')) { $required = 'required' }
    if ($key -match ':txt(?:AtcCd|TaxBase)\d+$|^AtcCode\d+$') { $required = 'conditional' }
    if ($key -match ':if(?:Refund|IssueCert|CarriedOver)$') { $required = 'conditional' }
    if ($computed) { $required = 'computed' }
    if ($kind -eq 'hidden-metadata') { $required = 'hidden' }

    $constraints = [ordered]@{}
    if ($control -and $control.maxlength -match '^\d+$') { $constraints.max_length = [int]$control.maxlength }
    if ($control -and $control.onkeypress) { $constraints.official_keypress_handler = $control.onkeypress }
    if ($control -and $control.value) { $constraints.official_control_value = $control.value }
    if ($control -and $control.name) { $constraints.official_control_group = $control.name }
    if ($logical -eq 'money') { $constraints.precision = 2; $constraints.storage_format = 'comma-grouped decimal string' }
    if ($logical -eq 'percentage') { $constraints.precision = 1; $constraints.unit = 'percent' }
    if ($logical -eq 'boolean') { $constraints.allowed_values = @('true','false') }
    if ($key -match '^AtcCode') { $constraints.slot_semantics = 'Position in the category-filtered runtime ATC catalog; Private exposes 111 slots, Government 96. The same slot number does not have one category-independent ATC meaning.' }

    $normalization = @()
    if ($control -and ($control.onblur -match 'capital|capitalize')) { $normalization += 'uppercase and trim on blur' }
    if (($control -and $control.onblur -match 'round\(this,2\)') -or $key -match ':txtTaxBase\d+$') { $normalization += 'official round() formats to two decimals and comma groups on blur' }
    if ($key -in @('frm1601EQ:txtTaxpayerName','frm1601EQ:txtAddress','frm1601EQ:txtLineBus')) { $normalization += 'escape() percent-encoding during save serialization' }

    $sourceRefs = @()
    if ($samplePresent) { $sourceRefs += ('xml-editable-v1#field:' + $key) }
    if ($control) { $sourceRefs += ('official-hta-runtime#L' + $control.line) }
    elseif ($dynamicRow) { $sourceRefs += @('official-hta-runtime#populateAtcPart2:L2861-L2875','official-hta-runtime#getATCCode:L2890-L3031') }
    elseif ($atcSlot) { $sourceRefs += @('official-hta-runtime#changedrpATCList:L2759-L2782','official-atc-catalog#sha256:16e4db6ce456a6fb0a97f085cf8ef19349c2f6fc183971e97d4e253f63cdd22b') }
    else { $sourceRefs += 'official-hta-runtime#saveXML:L2102-L2383' }
    if ($item) { $sourceRefs += ('official-pdf#p1:item-' + $item) }

    $requiredWhen = $null
    if ($key -match ':txtAtcCd\d+$|^AtcCode\d+$') { $requiredWhen = 'At least one ATC selection is required when Item 4 Any Taxes Withheld is Yes.' }
    if ($key -match ':txtTaxBase\d+$') { $requiredWhen = 'Required and nonzero for every selected ATC when Item 4 is Yes.' }
    if ($key -match ':if(?:Refund|IssueCert|CarriedOver)$') { $requiredWhen = 'Exactly one over-remittance disposition is required when Item 30 is negative.' }
    $enabledWhen = $null
    if ($key -match '^AtcCode\d+$') { $enabledWhen = 'Rendered in the ATC modal after Item 11 category selection; only slots present in that category are created.' }
    elseif ($key -match ':txtTaxBase\d+$') { $enabledWhen = 'Enabled for selected ATC rows during Edit; disabled after successful Validate.' }
    elseif ($key -match ':txtAtcCd\d+$|:txtTaxRate\d+$|:txtTaxbeWithHeld\d+$') { $enabledWhen = 'Runtime-generated display/computed control; not directly editable except the dead N/A tax-rate branch.' }
    elseif ($key -eq 'frm1601EQ:txtTax22') { $enabledWhen = 'Enabled only when Amended Return Yes is selected.' }
    elseif ($key -match ':if(?:Refund|IssueCert|CarriedOver)$') { $enabledWhen = 'Enabled only when Item 30 is negative; choices are mutually exclusive.' }
    elseif ($control -and $control.disabled) { $enabledWhen = 'Disabled in the static form; enablement, if any, is controlled by official UI state logic.' }

    $calculationId = $null
    if ($key -match ':txtTaxbeWithHeld\d+$') { $calculationId = 'calc-atc-row-withheld' }
    elseif ($key -match ':txtTaxRate\d+$') { $calculationId = 'calc-atc-rate-by-year' }
    elseif ($key -eq 'frm1601EQ:txtTax19') { $calculationId = 'calc-item-19' }
    elseif ($key -eq 'frm1601EQ:txtTotalOtherTax') { $calculationId = 'calc-additional-atc-total' }
    elseif ($item -and $computed) { $calculationId = 'calc-item-' + $item }

    $choiceMeaning = if ($control) { Get-ChoiceMeaning $key $control.value } else { $null }
    if ($kind -in @('radio','checkbox')) {
        $enumValues = @(
            [ordered]@{stored_value='true';control_value=$control.value;choice_meaning=$choiceMeaning;meaning='selected'},
            [ordered]@{stored_value='false';control_value=$control.value;choice_meaning=$choiceMeaning;meaning='not selected'}
        )
        $defaultValue = if ($control.checked) { 'true' } else { $null }
    } elseif ($kind -eq 'runtime-checkbox' -or $logical -eq 'boolean') {
        $enumValues = @('true','false'); $defaultValue = $null
    } else {
        $enumValues = @(); $defaultValue = if ($control) { $control.value } else { $null }
    }
    $notes = @()
    if ($samplePresent) {
        $notes += 'Representative saved value: ' + ($savedValue | ConvertTo-Json -Compress)
    } else {
        $notes += 'Potential serialized key derived from the source-proven maximum Private-category selection; absent from the 201-key representative save.'
    }
    if (-not $control) { $notes += 'No static HTA tag matched; this key is runtime-generated or serializer metadata.' }
    if ($key -match ':txt(?:Particular|Agency|Number|Date|Amount)3[3-6]$') { $notes += 'Official control IDs are offset by two from the printed payment item number.' }
    if ($key -match '^AtcCode') { $notes += 'The representative Private-category save serializes all 111 possible slots; Government renders only 96.' }

    $fields += [ordered]@{
        field_key=$key; label=(Get-Label $key $item); page=1; item_number=$item; control_kind=$kind;
        storage_type='string'; logical_type=$logical; required=$required; required_when=$requiredWhen; enabled_when=$enabledWhen;
        visible_when=$null; default_value=$defaultValue; empty_representation=''; constraints=$constraints; enum_values=$enumValues;
        normalization=$normalization; computed=$computed; calculation_id=$calculationId; source_refs=$sourceRefs;
        confidence=($(if ($control -or $dynamicRow -or $atcSlot) { 'high' } else { 'medium' })); notes=$notes
    }
}

$inventory = (($fields | ForEach-Object { $_.field_key }) -join "`n") + "`n"
$sha = [Security.Cryptography.SHA256]::Create().ComputeHash([Text.Encoding]::UTF8.GetBytes($inventory))
$shaHex = -join ($sha | ForEach-Object { $_.ToString('x2') })
$document = [ordered]@{
    '$schema'='../../schema/fields.schema.json'; schema_version='1.0.0'; form_id='1601eq-v2018'; revision='2018-01-01';
    field_count=$fields.Count; runtime_serializable_element_count=621; inventory_sha256=$shaHex; fields=$fields
}
$directory = Split-Path -Parent $OutputPath
[IO.Directory]::CreateDirectory($directory) | Out-Null
[IO.File]::WriteAllText($OutputPath, ($document | ConvertTo-Json -Depth 20) + "`n", [Text.UTF8Encoding]::new($false))
Write-Output ([ordered]@{field_count=$fields.Count;inventory_sha256=$shaHex;output=$OutputPath} | ConvertTo-Json)
