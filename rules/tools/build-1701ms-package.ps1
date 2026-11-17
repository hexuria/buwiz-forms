param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$HtaPath = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}\forms\BIR-Form1701MS.hta',
    [string]$SavePath = 'C:\eBIRForms\savefile\00000000000000-1701MS-122025.xml'
)

$ErrorActionPreference = 'Stop'
$formId = '1701ms-v2024'
$revision = '2024-08-01'
$packageVersion = '7.9.6.0'
$outDir = Join-Path $RepoRoot 'rules\forms\1701ms-v2024'
$fixtureDir = Join-Path $outDir 'fixtures'
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 40) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path, [string]$Value) { [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false)) }
function Get-Attr([string]$Tag,[string]$Name) {
    $m=[regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1'-f[regex]::Escape($Name)))
    if($m.Success){$m.Groups[2].Value}else{$null}
}
function Get-HashText([string[]]$Lines) {
    $sha=[Security.Cryptography.SHA256]::Create();try{$bytes=[Text.Encoding]::UTF8.GetBytes(($Lines-join"`n"));([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-','').ToLowerInvariant()}finally{$sha.Dispose()}
}

foreach($path in @($HtaPath,$SavePath)){if(-not(Test-Path -LiteralPath $path)){throw "Missing required source: $path"}}
$hta=[IO.File]::ReadAllText($HtaPath)
$scriptRanges=@([regex]::Matches($hta,'<script\b.*?</script>','IgnoreCase,Singleline'))
$controls=@();$ordinal=0
foreach($m in [regex]::Matches($hta,'<(input|select|textarea|button)\b[^>]*>','IgnoreCase,Singleline')){
    $inside=$false;foreach($s in $scriptRanges){if($m.Index-ge$s.Index-and$m.Index-lt($s.Index+$s.Length)){$inside=$true;break}}
    if($inside){continue};$ordinal++;$tag=$m.Value;$element=$m.Groups[1].Value.ToLowerInvariant();$kind=if($element-eq'input'){Get-Attr $tag 'type'}else{$element};if(-not$kind){$kind='text'}
    $controls += [pscustomobject][ordered]@{ordinal=$ordinal;id=Get-Attr $tag 'id';name=Get-Attr $tag 'name';element=$element;control_kind=$kind.ToLowerInvariant();source_line=1+[regex]::Matches($hta.Substring(0,$m.Index),"`n").Count;value=Get-Attr $tag 'value';maxlength=Get-Attr $tag 'maxlength';disabled=$tag-match'(?i)\bdisabled(?:\s*=|\s|>)';readonly=$tag-match'(?i)\breadonly(?:\s*=|\s|>)'}
}
if($controls.Count-ne300){throw "Expected 300 static controls; found $($controls.Count)."}

$saveText=[IO.File]::ReadAllText($SavePath)
$saveMatches=[regex]::Matches($saveText,'<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>')
$observedKeys=@($saveMatches|ForEach-Object{$_.Groups['key'].Value})
if($observedKeys.Count-ne201-or($observedKeys|Sort-Object -Unique).Count-ne201){throw "Expected 201 unique save keys; found $($observedKeys.Count)."}
$otherSave='C:\eBIRForms\savefile\00000000000000-1701MS-122025V1.xml'
$otherText=[IO.File]::ReadAllText($otherSave);$otherKeys=@([regex]::Matches($otherText,'<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>')|ForEach-Object{$_.Groups['key'].Value})
if((Compare-Object ($observedKeys|Sort-Object) ($otherKeys|Sort-Object)).Count-ne0){throw 'The two representative save key sets differ.'}

$controlById=@{};foreach($c in $controls){if($c.id-and-not$controlById.ContainsKey($c.id)){$controlById[$c.id]=$c}}
$required=@('frm1701MS:txtMonthNo1','frm1701MS:txtYearNo1','frm1701MS:txtPg2TIN1','frm1701MS:txtPg2TIN2','frm1701MS:txtPg2TIN3','frm1701MS:txtPg2BranchCode','frm1701MS:txtRDOCodea','frm1701MS:txtTaxpayerNo8a','frm1701MS:txtEmaila','frm1701MS:txtTelNum10a','frm1701MS:perjuryClause')
$computedPattern='(?i)(Total|Taxable|TaxDue|TaxPayable|AmountPayable|NetSpecial|Aggregate|TotalTaxRelief|txtCompensationBusiness|txtIncomeTax|txtGrossIncome|txtOptionalDeduction|txtNetIncomeLoss)'

function Field-Meta([string]$key,$control,[bool]$family){
    $label=$key;$page=$null;$item=$null;$logical='string';$status='optional';$constraints=[ordered]@{};$enum=[object[]]@();$normalization=[string[]]@();$computed=$false;$calc=$null;$kind=if($control){$control.control_kind}elseif($family){'runtime-indexed-family'}else{'serialized-runtime-control'}
    if($key-match'No(?<n>\d+)[ab]?(?:\{|$)'){$item=$Matches.n}
    if($key-eq'frm1701MS:txtMonthNo1'){$label='Return-period month';$item='1';$logical='month';$enum=[object[]]@('01','02','03','04','05','06','07','08','09','10','11','12');$status='required'}
    elseif($key-eq'frm1701MS:txtYearNo1'){$label='Return-period year';$item='1';$logical='integer';$constraints.minimum=2024;$status='required'}
    elseif($key-match'(Amended|ReturnPeriod|IfMarried|TaxpayerNo1[127]|SpouseNo17|ToBeRefunded$|ToBeIssued$|ToBeCarried$|perjuryClause)'){$logical='boolean';$enum=[object[]]@('true','false')}
    elseif($key-eq'frm1701MS:civilStatus'){$logical='enum';$enum=[object[]]@('Single','Married','Legally Separated','Widow/er')}
    elseif($key-match'(TIN[123]|BranchCode|RDOCode)'){$logical='code'}
    elseif($key-match'(Email)'){$logical='email-string'}
    elseif($key-match'(TelNum)'){$logical='phone-string'}
    elseif($key-match'(Date|No16[ab])'){$logical='date-string';$constraints.format='implementation passes the string to JavaScript Date'}
    elseif($key-match'(taxCodeDropdown)'){$logical='enum';$enum=[object[]]@('II011','II012','II013','II014','II015','II016','II017')}
    elseif($key-match'(?i)(Amount|Tax|Income|Gross|Sales|Revenue|Deductions|Deduction|Loss|Credits|Payments|Penalties|Surchange|Interest|Compromise|Share|Rate|Relief|Less|PriorYears)'){$logical='whole-peso-amount';$normalization=[string[]]@('NumWithComma','Math.round where invoked','formatCurrencyWithComma')}
    if($required-contains$key){$status='required'}
    if($key-match$computedPattern-and$key-notmatch'(?i)(TaxpayerNo1[12]|TaxRate[ab]|SpecialTaxCredits|ForeignTaxCredits|TaxDueAllowed|TaxPaidPreviously)'){$computed=$true;$status='computed';$calc='See calculations.json'}
    if($key-match'^(txtFinalFlag|txtEnroll|ebirOnline|driveSelectTPExport)'){$status='hidden';$kind='hidden/workflow-metadata'}
    if($control-and$control.maxlength){$constraints.max_length=[int]$control.maxlength}
    if($family){$constraints.index='N >= 0; no runtime maximum';$status='conditional'}
    [pscustomobject]@{label=$label;page=$page;item=$item;logical=$logical;status=$status;constraints=[pscustomobject]$constraints;enum=$enum;normalization=$normalization;computed=$computed;calc=$calc;kind=$kind}
}

$fields=@()
foreach($key in $observedKeys){
    $control=if($controlById.ContainsKey($key)){$controlById[$key]}else{$null};$meta=Field-Meta $key $control $false;$refs=@("xml-editable-v1#field:$key");if($control){$refs+="official-hta-runtime#control:L$($control.source_line)"}else{$refs+='official-hta-runtime#saveXML:L6776-L7060'}
    $fields += [pscustomobject][ordered]@{field_key=$key;serialized_key=$key;serialized_occurrence=1;label=$meta.label;page=$meta.page;item_number=$meta.item;control_kind=$meta.kind;storage_type='string';logical_type=$meta.logical;required=$meta.status;required_when=$null;enabled_when=$null;visible_when=$null;default_value=$null;empty_representation='';constraints=$meta.constraints;enum_values=$meta.enum;normalization=$meta.normalization;computed=$meta.computed;calculation_id=$meta.calc;source_refs=$refs;confidence=if($control){'high'}else{'medium'};notes=@('Observed in both dummy plaintext saves. Values are URL-encoded strings and are intentionally excluded.')}
}

$dynamicFamilies=@(
    @{prefix='frm1701MS:sched410A:Description';lines='12960,13206'},@{prefix='frm1701MS:sched410A:Amount';lines='12964,13210'},
    @{prefix='frm1701MS:sched410aSpouse:Description';lines='13277,13537'},@{prefix='frm1701MS:sched410aSpouse:Amount';lines='13281,13541'},
    @{prefix='frm1701MS:sched410A1:Description';lines='13742,13878'},@{prefix='frm1701MS:sched410A1:LegalBasis';lines='13746,13882'},@{prefix='frm1701MS:sched410A1:Amount';lines='13750,13886'},
    @{prefix='frm1701MS:sched410A2:Description';lines='14089,14234'},@{prefix='frm1701MS:sched410A2:LegalBasis';lines='14093,14238'},@{prefix='frm1701MS:sched410A2:Amount';lines='14097,14242'},
    @{prefix='frm1701MS:sched413A:Description';lines='14430,14556'},@{prefix='frm1701MS:sched413A:Amount';lines='14434,14560'},
    @{prefix='frm1701MS:Sched413B:Description';lines='14645,14662'},@{prefix='frm1701MS:Sched413B:Amount';lines='14649,14666'},
    @{prefix='frm1701MS:sched420A:Description';lines='15014,15271'},@{prefix='frm1701MS:sched420A:Amount';lines='15018,15275'},
    @{prefix='frm1701MS:sched42OB:Description';lines='15333,15583'},@{prefix='frm1701MS:sched420B:Amount';lines='15337,15587'},
    @{prefix='frm1701MS:sched5No9:Description';lines='15678,15753'},@{prefix='frm1701MS:sched5No9:Amount';lines='15679,15757'},@{prefix='frm1701MS:sched5No9:Spouse';lines='15680,15761'}
)
foreach($family in $dynamicFamilies){$key="$($family.prefix){N>=0}";$meta=Field-Meta $key $null $true;$fields += [pscustomobject][ordered]@{field_key=$key;serialized_key=$null;serialized_occurrence=$null;label="Indexed schedule field $($family.prefix)";page=$null;item_number=$meta.item;control_kind='runtime-indexed-family';storage_type='string';logical_type=$meta.logical;required='conditional';required_when='A corresponding modal row N exists.';enabled_when=$null;visible_when='A corresponding modal row N exists.';default_value=$null;empty_representation='';constraints=$meta.constraints;enum_values=$meta.enum;normalization=$meta.normalization;computed=$false;calculation_id=$null;source_refs=@("official-hta-runtime#dynamic-id:L$($family.lines)",'official-hta-runtime#saveXMLsubmit:L7494-L7628');confidence='high';notes=@('Unbounded indexed family. The exact casing/character spelling is significant and preserves source defects.')}}
if($fields.Count -ne 222 -or ($fields.field_key | Sort-Object -Unique).Count -ne 222){throw "Expected 222 unique fields; found $($fields.Count)."}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;field_count=$fields.Count;inventory_sha256=Get-HashText @($fields.field_key|Sort-Object);fields=$fields})

$runtimeControls=foreach($c in $controls){[pscustomobject][ordered]@{ordinal=$c.ordinal;id=$c.id;name=$c.name;element=$c.element;control_kind=$c.control_kind;source_line=$c.source_line;value=$c.value;maxlength=$c.maxlength;disabled=$c.disabled;readonly=$c.readonly;serializable_by_representative_save=($c.id-and$observedKeys-contains$c.id)}}
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v47.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;application_version='4.7';official_hta_sha256=(Get-FileHash -LiteralPath $HtaPath -Algorithm SHA256).Hash.ToLowerInvariant();static_control_count=$controls.Count;static_controls_with_id_count=@($controls|Where-Object id).Count;static_controls_without_id_count=@($controls|Where-Object{-not$_.id}).Count;representative_save_key_count=$observedKeys.Count;unbounded_family_count=$dynamicFamilies.Count;static_controls=$runtimeControls;unbounded_dynamic_families=$dynamicFamilies})
Write-Json (Join-Path $fixtureDir 'atc-options-v47.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;source='Hard-coded dropdowns; shared atcCodes.xml contains no 1701MS record.';entries=@(
    @{code='II011';description='Compensation Income'},@{code='II012';description='Business Income - Graduated IT Rates'},@{code='II013';description='Mixed Income - Graduated IT Rates'},@{code='II014';description='Income from Profession - Graduated IT Rates'},@{code='II015';description='Business Income - 8% IT Rate'},@{code='II016';description='Mixed Income - 8% IT Rate'},@{code='II017';description='Income from Profession - 8% IT Rate'})})

