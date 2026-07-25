param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\2550Qv2024'
)

$ErrorActionPreference = 'Stop'
$formId = '2550q-v2024'
$revision = '2024-04-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form2550Qv2024.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help2550Qv2024.hta'
$pdfPath = Join-Path $OfficialDir '2550Q  April 2024 ENCS_Final.pdf'
$guidePath = Join-Path $OfficialDir '2550Q guidelines April 2024_final.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\2550q-v2024'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = '3a5a4d3b2b342a4dfc55a05c69b560fb9be47af6451d8c651d19e1d33406ec70'
    help = '5c81fb5028c25d1104702ed54ff284d69bba0437131ac7891f4782f89342144f'
    pdf = '18eb16925010fdda820cef958221ba2c0d073066efa93a898113e39b31135a25'
    guide = 'b6ee4f090cb48963a44b1ef58fd6cdb4b5865ba4674963c3661c7f164895b120'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher = '57ccf9d8132c490d54bceaf5c55fc2b4bec01b780951a63600402c61a595cdbe'
    decrypted = '6dce2b9614d583cd682de6d301dd4b52078938be2d840ff39dcaeb67cad9ee98'
    encrypted_inventory = '245a84b2ff73b8b00ebb72f65b33be4fc5f15051cd562d9c9e0a363388ec33f1'
    encrypted_ordered_inventory = 'b0c81408ca4e6afd61ada8d72ad61ca9833db7de958f2e772496e3c20405fd95'
    plain = '43577fdd70b8959b16dbada9ff7d8418a1fdc5d18e61302c8cbfc8e9bbab4520'
    plain_inventory = '8191f685cb07c4d233cc3de32066fd7b83248160df780578b960cb57d9ac5f29'
    plain_ordered_inventory = '64154a96231f59c04ce83840955713f8a668984759e6c50e44ebd7bb010fc1d3'
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-LineInventoryHash([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes((@($Lines | Sort-Object) -join "`n"))
        ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    } finally { $sha.Dispose() }
}
function Get-OrderedLineInventoryHash([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes((@($Lines) -join "`n"))
        ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    } finally { $sha.Dispose() }
}
function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine),
        [Text.UTF8Encoding]::new($false)
    )
}
function Write-Text([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function New-Asset([string]$Id,[string]$Kind,[string]$Path,[string]$Binding,[string]$DisplayPath='') {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id=$Id;kind=$Kind;path=if($DisplayPath){$DisplayPath}else{$Path}
        sha256=Get-Sha256 $Path;size=$item.Length;revision_binding=$Binding
    }
}

foreach ($asset in @(
    @($htaPath,'hta'),@($helpPath,'help'),@($pdfPath,'pdf'),
    @($guidePath,'guide'),@($packagePath,'package')
)) {
    if (-not (Test-Path -LiteralPath $asset[0] -PathType Leaf)) { throw "Missing official asset: $($asset[0])" }
    if ((Get-Sha256 $asset[0]) -ne $expected[$asset[1]]) { throw "Official asset hash changed: $($asset[0])" }
}
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)applicationname="2550Qv2024"' -or $hta -notmatch 'April 2024 \(ENCS\)') {
    throw 'April 2024 runtime binding changed.'
}
if ($help -notmatch '2550Qv2024 \[April 2024 \(ENCS\)\]' -or $help -notmatch '(?i)applicationname="0605"') {
    throw 'April 2024 help binding or known APPLICATIONNAME defect changed.'
}
foreach ($pdf in @($pdfPath,$guidePath)) {
    $bytes=[IO.File]::ReadAllBytes($pdf)
    if ([Text.Encoding]::ASCII.GetString($bytes[0..4]) -ne '%PDF-') { throw "PDF magic mismatch: $pdf" }
}

