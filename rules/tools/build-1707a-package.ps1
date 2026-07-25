param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\1707Av2021',
    [string]$SampleDir = 'C:\Mac\Home\Downloads\forms\1707A'
)
$ErrorActionPreference = 'Stop'
$formId = '1707a-v2021'; $revision = '2021-04-01'; $packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1707Av2021.hta'
$legacyPath = Join-Path $ExtractedRoot 'forms\BIR-Form1707A.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1707Av2021.hta'
$legacyHelpPath = Join-Path $ExtractedRoot 'helpfile\Help1707A.hta'
$pdfPath = Join-Path $OfficialDir '1707-A April 2021 ENCS.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1707a-v2021'; $fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta='cd1112c6c14bc338cdd81b19f405bdd788866e0c0164591b968c630feb1b07f1'
    legacy='c119206a80a218f5b0cf78e4d9a94ff5f461bb8d8dfb25a262b2b30ed4aa0166'
    help='f0ca102e83ce897b66d23533ca5aed77b39bc072563ac8400632677853e6e18b'
    legacy_help='174191a1aa6ff502f3d6b10fe20309e2a3617e08765aba1b75ee427b283c7f57'
    pdf='5742d6bf0ca58c601f6c87e486984714a976c6a8b8c1bc6fb246b11fc87f08c3'
    cipher='fac538050e6ec5773e89dbef2287d13e231afbefa04bde90403268727d22087b'
    plain='174c19479a530638b7b4dfba60db22ffece2abe52e06d0062325ff2ebf968517'
    inventory='c2a0995e359a14a87370f573e37aba798f661745e07d9f908633707ab043ef71'
    package='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}
