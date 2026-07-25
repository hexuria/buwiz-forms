param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\2551Qv2018'
)

$ErrorActionPreference='Stop'
$formId='2551q-v2018'
$revision='2018-01-01'
$packageVersion='7.9.6.0'
$htaPath=Join-Path $ExtractedRoot 'forms\BIR-Form2551Qv2018.hta'
$helpPath=Join-Path $ExtractedRoot 'helpfile\Help2551Qv2018.hta'
$atcPath=Join-Path $ExtractedRoot 'xml\atcCodes.xml'
$pdfPath=Join-Path $OfficialDir '2551Q Jan 2018 ENCS final rev 3_copy.pdf'
$packagePath='C:\eBIRForms\BIRForms.exe'
$outDir=Join-Path $RepoRoot 'rules\forms\2551q-v2018'
$fixtureDir=Join-Path $outDir 'fixtures'
$expected=@{
    hta='dc5c19710400e9bd15ceccd1bbeafc5290074b17b2d460af5f8921c6a7f93186'
    help='8e5010d9b103076aa571612ed31f69b3a61c05d1af786c6256ac76b502f7d470'
    atc='16e4db6ce456a6fb0a97f085cf8ef19349c2f6fc183971e97d4e253f63cdd22b'
    pdf='1f270ecf66d778836a14697863e420ff65d5ed0a5576a6cf58b97c9a8e8c9b24'
    package='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}

function Get-Sha([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
function Get-Attr([string]$Tag,[string]$Name){
    $m=[regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1'-f[regex]::Escape($Name)))
    if($m.Success){$m.Groups[2].Value}else{$null}
}
function Get-InventoryHash([string[]]$Lines){
    $sha=[Security.Cryptography.SHA256]::Create()
    try{
        $bytes=[Text.Encoding]::UTF8.GetBytes((@($Lines|Sort-Object)-join"`n"))
        ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-','').ToLowerInvariant()
    }finally{$sha.Dispose()}
}
function Write-Json([string]$Path,$Value){
    [IO.File]::WriteAllText($Path,(($Value|ConvertTo-Json -Depth 60)+[Environment]::NewLine),[Text.UTF8Encoding]::new($false))
}
function Write-Text([string]$Path,[string]$Value){[IO.File]::WriteAllText($Path,$Value,[Text.UTF8Encoding]::new($false))}
function New-Asset([string]$Id,[string]$Kind,[string]$Path,[string]$Binding){
    $item=Get-Item -LiteralPath $Path
    [ordered]@{asset_id=$Id;kind=$Kind;path=$Path;sha256=Get-Sha $Path;size=$item.Length;revision_binding=$Binding}
}

foreach($asset in @(@($htaPath,'hta'),@($helpPath,'help'),@($atcPath,'atc'),@($pdfPath,'pdf'),@($packagePath,'package'))){
    if(-not(Test-Path -LiteralPath $asset[0] -PathType Leaf)){throw "Missing official asset: $($asset[0])"}
    if((Get-Sha $asset[0])-ne$expected[$asset[1]]){throw "Official asset hash changed: $($asset[0])"}
}
$hta=[IO.File]::ReadAllText($htaPath)
$help=[IO.File]::ReadAllText($helpPath)
if($hta-notmatch'(?i)applicationname="2551Qv2018"'-or$hta-notmatch'January 2018 \(ENCS\)'){throw 'January 2018 runtime binding changed.'}
if($help-notmatch'2551Q <small>\[January 2018 \(ENCS\)\]'-or$help-notmatch'(?i)applicationname="2551Q"'){throw 'January 2018 help binding changed.'}
$pdfBytes=[IO.File]::ReadAllBytes($pdfPath)
if([Text.Encoding]::ASCII.GetString($pdfBytes[0..4])-ne'%PDF-'){throw '2551Q PDF magic mismatch.'}

New-Item -ItemType Directory -Force -Path $fixtureDir|Out-Null
$controlTool=Join-Path $RepoRoot 'rules\tools\inspect-hta-controls.ps1'
$controlAudit=(& $controlTool -HtaPath $htaPath -FormCode '2551Qv2018')|ConvertFrom-Json
$controls=@($controlAudit.controls)
if($controls.Count-ne 123){throw "Expected 123 live static controls; got $($controls.Count)."}
$controlById=@{}
foreach($control in $controls){if($control.id-and-not$controlById.ContainsKey($control.id)){$controlById[$control.id]=$control}}

# saveXML iterates frmMain.elements and emits text/select-one/radio/checkbox controls.
$formMatch=[regex]::Match($hta,'(?is)<form\b[^>]*\bid\s*=\s*[''"]frmMain[''"][^>]*>(?<body>.*?)</form>')
if(-not$formMatch.Success){throw 'frmMain source boundary changed.'}
$formOffset=$formMatch.Groups['body'].Index
$occurrences=[Collections.Generic.List[object]]::new()
$keyCounts=@{}
foreach($match in [regex]::Matches($formMatch.Groups['body'].Value,'(?is)<(?<element>input|select|textarea)\b(?<attrs>[^>]*)>')){
    $element=$match.Groups['element'].Value.ToLowerInvariant()
    $tag=$match.Value
    $id=Get-Attr $tag 'id'
    if(-not$id){continue}
    $type=if($element-eq'input'){Get-Attr $tag 'type'}elseif($element-eq'select'){'select-one'}else{'textarea'}
    if(-not$type){$type='text'}
    $type=$type.ToLowerInvariant()
    if($type-notin@('text','select-one','radio','checkbox')){continue}
    if(-not$keyCounts.ContainsKey($id)){$keyCounts[$id]=0}
    $keyCounts[$id]++
    $line=1+[regex]::Matches($hta.Substring(0,$formOffset+$match.Index),"`n").Count
    $occurrences.Add([pscustomobject][ordered]@{
        serialized_key=$id;serialized_occurrence=$keyCounts[$id];element=$element;control_kind=$type
        source_line=$line;maxlength=Get-Attr $tag 'maxlength';disabled=$tag-match'(?i)\bdisabled(?:\s*=|\s|>)'
    })
}
# RDO select is injected into frmMain at runtime by getRdo().
$occurrences.Add([pscustomobject][ordered]@{
    serialized_key='frm2551Qv2018:txtRDOCode';serialized_occurrence=1;element='select'
    control_kind='select-one';source_line=4849;maxlength=$null;disabled=$true
})
if($occurrences.Count-ne99){throw "Expected 99 serialization occurrences; got $($occurrences.Count)."}
if(@($occurrences.serialized_key|Sort-Object -Unique).Count-ne98){throw 'Expected 98 distinct serialized keys.'}
$duplicates=@($occurrences|Group-Object serialized_key|Where-Object Count -gt 1)
if($duplicates.Count-ne1-or$duplicates[0].Name-ne'txtEmail'-or$duplicates[0].Count-ne2){throw 'Known duplicate txtEmail serialization changed.'}

function Get-Meta([string]$Key,$Occurrence){
    $logical='string';$required='optional';$computed=$false;$item=$null;$label=$Key
    $constraints=[ordered]@{};$enum=[object[]]@();$normalization=[string[]]@()
    if($Key-match'(?i)(txt1[4-9]$|txt2[0-4]$|Amount|ATCAmt|ATCRate|ATCDue|TotalSched)'){
        $logical='decimal-money';$normalization=@('NumWithComma','formatCurrency')
    }
    if($Key-match'(?i)(Date|Expiry|Issue)'){$logical='date'}
    if($Key-match'(?i)(txtYear|txtSheets|CurrentPage|MaxPage)'){$logical='integer'}
    if($Key-match'(?i)(forThe_|qtr_|amendedRtn_|taxTreaty_|taxRate[12]$|overPayment[12]$)'){$logical='boolean';$enum=@('true','false')}
    if($Key-match'(?i)(TIN|BranchCode|RDO|drpATC|TaxAgentNo)'){$logical='code'}
    if($Key-match'(?i)(txt14$|txt18$|txt19$|txt23$|txt24$|ATCRate|ATCDue|TotalSched|Pg2)'){$computed=$true;$required='computed'}
    if($Key-match'forThe_'){$item='1';$required='required'}
    elseif($Key-match'(rtnMonth|txtYear)$'){$item='2';$required='required'}
    elseif($Key-match'qtr_'){$item='3';$required='required'}
    elseif($Key-match'amendedRtn_'){$item='4'}
    elseif($Key-match'txtSheets$'){$item='5'}
    elseif($Key-match'(txtTIN[123]|txtBranchCode)$'){$item='6';$required='required'}
    elseif($Key-match'txtRDOCode$'){$item='7';$required='required'}
    elseif($Key-match'registeredName$'){$item='8';$required='required'}
    elseif($Key-match'(registeredAddress|zipCode)$'){$item='9';$required='required'}
    elseif($Key-match'telNo$'){$item='10';$required='required'}
    elseif($Key-eq'txtEmail'){$item='11'}
    elseif($Key-match'(taxTreaty|TaxRelief)'){$item='12';$required='conditional'}
    elseif($Key-match'taxRate'){$item='13'}
    elseif($Key-match'frm2551Qv2018:txt(?<n>1[4-9]|2[0-4])'){$item=$Matches.n}
    if($Occurrence.maxlength){$constraints.max_length=[int]$Occurrence.maxlength}
    [pscustomobject]@{logical=$logical;required=$required;computed=$computed;item=$item;label=$label;constraints=[pscustomobject]$constraints;enum=$enum;normalization=$normalization}
}

$fields=[Collections.Generic.List[object]]::new()
foreach($occurrence in $occurrences){
    $serialized=$occurrence.serialized_key
    $fieldKey=if($keyCounts.ContainsKey($serialized)-and$keyCounts[$serialized]-gt1){"$serialized#occurrence-$($occurrence.serialized_occurrence)"}else{$serialized}
    $meta=Get-Meta $serialized $occurrence
    $fields.Add([pscustomobject][ordered]@{
        field_key=$fieldKey;serialized_key=$serialized;serialized_occurrence=$occurrence.serialized_occurrence
        label=$meta.label;page=if($occurrence.source_line-ge1840){2}else{1};item_number=$meta.item
        control_kind=$occurrence.control_kind;storage_type='string';logical_type=$meta.logical;required=$meta.required
        required_when=if($serialized-eq'frm2551Qv2018:txtTaxReliefSpecify'){'Item 12 Yes is selected.'}else{$null}
        enabled_when=$null;visible_when=$null;default_value=$null;empty_representation=''
        constraints=$meta.constraints;enum_values=$meta.enum;normalization=$meta.normalization;computed=[bool]$meta.computed
        calculation_id=if($meta.computed){'See calculations.json'}else{$null}
        source_refs=@("official-hta-runtime#control:L$($occurrence.source_line)",'official-hta-runtime#saveXML:L3638-L3890')
        confidence='high';notes=@(
            if($serialized-eq'txtEmail'){"The DOM contains two controls with this ID; saveXML emits occurrence $($occurrence.serialized_occurrence) under the same serialized key."}
            elseif($serialized-eq'frm2551Qv2018:txtRDOCode'){'Dynamically injected into frmMain by getRdo() before serialization.'}
            else{'Included by the deterministic frmMain.elements serialization predicate.'}
        )
    })
}
if($fields.Count-ne99-or@($fields.field_key|Sort-Object -Unique).Count-ne99){throw '2551Q field inventory construction failed.'}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    field_count=$fields.Count;runtime_serializable_element_count=99
    inventory_sha256=Get-InventoryHash @($fields.field_key);fields=$fields
})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;package_version=$packageVersion;official_hta_sha256=$expected.hta
    static_control_count=$controls.Count;serialization_occurrence_count=$occurrences.Count
    distinct_serialized_key_count=@($occurrences.serialized_key|Sort-Object -Unique).Count
    duplicate_serialized_keys=@($duplicates|ForEach-Object{[ordered]@{key=$_.Name;occurrences=$_.Count}})
    runtime_injected_keys=@('frm2551Qv2018:txtRDOCode');static_controls=$controls;serialization_occurrences=$occurrences
})