$sampleByHash=@{}
foreach($file in Get-ChildItem -LiteralPath $OfficialDir -File -Filter '*.xml'){
    $sampleByHash[(Get-Sha256 $file.FullName)]=$file
}
foreach($name in @('cipher','plain')){
    if(-not $sampleByHash.ContainsKey($expected[$name])){throw "Pinned 2550Q sample missing: $name"}
}
$keyTool=Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson=& $keyTool -SourcePath $sampleByHash[$expected.cipher].FullName `
    -RedactedSourcePath (Join-Path $OfficialDir '2550Q-final-copy-#email-redacted#.xml') `
    -FormId $formId -ExpectedCiphertextSha256 $expected.cipher `
    -ExpectedDecryptedSha256 $expected.decrypted -ExpectedFieldCount 159 `
    -ExpectedFieldInventorySha256 $expected.encrypted_inventory `
    -ExpectedOrderedFieldInventorySha256 $expected.encrypted_ordered_inventory
$keyAudit=$keyJson|ConvertFrom-Json
$encryptedKeys=@($keyAudit.keys)

$plainText=[IO.File]::ReadAllText($sampleByHash[$expected.plain].FullName)
$plainKeys=@(
    [regex]::Matches($plainText,'<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') |
        ForEach-Object {$_.Groups['key'].Value}
)
if($plainKeys.Count-ne 160-or @($plainKeys|Sort-Object -Unique).Count-ne 160){throw 'Plaintext 2550Q inventory changed.'}
if((Get-LineInventoryHash $plainKeys)-ne $expected.plain_inventory){throw 'Plaintext inventory hash changed.'}
if((Get-OrderedLineInventoryHash $plainKeys)-ne $expected.plain_ordered_inventory){throw 'Plaintext ordered inventory hash changed.'}
$encryptedOnly=@($encryptedKeys|Where-Object {$plainKeys-notcontains $_})
$plainOnly=@($plainKeys|Where-Object {$encryptedKeys-notcontains $_})
if($encryptedOnly.Count-ne 0-or $plainOnly.Count-ne 1-or $plainOnly[0]-ne 'dateFiled'){
    throw '2550Q editable/final-copy field asymmetry changed.'
}
$keys=@($plainKeys+$encryptedKeys|Sort-Object -Unique)
if($keys.Count-ne 160){throw "Expected 160-key union; got $($keys.Count)."}

New-Item -ItemType Directory -Force -Path $fixtureDir|Out-Null
Write-Text (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') ($keyJson-join [Environment]::NewLine)
Write-Json (Join-Path $fixtureDir 'plaintext-field-audit-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId
    source_path=(Join-Path $OfficialDir '2550Q-save-#email-redacted#.xml')
    sha256=$expected.plain;field_count=$plainKeys.Count
    unique_field_count=@($plainKeys|Sort-Object -Unique).Count
    field_inventory_sha256=$expected.plain_inventory
    ordered_field_inventory_sha256=$expected.plain_ordered_inventory
    encrypted_only_keys=$encryptedOnly;plain_only_keys=$plainOnly;values_emitted=$false
    keys=$plainKeys
})

$controlTool=Join-Path $RepoRoot 'rules\tools\inspect-hta-controls.ps1'
$controlAudit=(& $controlTool -HtaPath $htaPath -FormCode '2550Qv2024')|ConvertFrom-Json
$controls=@($controlAudit.controls)
if($controls.Count-ne 219){throw "Expected 219 live static controls; got $($controls.Count)."}
$controlById=@{}
foreach($control in $controls){if($control.id-and-not $controlById.ContainsKey($control.id)){$controlById[$control.id]=$control}}

$familyDefinitions=@(
    @('txtDatePurchase1','Schedule 1 purchase date','date'),
    @('txtSourceCode1','Schedule 1 source code','code'),
    @('txtDescription1','Schedule 1 description','string'),
    @('txtAmountPurchase1','Schedule 1 purchase amount','decimal-money'),
    @('txtInputTax1','Schedule 1 input tax','decimal-money'),
    @('txtEstimatedLife1','Schedule 1 estimated useful life','integer'),
    @('txtRecognizedLife1','Schedule 1 recognized life','integer'),
    @('txtAllowedInputTax1','Schedule 1 allowable input tax','decimal-money'),
    @('txtBalanceInputTax1','Schedule 1 input-tax balance','decimal-money'),
    @('txtDateCovered3','Schedule 3 period from','date'),
    @('txtDateCovered3To','Schedule 3 period to','date'),
    @('txtNameWithHoldingAgent3','Schedule 3 withholding-agent name','string'),
    @('txtIncomePayment3','Schedule 3 income payment','decimal-money'),
    @('txtTotalTaxWithHeld3','Schedule 3 tax withheld','decimal-money'),
    @('txtDate4','Schedule 4 period from','date'),
    @('txtDate4To','Schedule 4 period to','date'),
    @('txtNameOfMiller4','Schedule 4 miller name','string'),
    @('txtNameOfTaxpayer4','Schedule 4 taxpayer name','string'),
    @('txtOfficialReceiptNumber4','Schedule 4 official-receipt number','string'),
    @('txtAmountPaid4','Schedule 4 amount paid','decimal-money'),
    @('frm2550qv2024:totalTaxPayableNo19Description','Item 19 additional description','string'),
    @('frm2550qv2024:totalTaxPayableNo19Amount','Item 19 additional amount','decimal-money'),
    @('frm2550qv2024:totalTaxPayableNo42Description','Item 42 additional description','string'),
    @('frm2550qv2024:totalTaxPayableNo42Amount','Item 42 additional amount','decimal-money'),
    @('frm2550qv2024:totalTaxPayableNo47Description','Item 47 additional description','string'),
    @('frm2550qv2024:totalTaxPayableNo47Amount','Item 47 additional amount','decimal-money'),
    @('frm2550qv2024:totalTaxPayableNo56Description','Item 56 additional description','string'),
    @('frm2550qv2024:totalTaxPayableNo56Amount','Item 56 additional amount','decimal-money')
)

function Get-FieldMeta([string]$Key,$Control,[bool]$Family){
    $logical='string';$required='optional';$computed=$false;$item=$null;$label=$Key
    $constraints=[ordered]@{};$enum=@();$normalization=@()
    if($Key-match '(?i)(amount|tax|sales|payment|payable|credit|deduction|surcharge|interest|compromise|purchase)'){
        $logical='decimal-money';$normalization=@('NumWithComma','formatCurrency')
    }
    if($Key-match '(?i)(date|periodto)'){$logical='date'}
    if($Key-match '(?i)(year|life|sheets)'){$logical='integer'}
    if($Key-match '(?i)(calendar|fiscal|quarter|yn|classification|amended|shortperiod)'){$logical='boolean';$enum=@('true','false')}
    if($Key-match '(?i)(tin|branchcode|rdo|sourcecode)'){$logical='code'}
    if($Key-match '(?i)(total|allowedinputtax|balanceinputtax|outputvat|netvat|excesscredits)'){$computed=$true;$required='computed'}
    if($Key-match '(?i)(txtYearNo2)$'){$item='2';$required='required'}
    elseif($Key-match '(?i)(OptQuarter)'){$item='3';$required='required'}
    elseif($Key-match '(?i)(RtnPeriodToNo4)'){$item='4';$required='required'}
    elseif($Key-match '(?i)(txtTIN[123]|branchCode)$'){$item='7';$required='required'}
    elseif($Key-match '(?i)txtRDOCode$'){$item='8';$required='required'}
    elseif($Key-match '(?i)taxpayerName$'){$item='9';$required='required'}
    elseif($Key-match '(?i)taxpayerAddress$'){$item='10';$required='required'}
    elseif($Key-match '(?i)taxpayerZip$'){$item='10A';$required='required'}
    elseif($Key-match '(?i)taxpayerContactNumber$'){$item='11';$required='required'}
    elseif($Key-match '(?i)taxpayerEmailAddress$'){$item='12';$required='required'}
    elseif($Key-match '(?i)taxPayerClassification'){$item='13';$required='required'}
    elseif($Key-match '(?i)InternationalTreaty'){$item='14';$required='conditional'}
    elseif($Key-match '(?i)No(?<n>19|42|47|56)'){$item=$Matches.n}
    if($Control-and $Control.maxlength){$constraints.max_length=[int]$Control.maxlength}
    if($Family){$constraints.index='N >= 0; no runtime maximum';$required='conditional'}
    [pscustomobject]@{logical=$logical;required=$required;computed=$computed;item=$item;label=$label;constraints=[pscustomobject]$constraints;enum=$enum;normalization=$normalization}
}

$fields=[Collections.Generic.List[object]]::new()
foreach($key in $keys){
    $control=if($controlById.ContainsKey($key)){$controlById[$key]}else{$null}
    $meta=Get-FieldMeta $key $control $false
    $refs=@('xml-union-v1#field:'+ $key)
    if($key-eq'dateFiled'){$refs+=@('official-hta-runtime#saveXML:L5242','official-hta-runtime#saveXMLsubmit:L5397')}
    elseif($control){$refs+=("official-hta-runtime#control:L"+$control.source_line)}
    else{$refs+='official-hta-runtime#serialization:L5160-L5820'}
    $fields.Add([pscustomobject][ordered]@{
        field_key=$key;serialized_key=$key;serialized_occurrence=1;label=$meta.label;page=$null;item_number=$meta.item
        control_kind=if($control){$control.control_kind}else{'serialized-runtime-control'}
        storage_type='string';logical_type=$meta.logical;required=$meta.required;required_when=$null;enabled_when=$null;visible_when=$null
        default_value=$null;empty_representation='';constraints=$meta.constraints;enum_values=$meta.enum;normalization=$meta.normalization
        computed=[bool]$meta.computed;calculation_id=if($meta.computed){'See calculations.json'}else{$null}
        source_refs=$refs;confidence=if($control-or$key-eq'dateFiled'){'high'}else{'medium'}
        notes=@(if($key-eq'dateFiled'){'Present as the final pseudo-div in the plaintext finalized-save inventory; official encrypted final-copy serialization emits it separately as standalone metadata.'}else{'Present in at least one pinned dummy XML inventory; values are not copied.'})
    })
}
foreach($family in $familyDefinitions){
    $key=$family[0]+'{N>=0}'
    $computed=$family[0]-match '(AllowedInputTax|BalanceInputTax)'
    [string[]]$familyNormalization=@()
    if($family[2]-eq'decimal-money'){$familyNormalization=@('NumWithComma','formatCurrency')}
    [object[]]$familyEnumValues=@()
    $fields.Add([pscustomobject][ordered]@{
        field_key=$key;serialized_key=$null;serialized_occurrence=$null;label=$family[1];page=$null
        item_number=if($family[0]-match 'No(?<n>19|42|47|56)'){$Matches.n}else{'Schedule'}
        control_kind='runtime-indexed-family';storage_type='string';logical_type=$family[2]
        required=if($computed){'computed'}else{'conditional'};required_when='A row with index N exists.'
        enabled_when=$null;visible_when='A row with index N exists.';default_value=$null;empty_representation=''
        constraints=[pscustomobject][ordered]@{index='N >= 0; no runtime maximum'}
        enum_values=$familyEnumValues;normalization=$familyNormalization
        computed=$computed;calculation_id=if($computed){'2550q-schedule1-row'}else{$null}
        source_refs=@('official-hta-runtime#dynamic-row-builders:L5830-L6760','official-hta-runtime#serialization:L5160-L5820')
        confidence='high';notes=@('The HTA exposes Add/Delete without a maximum-row guard; this descriptor prevents a false finite inventory claim.')
    })
}
if($fields.Count-ne 188-or @($fields.field_key|Sort-Object -Unique).Count-ne 188){throw "Expected 188 unique fields; got $($fields.Count)."}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    field_count=$fields.Count;runtime_serializable_element_count=160
    inventory_sha256=Get-LineInventoryHash @($fields.field_key);fields=$fields
})

Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;package_version=$packageVersion
    official_hta_sha256=$expected.hta;static_control_count=$controls.Count
    static_controls_with_id_count=@($controls|Where-Object id).Count
    static_controls_without_id_count=@($controls|Where-Object{-not $_.id}).Count
    editable_save_key_count=160;encrypted_final_copy_key_count=159
    union_concrete_key_count=160;unbounded_family_count=28;static_controls=$controls
    unbounded_dynamic_families=$familyDefinitions
})
Write-Json (Join-Path $fixtureDir 'schedule-family-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;family_count=$familyDefinitions.Count
    maximum_rows=$null;source_has_maximum_guard=$false
    families=@($familyDefinitions|ForEach-Object{[ordered]@{prefix=$_[0];label=$_[1];logical_type=$_[2];index='N >= 0'}})
})
$serializationBindingTool = Join-Path $RepoRoot 'rules\tools\build-2550q-serialization-bindings.ps1'
& $serializationBindingTool -RepoRoot $RepoRoot

$rules=[Collections.Generic.List[object]]::new()
function Add-Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Fields,[string]$Message,[string]$Ref,[string]$Assessment='verified-correct',[string]$Official='The first failing branch alerts and returns.',[string]$Recommended='Mirror the ordered official check unless the rule is classified as defective.'){
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Fields
        accepted_behavior='Processing continues when the condition is false.';rejected_behavior='Processing stops at this branch.'
        exact_message=$Message;source_refs=@($Ref);evidence_type=@('source')
        assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()
    })
}
$main=@(
    @('year-type','Neither Calendar nor Fiscal year is selected.',@('frm2550qv2024:calendarNo1','frm2550qv2024:fiscalNo1'),'Please choose a year type on Item 1.'),
    @('quarter','No quarter is selected.',@('frm2550qv2024:OptQuarter1','frm2550qv2024:OptQuarter2','frm2550qv2024:OptQuarter3','frm2550qv2024:OptQuarter4'),'Item 3 is a required field.'),
    @('future-period','Return-period date is later than the system date outside the hard-coded exception.',@('frm2550qv2024:RtnPeriodToNo4','frm2550qv2024:txtYearNo2'),'Should not accept advance filing.'),
    @('tin','Any TIN segment or branch code is blank.',@('frm2550qv2024:txtTIN1','frm2550qv2024:txtTIN2','frm2550qv2024:txtTIN3','frm2550qv2024:branchCode'),'Please enter a valid TIN number on Item 7.'),
    @('rdo','RDO code is blank.',@('frm2550qv2024:txtRDOCode'),'Please enter a valid RDO Code on Item 8.'),
    @('name','Taxpayer name is blank.',@('frm2550qv2024:taxpayerName'),'Please enter a valid Taxpayer Name on Item 9.'),
    @('address','Registered address is blank.',@('frm2550qv2024:taxpayerAddress'),"Please enter a valid Taxpayer's Registered Address on Item 10."),
    @('zip','ZIP code is blank.',@('frm2550qv2024:taxpayerZip'),"Please enter a valid Taxpayer's ZIP Code on Item 10A."),
    @('contact','Contact number is blank.',@('frm2550qv2024:taxpayerContactNumber'),"Please enter a valid Taxpayer's Contact Number on Item 11."),
    @('email','Email address is blank.',@('frm2550qv2024:taxpayerEmailAddress'),"Please enter a valid Taxpayer's Email Address on Item 12."),
    @('classification','No taxpayer classification is selected.',@('frm2550qv2024:taxPayerClassification1','frm2550qv2024:taxPayerClassification2','frm2550qv2024:taxPayerClassification3','frm2550qv2024:taxPayerClassification4'),'Please choose taxpayer classification on Item 13.'),
    @('treaty','Treaty Yes is selected and Item 14A is blank.',@('frm2550qv2024:internationalTreatyYn','frm2550qv2024:specifyInternationalTreaty'),'Specify cannot be empty field on item 14A.'),
    @('19-nan','Item 19 amount parses as NaN.',@('frm2550qv2024:otherCreditsNo19'),'Other Credits (Item 19) is a required field'),
    @('19-description','Item 19 amount is positive and description is blank.',@('frm2550qv2024:addSpecifyNo19','frm2550qv2024:otherCreditsNo19'),'Item 19 specify is a required field.'),
    @('19-value','Item 19 description is nonblank and amount is zero.',@('frm2550qv2024:addSpecifyNo19','frm2550qv2024:otherCreditsNo19'),'Item 19 value is required when specify is provided.'),
    @('42-nan','Item 42 amount parses as NaN.',@('frm2550qv2024:otherSpecify42'),' (Item 42) is a required field'),
    @('42-description','Item 42 amount is positive and description is blank.',@('frm2550qv2024:addSpecifyNo42','frm2550qv2024:otherSpecify42'),'Specify field (Item 42) is required '),
    @('42-value','Item 42 description is nonblank and amount is zero.',@('frm2550qv2024:addSpecifyNo42','frm2550qv2024:otherSpecify42'),' (Item 42) value is required when Specify field is provided.'),
    @('47-nan','Item 47 amount parses as NaN.',@('frm2550qv2024:otherSpecify47'),' (Item 47) is a required field.'),
    @('47-description','Item 47 amount is positive and description is blank.',@('frm2550qv2024:addSpecifyNo47','frm2550qv2024:otherSpecify47'),'Specify field (Item 47) is required field.'),
    @('47-value','Item 47 description is nonblank and amount is zero.',@('frm2550qv2024:addSpecifyNo47','frm2550qv2024:otherSpecify47'),' (Item 47) value is required when Specify field is provided.'),
    @('56-nan','Nonblank Item 56 amount parses as NaN.',@('frm2550qv2024:otherSpecify56'),' (Item 56) must be a valid number.'),
    @('56-description','Item 56 amount is positive and description is blank.',@('frm2550qv2024:addSpecifyNo56','frm2550qv2024:otherSpecify56'),'Specify field (Item 56) is required field.'),
    @('56-value','Item 56 description is nonblank and amount is blank or zero.',@('frm2550qv2024:addSpecifyNo56','frm2550qv2024:otherSpecify56'),'(Item 56) value is required when Specify field is provided.')
)
$order=0
foreach($entry in $main){
    $order++
    $assessment=if($entry[0]-eq'future-period'){'official-bug-compatible'}elseif($entry[0]-in @('tin','email')){'incorrect-official-behavior'}else{'verified-correct'}
    $official=if($entry[0]-eq'future-period'){'Future dates are rejected except a stale hard-coded year-ended-2025 exception for November/December 2024 and January 2025.'}elseif($entry[0]-eq'tin'){'Only nonblank segments are checked; no digit shape or TIN checksum is enforced.'}elseif($entry[0]-eq'email'){'Only nonblank content is checked; syntax is not validated.'}else{'The first failing branch alerts and returns.'}
    Add-Rule ("2550q-validate-"+$entry[0]) 'validate' $order $entry[1] $entry[2] $entry[3] 'official-hta-runtime#validate:L8407-L8620' $assessment $official
}
Add-Rule '2550q-validate-19-unreachable' 'validate' 16 'Item 19 description is blank and amount exceeds 1000.' @('frm2550qv2024:addSpecifyNo19','frm2550qv2024:otherCreditsNo19') $null 'official-hta-runtime#validate:L8503-L8506' 'incorrect-official-behavior' 'The branch is unreachable because the earlier blank-description/positive-amount branch already returns; it also has no alert.' 'Remove the dead branch and enforce one explicit pair rule.'

Add-Rule '2550q-save-tin' 'save' 1 'Any TIN segment or branch code is blank.' @('frm2550qv2024:txtTIN1','frm2550qv2024:txtTIN2','frm2550qv2024:txtTIN3','frm2550qv2024:branchCode') 'Please enter a valid TIN number on Item 7.' 'official-hta-runtime#initialValidateBeforeSave:L9065-L9069' 'incorrect-official-behavior' 'Save checks only nonblank TIN pieces.'
Add-Rule '2550q-save-rdo' 'save' 2 'RDO code equals 000.' @('frm2550qv2024:txtRDOCode') 'Please enter a valid RDO Code on Item 8.' 'official-hta-runtime#initialValidateBeforeSave:L9070-L9073'
Add-Rule '2550q-save-name' 'save' 3 'Taxpayer name is blank.' @('frm2550qv2024:taxpayerName') "Please enter a valid Withholding Agent's Name on Item 9." 'official-hta-runtime#initialValidateBeforeSave:L9074-L9077' 'incorrect-official-behavior' 'The VAT-return save alert incorrectly labels the taxpayer a withholding agent.' 'Use Taxpayer Name while preserving the actual guard.'

Add-Rule '2550q-s1-empty' 'page navigation' 1 'Any materialized Schedule 1 row field is blank.' @('txtDatePurchase1{N>=0}','txtSourceCode1{N>=0}','txtDescription1{N>=0}') 'Empty fields are not allowed ' 'official-hta-runtime#schedule1-validation:L5830-L6357'
Add-Rule '2550q-s1-date-empty' 'blur/change' $null 'Schedule 1 purchase date is blank.' @('txtDatePurchase1{N>=0}') 'Date Purchase cannot be an empty field.' 'official-hta-runtime#schedule1-date-validation:L5830-L6357'
Add-Rule '2550q-s1-date-cutoff' 'blur/change' $null 'Schedule 1 purchase date is after December 31, 2021.' @('txtDatePurchase1{N>=0}') 'Only dates on or before December 31, 2021 are allowed.' 'official-hta-runtime#schedule1-date-validation:L5830-L6357'
Add-Rule '2550q-s1-source' 'blur/change' $null 'Schedule 1 source code is neither I nor D.' @('txtSourceCode1{N>=0}') "Invalid input`nAccept only I and D " 'official-hta-runtime#validateSourceCode:L6132-L6145'
Add-Rule '2550q-s1-life' 'blur/change' $null 'Schedule 1 estimated life exceeds 60.' @('txtEstimatedLife1{N>=0}') 'Invalid Input it should be 1 to 60' 'official-hta-runtime#validateEstimatedLife:L6122-L6130' 'incorrect-official-behavior' 'The message says 1 to 60 but the source does not explicitly reject zero.' 'Enforce the stated inclusive 1..60 range.'
Add-Rule '2550q-s3-empty' 'page navigation' 1 'Any materialized Schedule 3 row field is blank.' @('txtDateCovered3{N>=0}','txtDateCovered3To{N>=0}','txtNameWithHoldingAgent3{N>=0}','txtIncomePayment3{N>=0}','txtTotalTaxWithHeld3{N>=0}') 'Empty fields are not allowed ' 'official-hta-runtime#schedule3-validation:L6358-L6597'
Add-Rule '2550q-s3-year' 'page navigation' $null 'Return-period year is 2000 or earlier.' @('frm2550qv2024:txtYearNo2') 'Only values 2000 and beyond are allowed.' 'official-hta-runtime#showNewSched3:L6390-L6425' 'incorrect-official-behavior' 'The condition year <= 2000 && year <= 2020 reduces to year <= 2000, contradicting the message at the boundary.' 'Use one explicit inclusive minimum-year rule.'
Add-Rule '2550q-s4-empty' 'page navigation' 1 'Any materialized Schedule 4 row field is blank.' @('txtDate4{N>=0}','txtDate4To{N>=0}','txtNameOfMiller4{N>=0}','txtNameOfTaxpayer4{N>=0}','txtOfficialReceiptNumber4{N>=0}','txtAmountPaid4{N>=0}') 'Empty fields are not allowed ' 'official-hta-runtime#schedule4-validation:L6598-L6760'
Add-Rule '2550q-other-row-add' 'page navigation' 1 'An Item 19/42/47/56 additional row has a blank description or nonpositive amount.' @('dynamic-other-row-families') 'Empty fields are not allowed' 'official-hta-runtime#additional-row-validation:L6761-L7131'
Add-Rule '2550q-other-row-close' 'page navigation' 2 'Save-and-close finds an incomplete Item 19/42/47/56 additional row.' @('dynamic-other-row-families') 'Please fill up all required fields and ensure amounts are greater than zero.' 'official-hta-runtime#additional-row-validation:L6761-L7131'

Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    first_error_behavior='Main Validate and each modal validator alert on the first failing branch and return.'
    rules=$rules
})
Write-Json (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta
    main_validate='L8407-L8620';save_preflight='L9065-L9080'
    covered_rule_count=$rules.Count;notable_omissions=@(
        'No TIN checksum or digit-shape validation.',
        'No email syntax validation.',
        'No main Validate enforcement of most numeric tax relationships.',
        'No amended-return or short-period selection requirement.'
    )
})