$functionTool=Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
$validationInventory=(& $functionTool -HtaPath $HtaPath -NamePrefixes 'validate,validation,initialValidate,enforceSelectionRules,checkButtons') -join [Environment]::NewLine
$calculationInventory=(& $functionTool -HtaPath $HtaPath -NamePrefixes 'compute,calculate,totalTax,creditable,taxRate,toBeCarried') -join [Environment]::NewLine
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v47.json') $validationInventory
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v47.json') $calculationInventory

$rules=[Collections.Generic.List[object]]::new()
function Rule([string]$id,[string]$phase,$order,[string]$condition,[string[]]$fieldKeys,[string]$accepted,[string]$rejected,$message,[string[]]$refs,[string]$assessment,[string]$official,[string]$recommended,[string]$confidence='high'){$rules.Add([pscustomobject][ordered]@{rule_id=$id;form_id=$formId;revision=$revision;phase=$phase;order=$order;condition=$condition;fields=$fieldKeys;accepted_behavior=$accepted;rejected_behavior=$rejected;exact_message=$message;source_refs=$refs;evidence_type=@('source');assessment=$assessment;official_behavior=$official;recommended_app_behavior=$recommended;confidence=$confidence;unresolved_questions=@()})}

Rule '1701ms-save-001' 'save' 1 'Any taxpayer TIN segment or branch code on the first-page copy is blank.' @('frm1701MS:txtPg1TIN1','frm1701MS:txtPg1TIN2','frm1701MS:txtPg1TIN3','frm1701MS:txtPg1BranchCode') 'All four are nonblank.' 'Save is blocked.' 'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L5533-L5539') 'incorrect-official-behavior' 'Save checks only nonblank TIN components and nothing else.' 'Preserve incomplete drafts losslessly; validate TIN shape/checksum before finalization.'
Rule '1701ms-save-002' 'save' 2 'Any non-TIN field, spouse field, schedule, calculation, or perjury choice is invalid.' @('return-body') 'Save still proceeds after the four nonblank TIN checks.' 'No rejection occurs.' $null @('official-hta-runtime#initialValidateBeforeSave:L5533-L5539','official-hta-runtime#saveXML:L6776-L7060') 'official-bug-compatible' 'The draft preflight is intentionally narrow.' 'Keep lossless draft saving but surface completeness separately.'