function Write-Json([string]$Path,$Value){[IO.File]::WriteAllText($Path,(($Value|ConvertTo-Json -Depth 60)+[Environment]::NewLine),[Text.UTF8Encoding]::new($false))}
function Write-Utf8([string]$Path,[string]$Value){[IO.File]::WriteAllText($Path,$Value,[Text.UTF8Encoding]::new($false))}
function Hash-Lines([string[]]$Lines){$s=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($s.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines-join"`n"))))).Replace('-','').ToLowerInvariant()}finally{$s.Dispose()}}
function Attr([string]$Tag,[string]$Name){$m=[regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1'-f[regex]::Escape($Name)));if($m.Success){$m.Groups[2].Value}else{$null}}
function Asset([string]$Id,[string]$Kind,[string]$Path,[string]$Binding,[string]$Display=''){$i=Get-Item -LiteralPath $Path;[pscustomobject][ordered]@{asset_id=$Id;kind=$Kind;path=if($Display){$Display}else{$Path};sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant();size=$i.Length;revision_binding=$Binding}}

foreach($pair in @(@($htaPath,'hta'),@($legacyPath,'legacy'),@($helpPath,'help'),@($legacyHelpPath,'legacy_help'),@($pdfPath,'pdf'),@($packagePath,'package'))){
    if(-not(Test-Path -LiteralPath $pair[0] -PathType Leaf)){throw "Missing source: $($pair[0])"}
    if((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected[$pair[1]]){throw "Hash changed: $($pair[0])"}
}
$sample=@(Get-ChildItem -LiteralPath $SampleDir -File -Filter '*.xml');if($sample.Count-ne1){throw "Expected one sample; found $($sample.Count)."}
if((Get-FileHash -LiteralPath $sample[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected.cipher){throw 'Encrypted sample hash changed.'}
$pdfBytes=[IO.File]::ReadAllBytes($pdfPath);if([Text.Encoding]::ASCII.GetString($pdfBytes[0..4])-ne'%PDF-'){throw 'PDF magic mismatch.'}
$hta=[IO.File]::ReadAllText($htaPath);$help=[IO.File]::ReadAllText($helpPath)
if($hta-notmatch'(?i)<title>\s*BIR\s+Form\s+No\.\s*1707Av2021\s*</title>'-or$hta-notmatch'(?i)var\s+formType\s*=\s*["'']1707Av2021["'']'-or$hta-notmatch'(?i)April\s+2021\s+\(ENCS\)'){throw 'April 2021 runtime binding changed.'}
if($help-notmatch'(?i)April\s+15'-or$help-notmatch'(?i)rates?\s+of\s+15%'){throw 'April 2021 help binding changed.'}
New-Item -ItemType Directory -Force -Path $fixtureDir|Out-Null

$keyTool=Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson=&$keyTool -SourcePath $sample[0].FullName -RedactedSourcePath (Join-Path $SampleDir '1707A-final-copy-#email-redacted#.xml') -FormId '1707a-v2001' `
    -ExpectedCiphertextSha256 $expected.cipher -ExpectedDecryptedSha256 $expected.plain -ExpectedFieldCount 127 -ExpectedFieldInventorySha256 $expected.inventory
$keyAudit=$keyJson|ConvertFrom-Json;$legacyKeys=@($keyAudit.keys)
Write-Utf8 (Join-Path $fixtureDir 'excluded-legacy-encrypted-field-keys-v796.json') ($keyJson-join[Environment]::NewLine)
$legacyIds=@([regex]::Matches([IO.File]::ReadAllText($legacyPath),'(?i)\bid\s*=\s*(["''])(?<id>.*?)\1')|ForEach-Object{$_.Groups['id'].Value}|Where-Object{$_}|Sort-Object -Unique)
$currentIds=@([regex]::Matches($hta,'(?i)\bid\s*=\s*(["''])(?<id>.*?)\1')|ForEach-Object{$_.Groups['id'].Value}|Where-Object{$_}|Sort-Object -Unique)
$legacyOverlap=@($legacyKeys|Where-Object{$legacyIds-contains$_});$currentOverlap=@($legacyKeys|Where-Object{$currentIds-contains$_})
if($legacyOverlap.Count-ne127-or$currentOverlap.Count-ne8){throw "Sample discrimination changed: legacy=$($legacyOverlap.Count), current=$($currentOverlap.Count)."}

$fm=[regex]::Match($hta,'(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>');if(-not$fm.Success){throw 'frmMain missing.'}
$body=$fm.Groups['body'].Value;$offset=$fm.Groups['body'].Index
$excluded=@(@([regex]::Matches($body,'(?is)<script\b.*?</script>'))+@([regex]::Matches($body,'(?is)<!--.*?-->')))
$controls=[Collections.Generic.List[object]]::new();$ordinal=0
foreach($m in [regex]::Matches($body,'(?is)<(input|select|textarea|button)\b[^>]*>')){
    $skip=$false;foreach($range in $excluded){if($m.Index-ge$range.Index-and$m.Index-lt($range.Index+$range.Length)){$skip=$true;break}};if($skip){continue}
    $ordinal++;$tag=$m.Value;$element=$m.Groups[1].Value.ToLowerInvariant();$kind=if($element-eq'input'){Attr $tag 'type'}else{$element};if(-not$kind){$kind='text'}
    $controls.Add([pscustomobject][ordered]@{ordinal=$ordinal;id=Attr $tag 'id';name=Attr $tag 'name';element=$element;control_kind=$kind.ToLowerInvariant();source_line=1+[regex]::Matches($hta.Substring(0,$offset+$m.Index),"`n").Count;value=Attr $tag 'value';maxlength=Attr $tag 'maxlength';disabled=$tag-match'(?i)\bdisabled(?:\s*=|\s|>)';readonly=$tag-match'(?i)\breadonly(?:\s*=|\s|>)'})
}
$serial=@($controls|Where-Object{$_.control_kind-in@('text','select','select-one','textarea','radio','checkbox')})
$uniqueSerial=@($serial.id|Where-Object{$_}|Sort-Object -Unique)
if($controls.Count-ne186-or$serial.Count-ne139-or$uniqueSerial.Count-ne138){throw "Expected 186 live/139 serial occurrences/138 unique IDs; found $($controls.Count)/$($serial.Count)/$($uniqueSerial.Count)."}
$duplicateGroups=@($serial|Where-Object{$_.id}|Group-Object id|Where-Object{$_.Count-gt1})
if($duplicateGroups.Count-ne1-or$duplicateGroups[0].Name-ne'frm1707Av2021:txtI11Email'-or$duplicateGroups[0].Count-ne2){throw 'Duplicate serialization inventory changed.'}

$families=@(
    @{field_pattern='frm1707Av2021:txtS1C1DateOfTransaction_{N>=1}';line=2898;logical='date-string-mm-dd-yyyy'},
    @{field_pattern='frm1707Av2021:txtS1C2NameOfCorporateStock_{N>=1}';line=2899;logical='string'},
    @{field_pattern='frm1707Av2021:txtS1C3SellingPrice_{N>=1}';line=2900;logical='decimal-amount'},
    @{field_pattern='frm1707Av2021:txtS1C4Cost_{N>=1}';line=2901;logical='decimal-amount'},
    @{field_pattern='frm1707Av2021:txtS1C5CapitalGains_{N>=1}';line=2902;logical='decimal-amount'},
    @{field_pattern='frm1707Av2021:txtS1C6TaxPaid_{N>=1}';line=2903;logical='decimal-amount'},
    @{field_pattern='frm1707Av2021:txtS2C1DateOfTransaction_{N>=1}';line=3224;logical='date-string-mm-dd-yyyy'},
    @{field_pattern='frm1707Av2021:txtS2C2NameOfCorporateStock_{N>=1}';line=3225;logical='string'},
    @{field_pattern='frm1707Av2021:txtS2C3SellingPrice_{N>=1}';line=3226;logical='decimal-amount'},
    @{field_pattern='frm1707Av2021:txtS2C4Cost_{N>=1}';line=3227;logical='decimal-amount'},
    @{field_pattern='frm1707Av2021:txtS2C5CapitalLoss_{N>=1}';line=3228;logical='decimal-amount'}
)
foreach($f in $families){$prefix=$f.field_pattern.Split('{')[0];if($hta-notmatch[regex]::Escape($prefix)){throw "Dynamic family source changed: $($f.field_pattern)"}}

$required=@('frm1707Av2021:txtI1YearEndMonth','frm1707Av2021:txtI1YearEndDay','frm1707Av2021:txtI1YearEndYear','frm1707Av2021:rdoI6RDO','frm1707Av2021:txtI8RegisteredName','frm1707Av2021:txtI9RegisteredAddress','frm1707Av2021:txtI10ZipCode')
function Meta([string]$Key,$Control,[bool]$Family,[string]$Logical=''){
    $page=$null;if($Key-match'(?i)(?:Pg|Page)(?<p>\d+)'){$page=[int]$Matches.p}elseif($Key-match'(?i)^frm1707Av2021:txtS[12]'){$page=2}else{$page=1}
    $item=$null;$ims=@([regex]::Matches($Key,'(?i)(?:Itm?|I)(?<i>\d+[A-Z]?)'));if($ims.Count){$item=$ims[-1].Groups['i'].Value}
    $logical=if($Logical){$Logical}else{'string'};$norm=[string[]]@();$enum=[object[]]@()
    if(($Control-and$Control.control_kind-in@('radio','checkbox'))-or$Key-match'(?i):(rdo|chk)'){$logical='boolean';$enum=[object[]]@('true','false')}
    elseif($Key-match'(?i)Email'){$logical='email-string'}
    elseif($Key-match'(?i)(Date|YearEnd)'){$logical='date-string-mm-dd-yyyy';$norm=[string[]]@('MM/DD/YYYY')}
    elseif($Key-match'(?i)(TIN|RDO|Zip|ATC)'){$logical='code'}
    elseif($Key-match'(?i)(SellingPrice|Cost|Capital|TaxPaid|TaxDue|TaxPayable|Surcharge|Interest|Compromise|Penalt|AmountPayable|ApplicableTaxRate)'){$logical='decimal-amount';$norm=[string[]]@('valForCompute','preciseCompute','roundAmount')}
    $computed=$false;if($Control-and($Control.disabled-or$Control.readonly)-and$logical-eq'decimal-amount'){$computed=$true}
    if($Key-match'(?i)(Total|CapitalGainsI|CapitalLossI|NetCapitalGainLoss|TaxDue|TaxStillPayable|Penalties|AmountPayable)$'){$computed=$true}
    $status=if($required-contains$Key){'required'}elseif($computed){'computed'}else{'optional'};if($Family){$status='conditional';$computed=$false}
    $constraints=[ordered]@{};if($Control-and$Control.maxlength-match'^\d+$'){$constraints.max_length=[int]$Control.maxlength};if($logical-eq'decimal-amount'){$constraints.precision=2;$constraints.sign='signed unless source validation constrains the field'}
    [pscustomobject]@{page=$page;item=$item;logical=$logical;norm=$norm;enum=$enum;computed=$computed;status=$status;constraints=[pscustomobject]$constraints}
}
$fields=[Collections.Generic.List[object]]::new();$seen=@{}
foreach($control in $serial){
    if(-not$control.id){continue};if(-not$seen.ContainsKey($control.id)){$seen[$control.id]=0};$seen[$control.id]++;$occ=$seen[$control.id]
    $fieldKey=if($occ-eq1){$control.id}else{"$($control.id)#occurrence-$occ"};$meta=Meta $control.id $control $false
    $baseLabel=if($control.id-like'frm1707Av2021:*'){$control.id.Substring(16)}else{$control.id}
    $fields.Add([pscustomobject][ordered]@{field_key=$fieldKey;serialized_key=$control.id;serialized_occurrence=$occ;label=if($occ-eq1){$baseLabel}else{"Duplicate serialized occurrence $occ of $baseLabel"};page=$meta.page;item_number=$meta.item;control_kind=$control.control_kind;storage_type='string';logical_type=$meta.logical;required=$meta.status;required_when=$null;enabled_when=$null;visible_when=$null;default_value=$control.value;empty_representation='';constraints=$meta.constraints;enum_values=$meta.enum;normalization=$meta.norm;computed=$meta.computed;calculation_id=if($meta.computed){'See calculations.json'}else{$null};source_refs=@('official-hta-runtime#saveXML:L6028-L6346',"official-hta-runtime#control:L$($control.source_line)");confidence='high';notes=@('Source-derived from the hash-pinned April 2021 runtime; the available encrypted sample is legacy and excluded.')})
}
foreach($f in $families){$meta=Meta $f.field_pattern $null $true $f.logical;$fields.Add([pscustomobject][ordered]@{field_key=$f.field_pattern;serialized_key=$null;serialized_occurrence=$null;label="Runtime-indexed family $($f.field_pattern)";page=$meta.page;item_number=$meta.item;control_kind='runtime-indexed-family';storage_type='string';logical_type=$meta.logical;required='conditional';required_when='The corresponding additional schedule row exists.';enabled_when='The row exists.';visible_when='The modal row exists.';default_value=$null;empty_representation='';constraints=[pscustomobject]@{index='one-based, source-unbounded'};enum_values=@();normalization=$meta.norm;computed=$false;calculation_id=$null;source_refs=@("official-hta-runtime#dynamic-id:L$($f.line)",'official-hta-runtime#schedule-serialization');confidence='high';notes=@('Source-derived unbounded schedule family.')})}
if($fields.Count-ne150){throw "Expected 150 fields; found $($fields.Count)."}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;field_count=$fields.Count;runtime_serializable_element_count=139;inventory_sha256=Hash-Lines @($fields.field_key|Sort-Object);fields=$fields})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta;live_control_count=$controls.Count;static_serialized_occurrence_count=$serial.Count;static_serialized_unique_id_count=$uniqueSerial.Count;duplicate_serialized_ids=@($duplicateGroups|ForEach-Object{@{serialized_key=$_.Name;occurrences=$_.Count}});revision_matched_final_copy_key_count=0;excluded_legacy_sample_key_count=$legacyKeys.Count;excluded_legacy_sample_overlap_with_legacy_runtime=$legacyOverlap.Count;excluded_legacy_sample_overlap_with_current_runtime=$currentOverlap.Count;active_runtime_family_count=$families.Count;controls=$controls;dynamic_families=$families})
$functionTool=Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm1707Av2021:' -NamePattern '(?i)valid|check|mandatory|save|enable|disable|date|email|submit|final')-join[Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm1707Av2021:' -NamePattern '(?i)compute|amount|sum|format|tax|penalty|interest')-join[Environment]::NewLine)