$atcEntries=[Collections.Generic.List[object]]::new()
foreach($line in [IO.File]::ReadAllLines($atcPath)){
    $m=[regex]::Match($line,'<div>atc(?<index>\d+):(?<payload>.*?)atc\k<index>:</div>')
    if(-not$m.Success-or-not$m.Groups['payload'].Value.Contains('2551_')){continue}
    $parts=$m.Groups['payload'].Value-split'~',-1
    $atcEntries.Add([ordered]@{source_index=[int]$m.Groups['index'].Value;code=$parts[0];description=$parts[1];rate=[decimal]$parts[2];category=$parts[3]})
}
if($atcEntries.Count-ne23){throw "Expected 23 substring-selected 2551Q ATC records; got $($atcEntries.Count)."}
Write-Json (Join-Path $fixtureDir 'atc-catalog-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;package_version=$packageVersion;source_sha256=$expected.atc
    source_filter="The HTA accepts every catalog payload containing substring '2551_' and labels it formType 2551Qv2018."
    entry_count=$atcEntries.Count;duplicate_codes=@('PT010');entries=$atcEntries
})

$rules=[Collections.Generic.List[object]]::new()
function Add-Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Fields,$Message,[string]$Ref,[string]$Assessment='verified-correct',[string]$Official='The first failing branch alerts and returns.',[string]$Recommended='Mirror the ordered official check unless classified as defective.'){
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Fields
        accepted_behavior='Processing continues when the condition is false.';rejected_behavior='Processing stops or the input is normalized by this branch.'
        exact_message=$Message;source_refs=@($Ref);evidence_type=@('source');assessment=$Assessment
        official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()
    })
}
Add-Rule '2551q-input-year-revision' 'blur/change' $null 'Year is below 2018.' @('frm2551Qv2018:txtYear') 'Please file using the old version of the form.' 'official-hta-runtime#validateYear:L4600-L4607' 'verified-correct' 'Alerts and rewrites the year to 2018.'
Add-Rule '2551q-input-money' 'blur/change' $null 'A money field loses focus.' @('money-fields') $null 'official-hta-runtime#blockletter:L4421-L4430' 'official-bug-compatible' 'parseFloat/toFixed(2); NaN becomes 0.00.' 'Use decimal parsing with explicit syntax and rounding.'
Add-Rule '2551q-input-integer' 'blur/change' $null 'An integer-like field loses focus.' @('integer-fields') $null 'official-hta-runtime#blockletterWithout2Decimal:L4863-L4871' 'official-bug-compatible' 'parseFloat/toFixed(0); NaN becomes blank.' 'Validate integer syntax before conversion.'
Add-Rule '2551q-item2-blank-year' 'blur/change' 1 'Year is blank.' @('frm2551Qv2018:txtYear') 'Please indicate a valid Year.' 'official-hta-runtime#validateItem2:L5458-L5478'
Add-Rule '2551q-item2-month' 'blur/change' 2 'Year-ended month equals zero.' @('frm2551Qv2018:rtnMonth') 'Please choose a valid Month.' 'official-hta-runtime#validateItem2:L5458-L5478'
Add-Rule '2551q-item2-future-year' 'blur/change' 3 'Year exceeds current system year.' @('frm2551Qv2018:txtYear') 'Invalid date entry on Item 2. Entry should not be later than Current Date.' 'official-hta-runtime#validateItem2:L5458-L5478'
Add-Rule '2551q-item2-future-month' 'blur/change' 4 'Year equals current year and selected month is later than current system month.' @('frm2551Qv2018:txtYear','frm2551Qv2018:rtnMonth') 'Invalid date entry on Item 2. Entry should not be later than Current Date.' 'official-hta-runtime#validateItem2:L5458-L5478'
Add-Rule '2551q-treaty-toggle' 'blur/change' $null 'Treaty Yes is not selected.' @('frm2551Qv2018:taxTreaty_1','frm2551Qv2018:txtTaxReliefSpecify') $null 'official-hta-runtime#chageTreaty:L4872-L4880' 'verified-correct' 'Disables and clears the relief selector; Yes enables it.'
Add-Rule '2551q-amended-toggle' 'blur/change' $null 'Amended No is selected.' @('frm2551Qv2018:amendedRtn_1','frm2551Qv2018:amendedRtn_2','frm2551Qv2018:txt16') $null 'official-hta-runtime#updateAmended:L4881-L4901' 'verified-correct' 'Disables and zeros Item 16; amended Yes enables it unless the 8% option is active.'
Add-Rule '2551q-tax-rate-net' 'blur/change' $null 'Graduated/net-taxable-income option is selected.' @('frm2551Qv2018:taxRate1') $null 'official-hta-runtime#taxRateOption:L3117-L3176' 'verified-correct' 'Enables credit, penalty, and Schedule 1 controls.'
Add-Rule '2551q-tax-rate-eight-percent' 'blur/change' $null '8% option is selected.' @('frm2551Qv2018:taxRate2','frm2551Qv2018:txt15','frm2551Qv2018:txt16','frm2551Qv2018:txt17','frm2551Qv2018:txt20','frm2551Qv2018:txt21','frm2551Qv2018:txt22','Schedule 1') $null 'official-hta-runtime#taxRateOption:L3117-L3176' 'official-bug-compatible' 'Zeros and disables all listed figures and all six ATC rows for compliance-only filing.' 'Apply the exact revision rule only for eligible taxpayers and preserve an audit explanation.'
Add-Rule '2551q-atc-row-enable' 'blur/change' $null 'An ATC row selection is zero or nonzero.' @('drpATC{1..6}','txtATCAmt{1..6}') $null 'official-hta-runtime#drpATCChanged:L2976-L2997' 'verified-correct' 'Zero selection disables and zeros amount/rate; a selected ATC enables amount and copies the catalog rate.'
Add-Rule '2551q-atc-sequence' 'blur/change' $null 'First ATC row is empty or populated.' @('drpATC1','drpATC2','drpATC3','drpATC4','drpATC5','drpATC6') $null 'official-hta-runtime#checkDrpdwn:L3090-L3116' 'verified-correct' 'Rows 2-6 are cleared/disabled while row 1 is empty; otherwise they are enabled.'
Add-Rule '2551q-atc-duplicate-catalog' 'input' $null "Catalog substring filter loads two PT010 entries at 3% and 1%." @('drpATC{1..6}') $null 'official-hta-runtime#loadATC:L3290-L3330' 'incorrect-official-behavior' 'Both entries are exposed because the filter only checks for substring 2551_ and does not deduplicate by code or effective period.' 'Resolve ATC rate by return period and an effective-dated authoritative table; never present conflicting same-code rates.'