$order=0
function V([string]$suffix,[string]$condition,[string[]]$fieldKeys,$message,[string]$lines,[string]$assessment='verified-correct',[string]$official='The first matching branch alerts and returns.',[string]$recommended='Retain as a structured field error.'){$script:order++;Rule "1701ms-validate-$suffix" 'validate' $script:order $condition $fieldKeys 'Condition is false and validation continues.' $official $message @("official-hta-runtime#validate:L$lines") $assessment $official $recommended}
V '001' 'Non-short-period return year is greater than or equal to the current calendar year.' @('frm1701MS:txtReturnPeriodYesNo3','frm1701MS:txtYearNo1') 'Future filing is not allowed. Please input correct year on Item 1' '7644-L7650' 'ambiguous' 'Current-year annual filing is rejected along with future years.' 'Apply the filing-period rules from the August 2024 guide explicitly.'
V '002' 'Short-period month is later than current month and year is current/future.' @('frm1701MS:txtReturnPeriodYesNo3','frm1701MS:txtMonthNo1','frm1701MS:txtYearNo1') 'Future filing is not allowed. Please input correct year on Item 1' '7651-L7656' 'official-bug-compatible' 'The message cites year even when month triggers.' 'Use a period-specific message.'
V '003' 'Civil status is Married and no Item 5 filing choice is checked.' @('frm1701MS:civilStatus','frm1701MS:txtIfMarriedJointlyNo5','frm1701MS:txtIfMarriedSeparatelyNo5','frm1701MS:txtIfMarriedNotApplicableNo5') 'Item 5 is required if Item 4 is marked as Married.' '7661-L7670'
foreach($entry in @(
    @('004','Joint filing and any spouse TIN component is blank.',@('frm1701MS:txtPg2TIN1Spouse','frm1701MS:txtPg2TIN2Spouse','frm1701MS:txtPg2TIN3Spouse','frm1701MS:txtPg2BranchCodeSpouse'),'Please enter a valid TIN number on Item 6 for Spouse','7674-L7683','incorrect-official-behavior','Only nonblankness is checked.'),
    @('005','Joint filing and spouse RDO is blank.',@('frm1701MS:txtRDOCodeb'),'Please enter a valid RDO code for Spouse on Item 7','7684-L7687','verified-correct','Blank spouse RDO is rejected.'),
    @('006','Joint filing and spouse name is blank.',@('frm1701MS:txtTaxpayerNo8b'),'Please enter a valid Name for Spouse on Item 8','7688-L7691','verified-correct','Blank spouse name is rejected.'),
    @('007','Joint filing and spouse email is blank.',@('frm1701MS:txtEmailb'),'Please enter a valid Email Address for Spouse on Item 9','7692-L7696','incorrect-official-behavior','Only nonblankness is checked; malformed email passes.'),
    @('008','Joint filing and spouse contact number is blank.',@('frm1701MS:txtTelNum10b'),'Please enter a valid Contact Number for Spouse on Item 10','7698-L7702','incorrect-official-behavior','Only nonblankness is checked; malformed phone passes.'),
    @('009','Joint filing and taxpayer Item 11 has no selection.',@('frm1701MS:txtTaxpayerNo11a','frm1701MS:txtTaxpayerNo11b','frm1701MS:txtTaxpayerNo11c'),'Please select an option on Item 11 for Taxpayer','7704-L7711','incorrect-official-behavior','Taxpayer Item 11 is checked only inside the joint-filing branch.'),
    @('010','Joint filing and spouse Item 11 has no selection.',@('frm1701MS:txtTaxpayerNo11a1','frm1701MS:txtTaxpayerNo11b1','frm1701MS:txtTaxpayerNo11c1','frm1701MS:txtTaxpayerNo11d1'),'Please select an option on Item 11 for Spouse','7713-L7721','verified-correct','Joint spouse source of income is required.'),
    @('011','Joint filing and spouse Item 12 has no selection.',@('frm1701MS:txtTaxpayerNo12b1','frm1701MS:txtTaxpayerNo12c1','frm1701MS:txtTaxpayerNo12d1','frm1701MS:txtTaxpayerNo12e1'),'Item 12 is required if Item 5 is marked as Jointly.','7723-L7731','verified-correct','Joint spouse tax regime is required.'),
    @('012','Joint spouse uses relevant business income with graduated rates and no Item 17 deduction method.',@('frm1701MS:txtSpouseNo17a','frm1701MS:txtSpouseNo17b'),'Item 17 is mandatory if Item 11 is Income from Business, Mixed Income and Income from Profession and Item 12 is Graduated Income Tax Rates.','7741-L7748','verified-correct','Deduction method is required for the branch.'),
    @('013','Aggregate Item 30 is negative and no Item 31 disposition is checked.',@('frm1701MS:txtAggregateAmountPayable30a','frm1701MS:txtToBeRefunded','frm1701MS:txtToBeIssued','frm1701MS:txtToBeCarried'),'Item 31 is mandatory because Item 30 is has negative value.','7768-L7778','official-bug-compatible','Negative aggregate requires disposition; message has duplicated grammar.'),
    @('014','Selected Item 31 amounts do not exactly equal abs(Item 27A + Item 27B).',@('frm1701MS:txtAmountPayableNo27a','frm1701MS:txtAmountPayableNo27b','frm1701MS:txtToBeRefunded1','frm1701MS:txtToBeIssued1','frm1701MS:txtToBeCarried1'),'The total amount in Item 31 must equal the sum of Item 27A and Item 27B ({amount}).','7780-L7791','ambiguous','Strict JavaScript number equality is used against abs(27A+27B), not Item 30.'),
    @('015','Foreign-tax amount exists in either column but Item 7 reference text is blank.',@('frm1701MS:addForeignTaxCredits','frm1701MS:txtForeignTaxCreditsNo7a','frm1701MS:txtForeignTaxCreditsNo7b'),'Part V - Item 7 Foreign Tax Credits is mandatory if the amount column has value.','7852-L7868','verified-correct','Cross-column reference requirement is active.'),
    @('016','Foreign-tax reference exists but both amount columns are blank/zero.',@('frm1701MS:addForeignTaxCredits','frm1701MS:txtForeignTaxCreditsNo7a','frm1701MS:txtForeignTaxCreditsNo7b'),'Part V - Item 7 Foreign Tax Amount is mandatory if Foreign Tax Credits field has value.','7869-L7874','verified-correct','At least one amount is required.'),
    @('017','Other-credit amount exists in either column but Item 9 description is blank.',@('frm1701MS:addOtherCreditsPayments','frm1701MS:txtOtherCreditsPaymentsNo9a','frm1701MS:txtOtherCreditsPaymentsNo9b'),'Part V - Item 9 Other Credits/Payments is mandatory if the Amount column has value.','7876-L7892','verified-correct','Cross-column description requirement is active.'),
    @('018','Other-credit description exists but both amount columns are blank/zero.',@('frm1701MS:addOtherCreditsPayments','frm1701MS:txtOtherCreditsPaymentsNo9a','frm1701MS:txtOtherCreditsPaymentsNo9b'),'Part V - Item 9 Amount is mandatory if the Other Credits/Payments field has value.','7893-L7898','verified-correct','At least one amount is required.')
)){V $entry[0] $entry[1] $entry[2] $entry[3] $entry[4] $entry[5] $entry[6] 'Retain with decimal-safe presence tests.'}
V '019' 'Year or month is NaN.' @('frm1701MS:txtYearNo1','frm1701MS:txtMonthNo1') 'Please enter a valid year and month.' '7919-L7923' 'official-bug-compatible' 'Coercive isNaN is used.' 'Validate exact month/year syntax.'
V '020' 'Year is greater than current year.' @('frm1701MS:txtYearNo1') 'Future filing is not allowed. Please enter a valid year.' '7925-L7929'
V '021' 'Year strictly equals numeric currentYear and month is later than current month.' @('frm1701MS:txtYearNo1','frm1701MS:txtMonthNo1') 'Future filing is not allowed. Please enter a valid month.' '7931-L7935' 'incorrect-official-behavior' 'The DOM year is a string, so strict equality with numeric currentYear is false and this duplicate guard does not fire.' 'Parse year once and compare typed values.'
V '022' 'Any taxpayer TIN component on the page-two copy is blank.' @('frm1701MS:txtPg2TIN1','frm1701MS:txtPg2TIN2','frm1701MS:txtPg2TIN3','frm1701MS:txtPg2BranchCode') 'Please enter a valid TIN number on Item 6' '7938-L7945' 'incorrect-official-behavior' 'Only nonblankness is checked.' 'Validate lengths, digits, branch, and checksum.'
V '023' 'Month or year is blank.' @('frm1701MS:txtMonthNo1','frm1701MS:txtYearNo1') 'Item 1 is a required field.' '7949-L7954'
foreach($entry in @(
 @('024','Taxpayer RDO is blank.',@('frm1701MS:txtRDOCodea'),'Please enter a valid RDO code on Item 7','7956-L7960'),
 @('025','Taxpayer name is blank.',@('frm1701MS:txtTaxpayerNo8a'),'Please enter a valid Taxpayer Name on Item 8','7962-L7966'),
 @('026','Taxpayer email is blank.',@('frm1701MS:txtEmaila'),'Please enter a valid Email Address on Item 9','7970-L7974'),
 @('027','Taxpayer contact number is blank.',@('frm1701MS:txtTelNum10a'),'Please enter a valid Contact Number on Item 10','7976-L7980'),
 @('028','Taxpayer Item 12 has no selection.',@('frm1701MS:txtTaxpayerNo12b','frm1701MS:txtTaxpayerNo12c','frm1701MS:txtTaxpayerNo12d','frm1701MS:txtTaxpayerNo12e'),'Taxpayer Item 12 is a required field ','7984-L7987'),
 @('029','Taxpayer exempt/special regime selected and any Item 13-16 detail is blank.',@('frm1701MS:txtTaxpayerNo13a','frm1701MS:txtTaxpayerNo14a','frm1701MS:txtTaxpayerNo15a','frm1701MS:txtTaxpayerNo16a','frm1701MS:txtTaxpayerNo16a1'),'Please make sure to enter details on Item 13 to 16 on Taxpayer','7991-L7995'),
 @('030','Spouse exempt/special regime selected and any Item 13-16 detail is blank.',@('frm1701MS:txtTaxpayerNo13b','frm1701MS:txtTaxpayerNo14b','frm1701MS:txtTaxpayerNo15b','frm1701MS:txtTaxpayerNo16b','frm1701MS:txtTaxpayerNo16b1'),'Please make sure to enter details on Item 13 to 16 on Spouse','7997-L8001'),
 @('031','Taxpayer business/mixed/profession with graduated rates and no Item 17 choice.',@('frm1701MS:txtTaxpayerNo17a','frm1701MS:txtTaxpayerNo17b'),'Item 17 is mandatory if Item 11 is Income from Business, Mixed Income and Income from Profession and Item 12 is Graduated Income Tax Rates.','8003-L8009'),
 @('032','Perjury clause is unchecked.',@('frm1701MS:perjuryClause'),'You need to agree to the Perjury Clause.','8014-L8022')
)){V $entry[0] $entry[1] $entry[2] $entry[3] $entry[4]}
V '033' 'Taxpayer Item 11 is blank during a non-joint filing.' @('frm1701MS:txtTaxpayerNo11a','frm1701MS:txtTaxpayerNo11b','frm1701MS:txtTaxpayerNo11c') 'Validation successful. Click on Edit if you wish to modify your entries.' '7704-L7711,8031-L8033' 'incorrect-official-behavior' 'The Item 11 check is nested inside the joint-filing branch, so non-joint returns bypass it.' 'Require taxpayer source of income for every applicable filer.'
V '034' 'Amended yes/no, short-period yes/no, civil status, amounts, schedules, or computed consistency is otherwise missing/invalid.' @('return-body') 'Validation successful. Click on Edit if you wish to modify your entries.' '7629-L8034' 'incorrect-official-behavior' 'No general completeness or calculation-consistency pass exists.' 'Validate every applicable branch and recompute before locking.'

