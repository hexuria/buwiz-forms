param(
  [Parameter(Mandatory=$true)][string]$XmlPath,
  [Parameter(Mandatory=$true)][string]$OutputPath
)
$ErrorActionPreference='Stop'
$text=[IO.File]::ReadAllText($XmlPath)
$entries=[Collections.Generic.List[object]]::new()
foreach($m in [regex]::Matches($text,'<div>(?<key>.*?)=(?<value>.*?)\k<key>=</div>','Singleline')){
  $entries.Add([ordered]@{key=$m.Groups['key'].Value;value=$m.Groups['value'].Value;source='representative-xml'})
}
# Current package catalog has 41 Private slots. The representative no-withholding save
# materializes 40 selectors and six computation rows, so add the maximum runtime union.
if(-not ($entries.key -contains 'AtcCd41')){$entries.Add([ordered]@{key='AtcCd41';value='false';source='runtime-union'})}
foreach($row in 7..41){
  foreach($prefix in @('txtAtcCode','txtTaxBase','txtTaxRate','txtTaxbeWithHeld')){
    $key="frm1601FQ:$prefix$row"
    if(-not ($entries.key -contains $key)){
      $value=if($prefix -eq 'txtTaxbeWithHeld'){'0.00'}else{''}
      $entries.Add([ordered]@{key=$key;value=$value;source='runtime-union'})
    }
  }
}
$required=@(
 'frm1601FQ:txtYear','frm1601FQ:txtTIN1','frm1601FQ:txtTIN2','frm1601FQ:txtTIN3','frm1601FQ:txtBranchCode',
 'frm1601FQ:txtRDOCode','frm1601FQ:txtTaxpayerName','frm1601FQ:txtAddress','frm1601FQ:txtZipCode','frm1601FQ:txtTelNum','txtEmail'
)
function Get-Page([string]$key){if($key -match 'Pg2|^(drpTreatyCode|drpATCCode|txtNatIncPayment|txtAmtIncomePay|txtRate|txtReqWithheld|txtDvTotalSchedI)'){2}else{1}}
function Get-Item([string]$key){
  if($key -match 'txtYear'){return '1'};if($key -match 'OptQuarter'){return '2'};if($key -match 'AmendedRtn'){return '3'}
  if($key -match 'TaxWithheld'){return '4'};if($key -match 'txtSheets'){return '5'};if($key -match 'txtTIN|txtBranchCode'){return '6'}
  if($key -match 'txtRDOCode'){return '7'};if($key -match 'txtTaxpayerName'){return '8'};if($key -match 'txtAddress|txtZipCode'){return '9'}
  if($key -match 'txtTelNum'){return '10'};if($key -match 'CatAgent'){return '11'};if($key -eq 'txtEmail'){return '12'}
  if($key -match 'SpecialTax|drpSpecialTax'){return '13'};if($key -match 'txtTax(\d+)$'){return $Matches[1]}
  return $null
}
function Get-Label([string]$key){
  $map=@{
   'frm1601FQ:txtYear'='Filing year';'frm1601FQ:txtRDOCode'='RDO code';'frm1601FQ:txtTaxpayerName'='Withholding agent name';
   'frm1601FQ:txtAddress'='Registered address';'frm1601FQ:txtZipCode'='ZIP code';'frm1601FQ:txtTelNum'='Telephone number';'txtEmail'='Email address';
   'frm1601FQ:txtTotalOtherTax'='Additional ATC tax total';'txtDvTotalSchedI'='Schedule 1 required withholding total'
  }
  if($map.ContainsKey($key)){return $map[$key]}
  if($key -match 'txtTIN([123])$'){return "TIN segment $($Matches[1])"};if($key -match 'txtBranchCode$'){return 'TIN branch code'}
  if($key -match 'OptQuarter([1-4])$'){return "Quarter $($Matches[1]) selection"};if($key -match 'AtcCd(\d+)$'){return "ATC catalog slot $($Matches[1]) selection"}
  if($key -match 'txtAtcCode(\d+)$'){return "Part II row $($Matches[1]) ATC code"};if($key -match 'txtTaxBase(\d+)$'){return "Part II row $($Matches[1]) tax base"}
  if($key -match 'txtTaxRate(\d+)$'){return "Part II row $($Matches[1]) tax rate"};if($key -match 'txtTaxbeWithHeld(\d+)$'){return "Part II row $($Matches[1]) tax withheld"}
  if($key -match '^(drpTreatyCode|drpATCCode|txtNatIncPayment|txtAmtIncomePay|txtRate|txtReqWithheld)(\d+)$'){return "Schedule 1 row $($Matches[2]) $($Matches[1])"}
  return ($key -replace '^frm1601FQ:','')
}
$fields=@()
foreach($entry in $entries){
  $key=$entry.key;$bool=$entry.value -in @('true','false') -or $key -match '^(AtcCd)|:(OptQuarter|AmendedRtn|TaxWithheld|CatAgent|SpecialTax)'
  $money=$key -match '(TaxBase|TaxRate|TaxbeWithHeld|txtTax\d+|txtAmount|txtAmtIncomePay|txtRate\d|txtReqWithheld|txtTotalOtherTax|txtDvTotalSchedI)'
  $computed=$key -match '(txtTaxbeWithHeld|txtTax2[0-9]|txtTax3[0-2]|txtTotalOtherTax|txtReqWithheld|txtDvTotalSchedI)'
  $hidden=$key -match '^(txtFinalFlag|txtEnroll|ebirOnline|driveSelectTPExport)|:txt(Current|Max)Page$'
  $normalization=@();if($money){$normalization += 'legacy numeric blur formatting'}
  $sourceRefs=@('xml-editable-v1','official-hta-runtime#frmMain');$notes=@()
  if($entry.source -eq 'runtime-union'){$sourceRefs=@('official-hta-runtime#getATCCode:L3840-L3979','fixtures/atc-catalog-v796.json');$notes=@('Exists only when the maximum ATC selection union is materialized.')}
  $fields += [ordered]@{
    field_key=$key;serialized_key=$key;serialized_occurrence=1;label=(Get-Label $key);page=if($hidden){$null}else{Get-Page $key};item_number=(Get-Item $key)
    control_kind=if($bool){'checkbox/radio'}elseif($key -match '^(drp|frm1601FQ:txtRDOCode)'){'select'}elseif($hidden){'hidden'}else{'text'}
    storage_type='string';logical_type=if($bool){'boolean'}elseif($money){'decimal'}elseif($key -match 'Date'){'date-string'}else{'string'}
    required=if($hidden){'hidden'}elseif($computed){'computed'}elseif($required -contains $key){'required'}elseif($key -match 'SpecialTax|drpTreaty|drpATC|txtNatInc|txtAmtIncome|txtRate\d|txtReqWithheld'){'conditional'}else{'optional'}
    required_when=if($key -match '^(drpTreatyCode|drpATCCode|txtNatIncPayment|txtAmtIncomePay|txtRate|txtReqWithheld)'){'Item 13 Special Tax Yes and Schedule 1 row is used.'}else{$null}
    enabled_when=$null;visible_when=$null;default_value=$entry.value;empty_representation=''
    constraints=if($key -match 'txtTIN[123]$'){@{exact_length=3}}elseif($key -match 'txtBranchCode$'){@{min_length=1;max_length=5}}else{@{}}
    enum_values=@();normalization=$normalization;computed=[bool]$computed;calculation_id=$null
    source_refs=$sourceRefs
    confidence='high';notes=$notes
  }
}
$keys=$fields.field_key
$hasher=[Security.Cryptography.SHA256]::Create()
$sha=(-join ($hasher.ComputeHash([Text.Encoding]::UTF8.GetBytes(($keys -join "`n"))) | ForEach-Object { $_.ToString('x2') }))
$hasher.Dispose()
$doc=[ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id='1601fq-v2018';revision='2018-01-01';field_count=$fields.Count;runtime_serializable_element_count=$fields.Count;inventory_sha256=$sha;fields=$fields}
[IO.Directory]::CreateDirectory((Split-Path -Parent $OutputPath))|Out-Null
[IO.File]::WriteAllText($OutputPath,($doc|ConvertTo-Json -Depth 10)+"`n",[Text.UTF8Encoding]::new($false))
[ordered]@{fields=$fields.Count;unique=@($keys|Sort-Object -Unique).Count;sha256=$sha}|ConvertTo-Json