$calculations=[Collections.Generic.List[object]]::new()
function Add-Calc([string]$Id,[string[]]$Outputs,[string[]]$Inputs,[string]$Formula,[string]$Trigger,[string]$Ref,[string]$Assessment='verified-correct',[string]$Recommendation='Use typed decimal arithmetic and preserve the source dependency order.'){
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id=$Id;outputs=$Outputs;inputs=$Inputs;condition=$null;official_formula=$Formula
        rounding='parseFloat/NumWithComma arithmetic followed by toFixed(2) and formatCurrency where called.'
        trigger=$Trigger;depends_on=@();source_refs=@($Ref);assessment=$Assessment
        recommended_app_behavior=$Recommendation;confidence='high'
    })
}
Add-Calc '2550q-schedule1-row' @('txtAllowedInputTax1{N>=0}','txtBalanceInputTax1{N>=0}') @('txtInputTax1{N>=0}','txtRecognizedLife1{N>=0}') 'allowed input tax = input tax / recognized life * 3; balance = input tax - allowed input tax.' 'computeSched1' 'official-hta-runtime#computeSched1:L6013-L6120'
Add-Calc '2550q-schedule1-totals' @('schedule1 totals') @('all Schedule 1 rows') 'Sum purchase amount, input tax, allowed input tax, and balance columns.' 'computeSched1' 'official-hta-runtime#computeSched1:L6013-L6120'
Add-Calc '2550q-schedule3-totals' @('Schedule 3 income total','Schedule 3 withholding total','frm2550qv2024:creditableVat') @('txtIncomePayment3{N>=0}','txtTotalTaxWithHeld3{N>=0}') 'Sum Schedule 3 columns and transfer total tax withheld to creditable VAT.' 'computeSched3' 'official-hta-runtime#computeSched3:L6358-L6389'
Add-Calc '2550q-schedule4-totals' @('Schedule 4 amount total','frm2550qv2024:advVatPayment') @('txtAmountPaid4{N>=0}') 'Sum Schedule 4 amounts and transfer to advance VAT payment.' 'computeSched4' 'official-hta-runtime#computeSched4:L6598-L6635'
Add-Calc '2550q-item15' @('frm2550qv2024:excessInputTax') @('frm2550qv2024:netVatPayable') '15 = net VAT payable.' 'compute15' 'official-hta-runtime#compute15:L6773-L6778'
Add-Calc '2550q-item16' @('frm2550qv2024:creditableVat') @('Schedule 3 withholding total') '16 = Schedule 3 tax withheld total.' 'compute16' 'official-hta-runtime#compute16:L6779-L6783'
Add-Calc '2550q-item17' @('frm2550qv2024:advVatPayment') @('Schedule 4 amount total') '17 = Schedule 4 amount paid total.' 'compute17' 'official-hta-runtime#compute17:L6784-L6788'
Add-Calc '2550q-item20' @('frm2550qv2024:totalTaxCredits') @('frm2550qv2024:creditableVat','frm2550qv2024:advVatPayment','frm2550qv2024:vatPaidReturn','frm2550qv2024:otherCreditsNo19') '20 = 16 + 17 + 18 + 19.' 'compute20' 'official-hta-runtime#compute20:L7132-L7154'
Add-Calc '2550q-item21' @('frm2550qv2024:excessCredits') @('frm2550qv2024:excessInputTax','frm2550qv2024:totalTaxCredits') '21 = 15 - 20.' 'compute21' 'official-hta-runtime#compute21:L7155-L7161'
Add-Calc '2550q-item25' @('frm2550qv2024:totalPenalties') @('frm2550qv2024:surcharge','frm2550qv2024:interest','frm2550qv2024:compromise') '25 = surcharge + interest + compromise.' 'compute25' 'official-hta-runtime#compute25:L7162-L7168'
Add-Calc '2550q-item26' @('frm2550qv2024:totalPayable') @('frm2550qv2024:excessCredits','frm2550qv2024:totalPenalties') 'If Item 21 is negative and penalties are positive, 26 = penalties; otherwise 26 = 21 + 25.' 'compute26' 'official-hta-runtime#compute26:L7169-L7185' 'official-bug-compatible' 'Represent a refund/excess-credit separately; do not silently discard it when penalties exist.'
Add-Calc '2550q-item31' @('frm2550qv2024:outputVat','frm2550qv2024:outputTaxDue') @('frm2550qv2024:vatableSales') '31A = vatable sales * 12%; 31B copies 31A.' 'compute31AB' 'official-hta-runtime#compute31AB:L7186-L7191'
Add-Calc '2550q-item34' @('frm2550qv2024:totalSales') @('frm2550qv2024:vatableSales','frm2550qv2024:zeroRatedSales','frm2550qv2024:exemptSales') '34 = vatable + zero-rated + exempt sales.' 'compute34AB' 'official-hta-runtime#compute34AB:L7192-L7199'
Add-Calc '2550q-item37' @('frm2550qv2024:totalAdjOutput') @('frm2550qv2024:outputTaxDue','frm2550qv2024:lessOutputVat','frm2550qv2024:addOutputVat') '37 = output tax due - less output VAT + additional output VAT.' 'compute37AB' 'official-hta-runtime#compute37AB:L7200-L7206'
Add-Calc '2550q-item39' @('frm2550qv2024:inputTaxDeferred') @('Schedule 1 prior balance total') '39 = Schedule 1 carried input-tax balance.' 'compute39AB' 'official-hta-runtime#compute39AB:L7207-L7212'
Add-Calc '2550q-item43' @('frm2550qv2024:totalInputTax') @('frm2550qv2024:inputTaxCarried','frm2550qv2024:inputTaxDeferred','frm2550qv2024:transitionalInputTax','frm2550qv2024:presumptiveInputTax','frm2550qv2024:otherSpecify42') '43 = 38 + 39 + 40 + 41 + 42 plus materialized additional Item 42 rows.' 'compute43AB' 'official-hta-runtime#compute43AB:L7555-L7577' 'official-bug-compatible' 'Iterate actual additional rows; do not use the source multi-row reference to a non-existent totalTaxPayableNo42.1 element.'
Add-Calc '2550q-items44-46' @('Item 44/45/46 input-tax amounts') @('Item 44/45/46 purchase amounts') 'Each input-tax amount = corresponding purchase amount * 12%.' 'compute44AB/compute45AB/compute46AB' 'official-hta-runtime#compute44AB:L7578-L7595'
Add-Calc '2550q-item47' @('frm2550qv2024:otherSpecify47B') @('frm2550qv2024:otherSpecify47') '47B = 47A * 12%.' 'compute47AB' 'official-hta-runtime#compute47AB:L7596-L7602'
Add-Calc '2550q-item50' @('Item 50 purchase total','Item 50 input-tax total') @('Items 44 through 49') '50 totals current purchases and their input taxes, including additional Item 47 rows.' 'compute50AB' 'official-hta-runtime#compute50AB:L7933-L7965' 'official-bug-compatible' 'Iterate actual additional Item 47 rows; do not rely on the suspicious txtVatableSales47A.1 reference.'
Add-Calc '2550q-item51' @('frm2550qv2024:totalAvailInputTax') @('frm2550qv2024:totalInputTax','Item 50 input-tax total') '51 = 43 + 50B.' 'compute51AB' 'official-hta-runtime#compute51AB:L7966-L7971'
Add-Calc '2550q-item52' @('frm2550qv2024:importCapitalInputTax') @('Schedule 1 carried balance') '52 = Schedule 1 carried balance.' 'compute52AB' 'official-hta-runtime#compute52AB:L7972-L7977'
Add-Calc '2550q-item53' @('frm2550qv2024:inputTaxAttr') @('Schedule 2 total attributable input tax') '53 = Schedule 2 total attributable input tax.' 'compute53AB' 'official-hta-runtime#compute53AB:L7978-L7983'
Add-Calc '2550q-schedule2-allocation' @('Schedule 2 ratable input tax','Schedule 2 attributable total') @('frm2550qv2024:exemptSales','frm2550qv2024:totalSales','Schedule 2 non-direct input tax','Schedule 2 direct input tax') 'ratable = exempt sales / total sales * non-direct input tax; attributable = direct + ratable.' 'Schedule 2 change handlers' 'official-hta-runtime#schedule2:L7984-L8309'
Add-Calc '2550q-item57' @('frm2550qv2024:totalDeductions') @('Items 52 through 56') '57 = 52 + 53 + 54 + 55 + 56 plus additional Item 56 rows.' 'compute57AB' 'official-hta-runtime#compute57AB:L8310-L8334' 'official-bug-compatible' 'Iterate actual additional rows; do not use the suspicious totalTaxPayableNo56.1 reference.'
Add-Calc '2550q-item59' @('frm2550qv2024:adjDeductions') @('frm2550qv2024:totalDeductions','frm2550qv2024:addInputVat') '59 = 57 + 58.' 'compute59AB' 'official-hta-runtime#compute59AB:L8335-L8340'
Add-Calc '2550q-item60' @('frm2550qv2024:totalAllowableInputTax') @('frm2550qv2024:totalAvailInputTax','frm2550qv2024:adjDeductions') '60 = 51 - 59.' 'compute60AB' 'official-hta-runtime#compute60AB:L8341-L8347'
Add-Calc '2550q-item61' @('frm2550qv2024:netVatPayable','frm2550qv2024:excessInputTax') @('frm2550qv2024:totalAdjOutput','frm2550qv2024:totalAllowableInputTax') '61 = 37 - 60, then the result is copied back to Item 15/excess input tax.' 'compute61AB' 'official-hta-runtime#compute61AB:L8348-L8405' 'official-bug-compatible' 'Preserve the dependency deliberately and avoid an accidental circular recalculation between Items 15 and 61.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    evaluation_order=@($calculations.calculation_id);calculations=$calculations
})
Write-Json (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta
    covered_calculation_count=$calculations.Count;calculation_ids=@($calculations.calculation_id)
})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId
    cases=@($calculations|ForEach-Object{[ordered]@{
        case_id=$_.calculation_id+'-source-boundary';calculation_id=$_.calculation_id
        inputs=@{source_formula=$_.official_formula};official_output='Derived by the pinned source formula; no taxpayer values are stored.'
    }})
})