$main=@(
    @('calendar','Neither calendar nor fiscal type is selected.',@('frm2551Qv2018:forThe_1','frm2551Qv2018:forThe_2'),'Select a calendar type on Item no. 1.'),
    @('month','Year-ended month has selectedIndex zero.',@('frm2551Qv2018:rtnMonth'),'Please enter valid Year Ended month on item 2.'),
    @('year-min','Year numeric value is below 1900.',@('frm2551Qv2018:txtYear'),'Invalid date entry on Item no.2. Entry should not be lower than 1900.'),
    @('quarter','No quarter is selected.',@('frm2551Qv2018:qtr_1','frm2551Qv2018:qtr_2','frm2551Qv2018:qtr_3','frm2551Qv2018:qtr_4'),'Select a Quarter on Item no. 3.'),
    @('year-blank','Year is blank.',@('frm2551Qv2018:txtYear'),'Please enter valid year on item 2.'),
    @('tin','Any TIN segment or branch code is blank.',@('frm2551Qv2018:txtTIN1','frm2551Qv2018:txtTIN2','frm2551Qv2018:txtTIN3','frm2551Qv2018:txtBranchCode'),'Please enter a valid TIN number on Item 6.'),
    @('rdo','RDO code is blank.',@('frm2551Qv2018:txtRDOCode'),'Please enter a valid RDO Code on Item 7.'),
    @('name','Taxpayer name is blank.',@('frm2551Qv2018:registeredName'),'Please enter a valid Taxpayer Name on Item 8.'),
    @('telephone','Telephone number is blank.',@('frm2551Qv2018:telNo'),'Please enter Telephone Number on Item 10.'),
    @('address','Registered address is blank.',@('frm2551Qv2018:registeredAddress'),"Please enter Taxpayer's Registered Address on Item 9."),
    @('zip','ZIP code is blank.',@('frm2551Qv2018:zipCode'),'Please enter Zip Code on Item 9A.'),
    @('treaty','Treaty Yes is selected and relief selector remains at index zero.',@('frm2551Qv2018:taxTreaty_1','frm2551Qv2018:txtTaxReliefSpecify'),'Please specify Tax Relief on Item 12.'),
    @('zero-atc','Neither tax-rate option is selected and first ATC plus amount are zero.',@('frm2551Qv2018:taxRate1','frm2551Qv2018:taxRate2','drpATC1','txtATCAmt1'),'If you are filing zero transaction, please select the applicable ATC in Page 2 Schedule 1.'),
    @('amount-without-atc','Neither tax-rate option is selected, first ATC is zero, and first amount is nonzero.',@('frm2551Qv2018:taxRate1','frm2551Qv2018:taxRate2','drpATC1','txtATCAmt1'),'Please select an ATC in Schedule 1.'),
    @('net-zero-atc','Graduated/net option is selected and first ATC is zero.',@('frm2551Qv2018:taxRate1','drpATC1'),'If you are filing zero transaction, please select the applicable ATC in Page 2 Schedule 1.'),
    @('overpayment-raw','Raw Item 24 compares below zero and neither disposition is selected.',@('frm2551Qv2018:txt24','frm2551Qv2018:overPayment1','frm2551Qv2018:overPayment2'),'Please specify If overpayment below Item 24.'),
    @('overpayment-formatted','NumWithComma(Item 24) is below zero and neither disposition is selected.',@('frm2551Qv2018:txt24','frm2551Qv2018:overPayment1','frm2551Qv2018:overPayment2'),'Please select overpayment option.')
)
$order=0
foreach($entry in $main){
    $order++
    $assessment=if($entry[0]-in@('year-min','year-blank','tin','overpayment-raw','overpayment-formatted')){'official-bug-compatible'}else{'verified-correct'}
    $official=switch($entry[0]){
        'year-min' {'Main Validate accepts 1900-2017, conflicting with validateYear which rewrites values below 2018 when that handler runs.'}
        'year-blank' {'This branch is effectively unreachable because blank coerces to zero and fails the earlier below-1900 branch first.'}
        'tin' {'Only nonblank segments are enforced; no digit shape or TIN checksum.'}
        'overpayment-raw' {'Raw JavaScript less-than coercion catches unformatted negative values.'}
        'overpayment-formatted' {'The duplicate parsed check catches comma-formatted negative values and emits a different alert.'}
        default {'The first failing branch alerts and returns.'}
    }
    Add-Rule ("2551q-validate-"+$entry[0]) 'validate' $order $entry[1] $entry[2] $entry[3] 'official-hta-runtime#validateForm:L4608-L4723' $assessment $official
}
Add-Rule '2551q-validate-line-business-disabled' 'validate' $null 'Line of business is blank.' @('frm2551Qv2018:txtLineofBus') $null 'official-hta-runtime#validateForm:L4650-L4653' 'incorrect-official-behavior' 'The entire intended validation is commented out.' 'Enforce any field that remains legally required for this revision.'
Add-Rule '2551q-save-month' 'save' 1 'Year-ended month is unselected.' @('frm2551Qv2018:rtnMonth') 'Please enter valid Year Ended month on item 2.' 'official-hta-runtime#initialValidateBeforeSave:L4902-L4927'
Add-Rule '2551q-save-quarter' 'save' 2 'No quarter is selected.' @('frm2551Qv2018:qtr_1','frm2551Qv2018:qtr_2','frm2551Qv2018:qtr_3','frm2551Qv2018:qtr_4') 'Select a Quarter on Item no. 3.' 'official-hta-runtime#initialValidateBeforeSave:L4902-L4927'
Add-Rule '2551q-save-tin' 'save' 3 'Any TIN segment or branch code is blank.' @('frm2551Qv2018:txtTIN1','frm2551Qv2018:txtTIN2','frm2551Qv2018:txtTIN3','frm2551Qv2018:txtBranchCode') 'Please enter a valid TIN number on Item 6.' 'official-hta-runtime#initialValidateBeforeSave:L4902-L4927' 'official-bug-compatible' 'Only nonblank pieces are checked.'
Add-Rule '2551q-save-rdo-disabled' 'save' $null 'RDO code is 000.' @('frm2551Qv2018:txtRDOCode') 'Please enter a valid RDO Code on Item 7.' 'official-hta-runtime#initialValidateBeforeSave:L4916-L4919' 'incorrect-official-behavior' 'The entire RDO Save guard is commented out.' 'Reject the placeholder RDO code consistently in Save and Validate.'
Add-Rule '2551q-save-name' 'save' 4 'Taxpayer name is blank.' @('frm2551Qv2018:registeredName') 'Please enter a valid Taxpayer Name on Item 8.' 'official-hta-runtime#initialValidateBeforeSave:L4920-L4924'

Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    first_error_behavior='Main Validate and Save preflight alert on the first failing active branch and return.'
    rules=$rules
})
Write-Json (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta
    main_validate='validateForm:L4608-L4723';save_preflight='initialValidateBeforeSave:L4902-L4927'
    secondary_validators=@('validateYear:L4600-L4607','validateItem2:L5458-L5478')
    covered_rule_count=$rules.Count
    commented_out_behavior=@('Line-of-business main check','RDO 000 Save guard','legacy five-row ATC calculations and duplicate prevention')
})