Rule '1701ms-change-001' 'blur/change' $null 'Short-period return is selected with month/year not equal to the current month/year.' @('frm1701MS:txtReturnPeriodYesNo3','frm1701MS:txtMonthNo1','frm1701MS:txtYearNo1') 'Current month/year passes.' 'Change validation rejects.' 'For short period return, the month should be the current month and the year should be the current year.' @('official-hta-runtime#validateCurrentYear:L9692-L9727') 'ambiguous' 'Short-period filing is hard-coded to the runtime current month.' 'Use the legally valid short-period end date range.'
Rule '1701ms-change-002' 'blur/change' $null 'Non-short-period month/year is not December of prior current year.' @('frm1701MS:txtReturnPeriodYesNo3','frm1701MS:txtMonthNo1','frm1701MS:txtYearNo1') 'December/prior year returns true.' 'Returns false without an alert.' $null @('official-hta-runtime#validateCurrentYear:L9722-L9727') 'official-bug-compatible' 'Silent false result can leave unclear UI state.' 'Return a structured period error.'
Rule '1701ms-change-003' 'blur/change' $null 'Taxpayer/spouse Item 16 From is on/after the filing month end or To is before it.' @('frm1701MS:txtTaxpayerNo16a','frm1701MS:txtTaxpayerNo16a1','frm1701MS:txtTaxpayerNo16b','frm1701MS:txtTaxpayerNo16b1') 'Dates straddle the filing month end.' 'Alerts and returns false.' 'Invalid date. {Taxpayer|Spouse} Item 16 {From|To} date must be {boundary wording}.' @('official-hta-runtime#validateEffectivityDateTaxpayer16:L10666-L10706','official-hta-runtime#validateEffectivityDateSpouse16:L10708-L10747') 'incorrect-official-behavior' 'Invalid/blank Date objects can bypass comparisons; From must be strictly earlier, not equal.' 'Parse strict dates and encode inclusive boundaries from the guide.'
Rule '1701ms-validate-035' 'validate' 35 'Spouse Item 12e alone is selected and Item 16 dates are invalid.' @('frm1701MS:txtTaxpayerNo12e1','frm1701MS:txtTaxpayerNo16b','frm1701MS:txtTaxpayerNo16b1') 'The invalid dates pass this call site.' 'No spouse effectivity validation occurs.' $null @('official-hta-runtime#validate:L7733-L7739') 'incorrect-official-behavior' 'The source compares the element object to true instead of checking .checked for Item 12e1.' 'Invoke date validation for both 12d1 and 12e1 checked states.'
Rule '1701ms-change-004' 'blur/change' $null 'Item 26 exceeds either 50% of Item 23 or Item 25.' @('frm1701MS:txtTaxDueAllowedNo26a','frm1701MS:txtTaxDueAllowedNo26b','frm1701MS:txtTotalIncomeTaxDueNo23a','frm1701MS:txtTotalIncomeTaxDueNo23b','frm1701MS:txtTaxPayableNo25a','frm1701MS:txtTaxPayableNo25b') 'Input is within both caps.' 'Value resets to 0 and focus returns.' 'The amount exceeds the allowed input based on the requirements' @('official-hta-runtime#validateInput26a:L11420-L11459','official-hta-runtime#validateInput26b:L11461-L11500') 'verified-correct' 'Both caps are enforced using JavaScript numbers.' 'Use whole-peso decimal/integer arithmetic.'
Rule '1701ms-change-005' 'blur/change' $null 'Refund and Tax Credit Certificate are both selected.' @('frm1701MS:txtToBeRefunded','frm1701MS:txtToBeIssued') 'At most one of this pair.' 'Tax Credit Certificate is unchecked.' "You cannot select BOTH 'To be refunded' and 'To be issued a Tax Credit Certificate'." @('official-hta-runtime#enforceSelectionRules:L10918-L10979') 'verified-correct' 'Mutually exclusive pair enforced.' 'Represent as an explicit incompatibility constraint.'
Rule '1701ms-change-006' 'blur/change' $null 'More than two Item 31 choices would be selected.' @('frm1701MS:txtToBeRefunded','frm1701MS:txtToBeIssued','frm1701MS:txtToBeCarried') 'At most two remain selectable.' 'Unchecked choices are disabled.' $null @('official-hta-runtime#enforceSelectionRules:L10964-L10976') 'verified-correct' 'UI limits selection count.' 'Enforce without silently clearing amounts.'
Rule '1701ms-change-007' 'blur/change' $null 'Taxpayer or spouse mixed-income ATC mapping reaches the final exempt/special branch.' @('taxCodeDropdown','taxCodeDropdownSpouse') 'Intended combinations map to II013.' 'Operator precedence and missing .checked permit special alone or mixed with a truthy element to map II013.' $null @('official-hta-runtime#checkButtons:L11109-L11113','official-hta-runtime#checkButtonsSP:L11190-L11194') 'incorrect-official-behavior' 'Boolean expressions use element objects and unparenthesized OR.' 'Use explicit checked booleans and a table-driven mapping.'
Rule '1701ms-change-008' 'blur/change' $null 'AllControlDisabled is invoked.' @('frm1701MS:txtAmendedYesNo2','frm1701MS:txtAmendedNoNo2') 'No reliable behavior.' 'ReferenceError because documentary is undefined.' $null @('official-hta-runtime#AllControlDisabled:L9677-L9680') 'incorrect-official-behavior' 'The first DOM access uses documentary instead of document.' 'Remove or correct the dead/broken helper.'

$scheduleRules=@(
 @('410a','Schedule 4 Item 10A taxpayer','frm1701MS:sched410A:Description{N>=0}','frm1701MS:sched410A:Amount{N>=0}','13229-L13239'),
 @('410a-sp','Schedule 4 Item 10A spouse','frm1701MS:sched410aSpouse:Description{N>=0}','frm1701MS:sched410aSpouse:Amount{N>=0}','13557-L13567'),
 @('410b-tp','Schedule 4 Item 10B taxpayer','frm1701MS:sched410A1:Description{N>=0}','frm1701MS:sched410A1:Amount{N>=0}','13902-L13912'),
 @('410b-sp','Schedule 4 Item 10B spouse','frm1701MS:sched410A2:Description{N>=0}','frm1701MS:sched410A2:Amount{N>=0}','14257-L14268'),
 @('413a','Schedule 4 Item 13 taxpayer','frm1701MS:sched413A:Description{N>=0}','frm1701MS:sched413A:Amount{N>=0}','14576-L14586'),
 @('413b','Schedule 4 Item 13 spouse','frm1701MS:Sched413B:Description{N>=0}','frm1701MS:Sched413B:Amount{N>=0}','14682-L14692'),
 @('420a','Schedule 4 Item 20 taxpayer','frm1701MS:sched420A:Description{N>=0}','frm1701MS:sched420A:Amount{N>=0}','15290-L15300'),
 @('420b','Schedule 4 Item 20 spouse','frm1701MS:sched42OB:Description{N>=0}','frm1701MS:sched420B:Amount{N>=0}','15603-L15613'))