$rules=[Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Keys,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',[string]$Recommended='Retain as a structured revision-aware error.'){
    $rules.Add([pscustomobject][ordered]@{rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Keys;accepted_behavior='Condition is false; processing continues.';rejected_behavior='The active operation stops unless official_behavior states otherwise.';exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()})
}
Rule '1707a-validate-001-year-end-shape' validate 1 'Year-end components do not form a valid MM/DD/YYYY date.' @('frm1707Av2021:txtI1YearEndMonth','frm1707Av2021:txtI1YearEndDay','frm1707Av2021:txtI1YearEndYear') 'Please provide a valid date (MM/DD/YYYY format) on Item 1' @('official-hta-runtime#validateYearEnd:L3515-L3522')
Rule '1707a-validate-002-zero-month' validate 2 'Year-end month is 00 with otherwise parseable components.' @('frm1707Av2021:txtI1YearEndMonth') $null @('official-hta-runtime#validateDate:L4150-L4212') 'incorrect-official-behavior' 'validateDate rejects month below 0 rather than below 1, so month 00 can roll into the previous year.' 'Require month 01 through 12.'
Rule '1707a-validate-003-effectivity' validate 3 'Year end is before July 1, 2001.' @('frm1707Av2021:txtI1YearEndMonth','frm1707Av2021:txtI1YearEndDay','frm1707Av2021:txtI1YearEndYear') 'Valid input for the Month and Year is June 2001 onwards on Item 1.' @('official-hta-runtime#validateYearEnd:L3524-L3533') 'official-bug-compatible' 'The code uses July 1, 2001 while the message says June 2001 onwards.' 'Align the cutoff and message to the authoritative revision rule.'
Rule '1707a-validate-004-future' validate 4 'Year end is after today.' @('frm1707Av2021:txtI1YearEndMonth','frm1707Av2021:txtI1YearEndDay','frm1707Av2021:txtI1YearEndYear') 'Year Ended should not be a future date on Item 1.' @('official-hta-runtime#validateYearEnd:L3534-L3537')
Rule '1707a-validate-005-fiscal-december' validate 5 'Fiscal is selected and year-end month is December.' @('frm1707Av2021:rdoI1Fiscal','frm1707Av2021:txtI1YearEndMonth') 'Month cannot be equal to December on Item 1.' @('official-hta-runtime#validateYearEnd:L3538-L3541')
Rule '1707a-validate-006-version-cutoff' validate 6 'Year end is on or before March 31, 2021.' @('frm1707Av2021:txtI1YearEndMonth','frm1707Av2021:txtI1YearEndDay','frm1707Av2021:txtI1YearEndYear') 'Year shall not be greater than the present year and not earlier than April 2021.' @('official-hta-runtime#checkVersion:L2865-L2885') 'incorrect-official-behavior' 'The branch replaces the year with current year minus one, and the message describes only a year despite a full-date cutoff.' 'Preserve user input and state the exact revision cutoff.'
Rule '1707a-validate-007-atc' validate 7 'Neither individual nor corporation ATC is selected.' @('frm1707Av2021:rdoI4ATCII030','frm1707Av2021:rdoI4ATCIC110') 'Please select an ATC Code on Item 5.' @('official-hta-runtime#validateAll:L4823-L4827')
Rule '1707a-validate-008-individual-calendar' validate 8 'Individual ATC and Fiscal are both selected.' @('frm1707Av2021:rdoI4ATCII030','frm1707Av2021:rdoI1Fiscal') 'If you are filing as Individual, please select Calendar on item 1.' @('official-hta-runtime#validateAll:L4828-L4832')
Rule '1707a-validate-009-rdo' validate 9 'RDO is 000.' @('frm1707Av2021:rdoI6RDO') 'Please provide a valid RDO Code on Item 7.' @('official-hta-runtime#validateAll:L4833')
Rule '1707a-validate-010-name' validate 10 'Registered name is blank.' @('frm1707Av2021:txtI8RegisteredName') 'Please provide a valid Line of Business/Occupation on Item 8.' @('official-hta-runtime#validateAll:L4835') 'incorrect-official-behavior' 'The message labels the registered-name field as Line of Business/Occupation.' 'Name the field as registered name.'
Rule '1707a-validate-011-address' validate 11 'Registered address is blank.' @('frm1707Av2021:txtI9RegisteredAddress') 'Please provide a valid Registered Address on Item 9.' @('official-hta-runtime#validateAll:L4837')
Rule '1707a-validate-012-zip' validate 12 'Zip code is blank.' @('frm1707Av2021:txtI10ZipCode') 'Zip Code is required, please validate your registration information and update accordingly using BIR Form 1905, if necessary.' @('official-hta-runtime#validateAll:L4839-L4842') 'incorrect-official-behavior' 'The branch alerts but does not return false, so validation continues.' 'Return false after the alert.'
Rule '1707a-validate-013-treaty-choice' validate 13 'Neither treaty-relief Yes nor No is selected.' @('frm1707Av2021:rdoI11TaxTreatyYes','frm1707Av2021:rdoI11TaxTreatyNo') 'Please choose at least one from item 12.' @('official-hta-runtime#validateAll:L4844-L4848')
Rule '1707a-validate-014-treaty-spec' validate 14 'Treaty-relief Yes is selected and specification is blank.' @('frm1707Av2021:rdoI11TaxTreatyYes','frm1707Av2021:txtI11ASpecify') 'Please specify the tax relief on item 12A.' @('official-hta-runtime#validateAll:L4850-L4854')
Rule '1707a-validate-015-schedule-presence' validate 15 'Both Schedule 1 and Schedule 2 selling-price and cost totals are zero.' @('frm1707Av2021:txtS1I20TotalSellingPrice','frm1707Av2021:txtS1I20TotalCost','frm1707Av2021:txtS2I21TotalSellingPrice','frm1707Av2021:txtS2I21TotalCost') 'You need to have at least one value on either Schedule 1 or Schedule 2.' @('official-hta-runtime#validateSched1and2:L4792-L4812','official-hta-runtime#validateAll:L4856-L4861')
Rule '1707a-schedule-016-date-shape' validate 16 'A schedule date is not a valid MM/DD/YYYY date.' @('schedule-date-fields') 'Please provide a valid date. (MM/DD/YYYY format) in Date of Transaction on row {row}{schedule}' @('official-hta-runtime#recheckSchedDate:L4618-L4650')
Rule '1707a-schedule-017-date-minimum' validate 17 'A schedule transaction year is before 2001.' @('schedule-date-fields') 'The year should not be less than 2001 in Date of Transaction on row {row}{schedule}' @('official-hta-runtime#recheckSchedDate:L4628-L4632')
Rule '1707a-schedule-018-date-after-period' validate 18 'A schedule transaction date is after year end.' @('schedule-date-fields') 'The Date of Transaction should not be greater than the Taxable Period on row {row}{schedule}' @('official-hta-runtime#recheckSchedDate:L4633-L4637')
Rule '1707a-schedule-019-date-range' validate 19 'A schedule transaction date is more than one year before year end.' @('schedule-date-fields') 'The Date of Transaction should be in the range of up to one year until the Taxable Period on row {row}{schedule}' @('official-hta-runtime#recheckSchedDate:L4638-L4642')
Rule '1707a-schedule-020-stock-numeric' validate 20 'Corporate-stock name is entirely numeric.' @('schedule-stock-name-fields') 'Name of Corporate Stock should not be all numeric in row {row}{schedule}' @('official-hta-runtime#validateSchedules:L4684-L4691')
Rule '1707a-schedule-021-negative-input' validate 21 'Selling price or cost uses the parenthesized negative representation.' @('schedule-selling-price-fields','schedule-cost-fields') 'There should be no negative value in row {row}{schedule}' @('official-hta-runtime#validateSchedules:L4714-L4721')
Rule '1707a-schedule-022-incomplete' validate 22 'A nonempty fixed schedule row has any missing date, stock name, selling price, or cost.' @('fixed-schedule-row-fields') 'There is an empty item on {schedule}. Please fill in all items in row {row}' @('official-hta-runtime#validateSchedules:L4735-L4739')
Rule '1707a-schedule-023-gain-negative' validate 23 'Schedule 1 capital gain is negative.' @('frm1707Av2021:txtS1C5CapitalGainsI1','frm1707Av2021:txtS1C5CapitalGains_{N>=1}') 'Your Capital Gains is negative in row {row} Schedule 1\n Please move and encode this in Schedule 2' @('official-hta-runtime#validateSchedules:L4749-L4753')
Rule '1707a-schedule-024-loss-negative' validate 24 'Schedule 2 capital loss is negative.' @('frm1707Av2021:txtS2C5CapitalLossI1','frm1707Av2021:txtS2C5CapitalLoss_{N>=1}') 'Your Capital Loss is negative in row {row} Schedule 2\n Please move and encode this in Schedule 1' @('official-hta-runtime#validateSchedules:L4757-L4761')
Rule '1707a-schedule-025-modal-incomplete' validate 25 'An additional modal row is empty or partially populated.' @('runtime-schedule-families') 'Row {row} is empty. Please remove row or fill in all items first in{schedule}' @('official-hta-runtime#validateSchedules:L4779-L4783')
Rule '1707a-validate-026-overpayment' validate 26 'Tax still payable is parenthesized negative.' @('frm1707Av2021:txtI17TaxStillPayable') 'You have a negative value for Part 2 Item 19. \n\nIn case of overpayment apply for tax refund using BIR Form No. 1914 (Application for Tax Credits/Refunds)' @('official-hta-runtime#validateAll:L4867-L4870') 'incorrect-official-behavior' 'The alert is shown but return false is commented out, so Validate succeeds.' 'Represent the overpayment separately and require an explicit disposition if applicable.'
Rule '1707a-rate-027-upper' 'blur/change' 1 'Applicable tax rate is at least 100.' @('frm1707Av2021:txtI16ApplicableTaxRate') 'Tax rate should be below 100.' @('official-hta-runtime#checkRate:L2844-L2851')
Rule '1707a-rate-028-negative' 'blur/change' 2 'Applicable tax rate is negative.' @('frm1707Av2021:txtI16ApplicableTaxRate') $null @('official-hta-runtime#checkRate:L2844-L2851') 'incorrect-official-behavior' 'Only the upper bound is checked, so a negative rate is accepted.' 'Require a rate from zero through less than 100.'
Rule '1707a-save-029-rdo' save 1 'RDO is 000.' @('frm1707Av2021:rdoI6RDO') 'Please select an RDO Code on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L6570-L6577') 'official-bug-compatible' 'Save labels the RDO as Item 6 while Validate labels it Item 7.' 'Use the printed item number consistently.'
Rule '1707a-save-030-name' save 2 'Registered name is blank.' @('frm1707Av2021:txtI8RegisteredName') 'Please enter a valid name on Item 8.' @('official-hta-runtime#initialValidateBeforeSave:L6578')
Rule '1707a-save-031-address' save 3 'Registered address is blank.' @('frm1707Av2021:txtI9RegisteredAddress') 'Please enter a valid Registered Address on Item 9.' @('official-hta-runtime#initialValidateBeforeSave:L6579')
Rule '1707a-save-032-year-end' save 4 'validateYearEnd fails.' @('frm1707Av2021:txtI1YearEndMonth','frm1707Av2021:txtI1YearEndDay','frm1707Av2021:txtI1YearEndYear') 'First validateYearEnd message.' @('official-hta-runtime#initialValidateBeforeSave:L6580')
Rule '1707a-save-033-sparse' save 5 'ATC, version cutoff, treaty, schedule, Zip, or overpayment validation fails.' @() $null @('official-hta-runtime#initialValidateBeforeSave:L6570-L6583','official-hta-runtime#validateAll:L4816-L4873') 'incorrect-official-behavior' 'Save bypasses most of the Validate graph.' 'Use a shared validation graph with explicit phase exceptions.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{'$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;first_error_behavior='Validate stops at the first blocking source-ordered failure, except Zip and overpayment branches only alert; Save runs a narrower graph.';rules=$rules})