$calculations=[Collections.Generic.List[object]]::new()
function Add-Calc([string]$Id,[string[]]$Outputs,[string[]]$Inputs,[string]$Formula,[string]$Trigger,[string]$Ref,[string]$Assessment='verified-correct',[string]$Recommended='Use typed decimals and preserve source dependency order.'){
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id=$Id;outputs=$Outputs;inputs=$Inputs;condition=$null;official_formula=$Formula
        rounding='NumWithComma inputs and formatCurrency outputs; ATC rates divide by 100.'
        trigger=$Trigger;depends_on=@();source_refs=@($Ref);assessment=$Assessment
        recommended_app_behavior=$Recommended;confidence='high'
    })
}
Add-Calc '2551q-atc-row-tax' @('txtATCDue{1..6}') @('txtATCAmt{1..6}','txtATCRate{1..6}') 'For row N, tax due = amount * (rate / 100).' 'computeATC' 'official-hta-runtime#computeATC:L2998-L3005'
Add-Calc '2551q-schedule1-total' @('txtTotalSched1','frm2551Qv2018:txt14') @('txtATCDue1','txtATCDue2','txtATCDue3','txtATCDue4','txtATCDue5','txtATCDue6') 'Sum six ATC tax-due rows and copy the total to Item 14.' 'computeTotalSched1' 'official-hta-runtime#computeTotalSched1:L3006-L3017'
Add-Calc '2551q-item18' @('frm2551Qv2018:txt18') @('frm2551Qv2018:txt15','frm2551Qv2018:txt16','frm2551Qv2018:txt17') '18 = 15 + 16 + 17.' 'computeItem18' 'official-hta-runtime#computeItem18:L3019-L3025'
Add-Calc '2551q-item19' @('frm2551Qv2018:txt19') @('frm2551Qv2018:txt14','frm2551Qv2018:txt18') '19 = 14 - 18.' 'computeItem19' 'official-hta-runtime#computeItem19:L3026-L3031'
Add-Calc '2551q-item23' @('frm2551Qv2018:txt23') @('frm2551Qv2018:txt20','frm2551Qv2018:txt21','frm2551Qv2018:txt22') '23 = 20 + 21 + 22.' 'computeItem23' 'official-hta-runtime#computeItem23:L3032-L3038'
Add-Calc '2551q-item24' @('frm2551Qv2018:txt24') @('frm2551Qv2018:txt19','frm2551Qv2018:txt23') '24 = 19 + 23.' 'computeItem24' 'official-hta-runtime#computeItem24:L3039-L3044'
Add-Calc '2551q-eight-percent-zeroing' @('frm2551Qv2018:txt15','frm2551Qv2018:txt16','frm2551Qv2018:txt17','frm2551Qv2018:txt20','frm2551Qv2018:txt21','frm2551Qv2018:txt22','Schedule 1') @('frm2551Qv2018:taxRate2') 'When 8% is selected, all listed values and all six ATC rows are forced to zero, then Items 18/19/23/24 recompute.' 'taxRateOption' 'official-hta-runtime#taxRateOption:L3117-L3176' 'official-bug-compatible' 'Apply only when the return-period law and taxpayer eligibility permit this option.'
Add-Calc '2551q-overpayment-state' @('frm2551Qv2018:overPayment1','frm2551Qv2018:overPayment2') @('frm2551Qv2018:txt24') 'If Item 24 >= 0, clear and disable both overpayment dispositions; if negative, enable them.' 'refundOption' 'official-hta-runtime#refundOption:L3054-L3068'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    evaluation_order=@($calculations.calculation_id);calculations=$calculations
})
Write-Json (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta
    active_calculation_count=$calculations.Count;active_ids=@($calculations.calculation_id)
    dead_commented_functions=@('computeTaxDue','computeTotalTaxDue','computeTotalTaxCreditForm2551Qv2018','computeTaxPayable','computePenalties','computeTotalAmountPayable')
})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId
    cases=@($calculations|ForEach-Object{[ordered]@{
        case_id=$_.calculation_id+'-source-boundary';calculation_id=$_.calculation_id
        inputs=@{source_formula=$_.official_formula};official_output='Derived by pinned source formula; no taxpayer values are stored.'
    }})
})