foreach($s in $scheduleRules){Rule "1701ms-modal-$($s[0])" 'page navigation' $null "$($s[1]) row has blank description/required legal basis or amount string equal to 0." @($s[2],$s[3]) 'Every materialized row passes.' 'First incomplete row is rejected.' 'Empty fields are not allowed' @("official-hta-runtime#schedule-row-validation:L$($s[4])") 'official-bug-compatible' 'String equality to 0 misses formatted zero variants.' 'Parse the amount and require a positive whole-peso value.'}
Rule '1701ms-modal-5-9' 'page navigation' $null 'Part V Item 9 row description is blank or both taxpayer/spouse amount strings equal 0.' @('frm1701MS:sched5No9:Description{N>=0}','frm1701MS:sched5No9:Amount{N>=0}','frm1701MS:sched5No9:Spouse{N>=0}') 'Description plus either column amount passes.' 'Row rejected.' 'Empty fields are not allowed' @('official-hta-runtime#validationSpecify:L14619-L14628') 'official-bug-compatible' 'Formatted 0.00 may bypass string comparison.' 'Require description and at least one positive parsed amount.'
Rule '1701ms-final-001' 'final-copy' 1 'Final Copy is requested.' @('txtFinalFlag') 'Confirmation/connectivity workflow proceeds.' 'Offline/encrypted-copy state differs by transport result.' $null @('official-hta-runtime#openAlertEmail:L6479-L6573','official-hta-runtime#saveEncryptedProfile:L6684-L6775') 'official-bug-compatible' 'Final copy remains coupled to transport workflow.' 'Create deterministic offline final copy independently of submission.'
Rule '1701ms-submit-001' 'submit' 1 'Online transport is invoked.' @('return-body') 'Encrypted payload is prepared.' 'No online submission was exercised in this research.' $null @('official-hta-runtime#sendEmail:L6574-L6659','official-hta-runtime#saveXMLsubmit:L7494-L7628') 'unverified' 'Source-derived only.' 'Keep local validation testable without network.' 'medium'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{'$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;first_error_behavior='validate() and each modal row validator alert and return at the first active failing branch. Change handlers may also reset/disable values immediately.';rules=$rules})

$calcs=[Collections.Generic.List[object]]::new()
function Calc([string]$id,[string[]]$outputs,[string[]]$inputs,[string]$formula,[string]$trigger,[string[]]$depends,[string[]]$refs,[string]$assessment='verified-correct',[string]$recommended='Use whole-peso decimal/integer arithmetic and recompute from authoritative inputs.'){$calcs.Add([pscustomobject][ordered]@{calculation_id=$id;outputs=$outputs;inputs=$inputs;condition=$null;official_formula=$formula;rounding='Whole-peso form: Math.round is used at tax-table/rate steps; formatCurrencyWithComma adds separators without a single consistent decimal policy.';trigger=$trigger;depends_on=$depends;source_refs=$refs;assessment=$assessment;recommended_app_behavior=$recommended;confidence='high'})}
Calc '1701ms-regular-tax-table' @('regular-income-tax') @('taxable-income') '0 through 250,000: 0; <=400,000: 15% of excess over 250,000; <=800,000: 22,500 + 20% over 400,000; <=2,000,000: 102,500 + 25% over 800,000; <=8,000,000: 402,500 + 30% over 2,000,000; above 8,000,000: 2,202,500 + 35% over 8,000,000.' 'computeRegularIncomeTax' @() @('official-hta-runtime#computeRegularIncomeTax:L12870-L12895')
Calc '1701ms-part4a-item3' @('frm1701MS:txtTaxableCompensationIncomea','frm1701MS:txtTaxableCompensationIncomeb') @('frm1701MS:txtGrossa','frm1701MS:txtGrossb','frm1701MS:txtLessNonTaxablea','frm1701MS:txtLessNonTaxableb') 'Item 3 = gross compensation - non-taxable compensation.' 'calculateNo3TP/SP' @() @('official-hta-runtime#calculateNo3TP:L11647-L11688','official-hta-runtime#calculateNo3SP:L11690-L11738')
Calc '1701ms-part4a-item4' @('frm1701MS:txtTaxpayerNo4a','frm1701MS:txtTaxpayerNo4b') @('frm1701MS:txtTaxableCompensationIncomea','frm1701MS:txtTaxableCompensationIncomeb') 'Item 4 = round(regular tax table(Item 3)).' 'calculateNo3TP/SP' @('1701ms-regular-tax-table','1701ms-part4a-item3') @('official-hta-runtime#calculateNo3TP:L11663-L11668','official-hta-runtime#calculateNo3SP:L11707-L11712')
Calc '1701ms-b1-item7' @('frm1701MS:txtTaxableCompensationIncomeNo7a','frm1701MS:txtTaxableCompensationIncomeNo7b') @('frm1701MS:txtSalesRevenueNo5a','frm1701MS:txtSalesRevenueNo5b','frm1701MS:txtSalesReturnsNo6a','frm1701MS:txtSalesReturnsNo6b') '7 = sales/revenue/fees - returns/allowances/discounts.' 'calculateNo7a/b' @() @('official-hta-runtime#calculateNo7a:L11741-L11748','official-hta-runtime#calculateNo7b:L11750-L11757')
Calc '1701ms-b1-item9' @('frm1701MS:txtGrossIncomeNo9a','frm1701MS:txtGrossIncomeNo9b') @('frm1701MS:txtTaxableCompensationIncomeNo7a','frm1701MS:txtTaxableCompensationIncomeNo7b','frm1701MS:txtCostofSalesNo8a','frm1701MS:txtCostofSalesNo8b') '9 = 7 - 8.' 'computeNo9/computeNo9b' @('1701ms-b1-item7') @('official-hta-runtime#computeNo9:L11760-L11769','official-hta-runtime#computeNo9b:L11771-L11780')
Calc '1701ms-b1-item10d' @('frm1701MS:txtTotalAllowableItemixedDeductionsNo10a','frm1701MS:txtTotalAllowableItemixedDeductionsNo10b') @('item10A','item10B','item10C') '10D = 10A + 10B + 10C.' 'computeTotalAllowableDeductionsNo10a/b' @() @('official-hta-runtime#computeTotalAllowableDeductionsNo10a:L11783-L11788','official-hta-runtime#computeTotalAllowableDeductionsNo10b:L11790-L11795')
Calc '1701ms-b1-item11-osd' @('frm1701MS:txtOptionalDeductionNo11a','frm1701MS:txtOptionalDeductionNo11b') @('frm1701MS:txtTaxableCompensationIncomeNo7a','frm1701MS:txtTaxableCompensationIncomeNo7b') 'If OSD selected, 11 = round(7 * 40%).' 'computeOptionalDeductionNo11a/b' @('1701ms-b1-item7') @('official-hta-runtime#computeOptionalDeductionNo11a:L11798-L11806','official-hta-runtime#computeOptionalDeductionNo11b:L11808-L11816')
Calc '1701ms-b1-item12' @('frm1701MS:txtNetIncomeLossNo12a','frm1701MS:txtNetIncomeLossNo12b') @('item9','item10D','item11','deduction-method') 'Itemized: 12 = 9 - 10D; OSD: 12 = 9 - 11.' 'computeNetIncomeLossNo12TP/SP' @('1701ms-b1-item9','1701ms-b1-item10d','1701ms-b1-item11-osd') @('official-hta-runtime#computeNetIncomeLossNo12TP:L11819-L11839','official-hta-runtime#computeNetIncomeLossNo12SP:L11841-L11858')
Calc '1701ms-b1-item14' @('frm1701MS:txtTaxableIncomeBusinesNo14a','frm1701MS:txtTaxableIncomeBusinesNo14b') @('frm1701MS:txtNetIncomeLossNo12a','frm1701MS:txtNetIncomeLossNo12b','frm1701MS:txtNonOperatingIncomeNo13a','frm1701MS:txtNonOperatingIncomeNo13b') '14 = 12 + 13.' 'computeTotalIncomeBusinessNo14TP/SP' @('1701ms-b1-item12') @('official-hta-runtime#computeTotalIncomeBusinessNo14TP:L11861-L11876','official-hta-runtime#computeTotalIncomeBusinessNo14SP:L11878-L11891')
Calc '1701ms-b1-item15' @('frm1701MS:txtCompensationBusinessNo15a','frm1701MS:txtCompensationBusinessNo15b') @('part4A-item3','part4B1-item14','income-source','tax-regime') 'Generally sum positive Item 3 and positive Item 14; spouse compensation-only uses Item 14 alone; 8% branches do not compute.' 'computeTaxableIncome15TP/SP' @('1701ms-part4a-item3','1701ms-b1-item14') @('official-hta-runtime#computeTaxableIncome15TP:L11894-L11931','official-hta-runtime#computeTaxableIncome15SP:L11933-L11978') 'ambiguous' 'Re-derive spouse compensation-only handling against the official guide before implementation.'
Calc '1701ms-b1-item17' @('frm1701MS:txtTaxDueSpeciala','frm1701MS:txtTaxDueSpecialb') @('item9','item14','item16-rate','tax-regime') 'Exempt branch: round(Item14 * rate/100); special/preferential branch: round(max(0,Item9) * rate/100).' 'computeTaxDueNo17TP/SP' @('1701ms-b1-item9','1701ms-b1-item14') @('official-hta-runtime#computeTaxDueNo17TP:L11980-L12017','official-hta-runtime#computeTaxDueNo17SP:L12020-L12057') 'ambiguous' 'Preserve both formulas and verify the surprising taxable calculation for the exempt branch.'
Calc '1701ms-b1-item18a' @('frm1701MS:TotalTaxDueCompensationATP','frm1701MS:TotalTaxDueCompensationASP') @('item17','part4A-item4','income-source','tax-regime') 'For applicable exempt/special paths, 18A is generally Item17 + compensation tax Item4; mixed special copies Item17 directly to Part II Item19.' 'totalTaxDue18ATP/ASP' @('1701ms-b1-item17','1701ms-part4a-item4') @('official-hta-runtime#totalTaxDue18ATP:L12096-L12112','official-hta-runtime#totalTaxDue18ASP:L12114-L12130')
Calc '1701ms-b1-item18b' @('frm1701MS:TotalTaxDueCompensationBTP','frm1701MS:TotalTaxDueCompensationBSP') @('frm1701MS:txtCompensationBusinessNo15a','frm1701MS:txtCompensationBusinessNo15b') 'If graduated rates, 18B = round(regular tax table(max-valid Item15)); negative/non-finite becomes 0.' 'totalTaxDue18BTP/BSP' @('1701ms-regular-tax-table','1701ms-b1-item15') @('official-hta-runtime#totalTaxDue18BTP:L12135-L12160','official-hta-runtime#totalTaxDue18BSP:L12162-L12187')
Calc '1701ms-b2-item21' @('frm1701MS:txtTotalIncomeNo21a','frm1701MS:txtTotalIncomeNo21b') @('item19','item20') '21 = 19 + 20.' 'computeTotalIncomeNo21a/b' @() @('official-hta-runtime#computeTotalIncomeNo21a:L12190-L12195','official-hta-runtime#computeTotalIncomeNo21b:L12197-L12202')
Calc '1701ms-b2-item23' @('frm1701MS:txtTaxableIncomeLoss23A','frm1701MS:txtTaxableIncomeLoss23B') @('item21','item22') '23 = 21 - 22.' 'computeIncomeLoss/computeIncomeLossb' @('1701ms-b2-item21') @('official-hta-runtime#computeIncomeLoss:L12205-L12209','official-hta-runtime#computeIncomeLossb:L12212-L12216')
Calc '1701ms-b2-item24' @('frm1701MS:txtTaxDueBusinessIncome24A','frm1701MS:txtTaxDueBusinessIncome24B') @('item23') 'If Item23 >= 1, 24 = round(Item23 * 8%); otherwise 0.' 'computeBusinessIncome/computeBusinessIncomeb' @('1701ms-b2-item23') @('official-hta-runtime#computeBusinessIncome:L12219-L12231','official-hta-runtime#computeBusinessIncomeb:L12234-L12246')
Calc '1701ms-b2-item25' @('frm1701MS:txtCompensationBusinessNo25a','frm1701MS:txtCompensationBusinessNo25b','frm1701MS:txtIncomeTaxRegularNo22a','frm1701MS:txtIncomeTaxRegularNo22b') @('part4A-item4','part4B2-item24','8%-regime') 'If 8% selected, 25 = compensation tax Item4 + business tax Item24, and Part II Item22 receives the same value; otherwise 0.' 'totalTaxDueCompensationAndBusinessIncome25TP/SP' @('1701ms-part4a-item4','1701ms-b2-item24') @('official-hta-runtime#totalTaxDueCompensationAndBusinessIncome25TP:L12250-L12261','official-hta-runtime#totalTaxDueCompensationAndBusinessIncome25SP:L12264-L12275')
Calc '1701ms-part2-item19' @('frm1701MS:txtIncomeTaxDueNo19a','frm1701MS:txtIncomeTaxDueNo19b') @('item17','item18A','tax-regime') 'Special copies Item17; exempt writes 0; otherwise copies Item18A.' 'computeIncomeTaxDueNo19a/b' @('1701ms-b1-item17','1701ms-b1-item18a') @('official-hta-runtime#computeIncomeTaxDueNo19a:L11203-L11221','official-hta-runtime#computeIncomeTaxDueNo19b:L11239-L11258')
Calc '1701ms-part2-item21' @('frm1701MS:txtNetSpecialRateNo21a','frm1701MS:txtNetSpecialRateNo21b') @('item19','item20') '21 = 19 - share of other agency Item20.' 'calculateSpecialRateNo21a/b' @('1701ms-part2-item19') @('official-hta-runtime#calculateSpecialRateNo21a:L11276-L11285','official-hta-runtime#calculateSpecialRateNo21b:L11287-L11297')
Calc '1701ms-part2-item22' @('frm1701MS:txtIncomeTaxRegularNo22a','frm1701MS:txtIncomeTaxRegularNo22b') @('tax-regime','part4A-item4','part4B1-item18B','part4B2-item25') 'Conditional copy from the regular-tax source appropriate to exempt/special, graduated, compensation-only, or 8% branches.' 'computeP2No22TP/SP and B2 Item25' @('1701ms-part4a-item4','1701ms-b1-item18b','1701ms-b2-item25') @('official-hta-runtime#computeP2No22TP:L11300-L11320','official-hta-runtime#computeP2No22SP:L11322-L11350')
Calc '1701ms-part2-item23' @('frm1701MS:txtTotalIncomeTaxDueNo23a','frm1701MS:txtTotalIncomeTaxDueNo23b') @('item21','item22') '23 = 21 + 22.' 'computeP2No23TP/SP' @('1701ms-part2-item21','1701ms-part2-item22') @('official-hta-runtime#computeP2No23TP:L11353-L11358','official-hta-runtime#computeP2No23SP:L11360-L11364')
Calc '1701ms-part5-item10' @('frm1701MS:txtTotalTaxCreditsPaymentsPartVNo10a','frm1701MS:txtTotalTaxCreditsPaymentsPartVNo10b') @('Part V Items 1..9 by column') '10 = sum Items 1 through 9, treating parse failures as 0.' 'computeCreditsPaymentsPartVNo10a/b' @() @('official-hta-runtime#computeCreditsPaymentsPartVNo10a:L12742-L12751','official-hta-runtime#computeCreditsPaymentsPartVNo10b:L12754-L12763')
Calc '1701ms-part2-item24' @('frm1701MS:txtTotalTaxCreditsPartIINo24a','frm1701MS:txtTotalTaxCreditsPartIINo24b') @('Part V Item10') 'Part II 24 copies Part V Item10.' 'computeP2No24TP/SP' @('1701ms-part5-item10') @('official-hta-runtime#computeP2No24TP:L11367-L11369','official-hta-runtime#computeP2No24SP:L11371-L11373')
Calc '1701ms-part2-item25' @('frm1701MS:txtTaxPayableNo25a','frm1701MS:txtTaxPayableNo25b') @('item23','item24') '25 = 23 - 24.' 'computeP2No25TP/SP' @('1701ms-part2-item23','1701ms-part2-item24') @('official-hta-runtime#computeP2No25TP:L11376-L11383','official-hta-runtime#computeP2No25SP:L11397-L11404')
Calc '1701ms-part2-item27' @('frm1701MS:txtAmountPayableNo27a','frm1701MS:txtAmountPayableNo27b') @('item25','item26') '27 = 25 - 26.' 'calculateP2No27TP/SP' @('1701ms-part2-item25') @('official-hta-runtime#calculateP2No27TP:L11503-L11507','official-hta-runtime#calculateP2No27SP:L11509-L11513')
Calc '1701ms-part2-item28' @('frm1701MS:txtTotalPenaltiesNo28a','frm1701MS:txtTotalPenaltiesNo28b') @('surcharge','interest','compromise') '28D = surcharge + interest + compromise.' 'computeTotalPenaltiesNo28a/b' @() @('official-hta-runtime#computeTotalPenaltiesNo28a:L11516-L11519','official-hta-runtime#computeTotalPenaltiesNo28b:L11521-L11524')
Calc '1701ms-part2-item29' @('frm1701MS:txtTotalAmountPayableNo29a','frm1701MS:txtTotalAmountPayableNo29b') @('item27','item28D') 'If Item27 < 0 and penalties > 0, Item29 = penalties only; otherwise Item29 = Item27 + penalties.' 'computeTotalAmountPayableNo29a/b' @('1701ms-part2-item27','1701ms-part2-item28') @('official-hta-runtime#computeTotalAmountPayableNo29a:L11527-L11542','official-hta-runtime#computeTotalAmountPayableNo29b:L11544-L11558') 'official-bug-compatible' 'Verify whether a negative Item27 should offset penalties; preserve official behavior separately.'
Calc '1701ms-part2-item30' @('frm1701MS:txtAggregateAmountPayable30a') @('item29A','item29B') 'If one Item29 is negative and the other positive, discard the negative and use the positive; if both negative or both nonnegative, add them.' 'computeAggregateAmountNo30' @('1701ms-part2-item29') @('official-hta-runtime#computeAggregateAmountNo30:L11561-L11622') 'incorrect-official-behavior' 'Do not discard one spouse column when aggregating; re-derive from the official guide.'
Calc '1701ms-part6-relief' @('frm1701MS:totalTaxReliefAvailmentA','frm1701MS:totalTaxReliefAvailmentB') @('item10B','item14','item17','Part V item8') 'max(0, regularTax(item10B + item14) - item17 + special tax credits), rounded.' 'computeTaxAvailmentTP/SP' @('1701ms-regular-tax-table','1701ms-b1-item14','1701ms-b1-item17') @('official-hta-runtime#computeTaxAvailmentTP:L12278-L12300','official-hta-runtime#computeTaxAvailmentSP:L12302-L12324')
Calc '1701ms-indexed-schedule-totals' @('Items 10A, 10B, 13, 20, and Part V Item9 modal totals') @('21 indexed runtime families') 'Each modal sums its materialized Amount column(s), rounds with Math.round in several schedules, and copies the total into the corresponding fixed return field.' 'computeOtherSched410A through computeOtherSpecifySpouse' @() @('official-hta-runtime#computeOtherSched410A:L13157-L13173','official-hta-runtime#computeOtherSched410aSpouse:L13317-L13345','official-hta-runtime#computeOthersched410A1:L13818-L13842','official-hta-runtime#computeOthersched410A2:L14164-L14198','official-hta-runtime#computeOtherSched413A:L14496-L14517','official-hta-runtime#computeOtherSched413B:L14904-L14961','official-hta-runtime#computeOtherSched420A:L15199-L15236','official-hta-runtime#computeOtherSched420B:L15514-L15549','official-hta-runtime#computeOtherSpecify:L15692-L15716') 'official-bug-compatible' 'Use exact integer summation; preserve ID spelling defects only in compatibility import.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=@($calcs.calculation_id);calculations=$calcs})

$negativeIds=@('1701ms-validate-001','1701ms-validate-003','1701ms-validate-004','1701ms-validate-009','1701ms-validate-011','1701ms-validate-013','1701ms-validate-014','1701ms-validate-015','1701ms-validate-016','1701ms-validate-017','1701ms-validate-018','1701ms-validate-019','1701ms-validate-021','1701ms-validate-022','1701ms-validate-023','1701ms-validate-024','1701ms-validate-025','1701ms-validate-026','1701ms-validate-027','1701ms-validate-028','1701ms-validate-029','1701ms-validate-031','1701ms-validate-032','1701ms-validate-033','1701ms-validate-035','1701ms-change-003','1701ms-change-004','1701ms-change-005','1701ms-change-007','1701ms-change-008','1701ms-modal-410a','1701ms-modal-420b','1701ms-modal-5-9','1701ms-save-002')
$cases=@();$n=0;foreach($id in $negativeIds){$n++;$r=$rules | Where-Object rule_id -eq $id;$cases += [pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}'-f$n,$id);phase=$r.phase;mutations=@{synthetic_condition=$r.condition};expected_message=$r.exact_message;expected_behavior=$r.official_behavior;rule_id=$id}}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
$calcCases=foreach($c in $calcs){[pscustomobject][ordered]@{case_id="$($c.calculation_id)-formula";calculation_id=$c.calculation_id;inputs=@{source_formula=$c.official_formula};official_output='Source-derived formula fixture; numeric boundary vectors are included for the tax table only.'}}
$calcCases += @(
 @{case_id='tax-table-250000';calculation_id='1701ms-regular-tax-table';inputs=@{taxable_income=250000};official_output='0'},
 @{case_id='tax-table-400000';calculation_id='1701ms-regular-tax-table';inputs=@{taxable_income=400000};official_output='22500'},
 @{case_id='tax-table-800000';calculation_id='1701ms-regular-tax-table';inputs=@{taxable_income=800000};official_output='102500'},
 @{case_id='tax-table-2000000';calculation_id='1701ms-regular-tax-table';inputs=@{taxable_income=2000000};official_output='402500'},
 @{case_id='tax-table-8000000';calculation_id='1701ms-regular-tax-table';inputs=@{taxable_income=8000000};official_output='2202500'},
 @{case_id='aggregate-discard-negative';calculation_id='1701ms-part2-item30';inputs=@{item29a=-100;item29b=200};official_output='200';recommended_output='100'})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=$calcCases})