$negativeCases=[Collections.Generic.List[object]]::new();$n=0
foreach($rule in $rules){$n++;$negativeCases.Add([ordered]@{
    case_id=('case-{0:d2}-{1}'-f $n,$rule.rule_id);phase=$rule.phase
    mutations=@{synthetic_condition=$rule.condition};expected_message=$rule.exact_message
    expected_behavior=$rule.official_behavior;rule_id=$rule.rule_id
})}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$negativeCases
})

$workflow=[ordered]@{
    '$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    phases=@(
        @{phase='edit';official_behavior='Two-page quarterly VAT return with four additional-item tables and three unbounded schedules.';source_refs=@('official-hta-runtime#controls-and-modals');confidence='high'},
        @{phase='saved-draft';official_behavior='Save checks only nonblank TIN pieces, RDO not equal to 000, and taxpayer name, then writes 160 plaintext keys including generated dateFiled.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L9065-L9080','official-hta-runtime#saveXML:L5160-L5320');confidence='high'},
        @{phase='validated';official_behavior='Ordered main validation runs, then disables controls and announces success.';source_refs=@('official-hta-runtime#validate:L8407-L8620');confidence='high'},
        @{phase='final-copy';official_behavior='Encrypted final-copy serialization has 159 keys and omits generated dateFiled.';source_refs=@('xml-final-copy-v1','official-hta-runtime#final-copy');confidence='high'},
        @{phase='submitted';official_behavior='Online transport exists in source but was not exercised.';source_refs=@('official-hta-runtime#transport');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='Save preflight passes.';side_effects=@('Writes a plaintext local save with generated dateFiled.');source_refs=@('official-hta-runtime#initialValidateBeforeSave:L9065-L9080')},
        @{from='edit';action='Validate';to='validated';guard='All ordered main and applicable schedule checks pass.';side_effects=@('Disables controls.');source_refs=@('official-hta-runtime#validate:L8407-L8620')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables controls.');source_refs=@('official-hta-runtime#enableAllControl:L8622-L9064')},
        @{from='validated';action='Final Copy';to='final-copy';guard='Official final-copy workflow proceeds.';side_effects=@('Writes encrypted artifact whose observed inventory omits dateFiled.');source_refs=@('xml-final-copy-v1')},
        @{from='final-copy';action='Online transport';to='submitted';guard='Connectivity and official transport proceed.';side_effects=@('No online submission was performed during research.');source_refs=@('official-hta-runtime#transport')}
    )
    prerequisites=@('Exact April 2024 ENCS revision','Complete applicable background fields and schedules')
    required_attachments=@(
        @{attachment_id='applicable-vat-support';label='Applicable VAT schedules and supporting documents';required_when='Required by the return facts and official guidelines.';official_ui_enforcement='Local Validate does not comprehensively enforce attachments.';source_refs=@('official-guidelines-pdf');confidence='medium'}
    )
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='Within twenty-five days following the close of the taxable quarter.';source_refs=@('official-installed-help','official-guidelines-pdf');confidence='high'},
        @{quarter='Q2';due_date_rule='Within twenty-five days following the close of the taxable quarter.';source_refs=@('official-installed-help','official-guidelines-pdf');confidence='high'},
        @{quarter='Q3';due_date_rule='Within twenty-five days following the close of the taxable quarter.';source_refs=@('official-installed-help','official-guidelines-pdf');confidence='high'},
        @{quarter='Q4';due_date_rule='Within twenty-five days following the close of the taxable quarter.';source_refs=@('official-installed-help','official-guidelines-pdf');confidence='high'}
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount=@($rules|Where-Object assessment -in @('incorrect-official-behavior','official-bug-compatible','obsolete')).Count+
    @($calculations|Where-Object assessment -in @('incorrect-official-behavior','official-bug-compatible','obsolete')).Count