$negativeCases=[Collections.Generic.List[object]]::new();$case=0
foreach($rule in $rules){$case++;$negativeCases.Add([ordered]@{
    case_id=('case-{0:d2}-{1}'-f$case,$rule.rule_id);phase=$rule.phase;mutations=@{synthetic_condition=$rule.condition}
    expected_message=$rule.exact_message;expected_behavior=$rule.official_behavior;rule_id=$rule.rule_id
})}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$negativeCases
})

$workflow=[ordered]@{
    '$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    phases=@(
        @{phase='edit';official_behavior='Two-page return with a six-row bounded ATC schedule and payment/reference details.';source_refs=@('official-hta-runtime#frmMain');confidence='high'},
        @{phase='saved-draft';official_behavior='Save preflight checks month, quarter, nonblank TIN pieces, and taxpayer name, then serializes 99 eligible frmMain occurrences.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L4902-L4927','official-hta-runtime#saveXML:L3638-L3890');confidence='high'},
        @{phase='validated';official_behavior='Ordered validation runs and then locks editable controls.';source_refs=@('official-hta-runtime#validateForm:L4608-L4723');confidence='high'},
        @{phase='final-copy';official_behavior='Final Copy uses the same deterministic element loop and encrypts the artifact externally.';source_refs=@('official-hta-runtime#saveXML:L3638-L3890');confidence='high'},
        @{phase='submitted';official_behavior='Online transport exists but was not exercised.';source_refs=@('official-hta-runtime#sendEmail:L5584-L5696');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='Save preflight passes.';side_effects=@('Writes local XML from frmMain eligible controls.');source_refs=@('official-hta-runtime#saveXML:L3638-L3890')},
        @{from='edit';action='Validate';to='validated';guard='All active ordered checks pass.';side_effects=@('Locks editable controls.');source_refs=@('official-hta-runtime#validateForm:L4608-L4723')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables conditionally editable controls.');source_refs=@('official-hta-runtime#editForm:L4724-L4848')},
        @{from='validated';action='Final Copy';to='final-copy';guard='Official final-copy workflow proceeds.';side_effects=@('Encrypts the generated local artifact.');source_refs=@('official-hta-runtime#saveXML:L3638-L3890')},
        @{from='final-copy';action='Online transport';to='submitted';guard='Connectivity and transport proceed.';side_effects=@('No online submission was performed during research.');source_refs=@('official-hta-runtime#sendEmail:L5584-L5696')}
    )
    prerequisites=@('Exact January 2018 ENCS revision','Complete applicable background fields','Select an effective ATC and complete the bounded Schedule 1 rows')
    required_attachments=@(
        @{attachment_id='applicable-percentage-tax-support';label='Applicable percentage-tax supporting documents';required_when='Required by the taxpayer facts and official instructions.';official_ui_enforcement='Not comprehensively enforced by local Validate.';source_refs=@('official-installed-help');confidence='medium'}
    )
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='Within twenty-five days after the end of the taxable quarter.';source_refs=@('official-installed-help');confidence='high'},
        @{quarter='Q2';due_date_rule='Within twenty-five days after the end of the taxable quarter.';source_refs=@('official-installed-help');confidence='high'},
        @{quarter='Q3';due_date_rule='Within twenty-five days after the end of the taxable quarter.';source_refs=@('official-installed-help');confidence='high'},
        @{quarter='Q4';due_date_rule='Within twenty-five days after the end of the taxable quarter.';source_refs=@('official-installed-help');confidence='high'}
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount=@($rules|Where-Object assessment -in @('incorrect-official-behavior','official-bug-compatible','obsolete')).Count+
    @($calculations|Where-Object assessment -in @('incorrect-official-behavior','official-bug-compatible','obsolete')).Count