$workflow=[ordered]@{'$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;phases=@(
 @{phase='edit';official_behavior='Multi-part taxpayer/spouse annual return with conditional schedules and nine unbounded modal row groups.';source_refs=@('official-hta-runtime#setFieldTPSPEnabled:L8628-L8867','official-hta-runtime#dynamic-schedules:L12929-L15966');confidence='high'},
 @{phase='saved-draft';official_behavior='Save requires only four nonblank taxpayer TIN components and serializes flat DOM state.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L5533-L5539','official-hta-runtime#saveXML:L6776-L7060');confidence='high'},
 @{phase='validated';official_behavior='Ordered validate branches run and then all fields disable on success.';source_refs=@('official-hta-runtime#validate:L7629-L8034','official-hta-runtime#disabledAllFields:L9117-L9130');confidence='high'},
 @{phase='final-copy';official_behavior='Final Copy is coupled to confirmation, encryption, and connectivity state.';source_refs=@('official-hta-runtime#openAlertEmail:L6479-L6573','official-hta-runtime#saveEncryptedProfile:L6684-L6775');confidence='high'},
 @{phase='submitted';official_behavior='Online payload path exists but was not exercised.';source_refs=@('official-hta-runtime#sendEmail:L6574-L6659','official-hta-runtime#saveXMLsubmit:L7494-L7628');confidence='medium'})
 transitions=@(
 @{from='edit';action='Save';to='saved-draft';guard='Four first-page taxpayer TIN components are nonblank.';side_effects=@('Writes plaintext local save.');source_refs=@('official-hta-runtime#initialValidateBeforeSave:L5533-L5539')},
 @{from='edit';action='Validate';to='validated';guard='All active ordered main rules pass.';side_effects=@('Disables fields.','Shows success alert.');source_refs=@('official-hta-runtime#validate:L7629-L8034')},
 @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables applicable controls.');source_refs=@('official-hta-runtime#enableAllControl:L9131-L9629')},
 @{from='validated';action='Final Copy';to='final-copy';guard='User confirms coupled workflow.';side_effects=@('Creates encrypted artifact.','Updates final-copy state.');source_refs=@('official-hta-runtime#openAlertEmail:L6479-L6573')},
 @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and transport succeed.';side_effects=@('Sends encrypted payload; not exercised here.');source_refs=@('official-hta-runtime#sendEmail:L6574-L6659')})
 prerequisites=@('August 2024 revision','Applicable taxpayer/spouse identity and income-source selections','Applicable schedules and tax-regime details','Perjury clause consent')
 required_attachments=@(
 @{attachment_id='bir-2316';label='BIR Form No. 2316';required_when='Creditable tax withheld from compensation is claimed.';official_ui_enforcement='ATC selection enables the corresponding Part V field; attachment itself is not locally enforced.';source_refs=@('official-guide-2024#attachments','official-hta-runtime#creditableTaxWithheldForm2316:L12766-L12815');confidence='high'},
 @{attachment_id='foreign-tax-proof';label='Proof of foreign tax credits';required_when='Part V Item 7 foreign tax credit is claimed.';official_ui_enforcement='Local Validate checks reference/amount pairing, not attachment presence.';source_refs=@('official-guide-2024#attachments','official-hta-runtime#validate:L7852-L7874');confidence='medium'},
 @{attachment_id='authorization';label='Authorization letter';required_when='Filed by an authorized representative.';official_ui_enforcement='Not enforced by local Validate.';source_refs=@('official-guide-2024#attachments');confidence='medium'})
 filing_deadlines=@()}