$calcs=[Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Out,[string[]]$In,[string]$Formula,[string]$Trigger,[string[]]$Deps,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Implement with typed decimals and the official preciseCompute/roundAmount order.'){
    $calcs.Add([pscustomobject][ordered]@{calculation_id=$Id;outputs=$Out;inputs=$In;condition=$null;official_formula=$Formula;rounding='preciseCompute followed by roundAmount at displayed boundaries.';trigger=$Trigger;depends_on=$Deps;source_refs=$Refs;assessment=$Assessment;recommended_app_behavior=$Recommended;confidence='high'})
}
Calc '1707a-s1-row-gain' @('frm1707Av2021:txtS1C5CapitalGainsI{1..4}','frm1707Av2021:txtS1C5CapitalGains_{N>=1}') @('schedule1-selling-price','schedule1-cost') 'Capital gain per row = selling price - cost.' 'selling price/cost blur' @() @('official-hta-runtime#row-gain:L4318-L4351','official-hta-runtime#elemModalSched1Compute')
Calc '1707a-s1-modal-totals' @('frm1707Av2021:txtS1ModalTotalSellingPrice','frm1707Av2021:txtS1ModalTotalCost','frm1707Av2021:txtS1ModalTotalCapitalGains','frm1707Av2021:txtS1ModalTotalTaxPaid') @('frm1707Av2021:txtS1C3SellingPrice_{N>=1}','frm1707Av2021:txtS1C4Cost_{N>=1}','frm1707Av2021:txtS1C5CapitalGains_{N>=1}','frm1707Av2021:txtS1C6TaxPaid_{N>=1}') 'Sum each additional Schedule 1 column.' modalSched1Subtotal @('1707a-s1-row-gain') @('official-hta-runtime#modalSched1Subtotal:L3088-L3110')
Calc '1707a-s1-fixed-totals' @('frm1707Av2021:txtS1I20TotalSellingPrice','frm1707Av2021:txtS1I20TotalCost','frm1707Av2021:txtS1I20TotalCapitalGains','frm1707Av2021:txtS1I20TotalTaxPaid','frm1707Av2021:txtI12TotalCapitalGains','frm1707Av2021:txtI16ATotalTaxPaid') @('Schedule 1 fixed rows 1..4, with row 4 carrying modal totals when used') 'Sum each Schedule 1 fixed column; copy total gain to Item 12 and total tax paid to Item 16A.' computeSched1 @('1707a-s1-modal-totals') @('official-hta-runtime#computeSched1SellingPrice:L4377-L4385','official-hta-runtime#computeSched1Cost:L4387-L4395','official-hta-runtime#computeSched1CapitalGains:L4397-L4408','official-hta-runtime#computeSched1TaxPaid:L4410-L4421')
Calc '1707a-s2-row-loss' @('frm1707Av2021:txtS2C5CapitalLossI{1..4}','frm1707Av2021:txtS2C5CapitalLoss_{N>=1}') @('schedule2-selling-price','schedule2-cost') 'Capital loss per row = cost - selling price.' 'selling price/cost blur' @() @('official-hta-runtime#row-loss:L4423-L4460','official-hta-runtime#elemModalSched2Compute')
Calc '1707a-s2-modal-totals' @('frm1707Av2021:txtS2ModalTotalSellingPrice','frm1707Av2021:txtS2ModalTotalCost','frm1707Av2021:txtS2ModalTotalCapitalLoss') @('frm1707Av2021:txtS2C3SellingPrice_{N>=1}','frm1707Av2021:txtS2C4Cost_{N>=1}','frm1707Av2021:txtS2C5CapitalLoss_{N>=1}') 'Sum each additional Schedule 2 column.' modalSched2Subtotal @('1707a-s2-row-loss') @('official-hta-runtime#modalSched2Subtotal:L3370-L3388')
Calc '1707a-s2-fixed-totals' @('frm1707Av2021:txtS2I21TotalSellingPrice','frm1707Av2021:txtS2I21TotalCost','frm1707Av2021:txtS2I21TotalCapitalLoss','frm1707Av2021:txtI13TotalCapitalLoss') @('Schedule 2 fixed rows 1..4, with row 4 carrying modal totals when used') 'Sum each Schedule 2 fixed column; copy total loss to Item 13.' computeSched2 @('1707a-s2-modal-totals') @('official-hta-runtime#computeSched2SellingPrice:L4467-L4475','official-hta-runtime#computeSched2Cost:L4477-L4485','official-hta-runtime#computeSched2CapitalLoss:L4487-L4499')
Calc '1707a-item14-net' @('frm1707Av2021:txtI14NetCapitalGainLoss') @('frm1707Av2021:txtI12TotalCapitalGains','frm1707Av2021:txtI13TotalCapitalLoss') 'Item 14 = Item 12 - Item 13.' computePart2Item14GainLoss @('1707a-s1-fixed-totals','1707a-s2-fixed-totals') @('official-hta-runtime#computePart2Item14GainLoss:L4511-L4520')
Calc '1707a-item15-tax' @('frm1707Av2021:txtI15TaxDue') @('frm1707Av2021:txtI14NetCapitalGainLoss','frm1707Av2021:txtI16ApplicableTaxRate') 'Item 15 = max(0, Item 14 × applicable rate / 100).' computePart2Item15TaxDue @('1707a-item14-net') @('official-hta-runtime#computePart2Item15TaxDue:L4522-L4534')
Calc '1707a-item16c-paid' @('frm1707Av2021:txtI16CTotalTaxPaid') @('frm1707Av2021:txtI16ATotalTaxPaid','frm1707Av2021:txtI16BTotalTaxPaid') 'Item 16C = Item 16A + Item 16B.' computePart2Item16CTaxPaid @('1707a-s1-fixed-totals') @('official-hta-runtime#computePart2Item16CTaxPaid:L4536-L4545')
Calc '1707a-item17-payable' @('frm1707Av2021:txtI17TaxStillPayable') @('frm1707Av2021:txtI15TaxDue','frm1707Av2021:txtI16CTotalTaxPaid') 'Item 17 = Item 15 - Item 16C.' computePart2Item17TaxPayable @('1707a-item15-tax','1707a-item16c-paid') @('official-hta-runtime#computePart2Item17TaxPayable:L4547-L4567')
Calc '1707a-item18d-penalties' @('frm1707Av2021:txtI18DPenalties') @('frm1707Av2021:txtI18ASurcharge','frm1707Av2021:txtI18BInterest','frm1707Av2021:txtI18CCompromise') 'Item 18D = surcharge + interest + compromise.' computePart2Item18DPenalties @() @('official-hta-runtime#computePart2Item18DPenalties:L4571-L4581')
Calc '1707a-item19-payable' @('frm1707Av2021:txtI19TotalAmountPayable') @('frm1707Av2021:txtI17TaxStillPayable','frm1707Av2021:txtI18DPenalties') 'If Item 17 is negative and penalties are positive, Item 19 = penalties; otherwise Item 19 = Item 17 + penalties.' computePart2Item19TotalPayable @('1707a-item17-payable','1707a-item18d-penalties') @('official-hta-runtime#computePart2Item19TotalPayable:L4583-L4598') 'incorrect-official-behavior' 'When Item 17 is negative and penalties are zero, Item 19 remains negative; represent overpayment separately and payable as zero.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=@($calcs.calculation_id);calculations=$calcs})
$cases=@();$n=0;foreach($r in @($rules|Where-Object{$_.exact_message})){$n++;$cases+=[pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}'-f$n,$r.rule_id);phase=$r.phase;mutations=@{synthetic_condition=$r.condition};expected_message=$r.exact_message;expected_behavior=$r.official_behavior;rule_id=$r.rule_id}}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=@(
    @{case_id='gain';calculation_id='1707a-s1-row-gain';selling_price=1000;cost=700;official_output=300},
    @{case_id='loss';calculation_id='1707a-s2-row-loss';selling_price=700;cost=1000;official_output=300},
    @{case_id='tax-15-percent';calculation_id='1707a-item15-tax';net_gain=1000;rate=15;official_output=150},
    @{case_id='negative-payable-defect';calculation_id='1707a-item19-payable';tax_still_payable=-100;penalties=0;official_output=-100;recommended_output=0}
)})
$resources=@();foreach($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1')|ForEach-Object{$_.Groups['v'].Value}|Sort-Object -Unique)){$full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src));if(Test-Path -LiteralPath $full){$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}}else{$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}}}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$deadline='Individual: April 15 covering the preceding taxable year. Corporate: fifteenth day of the fourth month following taxable-year close.'
$workflow=[ordered]@{'$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;phases=@(
    @{phase='edit';official_behavior='April 2021 annual consolidated capital-gains return with fixed and source-unbounded Schedule 1/2 rows.';source_refs=@('official-hta-runtime','official-help-runtime');confidence='high'},
    @{phase='saved-draft';official_behavior='Save checks RDO, name, address, and year end only, then records modal lengths and serializes controls.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L6570-L6583','official-hta-runtime#saveXML:L6028-L6346');confidence='high'},
    @{phase='validated';official_behavior='Validate checks revision, ATC, identity, treaty selection, and schedules; Zip and overpayment branches merely alert.';source_refs=@('official-hta-runtime#validateAll:L4816-L4873');confidence='high'},
    @{phase='final-copy';official_behavior='Final-copy encryption is defined, but no revision-matched April 2021 encrypted sample is available.';source_refs=@('official-hta-runtime#saveXML:L6028-L6346');confidence='medium'},
    @{phase='submitted';official_behavior='Online transport exists but was not exercised.';source_refs=@('official-hta-runtime#processSubmit','official-hta-runtime#sendEmail');confidence='medium'}
);transitions=@(
    @{from='edit';action='Save';to='saved-draft';guard='Sparse Save checks pass.';side_effects=@('Writes flat pseudo-XML.','Records modal schedule lengths.');source_refs=@('official-hta-runtime#initialValidateBeforeSave:L6570-L6583','official-hta-runtime#saveXML:L6028-L6346')},
    @{from='edit';action='Validate';to='validated';guard='All blocking validateAll checks pass.';side_effects=@('Disables controls.','Enables print/final-copy actions.');source_refs=@('official-hta-runtime#validateAll:L4816-L4873','official-hta-runtime#validate')},
    @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables controls subject to ATC/treaty conditions.');source_refs=@('official-hta-runtime#enableAllControl')},
    @{from='validated';action='Final Copy';to='final-copy';guard='Finalization succeeds.';side_effects=@('Encrypts/compresses the final copy.');source_refs=@('official-hta-runtime#saveXML:L6028-L6346')},
    @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and acceptance succeed.';side_effects=@('Online attempt; untested.');source_refs=@('official-hta-runtime#processSubmit','official-hta-runtime#sendEmail')}
);prerequisites=@('April 2021-compatible year end','ATC and calendar/fiscal choice','RDO and identity','Treaty-relief selection','At least one complete gain/loss schedule row');required_attachments=@(
    @{attachment_id='tax-payment-proofs';label='Photocopies of proof of capital-gains tax payments covering all taxable transactions of the preceding year.';required_when='Annual consolidated return filing.';official_ui_enforcement='Not enforced by local Validate.';source_refs=@('official-help-runtime#L249-L256');confidence='high'}
);filing_deadlines=@(
    @{quarter='Q1';due_date_rule=$deadline;source_refs=@('official-help-runtime#L122-L130');confidence='high'},@{quarter='Q2';due_date_rule=$deadline;source_refs=@('official-help-runtime#L122-L130');confidence='high'},@{quarter='Q3';due_date_rule=$deadline;source_refs=@('official-help-runtime#L122-L130');confidence='high'},@{quarter='Q4';due_date_rule=$deadline;source_refs=@('official-help-runtime#L122-L130');confidence='high'}
)}
Write-Json (Join-Path $outDir 'workflow.json') $workflow
$bugs=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'Title/formType 1707Av2021; April 2021 ENCS. APPLICATIONNAME remains stale as 1707.'
    Asset 'legacy-2001-excluded' 'runtime-extracted-hta' $legacyPath 'June 2001 predecessor; excluded.'
    Asset 'official-help-runtime' 'official-runtime-help' $helpPath 'Revision-matched April 2021 filing/rate/attachment guidance.'
    Asset 'legacy-help-excluded' 'official-runtime-help' $legacyHelpPath 'Predecessor help; excluded.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'April 2021 ENCS form.'
    Asset 'legacy-encrypted-sample-excluded' 'dummy-profile-encrypted-final-copy' $sample[0].FullName 'Excluded: all 127 keys overlap the June 2001 runtime, while only 8 overlap April 2021.' (Join-Path $SampleDir '1707A-final-copy-#email-redacted#.xml')
)
$manifest=[ordered]@{'$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='1707A';revision=$revision;package_version=$packageVersion;status='complete';official_assets=$assets;counts=[ordered]@{concrete_fields=139;runtime_field_families=11;fields_total=$fields.Count;typed_fields=$fields.Count;validation_rules=$rules.Count;confirmed_official_bugs=$bugs;calculations=$calcs.Count;negative_fixtures=$cases.Count;unverified_gaps=2};artifacts=[ordered]@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';excluded_legacy_encrypted_keys='fixtures/excluded-legacy-encrypted-field-keys-v796.json';runtime_controls='fixtures/runtime-control-inventory-v796.json';validation_functions='fixtures/validation-function-inventory-v796.json';calculation_functions='fixtures/calculation-function-inventory-v796.json';resources='fixtures/official-resource-hashes-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'};scope_notes=@('Research only; no renderer/release metadata changed.','No decrypted values or email-bearing filenames are emitted.','The 127-key sample is proven legacy and excluded.','139 serialized control occurrences, including the duplicated email key, plus 11 unbounded schedule families are preserved.')}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1707-A - April 2021`n`nRevision-specific package with 139 serialized control occurrences and 11 unbounded schedule families.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') "# Evidence`n`n- April 2021 runtime: $($expected.hta).`n- June 2001 predecessor: $($expected.legacy), excluded.`n- Revision-matched help: $($expected.help).`n- April 2021 PDF: $($expected.pdf).`n- Encrypted sample: ciphertext $($expected.cipher), decrypted $($expected.plain), 127 keys, inventory $($expected.inventory); values never emitted.`n- All 127 sample keys overlap the predecessor, while only 8 overlap April 2021; the sample is excluded.`n- Source inventory: 139 serialized occurrences (138 unique IDs, duplicate email preserved) plus 11 unbounded schedule families.`n`nAll email-bearing filenames are represented as `#email-redacted#`.`n"
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. No revision-matched April 2021 encrypted final copy is available; the supplied sample is proven predecessor evidence and excluded.`n2. Online submission was deliberately not exercised.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- April 2021 revision separation: pass.`n- Legacy sample excluded: 127/127 predecessor overlap versus 8/127 current overlap.`n- Typed inventory: 139 concrete occurrences + 11 families = $($fields.Count).`n- Duplicate email serialized key preserved with occurrence metadata.`n- Validations: $($rules.Count); calculations: $($calcs.Count); negative fixtures: $($cases.Count); official defects: $bugs.`n- Full structural/schema audit must run after generation.`n- No renderer/release/capability/commit/push changes.`n"
$indexPath=Join-Path $RepoRoot 'rules\index.json';$index=Get-Content -Raw -LiteralPath $indexPath|ConvertFrom-Json;$entry=$index.forms|Where-Object{$_.form_id-eq$formId}
if($entry){$entry.form_code='1707A';$entry.revision=$revision;$entry.package_version=$packageVersion;$entry.priority=28;$entry.status='complete';$entry.path='forms/1707a-v2021/manifest.json'}else{$index.forms+=[pscustomobject][ordered]@{form_id=$formId;form_code='1707A';revision=$revision;package_version=$packageVersion;priority=28;status='complete';path='forms/1707a-v2021/manifest.json'}}
$index.forms=@($index.forms|Sort-Object priority);$index.updated='2026-07-23';Write-Json $indexPath $index
[pscustomobject]@{form_id=$formId;concrete_fields=139;families=11;typed_fields=$fields.Count;validations=$rules.Count;calculations=$calcs.Count;negative_fixtures=$cases.Count;confirmed_official_bugs=$bugs;next_form='2552'}|ConvertTo-Json