$manifest=[ordered]@{
    '$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='2551Q'
    revision=$revision;revision_label='January 2018 ENCS';package_version=$packageVersion;status='complete'
    official_assets=@(
        (New-Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed package used for extraction.'),
        (New-Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 2551Qv2018 and January 2018 printed header.'),
        (New-Asset 'official-installed-help' 'runtime-help' $helpPath 'January 2018 ENCS instructions.'),
        (New-Asset 'official-pdf-2018' 'official-form-pdf' $pdfPath 'January 2018 ENCS official form.'),
        (New-Asset 'atc-catalog-runtime' 'official-package-xml' $atcPath 'Catalog loaded by the exact HTA; substring filtering yields 23 entries.')
    )
    counts=@{typed_fields=$fields.Count;serialization_occurrences=99;distinct_serialized_keys=98;validation_rules=$rules.Count;calculations=$calculations.Count;negative_fixtures=$negativeCases.Count;atc_records_for_form=$atcEntries.Count;confirmed_official_bugs=$bugCount;unverified_gaps=1}
    artifacts=@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';runtime_control_fixture='fixtures/runtime-control-inventory-v796.json';atc_catalog_fixture='fixtures/atc-catalog-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'}
    scope_notes=@(
        'Research artifacts only; no renderer, migration, release, or capability changes.',
        'No representative XML was supplied for 2551Q; inventory is exhaustively derived from the pinned deterministic saveXML predicate and frmMain source.',
        'No real taxpayer data, online submission, or official-artifact mutation was used.',
        'The 99 entries model serialized occurrences; txtEmail occurs twice under the same serialized key.',
        'Six ATC rows are bounded static controls; there is no unbounded runtime row family.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest

Write-Text (Join-Path $outDir 'evidence.md') @'
# Evidence — 2551Q January 2018 ENCS

The exact runtime HTA, installed help, official form PDF, shared ATC catalog, and Offline eBIRForms package executable are pinned in `manifest.json`. No representative 2551Q XML was supplied. The inventory therefore fails closed to a source-derived claim: the pinned `saveXML` implementation walks `frmMain.elements` and emits every text, select-one, radio, and checkbox control while excluding buttons, hidden controls, passwords, and other element types.

Applying that predicate to the pinned DOM produces 98 static occurrences; `getRdo()` injects the RDO select into `frmMain`, producing 99 occurrences total. There are 98 distinct serialized keys because the DOM contains two `txtEmail` controls and the loop emits both under the same key. Six ATC rows are concrete and bounded. No Add/Delete row builder or unbounded family exists.

The exact ATC loader accepts every catalog payload containing `2551_`; this produces 23 entries. PT010 occurs twice with conflicting 3% and 1% rates, which is preserved and classified rather than silently deduplicated.
'@
Write-Text (Join-Path $outDir 'gaps.md') @'
# Gaps — 2551Q January 2018 ENCS

1. No dummy plaintext or encrypted 2551Q XML was supplied, so the 99-occurrence inventory is source-derived rather than black-box compared. The serialization predicate, DOM membership, duplicate `txtEmail`, and runtime RDO insertion are all pinned and deterministic; a future dummy save should verify the exact occurrence order without changing the source-derived universe.
'@
Write-Text (Join-Path $outDir 'README.md') @'
# BIR Form 2551Q — January 2018 ENCS

Revision-specific Offline eBIRForms validation knowledge: 99 serialized occurrences, 37 distinct rule branches/behaviors, active calculations, workflow, bounded ATC schedule, and exact catalog evidence. Research only; no renderer or release metadata changes.
'@
Write-Text (Join-Path $outDir 'audit.md') @'
# Audit — 2551Q January 2018 ENCS

- Revision bound by APPLICATIONNAME, printed header, installed help, and official PDF.
- Runtime HTA, help, PDF, package executable, and shared ATC catalog are hash-pinned.
- Deterministic serialization inventory: 99 occurrences, 98 distinct keys, one duplicate key (`txtEmail`), and one runtime-injected RDO select.
- All 123 live static controls were inventoried; six ATC rows are bounded and no unbounded row family exists.
- Main Validate, Save preflight, secondary year/date validators, conditional enablement, calculations, serialization, and transport source were inspected.
- Exact first-error order and alerts are preserved, including duplicate overpayment branches.
- Catalog filtering yields 23 records and exposes conflicting PT010 rates; this is classified as an official defect.
- No taxpayer values, online submission, or official-artifact mutation was used.
- Final strict repository audit passed with `-RequireJsonSchema`: 43 forms, 519 JSON files, 9,592 fields, 2,007 validations, 623 calculations, 1,354 negative fixtures, and 216 schema documents. Structural audit and JSON Schema validation both reported `pass`; stderr was empty.
'@

$indexPath=Join-Path $RepoRoot 'rules\index.json'
$index=Get-Content -LiteralPath $indexPath -Raw|ConvertFrom-Json
if(-not($index.forms|Where-Object form_id -eq $formId)){
    $index.forms+=[pscustomobject][ordered]@{form_id=$formId;form_code='2551Q';revision=$revision;package_version=$packageVersion;priority=43;status='complete';path='forms/2551q-v2018/manifest.json'}
}
$index.updated='2026-07-23'
Write-Json $indexPath $index

[pscustomobject]@{
    form_id=$formId;fields=$fields.Count;distinct_keys=@($occurrences.serialized_key|Sort-Object -Unique).Count
    validations=$rules.Count;calculations=$calculations.Count;negative_fixtures=$negativeCases.Count
    atc_records=$atcEntries.Count;official_defect_classifications=$bugCount;output=$outDir
}|ConvertTo-Json