foreach($q in @('Q1','Q2','Q3','Q4')){$workflow.filing_deadlines += @{quarter=$q;due_date_rule='Annual return deadline and short-period rules are defined by the August 2024 official guide; the runtime itself hard-codes non-short-period filing to December of the prior calendar year and short-period filing to the current month/year.';source_refs=@('official-guide-2024#filing-deadline','official-hta-runtime#validateCurrentYear:L9692-L9727');confidence='medium'}}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount=@($rules | Where-Object assessment -in @('incorrect-official-behavior','official-bug-compatible','obsolete')).Count
$manifest=[ordered]@{'$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='1701-MS';revision=$revision;revision_label='August 2024';package_version=$packageVersion;status='complete';official_assets=@(
 @{asset_id='package-7.9.6';kind='official-package-executable';path='C:\eBIRForms\BIRForms.exe';sha256='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca';size=57506304},
 @{asset_id='official-hta-runtime';kind='runtime-extracted-hta';path=$HtaPath;sha256='5737ddbd86d467457b613cc1f016c51d1add7fa91b396f6af88d213bccc48202';size=1061606;revision_binding='Application 1701MS version 4.7 and printed August 2024 header.'},
 @{asset_id='official-pdf-2024';kind='official-form-pdf';path='C:\Mac\Home\Downloads\forms\1701MSv2024\1701-MS August 2024 Fillable_01.pdf';sha256='802912a60607bdd437faf429105e0efbcbd862894825b0682ded188c8a1aa38c';size=585707;revision_binding='Official August 2024 fillable form.'},
 @{asset_id='official-guide-2024';kind='official-guide-pdf';path='C:\Mac\Home\Downloads\forms\1701MSv2024\1701-MS Guide August 2024 ENCS_Final.pdf';sha256='263c9087feedc4d50e39544117026fa20378b25982bfb953e063c5f99c5c461e';size=102380;revision_binding='Official August 2024 guide.'},
 @{asset_id='xml-editable-v1';kind='dummy-profile-editable-save';path=$SavePath;sha256='ee1ed0aa2ced7bb1e9e7311f315869b3c4e5433e30c5dd4485ad160e24b3feb7';size=15151;revision_binding='Dummy 201-key plaintext save; values excluded.'},
 @{asset_id='xml-amended-v1';kind='dummy-profile-editable-save';path=$otherSave;sha256='3706b188c688656a95c7b43bfdd81306e5e0e0f0a35ba9394be28e8b780612a7';size=15175;revision_binding='Dummy version save with identical 201-key set.'},
 @{asset_id='string-util';kind='runtime-javascript';path='runtime-extraction\js\string-util.js';sha256='bc7f86f70bf993389a3a0135dcbd76c3e370c49d2eb95e2fc66ff318a2ebe43c';size=54582},
 @{asset_id='string-util-2014';kind='runtime-javascript';path='runtime-extraction\js\string-util2014.js';sha256='ca42592694e7416a15eca97fa25491c01da17e383038fc97dd9d6261e67bcf7d';size=15980},
 @{asset_id='ebir-tools';kind='runtime-vbscript';path='runtime-extraction\js\eBIRTools.vbs';sha256='7d0ceb5aad2c0eb90aeca189d6104ff05163ecd1820379f456125634ff7460f7';size=7557})
 counts=@{typed_fields=$fields.Count;validation_rules=$rules.Count;calculations=$calcs.Count;negative_fixtures=$cases.Count;atc_records_for_form=7;confirmed_official_bugs=$bugCount;unverified_gaps=2};artifacts=@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';runtime_control_fixture='fixtures/runtime-control-inventory-v47.json';atc_catalog_fixture='fixtures/atc-options-v47.json';calculation_fixtures='fixtures/calculation-boundaries.json'};scope_notes=@('Research artifacts only; no renderer, migration, release, or capability changes.','No online submission or mutation of source saves/encrypted artifacts.','222 inventory entries comprise 201 observed concrete keys and 21 unbounded indexed families.','The missing copied 2200C.js reference is recorded as a defect, not treated as evidence.')}