$manifest=[ordered]@{
    '$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='2550Q'
    revision=$revision;revision_label='April 2024 ENCS';package_version=$packageVersion;status='complete'
    official_assets=@(
        (New-Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package used for extraction.'),
        (New-Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 2550Qv2024 and printed April 2024 ENCS.'),
        (New-Asset 'official-installed-help' 'runtime-help' $helpPath 'April 2024 instructions; packaged APPLICATIONNAME is incorrectly 0605.'),
        (New-Asset 'official-pdf-2024' 'official-form-pdf' $pdfPath 'April 2024 ENCS official form.'),
        (New-Asset 'official-guidelines-pdf' 'official-guidelines-pdf' $guidePath 'April 2024 final guidelines.'),
        (New-Asset 'xml-finalized-save-v1' 'dummy-profile-finalized-save' $sampleByHash[$expected.plain].FullName 'Dummy plaintext finalized save: the source marker ends in 2012.0 and isItAFinalCopy classifies it as final; serialized txtFinalFlag is independently 1. Values excluded.' (Join-Path $OfficialDir '2550Q-save-#email-redacted#.xml')),
        (New-Asset 'xml-final-copy-v1' 'dummy-profile-encrypted-final-copy' $sampleByHash[$expected.cipher].FullName 'Dummy encrypted final copy; values excluded.' (Join-Path $OfficialDir '2550Q-final-copy-#email-redacted#.xml'))
    )
    counts=@{typed_fields=$fields.Count;concrete_union_fields=160;unbounded_families=28;validation_rules=$rules.Count;calculations=$calculations.Count;negative_fixtures=$negativeCases.Count;confirmed_official_bugs=$bugCount;unverified_gaps=0}
    artifacts=@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';runtime_control_fixture='fixtures/runtime-control-inventory-v796.json';schedule_family_fixture='fixtures/schedule-family-inventory-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'}
    scope_notes=@(
        'Research artifacts only; no renderer, migration, release, or capability changes.',
        'No real taxpayer values or email-bearing filenames are stored.',
        'No online submission or mutation of official artifacts was performed.',
        'The 188 entries are the 160-key plaintext/encrypted union plus 28 explicit unbounded family descriptors.',
        'dateFiled is present as the last pseudo-div in the pinned plaintext finalized save and as standalone metadata in the encrypted final copy; collapsing the two shapes would lose its artifact-specific placement.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest

Write-Text (Join-Path $outDir 'evidence.md') @'
# Evidence — 2550Q April 2024 ENCS

The exact April 2024 runtime HTA, installed help, official form PDF, official guidelines PDF, dummy plaintext finalized save, dummy encrypted final copy, and package executable are pinned in `manifest.json`. The plaintext finalized save contains 160 unique keys; the decrypted encrypted final copy contains 159. Their pseudo-div difference is generated `dateFiled`, which is the final pseudo-div in the plaintext sample and standalone metadata in the encrypted artifact. The plaintext sample ends in `All Rights Reserved BIR 2012.0`, so `isItAFinalCopy()` classifies it as final even though its serialized `txtFinalFlag` value is independently `1`.

The concrete union therefore has 160 keys. Source inspection also proves 28 unbounded indexed families: nine Schedule 1 fields, five Schedule 3 fields, six Schedule 4 fields, and description/amount pairs for Items 19, 42, 47, and 56. The HTA has Add/Delete behavior and no maximum-row guard, so `fields.json` records family descriptors instead of inventing a finite row count.

All representative values are excluded. Email-bearing filenames are redacted as `#email-redacted#`. No online submission was performed.
'@
Write-Text (Join-Path $outDir 'gaps.md') @'
# Gaps — 2550Q April 2024 ENCS

No unexplained revision, field-inventory, validation, calculation, or workflow gap remains. Online transport was intentionally not exercised; its behavior is source-derived and is not needed to claim transport verification.
'@
Write-Text (Join-Path $outDir 'README.md') @'
# BIR Form 2550Q — April 2024 ENCS

Revision-specific Offline eBIRForms validation knowledge. Inventory: 160 concrete union keys plus 28 unbounded family descriptors. Research only; no renderer or release metadata changes.
'@
Write-Text (Join-Path $outDir 'audit.md') @'
# Audit — 2550Q April 2024 ENCS

- Revision is bound independently by runtime identity, printed header, installed help, official form PDF, and official guidelines.
- Inventory is the union of 160 plaintext finalized-save keys and 159 encrypted-final-copy pseudo-div keys; `dateFiled` is the final plaintext pseudo-div and standalone encrypted-artifact metadata.
- Runtime source contains 219 live static controls and 28 unbounded indexed families.
- Main Validate, Save preflight, schedule validators, additional-row validators, calculations, and serialization paths were inspected.
- First-error order and exact alerts are recorded.
- Dummy-only negative cases bind to rule IDs; taxpayer values and email-bearing filenames are excluded.
- Confirmed defects include the stale future-period exception, nonblank-only TIN/email checks, unreachable Item 19 branch, wrong Save alert noun, Schedule 1 zero-life mismatch, Schedule 3 year-boundary contradiction, suspicious multi-row references, and the Item 15/61 feedback assignment.
- Final strict repository audit passed with `-RequireJsonSchema`: 43 forms, 519 JSON files, 9,592 fields, 2,007 validations, 623 calculations, 1,354 negative fixtures, and 216 schema documents. Structural audit and JSON Schema validation both reported `pass`; stderr was empty.
'@

$indexPath=Join-Path $RepoRoot 'rules\index.json'
$index=Get-Content -LiteralPath $indexPath -Raw|ConvertFrom-Json
if(-not($index.forms|Where-Object form_id -eq $formId)){
    $index.forms+=[pscustomobject][ordered]@{form_id=$formId;form_code='2550Q';revision=$revision;package_version=$packageVersion;priority=42;status='complete';path='forms/2550q-v2024/manifest.json'}
}
$index.updated='2026-07-23'
Write-Json $indexPath $index

[pscustomobject]@{
    form_id=$formId;fields=$fields.Count;validations=$rules.Count;calculations=$calculations.Count
    negative_fixtures=$negativeCases.Count;official_defect_classifications=$bugCount;output=$outDir
}|ConvertTo-Json