Write-Json (Join-Path $outDir 'manifest.json') $manifest

Write-Utf8 (Join-Path $outDir 'README.md') @"
# BIR Form 1701-MS — August 2024

Revision-specific validation knowledge for Offline eBIRForms application version 4.7. The package preserves 201 concrete save keys and 21 unbounded indexed runtime families, including the official casing and `sched42OB` spelling defects. It separates verified rules, compatibility behavior, and recommended application behavior.

Research only. No online submission, renderer change, or release-metadata change was performed.
"@
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence — 1701-MS August 2024

The exact HTA, official fillable PDF, official guide, two dummy plaintext saves, package executable, and linked shared scripts are pinned in `manifest.json`. Both PDFs have `%PDF-` magic. Both saves contain the same 201 unique keys; values and the profile email are not copied.

Static inspection found 300 controls (296 with IDs) and 21 unbounded indexed input families across nine modal row groups. The source contains 20 validation-related functions with 64 alert sites and 71 calculation-related functions; their names, ranges, hashes, alerts, and referenced controls are retained in the function-inventory fixtures.

The HTA hard-codes seven ATC choices because the shared `atcCodes.xml` contains no `1701MS` record. It also references `../js/lib/2200C.js`, which is absent from the extraction tree.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps — 1701-MS August 2024

1. Online submission and destructive black-box save mutation were not performed. Transport behavior is source-derived.
2. The 21 indexed families are genuinely unbounded, so no finite concrete runtime count exists. PDF guide text could not be mechanically extracted in the Windows runtime; attachment/deadline statements remain medium confidence where only guide headings and source behavior bind them.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit — 1701-MS August 2024

- Revision bound by application identity/version, printed header, PDF, and guide.
- 201 observed unique save keys plus 21 explicit unbounded families = 222 inventory entries.
- Both dummy save variants have identical key sets.
- 300 static controls: 296 with IDs, four without IDs.
- Main Validate, narrow Save preflight, conditional enablement, nine modal row groups, 71 calculation-related functions, final-copy, and transport paths inspected.
- Seven hard-coded ATCs recorded; shared catalog has no 1701MS record.
- Confirmed hazards include nested taxpayer Item 11 validation, spouse `.checked` omission, strict string/number year comparison, operator-precedence ATC mapping, invalid-Date bypasses, inconsistent/corrupt dynamic IDs, missing 2200C.js, and Item 30 discarding one negative spouse column.
- Fixtures use dummy mutations only and bind to stable rule/calculation IDs.
"@
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') @"
# Handoff

- Completed: 1701-MS August 2024 (`1701ms-v2024`)
- Inventory: 222 entries (201 concrete + 21 unbounded families)
- Rules: $($rules.Count)
- Calculations: $($calcs.Count)
- Negative fixtures: $($cases.Count)
- Hard-coded ATCs: 7
- Next priority: 1701
"@

$indexPath=Join-Path $RepoRoot 'rules\index.json';$index=Get-Content -LiteralPath $indexPath -Raw|ConvertFrom-Json
if(-not($index.forms | Where-Object form_id -eq $formId)){$index.forms += [pscustomobject][ordered]@{form_id=$formId;form_code='1701-MS';revision=$revision;package_version=$packageVersion;priority=9;status='complete';path='forms/1701ms-v2024/manifest.json'}}
$index.updated='2026-07-23';Write-Json $indexPath $index
[pscustomobject]@{form_id=$formId;fields=$fields.Count;validations=$rules.Count;calculations=$calcs.Count;negative_fixtures=$cases.Count;official_defect_classifications=$bugCount;output=$outDir}|ConvertTo-Json
